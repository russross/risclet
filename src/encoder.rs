// encoder.rs
//
// RISC-V instruction encoder
//
// This module takes parsed AST lines with resolved symbols and generates
// binary machine code. It handles:
// - Base rv32i instructions
// - m extension (multiply/divide)
// - Pseudo-instruction expansion (li, la, call, tail, etc.)
// - Data directives
// - BSS segment validation (space-only)
//
// The encoder enforces type constraints (Integer vs Address) and validates
// immediate ranges for all instruction formats.

use crate::ast::{
    BTypeOp, Directive, ITypeOp, Instruction, JTypeOp, Line, LineContent,
    LoadStoreOp, PseudoOp, RTypeOp, Register, Segment, Source, SpecialOp,
    UTypeOp,
};
use crate::encoder_compressed;
use crate::error::AssemblerError;
use crate::expressions::{EvaluatedValue, EvaluationContext, eval_expr};

type Result<T> = std::result::Result<T, AssemblerError>;

// ============================================================================
// Public API
// ============================================================================

/// Encoding context containing source and expression evaluator
pub struct EncodingContext<'a> {
    #[allow(dead_code)]
    pub source: &'a Source,
    pub eval_context: &'a mut EvaluationContext,
}

/// Encode all lines in a source, returning (text_bytes, data_bytes, bss_size)
#[allow(dead_code)]
pub fn encode_source(
    source: &Source,
    eval_context: &mut EvaluationContext,
) -> Result<(Vec<u8>, Vec<u8>, u32)> {
    let mut text_bytes = Vec::new();
    let mut data_bytes = Vec::new();
    let mut bss_size: u32 = 0;

    let mut context = EncodingContext { source, eval_context };

    // Process each file and line
    for file in &source.files {
        for line in &file.lines {
            match line.segment {
                Segment::Text => {
                    let bytes = encode_line(
                        line,
                        &mut context,
                        source.uses_global_pointer,
                    )?;
                    text_bytes.extend_from_slice(&bytes);
                }
                Segment::Data => {
                    let bytes = encode_line(
                        line,
                        &mut context,
                        source.uses_global_pointer,
                    )?;
                    data_bytes.extend_from_slice(&bytes);
                }
                Segment::Bss => {
                    // BSS only tracks size, doesn't generate bytes
                    let size = encode_bss_line(line, &mut context)?;
                    bss_size += size;
                }
            }
        }
    }

    Ok((text_bytes, data_bytes, bss_size))
}

/// Encode all lines with size tracking for the convergence loop
///
/// This function encodes all lines and updates each line's size field.
/// It sets `any_changed` to true if any line's actual size differs from
/// its guessed size. Returns (text_bytes, data_bytes, bss_size).
pub fn encode_source_with_size_tracking(
    source: &mut Source,
    eval_context: &mut EvaluationContext,
    any_changed: &mut bool,
) -> Result<(Vec<u8>, Vec<u8>, i64)> {
    let mut text_bytes = Vec::new();
    let mut data_bytes = Vec::new();
    let mut bss_size: i64 = 0;

    // Process each file and line, updating sizes as we go
    for file_idx in 0..source.files.len() {
        for line_idx in 0..source.files[file_idx].lines.len() {
            // Extract the data we need from the line before encoding
            let old_size = source.files[file_idx].lines[line_idx].size;
            let segment = source.files[file_idx].lines[line_idx].segment;

            // Create encoding context
            let mut context = EncodingContext {
                source: &*source, // Convert &mut to & temporarily
                eval_context,
            };

            // Get reference to line for encoding
            let line = &source.files[file_idx].lines[line_idx];

            // Encode the line
            let (bytes, actual_size) = match segment {
                Segment::Text | Segment::Data => {
                    let bytes = encode_line(
                        line,
                        &mut context,
                        source.uses_global_pointer,
                    )?;
                    let size = bytes.len() as u32;
                    (Some(bytes), size)
                }
                Segment::Bss => {
                    let size = encode_bss_line(line, &mut context)?;
                    (None, size)
                }
            };

            // Update the line's size if it changed
            if old_size != actual_size {
                source.files[file_idx].lines[line_idx].size = actual_size;
                *any_changed = true;
            }

            // Collect bytes for output
            if let Some(bytes) = bytes {
                match segment {
                    Segment::Text => text_bytes.extend_from_slice(&bytes),
                    Segment::Data => data_bytes.extend_from_slice(&bytes),
                    Segment::Bss => unreachable!(),
                }
            } else {
                bss_size += actual_size as i64;
            }
        }
    }

    Ok((text_bytes, data_bytes, bss_size))
}

/// Encode a single line and return the generated bytes
pub fn encode_line(
    line: &Line,
    context: &mut EncodingContext,
    uses_global_pointer: bool,
) -> Result<Vec<u8>> {
    // BSS segment should be handled separately in encode_source
    assert!(
        line.segment != Segment::Bss,
        "BSS lines should be handled separately"
    );

    // Encode based on line content
    match &line.content {
        LineContent::Label(_) => Ok(Vec::new()), // Labels don't generate code

        LineContent::Instruction(inst) => {
            encode_instruction(inst, line, context, uses_global_pointer)
        }

        LineContent::Directive(dir) => encode_directive(dir, line, context),
    }
}

// ============================================================================
// BSS Segment Encoding
// ============================================================================

/// Special encoder for .bss segment lines
/// Returns Ok(size) for valid content, error otherwise
fn encode_bss_line(line: &Line, context: &mut EncodingContext) -> Result<u32> {
    match &line.content {
        LineContent::Label(_) => Ok(0),

        LineContent::Directive(directive) => match directive {
            Directive::Space(expr) => {
                let val = eval_expr(expr, line, context.eval_context)?;
                let size =
                    require_integer(val, ".space in .bss", &line.location)?;
                if size < 0 {
                    return Err(AssemblerError::from_context(
                        format!(".space size cannot be negative: {}", size),
                        line.location.clone(),
                    ));
                }
                Ok(size as u32)
            }

            Directive::Balign(_) => {
                // Return the already-computed size (padding)
                Ok(line.size)
            }

            // Non-data directives are allowed but generate no bytes
            Directive::Text
            | Directive::Data
            | Directive::Bss
            | Directive::Global(_)
            | Directive::Equ(_, _) => Ok(0),

            // Data directives are errors in .bss
            Directive::Byte(_)
            | Directive::TwoByte(_)
            | Directive::FourByte(_)
            | Directive::String(_)
            | Directive::Asciz(_) => Err(AssemblerError::from_context(
                format!(
                    "{} directive not allowed in .bss segment (use .data for initialized data)",
                    directive_name(directive)
                ),
                line.location.clone(),
            )),
        },

        LineContent::Instruction(_) => Err(AssemblerError::from_context(
            "Instructions not allowed in .bss segment (use .text for code)"
                .to_string(),
            line.location.clone(),
        )),
    }
}

/// Get a human-readable name for a directive
fn directive_name(directive: &Directive) -> &str {
    match directive {
        Directive::Byte(_) => ".byte",
        Directive::TwoByte(_) => ".2byte",
        Directive::FourByte(_) => ".4byte",
        Directive::String(_) => ".string",
        Directive::Asciz(_) => ".asciz",
        Directive::Space(_) => ".space",
        Directive::Balign(_) => ".balign",
        Directive::Text => ".text",
        Directive::Data => ".data",
        Directive::Bss => ".bss",
        Directive::Global(_) => ".global",
        Directive::Equ(_, _) => ".equ",
    }
}

// ============================================================================
// Type Checking Helpers
// ============================================================================

/// Require that a value is an Integer, return error if Address (RV32: returns i64 for compatibility)
fn require_integer(
    val: EvaluatedValue,
    context: &str,
    location: &crate::ast::Location,
) -> Result<i64> {
    match val {
        EvaluatedValue::Integer(i) => Ok(i as i64),
        EvaluatedValue::Address(_) => Err(AssemblerError::from_context(
            format!("{}: expected Integer, got Address", context),
            location.clone(),
        )),
    }
}

/// Require that a value is an Address, return error if Integer (RV32: returns u32 as i64 for compatibility)
fn require_address(
    val: EvaluatedValue,
    context: &str,
    location: &crate::ast::Location,
) -> Result<i64> {
    match val {
        EvaluatedValue::Address(a) => Ok(a as i64),
        EvaluatedValue::Integer(_) => Err(AssemblerError::from_context(
            format!("{}: expected Address, got Integer", context),
            location.clone(),
        )),
    }
}

// ============================================================================
// Register Conversion
// ============================================================================

/// Convert Register enum to u32 for encoding
fn reg_to_u32(reg: Register) -> u32 {
    match reg {
        Register::X0 => 0,
        Register::X1 => 1,
        Register::X2 => 2,
        Register::X3 => 3,
        Register::X4 => 4,
        Register::X5 => 5,
        Register::X6 => 6,
        Register::X7 => 7,
        Register::X8 => 8,
        Register::X9 => 9,
        Register::X10 => 10,
        Register::X11 => 11,
        Register::X12 => 12,
        Register::X13 => 13,
        Register::X14 => 14,
        Register::X15 => 15,
        Register::X16 => 16,
        Register::X17 => 17,
        Register::X18 => 18,
        Register::X19 => 19,
        Register::X20 => 20,
        Register::X21 => 21,
        Register::X22 => 22,
        Register::X23 => 23,
        Register::X24 => 24,
        Register::X25 => 25,
        Register::X26 => 26,
        Register::X27 => 27,
        Register::X28 => 28,
        Register::X29 => 29,
        Register::X30 => 30,
        Register::X31 => 31,
    }
}

// ============================================================================
// Immediate Validation
// ============================================================================

/// Check if value fits in N-bit signed field
fn fits_signed(value: i64, bits: u32) -> bool {
    let min = -(1i64 << (bits - 1));
    let max = (1i64 << (bits - 1)) - 1;
    value >= min && value <= max
}

/// Validate 12-bit signed immediate (I-type, S-type)
fn check_i_imm(imm: i64, location: &crate::ast::Location) -> Result<()> {
    if !fits_signed(imm, 12) {
        return Err(AssemblerError::from_context(
            format!(
                "Immediate {} out of range (must fit in 12-bit signed: -2048 to 2047)",
                imm
            ),
            location.clone(),
        ));
    }
    Ok(())
}

/// Validate 13-bit signed offset for branches (must be 2-byte aligned)
fn check_b_imm(offset: i64, location: &crate::ast::Location) -> Result<()> {
    if offset % 2 != 0 {
        return Err(AssemblerError::from_context(
            format!("Branch offset {} must be 2-byte aligned", offset),
            location.clone(),
        ));
    }
    if !fits_signed(offset, 13) {
        return Err(AssemblerError::from_context(
            format!(
                "Branch offset {} out of range (must fit in 13-bit signed: ±4 KiB)",
                offset
            ),
            location.clone(),
        ));
    }
    Ok(())
}

/// Validate 21-bit signed offset for JAL (must be 2-byte aligned)
fn check_j_imm(offset: i64, location: &crate::ast::Location) -> Result<()> {
    if offset % 2 != 0 {
        return Err(AssemblerError::from_context(
            format!("Jump offset {} must be 2-byte aligned", offset),
            location.clone(),
        ));
    }
    if !fits_signed(offset, 21) {
        return Err(AssemblerError::from_context(
            format!(
                "Jump offset {} out of range (must fit in 21-bit signed: ±1 MiB)",
                offset
            ),
            location.clone(),
        ));
    }
    Ok(())
}

/// Validate 20-bit immediate for U-type (upper 20 bits of 32-bit value)
fn check_u_imm(imm: i64, location: &crate::ast::Location) -> Result<()> {
    // U-type immediate is the upper 20 bits of a 32-bit value
    // So valid range is 0 to 0xFFFFF (20 bits)
    if !(0..=0xFFFFF).contains(&imm) {
        return Err(AssemblerError::from_context(
            format!(
                "U-type immediate {} out of range (must fit in 20 bits: 0 to 0xFFFFF)",
                imm
            ),
            location.clone(),
        ));
    }
    Ok(())
}

// ============================================================================
// Core Instruction Encoding Functions
// ============================================================================

/// Encode R-type instruction
fn encode_r_type(
    opcode: u32,
    rd: Register,
    funct3: u32,
    rs1: Register,
    rs2: Register,
    funct7: u32,
) -> u32 {
    let rd_bits = reg_to_u32(rd);
    let rs1_bits = reg_to_u32(rs1);
    let rs2_bits = reg_to_u32(rs2);

    opcode
        | (rd_bits << 7)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (rs2_bits << 20)
        | (funct7 << 25)
}

/// Encode I-type instruction
fn encode_i_type(
    opcode: u32,
    rd: Register,
    funct3: u32,
    rs1: Register,
    imm: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    check_i_imm(imm, location)?;

    let rd_bits = reg_to_u32(rd);
    let rs1_bits = reg_to_u32(rs1);
    let imm_bits = (imm as u32) & 0xFFF; // Lower 12 bits

    Ok(opcode
        | (rd_bits << 7)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (imm_bits << 20))
}

/// Encode S-type instruction (stores)
fn encode_s_type(
    opcode: u32,
    funct3: u32,
    rs1: Register,
    rs2: Register,
    imm: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    check_i_imm(imm, location)?;

    let rs1_bits = reg_to_u32(rs1);
    let rs2_bits = reg_to_u32(rs2);
    let imm_bits = imm as u32;
    let imm_low = imm_bits & 0x1F; // imm[4:0]
    let imm_high = (imm_bits >> 5) & 0x7F; // imm[11:5]

    Ok(opcode
        | (imm_low << 7)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (rs2_bits << 20)
        | (imm_high << 25))
}

/// Encode B-type instruction (branches)
fn encode_b_type(
    opcode: u32,
    funct3: u32,
    rs1: Register,
    rs2: Register,
    offset: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    check_b_imm(offset, location)?;

    let rs1_bits = reg_to_u32(rs1);
    let rs2_bits = reg_to_u32(rs2);
    let offset_bits = offset as u32;

    // B-type encoding: imm[12|10:5|4:1|11]
    let imm_11 = (offset_bits >> 11) & 0x1;
    let imm_4_1 = (offset_bits >> 1) & 0xF;
    let imm_10_5 = (offset_bits >> 5) & 0x3F;
    let imm_12 = (offset_bits >> 12) & 0x1;

    Ok(opcode
        | (imm_11 << 7)
        | (imm_4_1 << 8)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (rs2_bits << 20)
        | (imm_10_5 << 25)
        | (imm_12 << 31))
}

/// Encode U-type instruction (lui, auipc)
fn encode_u_type(
    opcode: u32,
    rd: Register,
    imm: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    check_u_imm(imm, location)?;

    let rd_bits = reg_to_u32(rd);
    let imm_bits = (imm as u32) & 0xFFFFF; // Lower 20 bits

    Ok(opcode | (rd_bits << 7) | (imm_bits << 12))
}

/// Encode J-type instruction (jal)
fn encode_j_type(
    opcode: u32,
    rd: Register,
    offset: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    check_j_imm(offset, location)?;

    let rd_bits = reg_to_u32(rd);
    let offset_bits = offset as u32;

    // J-type encoding: imm[20|10:1|11|19:12]
    let imm_10_1 = (offset_bits >> 1) & 0x3FF;
    let imm_11 = (offset_bits >> 11) & 0x1;
    let imm_19_12 = (offset_bits >> 12) & 0xFF;
    let imm_20 = (offset_bits >> 20) & 0x1;

    Ok(opcode
        | (rd_bits << 7)
        | (imm_19_12 << 12)
        | (imm_11 << 20)
        | (imm_10_1 << 21)
        | (imm_20 << 31))
}

/// Convert u32 instruction to little-endian bytes
fn u32_to_le_bytes(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

// ============================================================================
// Instruction Encoding
// ============================================================================

fn encode_instruction(
    inst: &Instruction,
    line: &Line,
    context: &mut EncodingContext,
    uses_global_pointer: bool,
) -> Result<Vec<u8>> {
    match inst {
        Instruction::RType(op, rd, rs1, rs2) => {
            let encoded = encode_r_type_inst(op, *rd, *rs1, *rs2);
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::IType(op, rd, rs1, imm_expr) => {
            // Evaluate immediate and check type
            let val = eval_expr(imm_expr, line, context.eval_context)?;
            let imm = require_integer(val, "I-type immediate", &line.location)?;

            let encoded =
                encode_i_type_inst(op, *rd, *rs1, imm, &line.location)?;
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::BType(op, rs1, rs2, target_expr) => {
            // Evaluate target address and check type
            let val = eval_expr(target_expr, line, context.eval_context)?;
            let target = require_address(val, "Branch target", &line.location)?;

            // Calculate PC-relative offset
            let current_pc = get_line_address(line, context);
            let offset = target - current_pc;

            let encoded =
                encode_b_type_inst(op, *rs1, *rs2, offset, &line.location)?;
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::UType(op, rd, imm_expr) => {
            // Evaluate immediate and check type
            let val = eval_expr(imm_expr, line, context.eval_context)?;
            let imm = require_integer(val, "U-type immediate", &line.location)?;

            let encoded = encode_u_type_inst(op, *rd, imm, &line.location)?;
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::JType(op, rd, target_expr) => {
            // Evaluate target address and check type
            let val = eval_expr(target_expr, line, context.eval_context)?;
            let target = require_address(val, "Jump target", &line.location)?;

            // Calculate PC-relative offset
            let current_pc = get_line_address(line, context);
            let offset = target - current_pc;

            let encoded = encode_j_type_inst(op, *rd, offset, &line.location)?;
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::Special(op) => {
            let encoded = encode_special(op);
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::LoadStore(op, rd, offset_expr, rs) => {
            // Evaluate offset and check type
            let val = eval_expr(offset_expr, line, context.eval_context)?;
            let offset =
                require_integer(val, "Load/Store offset", &line.location)?;

            let encoded =
                encode_load_store(op, *rd, offset, *rs, &line.location)?;
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::Atomic(op, rd, rs1, rs2, ordering) => {
            let encoded = encode_atomic(op, *rd, *rs1, *rs2, ordering);
            Ok(u32_to_le_bytes(encoded))
        }

        Instruction::Compressed(op, operands) => {
            encoder_compressed::encode_compressed(op, operands, &line.location)
        }

        Instruction::Pseudo(pseudo) => {
            encode_pseudo(pseudo, line, context, uses_global_pointer)
        }
    }
}

/// Get the absolute address of a line (returns i64 for compatibility with offset calculations)
fn get_line_address(line: &Line, context: &EncodingContext) -> i64 {
    let segment_start = match line.segment {
        Segment::Text => context.eval_context.text_start,
        Segment::Data => context.eval_context.data_start,
        Segment::Bss => context.eval_context.bss_start,
    };
    (segment_start as i64) + (line.offset as i64)
}

/// Encode R-type instruction with opcode lookup
fn encode_r_type_inst(
    op: &RTypeOp,
    rd: Register,
    rs1: Register,
    rs2: Register,
) -> u32 {
    use crate::ast::RTypeOp::*;

    let (opcode, funct3, funct7) = match op {
        // Base RV64I
        Add => (0b0110011, 0b000, 0b0000000),
        Sub => (0b0110011, 0b000, 0b0100000),
        Sll => (0b0110011, 0b001, 0b0000000),
        Slt => (0b0110011, 0b010, 0b0000000),
        Sltu => (0b0110011, 0b011, 0b0000000),
        Xor => (0b0110011, 0b100, 0b0000000),
        Srl => (0b0110011, 0b101, 0b0000000),
        Sra => (0b0110011, 0b101, 0b0100000),
        Or => (0b0110011, 0b110, 0b0000000),
        And => (0b0110011, 0b111, 0b0000000),

        // M extension
        Mul => (0b0110011, 0b000, 0b0000001),
        Mulh => (0b0110011, 0b001, 0b0000001),
        Mulhsu => (0b0110011, 0b010, 0b0000001),
        Mulhu => (0b0110011, 0b011, 0b0000001),
        Div => (0b0110011, 0b100, 0b0000001),
        Divu => (0b0110011, 0b101, 0b0000001),
        Rem => (0b0110011, 0b110, 0b0000001),
        Remu => (0b0110011, 0b111, 0b0000001),
    };

    encode_r_type(opcode, rd, funct3, rs1, rs2, funct7)
}

/// Encode I-type instruction with opcode lookup
fn encode_i_type_inst(
    op: &ITypeOp,
    rd: Register,
    rs1: Register,
    imm: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    use crate::ast::ITypeOp::*;

    let (opcode, funct3) = match op {
        Addi => (0b0010011, 0b000),
        Slti => (0b0010011, 0b010),
        Sltiu => (0b0010011, 0b011),
        Xori => (0b0010011, 0b100),
        Ori => (0b0010011, 0b110),
        Andi => (0b0010011, 0b111),
        Slli => (0b0010011, 0b001), // Note: upper bits of imm must be 0
        Srli => (0b0010011, 0b101), // Note: upper bits of imm must be 0
        Srai => (0b0010011, 0b101), // Note: upper bits of imm must be 0x20
        Jalr => (0b1100111, 0b000),
    };

    // For shift instructions, validate that shift amount fits in 5 bits (RV32)
    let imm_to_encode = match op {
        Slli | Srli | Srai => {
            if !(0..32).contains(&imm) {
                return Err(AssemblerError::from_context(
                    format!(
                        "Shift amount {} out of range (must be 0-31 for RV32)",
                        imm
                    ),
                    location.clone(),
                ));
            }
            // For srai, set bit 10 (0x400)
            if matches!(op, Srai) { imm | 0x400 } else { imm }
        }
        _ => imm,
    };

    encode_i_type(opcode, rd, funct3, rs1, imm_to_encode, location)
}

/// Encode B-type instruction with opcode lookup
fn encode_b_type_inst(
    op: &BTypeOp,
    rs1: Register,
    rs2: Register,
    offset: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    use crate::ast::BTypeOp::*;

    let (opcode, funct3) = match op {
        Beq => (0b1100011, 0b000),
        Bne => (0b1100011, 0b001),
        Blt => (0b1100011, 0b100),
        Bge => (0b1100011, 0b101),
        Bltu => (0b1100011, 0b110),
        Bgeu => (0b1100011, 0b111),
    };

    encode_b_type(opcode, funct3, rs1, rs2, offset, location)
}

/// Encode U-type instruction with opcode lookup
fn encode_u_type_inst(
    op: &UTypeOp,
    rd: Register,
    imm: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    use crate::ast::UTypeOp::*;

    let opcode = match op {
        Lui => 0b0110111,
        Auipc => 0b0010111,
    };

    encode_u_type(opcode, rd, imm, location)
}

/// Encode J-type instruction with opcode lookup
fn encode_j_type_inst(
    op: &JTypeOp,
    rd: Register,
    offset: i64,
    location: &crate::ast::Location,
) -> Result<u32> {
    use crate::ast::JTypeOp::*;

    let opcode = match op {
        Jal => 0b1101111,
    };

    encode_j_type(opcode, rd, offset, location)
}

/// Encode special instructions
fn encode_special(op: &SpecialOp) -> u32 {
    use crate::ast::SpecialOp::*;

    match op {
        Ecall => 0x00000073,
        Ebreak => 0x00100073,
    }
}

/// Encode load/store instructions
fn encode_load_store(
    op: &LoadStoreOp,
    rd: Register,
    offset: i64,
    rs: Register,
    location: &crate::ast::Location,
) -> Result<u32> {
    use crate::ast::LoadStoreOp::*;

    let (opcode, funct3, is_store) = match op {
        // Loads
        Lb => (0b0000011, 0b000, false),
        Lh => (0b0000011, 0b001, false),
        Lw => (0b0000011, 0b010, false),
        Lbu => (0b0000011, 0b100, false),
        Lhu => (0b0000011, 0b101, false),

        // Stores
        Sb => (0b0100011, 0b000, true),
        Sh => (0b0100011, 0b001, true),
        Sw => (0b0100011, 0b010, true),
    };

    if is_store {
        // For stores, rd is actually rs2 (source register)
        encode_s_type(opcode, funct3, rs, rd, offset, location)
    } else {
        // For loads, rd is destination, rs is base
        encode_i_type(opcode, rd, funct3, rs, offset, location)
    }
}

/// Encode atomic instructions (A extension)
///
/// Format: R-type with special fields
/// - opcode: 0b0101111 (AMO)
/// - funct3: 010 (W=32-bit), 011 (D=64-bit)
/// - funct5 (bits 31-27): operation type
/// - aq (bit 26): acquire ordering
/// - rl (bit 25): release ordering
/// - rs2: source register (unused for LR, must be x0)
/// - rs1: address register
/// - rd: destination register
fn encode_atomic(
    op: &crate::ast::AtomicOp,
    rd: Register,
    rs1: Register,
    rs2: Register,
    ordering: &crate::ast::MemoryOrdering,
) -> u32 {
    use crate::ast::AtomicOp::*;
    use crate::ast::MemoryOrdering;

    // Determine funct5 based on operation
    let funct5 = match op {
        LrW => 0b00010,
        ScW => 0b00011,
        AmoswapW => 0b00001,
        AmoaddW => 0b00000,
        AmoxorW => 0b00100,
        AmoandW => 0b01100,
        AmoorW => 0b01000,
        AmominW => 0b10000,
        AmomaxW => 0b10100,
        AmominuW => 0b11000,
        AmomaxuW => 0b11100,
    };

    // Parse ordering bits
    let (aq, rl) = match ordering {
        MemoryOrdering::None => (0, 0),
        MemoryOrdering::Aq => (1, 0),
        MemoryOrdering::Rel => (0, 1),
        MemoryOrdering::AqRl => (1, 1),
    };

    // Common parameters for all atomic ops
    let opcode = 0b0101111; // AMO
    let funct3 = 0b010; // W (32-bit width)

    let rd_bits = reg_to_u32(rd);
    let rs1_bits = reg_to_u32(rs1);
    let rs2_bits = reg_to_u32(rs2);

    // Encode: funct5 | aq | rl | rs2 | rs1 | funct3 | rd | opcode
    opcode
        | (rd_bits << 7)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (rs2_bits << 20)
        | (rl << 25)
        | (aq << 26)
        | (funct5 << 27)
}

// ============================================================================
// Pseudo-Instruction Encoding
// ============================================================================

fn encode_pseudo(
    pseudo: &PseudoOp,
    line: &Line,
    context: &mut EncodingContext,
    uses_global_pointer: bool,
) -> Result<Vec<u8>> {
    match pseudo {
        PseudoOp::Li(rd, imm_expr) => {
            // Evaluate immediate and check type
            let val = eval_expr(imm_expr, line, context.eval_context)?;
            let imm =
                require_integer(val, "li pseudo-instruction", &line.location)?;

            expand_li(*rd, imm, &line.location)
        }

        PseudoOp::La(rd, addr_expr) => {
            // Evaluate address and check type
            let val = eval_expr(addr_expr, line, context.eval_context)?;
            let addr =
                require_address(val, "la pseudo-instruction", &line.location)?;

            let current_pc = get_line_address(line, context);
            let gp = (context.eval_context.data_start as i64) + 2048;

            expand_la(
                *rd,
                addr,
                current_pc,
                gp,
                &line.location,
                uses_global_pointer,
            )
        }

        PseudoOp::Call(target_expr) => {
            // Evaluate target address and check type
            let val = eval_expr(target_expr, line, context.eval_context)?;
            let target = require_address(
                val,
                "call pseudo-instruction",
                &line.location,
            )?;

            let current_pc = get_line_address(line, context);
            expand_call(target, current_pc, &line.location)
        }

        PseudoOp::Tail(target_expr) => {
            // Evaluate target address and check type
            let val = eval_expr(target_expr, line, context.eval_context)?;
            let target = require_address(
                val,
                "tail pseudo-instruction",
                &line.location,
            )?;

            let current_pc = get_line_address(line, context);
            expand_tail(target, current_pc, &line.location)
        }

        PseudoOp::LoadGlobal(op, rd, addr_expr) => {
            // Evaluate address and check type
            let val = eval_expr(addr_expr, line, context.eval_context)?;
            let addr = require_address(
                val,
                "load global pseudo-instruction",
                &line.location,
            )?;

            let current_pc = get_line_address(line, context);
            expand_load_global(op, *rd, addr, current_pc, &line.location)
        }

        PseudoOp::StoreGlobal(op, rs, addr_expr, temp) => {
            // Evaluate address and check type
            let val = eval_expr(addr_expr, line, context.eval_context)?;
            let addr = require_address(
                val,
                "store global pseudo-instruction",
                &line.location,
            )?;

            let current_pc = get_line_address(line, context);
            expand_store_global(
                op,
                *rs,
                addr,
                *temp,
                current_pc,
                &line.location,
            )
        }
    }
}

// ============================================================================
// Pseudo-Instruction Expansion Helpers
// ============================================================================

/// Split a 32-bit offset into upper 20 bits and lower 12 bits for auipc + addi
fn split_offset_hi_lo(offset: i64) -> (i64, i64) {
    // Add 0x800 to account for sign extension of lower 12 bits
    let hi = (offset + 0x800) >> 12;
    let lo = offset & 0xFFF;
    // Sign-extend the lower 12 bits
    let lo_signed = if lo & 0x800 != 0 { lo | !0xFFF } else { lo };
    (hi, lo_signed)
}

/// Expand `li rd, imm` - Load Immediate
fn expand_li(
    rd: Register,
    imm: i64,
    location: &crate::ast::Location,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    // Case 1: Fits in 12-bit signed immediate
    if fits_signed(imm, 12) {
        // addi rd, x0, imm
        let inst =
            encode_i_type(0b0010011, rd, 0b000, Register::X0, imm, location)?;
        bytes.extend_from_slice(&u32_to_le_bytes(inst));
        return Ok(bytes);
    }

    // Case 2: Fits in 32 bits
    if imm >= i32::MIN as i64 && imm <= i32::MAX as i64 {
        let imm_32 = imm as i32 as u32;
        let upper = (imm_32 >> 12) as i64;
        let lower = (imm_32 & 0xFFF) as i64;

        // Check if lower 12 bits are sign-extended (bit 11 set)
        let adjusted_upper = if lower & 0x800 != 0 { upper + 1 } else { upper };

        // lui rd, upper
        let lui_inst =
            encode_u_type(0b0110111, rd, adjusted_upper & 0xFFFFF, location)?;
        bytes.extend_from_slice(&u32_to_le_bytes(lui_inst));

        // Sign-extend lower 12 bits for addi
        let lower_signed =
            if lower & 0x800 != 0 { lower | !0xFFF } else { lower };

        // addi rd, rd, lower
        let addi_inst =
            encode_i_type(0b0010011, rd, 0b000, rd, lower_signed, location)?;
        bytes.extend_from_slice(&u32_to_le_bytes(addi_inst));

        return Ok(bytes);
    }

    // Case 3: Full 64-bit value - build it step by step
    // This is complex - for now, return an error
    Err(AssemblerError::from_context(
        format!("64-bit immediate values not yet supported for li: {}", imm),
        location.clone(),
    ))
}

/// Expand `la rd, symbol` - Load Address
fn expand_la(
    rd: Register,
    addr: i64,
    current_pc: i64,
    gp: i64,
    location: &crate::ast::Location,
    uses_global_pointer: bool,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    // Special case: la gp, __global_pointer$ must use PC-relative
    let is_gp_init = rd == Register::X3 && addr == gp;

    // Check if we can use GP-relative addressing (only if __global_pointer$ is referenced)
    let gp_offset = addr - gp;
    if !is_gp_init && uses_global_pointer && fits_signed(gp_offset, 12) {
        // addi rd, gp, offset
        let inst = encode_i_type(
            0b0010011,
            rd,
            0b000,
            Register::X3,
            gp_offset,
            location,
        )?;
        bytes.extend_from_slice(&u32_to_le_bytes(inst));
        return Ok(bytes);
    }

    // Use PC-relative addressing: auipc + addi
    let offset = addr - current_pc;
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc rd, hi
    let auipc_inst = encode_u_type(0b0010111, rd, hi & 0xFFFFF, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(auipc_inst));

    // addi rd, rd, lo
    let addi_inst = encode_i_type(0b0010011, rd, 0b000, rd, lo, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(addi_inst));

    Ok(bytes)
}

/// Expand `call target` - Call subroutine
///
/// Relaxation optimization:
/// - If target is within ±1 MiB (fits in 21-bit signed immediate), use `jal ra, offset` (4 bytes)
/// - Otherwise, use `auipc ra, hi; jalr ra, ra, lo` (8 bytes)
fn expand_call(
    target: i64,
    current_pc: i64,
    location: &crate::ast::Location,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    let offset = target - current_pc;

    // Try optimized encoding: single JAL if offset fits in 21 bits
    if fits_signed(offset, 21) && offset % 2 == 0 {
        // jal ra, offset (single 4-byte instruction)
        let jal_inst =
            encode_j_type(0b1101111, Register::X1, offset, location)?;
        bytes.extend_from_slice(&u32_to_le_bytes(jal_inst));
        return Ok(bytes);
    }

    // Fall back to full 8-byte encoding for far targets
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc ra, hi
    let auipc_inst =
        encode_u_type(0b0010111, Register::X1, hi & 0xFFFFF, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(auipc_inst));

    // jalr ra, ra, lo
    let jalr_inst = encode_i_type(
        0b1100111,
        Register::X1,
        0b000,
        Register::X1,
        lo,
        location,
    )?;
    bytes.extend_from_slice(&u32_to_le_bytes(jalr_inst));

    Ok(bytes)
}

/// Expand `tail target` - Tail call
///
/// Relaxation optimization:
/// - If target is within ±1 MiB (fits in 21-bit signed immediate), use `jal x0, offset` (4 bytes)
///   which is the `j offset` pseudo-instruction
/// - Otherwise, use `auipc t1, hi; jalr x0, t1, lo` (8 bytes)
fn expand_tail(
    target: i64,
    current_pc: i64,
    location: &crate::ast::Location,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    let offset = target - current_pc;

    // Try optimized encoding: single JAL (j offset) if offset fits in 21 bits
    if fits_signed(offset, 21) && offset % 2 == 0 {
        // jal x0, offset (j offset - single 4-byte instruction)
        let jal_inst =
            encode_j_type(0b1101111, Register::X0, offset, location)?;
        bytes.extend_from_slice(&u32_to_le_bytes(jal_inst));
        return Ok(bytes);
    }

    // Fall back to full 8-byte encoding for far targets
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc t1, hi
    let auipc_inst =
        encode_u_type(0b0010111, Register::X6, hi & 0xFFFFF, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(auipc_inst));

    // jalr x0, t1, lo
    let jalr_inst = encode_i_type(
        0b1100111,
        Register::X0,
        0b000,
        Register::X6,
        lo,
        location,
    )?;
    bytes.extend_from_slice(&u32_to_le_bytes(jalr_inst));

    Ok(bytes)
}

/// Expand load global pseudo-instruction
fn expand_load_global(
    op: &LoadStoreOp,
    rd: Register,
    addr: i64,
    current_pc: i64,
    location: &crate::ast::Location,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    let offset = addr - current_pc;
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc rd, hi
    let auipc_inst = encode_u_type(0b0010111, rd, hi & 0xFFFFF, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(auipc_inst));

    // load rd, lo(rd)
    let load_inst = encode_load_store(op, rd, lo, rd, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(load_inst));

    Ok(bytes)
}

/// Expand store global pseudo-instruction
fn expand_store_global(
    op: &LoadStoreOp,
    rs: Register,
    addr: i64,
    temp: Register,
    current_pc: i64,
    location: &crate::ast::Location,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    let offset = addr - current_pc;
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc temp, hi
    let auipc_inst = encode_u_type(0b0010111, temp, hi & 0xFFFFF, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(auipc_inst));

    // store rs, lo(temp)
    let store_inst = encode_load_store(op, rs, lo, temp, location)?;
    bytes.extend_from_slice(&u32_to_le_bytes(store_inst));

    Ok(bytes)
}

// ============================================================================
// Directive Encoding
// ============================================================================

fn encode_directive(
    dir: &Directive,
    line: &Line,
    context: &mut EncodingContext,
) -> Result<Vec<u8>> {
    match dir {
        // Non-data directives generate no bytes
        Directive::Text
        | Directive::Data
        | Directive::Bss
        | Directive::Global(_)
        | Directive::Equ(_, _) => Ok(Vec::new()),

        Directive::Byte(exprs) => {
            let mut bytes = Vec::new();
            for expr in exprs {
                let val = eval_expr(expr, line, context.eval_context)?;
                // Allow both Integer and Address types, use the numeric value
                let byte_val = match val {
                    EvaluatedValue::Integer(i) => i as u8,
                    EvaluatedValue::Address(a) => a as u8,
                };
                bytes.push(byte_val);
            }
            Ok(bytes)
        }

        Directive::TwoByte(exprs) => {
            let mut bytes = Vec::new();
            for expr in exprs {
                let val = eval_expr(expr, line, context.eval_context)?;
                let short_val = match val {
                    EvaluatedValue::Integer(i) => i as u16,
                    EvaluatedValue::Address(a) => a as u16,
                };
                bytes.extend_from_slice(&short_val.to_le_bytes());
            }
            Ok(bytes)
        }

        Directive::FourByte(exprs) => {
            let mut bytes = Vec::new();
            for expr in exprs {
                let val = eval_expr(expr, line, context.eval_context)?;
                let word_val = match val {
                    EvaluatedValue::Integer(i) => i as u32,
                    EvaluatedValue::Address(a) => a,
                };
                bytes.extend_from_slice(&word_val.to_le_bytes());
            }
            Ok(bytes)
        }

        Directive::String(strings) => {
            let mut bytes = Vec::new();
            for s in strings {
                bytes.extend_from_slice(s.as_bytes());
            }
            Ok(bytes)
        }

        Directive::Asciz(strings) => {
            let mut bytes = Vec::new();
            for s in strings {
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(0); // Null terminator
            }
            Ok(bytes)
        }

        Directive::Space(expr) => {
            let val = eval_expr(expr, line, context.eval_context)?;
            let size =
                require_integer(val, ".space directive", &line.location)?;

            if size < 0 {
                return Err(AssemblerError::from_context(
                    format!(".space size cannot be negative: {}", size),
                    line.location.clone(),
                ));
            }

            Ok(vec![0; size as usize])
        }

        Directive::Balign(_expr) => {
            // Padding size is already computed in line.size
            // Just emit that many zero bytes
            Ok(vec![0; line.size as usize])
        }
    }
}
