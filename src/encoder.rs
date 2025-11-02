// encoder.rs
//
// RISC-V instruction encoder with unified relaxation
//
// This module encodes RISC-V instructions with integrated support for all
// relaxations (compressed instructions, optimized pseudo-instructions, GP-relative).
// Each instruction family is handled in one place with all encoding variants inline.

#![allow(clippy::unusual_byte_groupings)]
#![allow(clippy::too_many_arguments)]

use crate::ast::{
    AtomicOp, BTypeOp, CompressedOp, CompressedOperands, Directive, Expression,
    ITypeOp, Instruction, Line, LineContent, LinePointer, LoadStoreOp,
    Location, MemoryOrdering, PseudoOp, RTypeOp, Register, Segment, Source,
    SpecialOp, UTypeOp,
};
use crate::config::Config;
use crate::error::{AssemblerError, Result};
use crate::expressions::{EvaluatedValue, SymbolValues, eval_expr};
use crate::layout::{Layout, LineLayout};
use crate::symbols::SymbolLinks;

// ============================================================================
// Public API
// ============================================================================

/// Encode all lines and track if any sizes changed
///
/// Returns (any_changed, text_bytes, data_bytes, bss_size) where any_changed
/// is true if any line's actual size differs from its guessed size.
pub fn encode(
    config: &Config,
    source: &Source,
    symbol_links: &SymbolLinks,
    symbol_values: &SymbolValues,
    layout: &mut Layout,
) -> Result<(bool, Vec<u8>, Vec<u8>, u32)> {
    let mut text_bytes = Vec::new();
    let mut data_bytes = Vec::new();
    let mut bss_size: u32 = 0;
    let mut any_changed = false;

    for file_index in 0..source.files.len() {
        for line_index in 0..source.files[file_index].lines.len() {
            let pointer = LinePointer { file_index, line_index };
            let &LineLayout { segment, offset, size } = layout.get(pointer);
            let current_address = layout.get_line_address(pointer);
            let data_start = layout.data_start;

            let line = &source.files[file_index].lines[line_index];

            let (bytes, actual_size) = match segment {
                Segment::Text | Segment::Data => {
                    let bytes = encode_line(
                        config,
                        source,
                        symbol_links,
                        symbol_values,
                        line,
                        pointer,
                        current_address,
                        data_start,
                    )?;
                    let size = bytes.len() as u32;
                    (Some(bytes), size)
                }
                Segment::Bss => {
                    let size = encode_bss_line(
                        source,
                        symbol_links,
                        symbol_values,
                        line,
                        pointer,
                        current_address,
                    )?;
                    (None, size)
                }
            };

            if size != actual_size {
                let updated_layout =
                    LineLayout { segment, offset, size: actual_size };
                layout.set(pointer, updated_layout);
                any_changed = true;
            }

            if let Some(bytes) = bytes {
                match segment {
                    Segment::Text => text_bytes.extend_from_slice(&bytes),
                    Segment::Data => data_bytes.extend_from_slice(&bytes),
                    Segment::Bss => unreachable!(),
                }
            } else {
                bss_size += actual_size;
            }
        }
    }

    Ok((any_changed, text_bytes, data_bytes, bss_size))
}

/// Encode a single line
fn encode_line(
    config: &Config,
    source: &Source,
    symbol_links: &SymbolLinks,
    symbol_values: &SymbolValues,
    line: &Line,
    pointer: LinePointer,
    current_address: u32,
    data_start: u32,
) -> Result<Vec<u8>> {
    match &line.content {
        LineContent::Label(_) => Ok(Vec::new()),
        LineContent::Instruction(inst) => encode_instruction(
            config,
            source,
            symbol_links,
            symbol_values,
            line,
            pointer,
            inst,
            current_address,
            data_start,
        ),
        LineContent::Directive(dir) => encode_directive(
            dir,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
        ),
    }
}

/// Encode BSS segment line (must be space directive only)
fn encode_bss_line(
    source: &Source,
    symbol_links: &SymbolLinks,
    symbol_values: &SymbolValues,
    line: &Line,
    pointer: LinePointer,
    current_address: u32,
) -> Result<u32> {
    match &line.content {
        LineContent::Label(_) => Ok(0),
        LineContent::Directive(Directive::Space(expr)) => {
            let refs = symbol_links.get_line_refs(pointer);
            let val = eval_expr(
                expr,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let size =
                require_integer(val, ".space directive", &line.location)?;
            if size < 0 {
                return Err(AssemblerError::from_context(
                    format!(".space size cannot be negative: {}", size),
                    line.location.clone(),
                ));
            }
            Ok(size as u32)
        }
        // Segment directives themselves don't produce bytes in BSS
        LineContent::Directive(Directive::Text)
        | LineContent::Directive(Directive::Data)
        | LineContent::Directive(Directive::Bss)
        | LineContent::Directive(Directive::Global(_))
        | LineContent::Directive(Directive::Equ(_, _)) => Ok(0),
        LineContent::Directive(dir) => {
            let dir_name = match dir {
                Directive::Byte(_) => ".byte",
                Directive::TwoByte(_) => ".2byte",
                Directive::FourByte(_) => ".4byte",
                Directive::String(_) => ".string",
                Directive::Asciz(_) => ".asciz",
                Directive::Balign(_) => ".balign",
                _ => "directive",
            };
            Err(AssemblerError::from_context(
                format!(
                    "{} not allowed in .bss segment (only .space is allowed)",
                    dir_name
                ),
                line.location.clone(),
            ))
        }
        LineContent::Instruction(_) => Err(AssemblerError::from_context(
            "Instructions not allowed in .bss segment (only .space is allowed)"
                .to_string(),
            line.location.clone(),
        )),
    }
}

// ============================================================================
// Instruction Encoding Dispatcher
// ============================================================================

fn encode_instruction(
    config: &Config,
    source: &Source,
    symbol_links: &SymbolLinks,
    symbol_values: &SymbolValues,
    line: &Line,
    pointer: LinePointer,
    inst: &Instruction,
    current_address: u32,
    data_start: u32,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);

    match inst {
        Instruction::RType(op, rd, rs1, rs2) => {
            encode_r_type_family(config, op, *rd, *rs1, *rs2)
        }
        Instruction::IType(op, rd, rs1, imm) => {
            let val = eval_expr(
                imm,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let imm_val =
                require_integer(val, "I-type immediate", &line.location)?;
            encode_i_type_family(config, &line.location, op, *rd, *rs1, imm_val)
        }
        Instruction::LoadStore(op, rd_or_rs, offset, rs1) => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val =
                require_integer(val, "Load/Store offset", &line.location)?;
            encode_load_store_family(
                op,
                *rd_or_rs,
                *rs1,
                offset_val,
                &line.location,
                config,
            )
        }
        Instruction::BType(op, rs1, rs2, target) => {
            let target_val = eval_expr(
                target,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let target_addr =
                require_address(target_val, "Branch target", &line.location)?;
            let current_pc = current_address as i64;
            let offset = target_addr as i64 - current_pc;
            encode_branch_family(op, *rs1, *rs2, offset, &line.location, config)
        }
        Instruction::UType(op, rd, imm) => {
            let val = eval_expr(
                imm,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let imm_val =
                require_integer(val, "U-type immediate", &line.location)?;
            encode_u_type_family(op, *rd, imm_val, &line.location)
        }
        Instruction::JType(_op, rd, target) => {
            let target_val = eval_expr(
                target,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let target_addr =
                require_address(target_val, "Jump target", &line.location)?;
            let current_pc = current_address as i64;
            let offset = target_addr as i64 - current_pc;
            encode_jal_family(*rd, offset, &line.location, config)
        }
        Instruction::Pseudo(pseudo_op) => encode_pseudo(
            pseudo_op,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
            data_start,
            config,
        ),
        Instruction::Atomic(op, rd, rs1, rs2, ordering) => {
            encode_atomic(op, *rd, *rs1, *rs2, ordering)
        }
        Instruction::Special(op) => encode_special(op),
        Instruction::Compressed(op, operands) => encode_compressed_explicit(
            op,
            operands,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
        ),
    }
}

// ============================================================================
// R-Type Instruction Family
// ============================================================================

fn encode_r_type_family(
    config: &Config,
    op: &RTypeOp,
    rd: Register,
    rs1: Register,
    rs2: Register,
) -> Result<Vec<u8>> {
    // Try compressed encoding if enabled
    if config.relax.compressed {
        // c.add rd, rs2 (rd is also rs1, rs2 != x0, rd != x0)
        if matches!(op, RTypeOp::Add)
            && rd == rs1
            && rs2 != Register::X0
            && rd != Register::X0
        {
            return Ok(encode_c_add(rd, rs2).to_le_bytes().to_vec());
        }

        // c.mv rd, rs2 (rd != rs2, rs1 == x0, rd != x0, rs2 != x0)
        // This handles: add rd, x0, rs2 (copy rs2 to rd)
        if matches!(op, RTypeOp::Add)
            && rs1 == Register::X0
            && rd != Register::X0
            && rs2 != Register::X0
        {
            return Ok(encode_c_mv(rd, rs2).to_le_bytes().to_vec());
        }

        // c.sub, c.and, c.or, c.xor (compressed register set only)
        if is_compressed_reg(rd) && rd == rs1 && is_compressed_reg(rs2) {
            let c_inst = match op {
                RTypeOp::Sub => Some(encode_c_sub(rd, rs2)),
                RTypeOp::And => Some(encode_c_and(rd, rs2)),
                RTypeOp::Or => Some(encode_c_or(rd, rs2)),
                RTypeOp::Xor => Some(encode_c_xor(rd, rs2)),
                _ => None,
            };
            if let Some(inst) = c_inst {
                return Ok(inst.to_le_bytes().to_vec());
            }
        }
    }

    // Base encoding
    let (opcode, funct3, funct7) = match op {
        RTypeOp::Add => (0b0110011, 0b000, 0b0000000),
        RTypeOp::Sub => (0b0110011, 0b000, 0b0100000),
        RTypeOp::Sll => (0b0110011, 0b001, 0b0000000),
        RTypeOp::Slt => (0b0110011, 0b010, 0b0000000),
        RTypeOp::Sltu => (0b0110011, 0b011, 0b0000000),
        RTypeOp::Xor => (0b0110011, 0b100, 0b0000000),
        RTypeOp::Srl => (0b0110011, 0b101, 0b0000000),
        RTypeOp::Sra => (0b0110011, 0b101, 0b0100000),
        RTypeOp::Or => (0b0110011, 0b110, 0b0000000),
        RTypeOp::And => (0b0110011, 0b111, 0b0000000),
        RTypeOp::Mul => (0b0110011, 0b000, 0b0000001),
        RTypeOp::Mulh => (0b0110011, 0b001, 0b0000001),
        RTypeOp::Mulhsu => (0b0110011, 0b010, 0b0000001),
        RTypeOp::Mulhu => (0b0110011, 0b011, 0b0000001),
        RTypeOp::Div => (0b0110011, 0b100, 0b0000001),
        RTypeOp::Divu => (0b0110011, 0b101, 0b0000001),
        RTypeOp::Rem => (0b0110011, 0b110, 0b0000001),
        RTypeOp::Remu => (0b0110011, 0b111, 0b0000001),
    };

    let inst = encode_r_type(opcode, rd, funct3, rs1, rs2, funct7);
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// I-Type Instruction Family
// ============================================================================

fn encode_i_type_family(
    config: &Config,
    location: &Location,
    op: &ITypeOp,
    rd: Register,
    rs1: Register,
    imm: i64,
) -> Result<Vec<u8>> {
    // Special handling for JALR (it can become c.jr or c.jalr)
    if matches!(op, ITypeOp::Jalr) {
        return encode_jalr_family(config, location, rd, rs1, imm);
    }

    // Try compressed encoding if enabled
    if config.relax.compressed {
        // c.addi rd, imm (rd == rs1, rd != x0, imm fits in 6-bit signed)
        if matches!(op, ITypeOp::Addi)
            && rd == rs1
            && rd != Register::X0
            && fits_signed(imm, 6)
        {
            return Ok(encode_c_addi(rd, imm as i32).to_le_bytes().to_vec());
        }

        // c.li rd, imm (rs1 == x0, rd != x0, imm fits in 6-bit signed)
        if matches!(op, ITypeOp::Addi)
            && rs1 == Register::X0
            && rd != Register::X0
            && fits_signed(imm, 6)
        {
            return Ok(encode_c_li(rd, imm as i32).to_le_bytes().to_vec());
        }

        // c.addi16sp (rd == sp, rs1 == sp, imm in range and aligned)
        if matches!(op, ITypeOp::Addi)
            && rd == Register::X2
            && rs1 == Register::X2
            && imm != 0
            && imm % 16 == 0
            && fits_signed(imm, 10)
        {
            return Ok(encode_c_addi16sp(imm as i32).to_le_bytes().to_vec());
        }

        // c.lwsp rd, offset(sp) (rd != x0, offset aligned and in range)
        // This is lw rd, offset(sp) where offset is 4-byte aligned and fits
        if matches!(op, ITypeOp::Addi) {
            // Wait, ADDI doesn't become LWSP - this is handled in load_store_family
        }

        // c.slli rd, imm (rd == rs1, rd != x0, 0 < imm < 32)
        if matches!(op, ITypeOp::Slli)
            && rd == rs1
            && rd != Register::X0
            && imm > 0
            && imm < 32
        {
            return Ok(encode_c_slli(rd, imm as u32).to_le_bytes().to_vec());
        }

        // c.srli, c.srai, c.andi (compressed register set only)
        if is_compressed_reg(rd) && rd == rs1 {
            let c_inst = match op {
                ITypeOp::Srli if imm > 0 && imm < 32 => {
                    Some(encode_c_srli(rd, imm as u32))
                }
                ITypeOp::Srai if imm > 0 && imm < 32 => {
                    Some(encode_c_srai(rd, imm as u32))
                }
                ITypeOp::Andi if fits_signed(imm, 6) => {
                    Some(encode_c_andi(rd, imm as i32))
                }
                _ => None,
            };
            if let Some(inst) = c_inst {
                return Ok(inst.to_le_bytes().to_vec());
            }
        }
    }

    // Base encoding
    let (opcode, funct3) = match op {
        ITypeOp::Addi => (0b0010011, 0b000),
        ITypeOp::Slti => (0b0010011, 0b010),
        ITypeOp::Sltiu => (0b0010011, 0b011),
        ITypeOp::Xori => (0b0010011, 0b100),
        ITypeOp::Ori => (0b0010011, 0b110),
        ITypeOp::Andi => (0b0010011, 0b111),
        ITypeOp::Slli => (0b0010011, 0b001),
        ITypeOp::Srli => (0b0010011, 0b101),
        ITypeOp::Srai => (0b0010011, 0b101),
        ITypeOp::Jalr => (0b1100111, 0b000),
    };

    // Validate and adjust shift immediates
    let imm_to_encode = match op {
        ITypeOp::Slli | ITypeOp::Srli | ITypeOp::Srai => {
            if !(0..32).contains(&imm) {
                return Err(AssemblerError::from_context(
                    format!(
                        "Shift amount {} out of range (must be 0-31 for RV32)",
                        imm
                    ),
                    location.clone(),
                ));
            }
            if matches!(op, ITypeOp::Srai) { imm | 0x400 } else { imm }
        }
        _ => imm,
    };

    let inst = encode_i_type(opcode, rd, funct3, rs1, imm_to_encode, location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// JALR Instruction Family
// ============================================================================

fn encode_jalr_family(
    config: &Config,
    location: &Location,
    rd: Register,
    rs1: Register,
    offset: i64,
) -> Result<Vec<u8>> {
    // Try compressed encoding if enabled
    if config.relax.compressed && offset == 0 {
        // c.jr rs1 (rd == x0, rs1 != x0, offset == 0)
        if rd == Register::X0 && rs1 != Register::X0 {
            return Ok(encode_c_jr(rs1).to_le_bytes().to_vec());
        }

        // c.jalr rs1 (rd == ra, rs1 != x0, offset == 0)
        if rd == Register::X1 && rs1 != Register::X0 {
            return Ok(encode_c_jalr(rs1).to_le_bytes().to_vec());
        }
    }

    // Base encoding
    let inst = encode_i_type(0b1100111, rd, 0b000, rs1, offset, location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// Load/Store Instruction Family
// ============================================================================

fn encode_load_store_family(
    op: &LoadStoreOp,
    rd_or_rs: Register,
    rs1: Register,
    offset: i64,
    location: &Location,
    config: &Config,
) -> Result<Vec<u8>> {
    let _is_load = matches!(
        op,
        LoadStoreOp::Lw
            | LoadStoreOp::Lh
            | LoadStoreOp::Lb
            | LoadStoreOp::Lhu
            | LoadStoreOp::Lbu
    );

    // Try compressed encoding if enabled
    if config.relax.compressed {
        // c.lw rd, offset(rs1) (compressed regs, offset 4-byte aligned, 0-124)
        if matches!(op, LoadStoreOp::Lw)
            && is_compressed_reg(rd_or_rs)
            && is_compressed_reg(rs1)
            && (0..=124).contains(&offset)
            && offset % 4 == 0
        {
            return Ok(encode_c_lw(rd_or_rs, rs1, offset as u32)
                .to_le_bytes()
                .to_vec());
        }

        // c.lwsp rd, offset(sp) (rd != x0, rs1 == sp, offset 4-byte aligned, 0-252)
        if matches!(op, LoadStoreOp::Lw)
            && rs1 == Register::X2
            && rd_or_rs != Register::X0
            && (0..=252).contains(&offset)
            && offset % 4 == 0
        {
            return Ok(encode_c_lwsp(rd_or_rs, offset as u32)
                .to_le_bytes()
                .to_vec());
        }

        // c.sw rs2, offset(rs1) (compressed regs, offset 4-byte aligned, 0-124)
        if matches!(op, LoadStoreOp::Sw)
            && is_compressed_reg(rd_or_rs)
            && is_compressed_reg(rs1)
            && (0..=124).contains(&offset)
            && offset % 4 == 0
        {
            return Ok(encode_c_sw(rd_or_rs, rs1, offset as u32)
                .to_le_bytes()
                .to_vec());
        }

        // c.swsp rs2, offset(sp) (rs1 == sp, offset 4-byte aligned, 0-252)
        if matches!(op, LoadStoreOp::Sw)
            && rs1 == Register::X2
            && (0..=252).contains(&offset)
            && offset % 4 == 0
        {
            return Ok(encode_c_swsp(rd_or_rs, offset as u32)
                .to_le_bytes()
                .to_vec());
        }
    }

    // Base encoding
    let inst = encode_load_store(op, rd_or_rs, offset, rs1, location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// Branch Instruction Family
// ============================================================================

fn encode_branch_family(
    op: &BTypeOp,
    rs1: Register,
    rs2: Register,
    offset: i64,
    location: &Location,
    config: &Config,
) -> Result<Vec<u8>> {
    // Try compressed encoding if enabled
    if config.relax.compressed && is_compressed_reg(rs1) && rs2 == Register::X0
    {
        // c.beqz rs1, offset (rs2 == x0, compressed reg rs1, offset in ±256, even)
        if matches!(op, BTypeOp::Beq)
            && (-256..256).contains(&offset)
            && offset % 2 == 0
        {
            return Ok(encode_c_beqz(rs1, offset as i32)
                .to_le_bytes()
                .to_vec());
        }

        // c.bnez rs1, offset (rs2 == x0, compressed reg rs1, offset in ±256, even)
        if matches!(op, BTypeOp::Bne)
            && (-256..256).contains(&offset)
            && offset % 2 == 0
        {
            return Ok(encode_c_bnez(rs1, offset as i32)
                .to_le_bytes()
                .to_vec());
        }
    }

    // Base encoding
    let (opcode, funct3) = match op {
        BTypeOp::Beq => (0b1100011, 0b000),
        BTypeOp::Bne => (0b1100011, 0b001),
        BTypeOp::Blt => (0b1100011, 0b100),
        BTypeOp::Bge => (0b1100011, 0b101),
        BTypeOp::Bltu => (0b1100011, 0b110),
        BTypeOp::Bgeu => (0b1100011, 0b111),
    };

    let inst = encode_b_type(opcode, rs1, funct3, rs2, offset, location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// JAL Instruction Family
// ============================================================================

fn encode_jal_family(
    rd: Register,
    offset: i64,
    location: &Location,
    config: &Config,
) -> Result<Vec<u8>> {
    // Try compressed encoding if enabled
    if config.relax.compressed && fits_signed(offset, 12) && offset % 2 == 0 {
        // c.j offset (rd == x0)
        if rd == Register::X0 {
            return Ok(encode_c_j(offset as i32).to_le_bytes().to_vec());
        }

        // c.jal offset (rd == ra) - RV32C only
        if rd == Register::X1 {
            return Ok(encode_c_jal(offset as i32).to_le_bytes().to_vec());
        }
    }

    // Base encoding
    let inst = encode_j_type(0b1101111, rd, offset, location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// U-Type Instruction Family
// ============================================================================

fn encode_u_type_family(
    op: &UTypeOp,
    rd: Register,
    imm: i64,
    location: &Location,
) -> Result<Vec<u8>> {
    // U-type instructions don't have compressed variants

    // Validate immediate fits in 20 bits (unsigned)
    if !(0..=0xFFFFF).contains(&imm) {
        return Err(AssemblerError::from_context(
            format!(
                "Immediate {} out of range for U-type (must fit in 20 bits)",
                imm
            ),
            location.clone(),
        ));
    }

    let opcode = match op {
        UTypeOp::Lui => 0b0110111,
        UTypeOp::Auipc => 0b0010111,
    };

    let inst = encode_u_type(opcode, rd, imm as u32, location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// Pseudo-Instruction Encoding
// ============================================================================

fn encode_pseudo(
    op: &PseudoOp,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
    data_start: u32,
    config: &Config,
) -> Result<Vec<u8>> {
    match op {
        PseudoOp::Li(rd, imm) => encode_li(
            *rd,
            imm,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
            config,
        ),
        PseudoOp::La(rd, addr_expr) => encode_la(
            *rd,
            addr_expr,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
            data_start,
            config,
        ),
        PseudoOp::Call(target) => encode_call(
            target,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
            config,
        ),
        PseudoOp::Tail(target) => encode_tail(
            target,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
            config,
        ),
        PseudoOp::LoadGlobal(op, rd, addr) => encode_load_global_pseudo(
            op,
            *rd,
            addr,
            line,
            current_address,
            source,
            symbol_values,
            symbol_links,
            pointer,
        ),
        PseudoOp::StoreGlobal(op, rs, addr, temp) => {
            encode_store_global_pseudo(
                op,
                *rs,
                addr,
                *temp,
                line,
                current_address,
                source,
                symbol_values,
                symbol_links,
                pointer,
            )
        }
    }
}

/// Encode `li rd, imm` pseudo-instruction
fn encode_li(
    rd: Register,
    imm_expr: &Expression,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
    config: &Config,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);
    let val = eval_expr(
        imm_expr,
        current_address,
        refs,
        symbol_values,
        source,
        pointer,
    )?;
    let imm = require_integer(val, "li immediate", &line.location)?;

    // If immediate fits in 12-bit signed, use addi rd, x0, imm
    if fits_signed(imm, 12) {
        return encode_i_type_family(
            config,
            &line.location,
            &ITypeOp::Addi,
            rd,
            Register::X0,
            imm,
        );
    }

    // Otherwise use lui + addi
    let mut bytes = Vec::new();
    let (hi, lo) = split_offset_hi_lo(imm);

    // lui rd, hi
    let lui_inst =
        encode_u_type(0b0110111, rd, (hi & 0xFFFFF) as u32, &line.location)?;
    bytes.extend_from_slice(&lui_inst.to_le_bytes());

    // addi rd, rd, lo
    if lo != 0 {
        let addi_bytes = encode_i_type_family(
            config,
            &line.location,
            &ITypeOp::Addi,
            rd,
            rd,
            lo,
        )?;
        bytes.extend_from_slice(&addi_bytes);
    }

    Ok(bytes)
}

/// Encode `la rd, symbol` pseudo-instruction with GP-relative optimization
fn encode_la(
    rd: Register,
    addr_expr: &Expression,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
    data_start: u32,
    config: &Config,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);
    let target_val = eval_expr(
        addr_expr,
        current_address,
        refs,
        symbol_values,
        source,
        pointer,
    )?;
    let target_addr = require_address(target_val, "la target", &line.location)?;
    let current_pc = current_address as i64;

    // Check if initializing GP register itself (can't use GP-relative addressing to init GP)
    let is_gp_init = rd == Register::X3;

    // Try GP-relative encoding if enabled and not initializing GP
    if !is_gp_init && config.relax.gp {
        // GP value is always data_start + 2048
        let gp_addr = data_start as i64 + 2048;
        let gp_offset = target_addr as i64 - gp_addr;

        // If within ±2 KiB of GP, use addi rd, gp, offset
        if fits_signed(gp_offset, 12) {
            return encode_i_type_family(
                config,
                &line.location,
                &ITypeOp::Addi,
                rd,
                Register::X3,
                gp_offset,
            );
        }
    }

    // PC-relative encoding: auipc + addi
    let mut bytes = Vec::new();
    let offset = target_addr as i64 - current_pc;
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc rd, hi
    let auipc_inst =
        encode_u_type(0b0010111, rd, (hi & 0xFFFFF) as u32, &line.location)?;
    bytes.extend_from_slice(&auipc_inst.to_le_bytes());

    // addi rd, rd, lo
    let addi_bytes = encode_i_type_family(
        config,
        &line.location,
        &ITypeOp::Addi,
        rd,
        rd,
        lo,
    )?;
    bytes.extend_from_slice(&addi_bytes);

    Ok(bytes)
}

/// Encode `call target` pseudo-instruction
fn encode_call(
    target_expr: &Expression,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
    config: &Config,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);
    let target_val = eval_expr(
        target_expr,
        current_address,
        refs,
        symbol_values,
        source,
        pointer,
    )?;
    let target_addr =
        require_address(target_val, "call target", &line.location)?;
    let current_pc = current_address as i64;
    let offset = target_addr as i64 - current_pc;

    // Try relaxed encoding if enabled and within range
    if config.relax.pseudo && fits_signed(offset, 21) && offset % 2 == 0 {
        // jal ra, offset (may become c.jal if compression enabled)
        return encode_jal_family(Register::X1, offset, &line.location, config);
    }

    // Fall back to auipc + jalr
    let mut bytes = Vec::new();
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc ra, hi
    let auipc_inst = encode_u_type(
        0b0010111,
        Register::X1,
        (hi & 0xFFFFF) as u32,
        &line.location,
    )?;
    bytes.extend_from_slice(&auipc_inst.to_le_bytes());

    // jalr ra, ra, lo
    let jalr_bytes = encode_jalr_family(
        config,
        &line.location,
        Register::X1,
        Register::X1,
        lo,
    )?;
    bytes.extend_from_slice(&jalr_bytes);

    Ok(bytes)
}

/// Encode `tail target` pseudo-instruction
fn encode_tail(
    target_expr: &Expression,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
    config: &Config,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);
    let target_val = eval_expr(
        target_expr,
        current_address,
        refs,
        symbol_values,
        source,
        pointer,
    )?;
    let target_addr =
        require_address(target_val, "tail target", &line.location)?;
    let current_pc = current_address as i64;
    let offset = target_addr as i64 - current_pc;

    // Try relaxed encoding if enabled and within range
    if config.relax.pseudo && fits_signed(offset, 21) && offset % 2 == 0 {
        // jal x0, offset (may become c.j if compression enabled)
        return encode_jal_family(Register::X0, offset, &line.location, config);
    }

    // Fall back to auipc + jalr using t1
    let mut bytes = Vec::new();
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc t1, hi
    let auipc_inst = encode_u_type(
        0b0010111,
        Register::X6,
        (hi & 0xFFFFF) as u32,
        &line.location,
    )?;
    bytes.extend_from_slice(&auipc_inst.to_le_bytes());

    // jalr x0, t1, lo
    let jalr_bytes = encode_jalr_family(
        config,
        &line.location,
        Register::X0,
        Register::X6,
        lo,
    )?;
    bytes.extend_from_slice(&jalr_bytes);

    Ok(bytes)
}

/// Encode load global pseudo-instruction
fn encode_load_global_pseudo(
    op: &LoadStoreOp,
    rd: Register,
    addr_expr: &Expression,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);
    let addr_val = eval_expr(
        addr_expr,
        current_address,
        refs,
        symbol_values,
        source,
        pointer,
    )?;
    let addr =
        require_address(addr_val, "load global address", &line.location)?;
    let current_pc = current_address as i64;

    let mut bytes = Vec::new();
    let offset = addr as i64 - current_pc;
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc rd, hi
    let auipc_inst =
        encode_u_type(0b0010111, rd, (hi & 0xFFFFF) as u32, &line.location)?;
    bytes.extend_from_slice(&auipc_inst.to_le_bytes());

    // load rd, lo(rd)
    let load_inst = encode_load_store(op, rd, lo, rd, &line.location)?;
    bytes.extend_from_slice(&load_inst.to_le_bytes());

    Ok(bytes)
}

/// Encode store global pseudo-instruction
fn encode_store_global_pseudo(
    op: &LoadStoreOp,
    rs: Register,
    addr_expr: &Expression,
    temp: Register,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);
    let addr_val = eval_expr(
        addr_expr,
        current_address,
        refs,
        symbol_values,
        source,
        pointer,
    )?;
    let addr =
        require_address(addr_val, "store global address", &line.location)?;
    let current_pc = current_address as i64;

    let mut bytes = Vec::new();
    let offset = addr as i64 - current_pc;
    let (hi, lo) = split_offset_hi_lo(offset);

    // auipc temp, hi
    let auipc_inst =
        encode_u_type(0b0010111, temp, (hi & 0xFFFFF) as u32, &line.location)?;
    bytes.extend_from_slice(&auipc_inst.to_le_bytes());

    // store rs, lo(temp)
    let store_inst = encode_load_store(op, rs, lo, temp, &line.location)?;
    bytes.extend_from_slice(&store_inst.to_le_bytes());

    Ok(bytes)
}

// ============================================================================
// Explicit Compressed Instructions (c.* written directly in source)
// ============================================================================

fn encode_compressed_explicit(
    op: &CompressedOp,
    operands: &CompressedOperands,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
) -> Result<Vec<u8>> {
    // Evaluate any expressions in operands first
    let evaluated_operands = eval_compressed_operands(
        operands,
        current_address,
        source,
        symbol_values,
        symbol_links,
        pointer,
    )?;

    let inst = encode_compressed_inst(op, &evaluated_operands, &line.location)?;
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// Atomic Instructions
// ============================================================================

fn encode_atomic(
    op: &AtomicOp,
    rd: Register,
    rs1: Register,
    rs2: Register,
    ordering: &MemoryOrdering,
) -> Result<Vec<u8>> {
    let (funct5, funct3) = match op {
        AtomicOp::LrW => (0b00010, 0b010),
        AtomicOp::ScW => (0b00011, 0b010),
        AtomicOp::AmoswapW => (0b00001, 0b010),
        AtomicOp::AmoaddW => (0b00000, 0b010),
        AtomicOp::AmoxorW => (0b00100, 0b010),
        AtomicOp::AmoandW => (0b01100, 0b010),
        AtomicOp::AmoorW => (0b01000, 0b010),
        AtomicOp::AmominW => (0b10000, 0b010),
        AtomicOp::AmomaxW => (0b10100, 0b010),
        AtomicOp::AmominuW => (0b11000, 0b010),
        AtomicOp::AmomaxuW => (0b11100, 0b010),
    };

    let (aq, rl) = match ordering {
        MemoryOrdering::None => (0, 0),
        MemoryOrdering::Aq => (1, 0),
        MemoryOrdering::Rel => (0, 1),
        MemoryOrdering::AqRl => (1, 1),
    };

    let funct7 = (funct5 << 2) | (aq << 1) | rl;
    let inst = encode_r_type(0b0101111, rd, funct3, rs1, rs2, funct7);
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// Special Instructions
// ============================================================================

fn encode_special(op: &SpecialOp) -> Result<Vec<u8>> {
    let inst = match op {
        SpecialOp::Ecall => 0b00000000000000000000000001110011u32,
        SpecialOp::Ebreak => 0b00000000000100000000000001110011u32,
    };
    Ok(inst.to_le_bytes().to_vec())
}

// ============================================================================
// Directive Encoding
// ============================================================================

fn encode_directive(
    dir: &Directive,
    line: &Line,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
) -> Result<Vec<u8>> {
    let refs = symbol_links.get_line_refs(pointer);

    match dir {
        Directive::Text
        | Directive::Data
        | Directive::Bss
        | Directive::Global(_)
        | Directive::Equ(_, _) => Ok(Vec::new()),

        Directive::Byte(exprs) => {
            let mut bytes = Vec::new();
            for expr in exprs {
                let val = eval_expr(
                    expr,
                    current_address,
                    refs,
                    symbol_values,
                    source,
                    pointer,
                )?;
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
                let val = eval_expr(
                    expr,
                    current_address,
                    refs,
                    symbol_values,
                    source,
                    pointer,
                )?;
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
                let val = eval_expr(
                    expr,
                    current_address,
                    refs,
                    symbol_values,
                    source,
                    pointer,
                )?;
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
                bytes.push(0);
            }
            Ok(bytes)
        }

        Directive::Space(expr) => {
            let val = eval_expr(
                expr,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
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

        Directive::Balign(expr) => {
            let val = eval_expr(
                expr,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let alignment =
                require_integer(val, ".balign directive", &line.location)?;
            if alignment <= 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        ".balign alignment must be positive: {}",
                        alignment
                    ),
                    line.location.clone(),
                ));
            }
            let abs_addr = current_address as i64;
            let padding = (alignment - (abs_addr % alignment)) % alignment;
            Ok(vec![0; padding as usize])
        }
    }
}

// ============================================================================
// Base Instruction Encoding Functions (bit-level)
// ============================================================================

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

fn encode_i_type(
    opcode: u32,
    rd: Register,
    funct3: u32,
    rs1: Register,
    imm: i64,
    location: &Location,
) -> Result<u32> {
    check_i_imm(imm, location)?;
    let rd_bits = reg_to_u32(rd);
    let rs1_bits = reg_to_u32(rs1);
    let imm_bits = (imm & 0xFFF) as u32;

    Ok(opcode
        | (rd_bits << 7)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (imm_bits << 20))
}

fn encode_s_type(
    opcode: u32,
    rs1: Register,
    funct3: u32,
    rs2: Register,
    imm: i64,
    location: &Location,
) -> Result<u32> {
    check_i_imm(imm, location)?;
    let rs1_bits = reg_to_u32(rs1);
    let rs2_bits = reg_to_u32(rs2);
    let imm_bits = imm & 0xFFF;
    let imm_low = (imm_bits & 0x1F) as u32;
    let imm_high = ((imm_bits >> 5) & 0x7F) as u32;

    Ok(opcode
        | (imm_low << 7)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (rs2_bits << 20)
        | (imm_high << 25))
}

fn encode_b_type(
    opcode: u32,
    rs1: Register,
    funct3: u32,
    rs2: Register,
    offset: i64,
    location: &Location,
) -> Result<u32> {
    check_b_imm(offset, location)?;
    let rs1_bits = reg_to_u32(rs1);
    let rs2_bits = reg_to_u32(rs2);
    let offset_bits = offset & 0x1FFF;
    let imm_11 = ((offset_bits >> 11) & 0x1) as u32;
    let imm_4_1 = ((offset_bits >> 1) & 0xF) as u32;
    let imm_10_5 = ((offset_bits >> 5) & 0x3F) as u32;
    let imm_12 = ((offset_bits >> 12) & 0x1) as u32;

    Ok(opcode
        | (imm_11 << 7)
        | (imm_4_1 << 8)
        | (funct3 << 12)
        | (rs1_bits << 15)
        | (rs2_bits << 20)
        | (imm_10_5 << 25)
        | (imm_12 << 31))
}

fn encode_u_type(
    opcode: u32,
    rd: Register,
    imm: u32,
    location: &Location,
) -> Result<u32> {
    check_u_imm(imm, location)?;
    let rd_bits = reg_to_u32(rd);
    Ok(opcode | (rd_bits << 7) | (imm << 12))
}

fn encode_j_type(
    opcode: u32,
    rd: Register,
    offset: i64,
    location: &Location,
) -> Result<u32> {
    check_j_imm(offset, location)?;
    let rd_bits = reg_to_u32(rd);
    let offset_bits = offset & 0x1FFFFF;
    let imm_19_12 = ((offset_bits >> 12) & 0xFF) as u32;
    let imm_11 = ((offset_bits >> 11) & 0x1) as u32;
    let imm_10_1 = ((offset_bits >> 1) & 0x3FF) as u32;
    let imm_20 = ((offset_bits >> 20) & 0x1) as u32;

    Ok(opcode
        | (rd_bits << 7)
        | (imm_19_12 << 12)
        | (imm_11 << 20)
        | (imm_10_1 << 21)
        | (imm_20 << 31))
}

fn encode_load_store(
    op: &LoadStoreOp,
    rd_or_rs: Register,
    offset: i64,
    rs1: Register,
    location: &Location,
) -> Result<u32> {
    match op {
        LoadStoreOp::Lw => {
            encode_i_type(0b0000011, rd_or_rs, 0b010, rs1, offset, location)
        }
        LoadStoreOp::Lh => {
            encode_i_type(0b0000011, rd_or_rs, 0b001, rs1, offset, location)
        }
        LoadStoreOp::Lb => {
            encode_i_type(0b0000011, rd_or_rs, 0b000, rs1, offset, location)
        }
        LoadStoreOp::Lhu => {
            encode_i_type(0b0000011, rd_or_rs, 0b101, rs1, offset, location)
        }
        LoadStoreOp::Lbu => {
            encode_i_type(0b0000011, rd_or_rs, 0b100, rs1, offset, location)
        }
        LoadStoreOp::Sw => {
            encode_s_type(0b0100011, rs1, 0b010, rd_or_rs, offset, location)
        }
        LoadStoreOp::Sh => {
            encode_s_type(0b0100011, rs1, 0b001, rd_or_rs, offset, location)
        }
        LoadStoreOp::Sb => {
            encode_s_type(0b0100011, rs1, 0b000, rd_or_rs, offset, location)
        }
    }
}

// ============================================================================
// Compressed Instruction Encoding Functions (16-bit)
// ============================================================================

fn encode_c_add(rd: Register, rs2: Register) -> u16 {
    // CR format: funct4=1001, rd/rs1, rs2, op=10
    0b1001_00000_00000_10
        | ((reg_to_u32(rd) as u16) << 7)
        | ((reg_to_u32(rs2) as u16) << 2)
}

fn encode_c_mv(rd: Register, rs2: Register) -> u16 {
    // CR format: funct4=1000, rd, rs2, op=10
    0b1000_00000_00000_10
        | ((reg_to_u32(rd) as u16) << 7)
        | ((reg_to_u32(rs2) as u16) << 2)
}

fn encode_c_sub(rd: Register, rs2: Register) -> u16 {
    // CA format: funct6=100011, rd', funct2=00, rs2', op=01
    let rd_compressed = compress_reg_index(rd);
    let rs2_compressed = compress_reg_index(rs2);
    0b100011_000_00_000_01
        | ((rd_compressed as u16) << 7)
        | ((rs2_compressed as u16) << 2)
}

fn encode_c_and(rd: Register, rs2: Register) -> u16 {
    // CA format: funct6=100011, rd', funct2=11, rs2', op=01
    let rd_compressed = compress_reg_index(rd);
    let rs2_compressed = compress_reg_index(rs2);
    0b100011_000_11_000_01
        | ((rd_compressed as u16) << 7)
        | ((rs2_compressed as u16) << 2)
}

fn encode_c_or(rd: Register, rs2: Register) -> u16 {
    // CA format: funct6=100011, rd', funct2=10, rs2', op=01
    let rd_compressed = compress_reg_index(rd);
    let rs2_compressed = compress_reg_index(rs2);
    0b100011_000_10_000_01
        | ((rd_compressed as u16) << 7)
        | ((rs2_compressed as u16) << 2)
}

fn encode_c_xor(rd: Register, rs2: Register) -> u16 {
    // CA format: funct6=100011, rd', funct2=01, rs2', op=01
    let rd_compressed = compress_reg_index(rd);
    let rs2_compressed = compress_reg_index(rs2);
    0b100011_000_01_000_01
        | ((rd_compressed as u16) << 7)
        | ((rs2_compressed as u16) << 2)
}

fn encode_c_addi(rd: Register, imm: i32) -> u16 {
    // CI format: funct3=000, imm[5], rd, imm[4:0], op=01
    let imm_5 = ((imm >> 5) & 0x1) as u16;
    let imm_4_0 = (imm & 0x1F) as u16;
    0b000_0_00000_00000_01
        | (imm_5 << 12)
        | ((reg_to_u32(rd) as u16) << 7)
        | (imm_4_0 << 2)
}

fn encode_c_li(rd: Register, imm: i32) -> u16 {
    // CI format: funct3=010, imm[5], rd, imm[4:0], op=01
    let imm_5 = ((imm >> 5) & 0x1) as u16;
    let imm_4_0 = (imm & 0x1F) as u16;
    0b010_0_00000_00000_01
        | (imm_5 << 12)
        | ((reg_to_u32(rd) as u16) << 7)
        | (imm_4_0 << 2)
}

fn encode_c_lui(rd: Register, imm: i32) -> u16 {
    // CI format: funct3=011, imm[5], rd!=0, imm[4:0], op=01
    let imm_5 = ((imm >> 5) & 0x1) as u16;
    let imm_4_0 = (imm & 0x1F) as u16;
    0b011_0_00000_00000_01
        | (imm_5 << 12)
        | ((reg_to_u32(rd) as u16) << 7)
        | (imm_4_0 << 2)
}

fn encode_c_addi4spn(rd_prime: Register, imm: u32) -> u16 {
    // CIW format: funct3=000, imm[5:4|9:6|2|3], rd', op=00
    // imm encoding: bits [5:4] go to [12:11], [9:6] to [10:7], [3] to [5], [2] to [6]
    let rd_bits = (reg_to_u32(rd_prime) - 8) as u16; // Compressed register encoding
    let imm_5_4 = ((imm >> 4) & 0x3) as u16;
    let imm_9_6 = ((imm >> 6) & 0xF) as u16;
    let imm_2 = ((imm >> 2) & 0x1) as u16;
    let imm_3 = ((imm >> 3) & 0x1) as u16;
    (imm_5_4 << 11)
        | (imm_9_6 << 7)
        | (imm_2 << 6)
        | (imm_3 << 5)
        | (rd_bits << 2)
}

fn encode_c_addi16sp(imm: i32) -> u16 {
    // CI format: funct3=011, imm[9], rd=2 (sp), imm[4|6|8:7|5], op=01
    let imm_9 = ((imm >> 9) & 0x1) as u16;
    let imm_5 = ((imm >> 5) & 0x1) as u16;
    let imm_8_7 = ((imm >> 7) & 0x3) as u16;
    let imm_6 = ((imm >> 6) & 0x1) as u16;
    let imm_4 = ((imm >> 4) & 0x1) as u16;
    0b011_0_00010_00000_01
        | (imm_9 << 12)
        | (imm_4 << 6)
        | (imm_6 << 5)
        | (imm_8_7 << 3)
        | (imm_5 << 2)
}

fn encode_c_slli(rd: Register, shamt: u32) -> u16 {
    // CI format: funct3=000, shamt[5], rd, shamt[4:0], op=10
    let shamt_5 = ((shamt >> 5) & 0x1) as u16;
    let shamt_4_0 = (shamt & 0x1F) as u16;
    0b000_0_00000_00000_10
        | (shamt_5 << 12)
        | ((reg_to_u32(rd) as u16) << 7)
        | (shamt_4_0 << 2)
}

fn encode_c_srli(rd: Register, shamt: u32) -> u16 {
    // CB format: funct3=100, shamt[5], funct2=00, rd', shamt[4:0], op=01
    let rd_compressed = compress_reg_index(rd);
    let shamt_5 = ((shamt >> 5) & 0x1) as u16;
    let shamt_4_0 = (shamt & 0x1F) as u16;
    0b100_0_00_000_00000_01
        | (shamt_5 << 12)
        | ((rd_compressed as u16) << 7)
        | (shamt_4_0 << 2)
}

fn encode_c_srai(rd: Register, shamt: u32) -> u16 {
    // CB format: funct3=100, shamt[5], funct2=01, rd', shamt[4:0], op=01
    let rd_compressed = compress_reg_index(rd);
    let shamt_5 = ((shamt >> 5) & 0x1) as u16;
    let shamt_4_0 = (shamt & 0x1F) as u16;
    0b100_0_01_000_00000_01
        | (shamt_5 << 12)
        | ((rd_compressed as u16) << 7)
        | (shamt_4_0 << 2)
}

fn encode_c_andi(rd: Register, imm: i32) -> u16 {
    // CB format: funct3=100, imm[5], funct2=10, rd', imm[4:0], op=01
    let rd_compressed = compress_reg_index(rd);
    let imm_5 = ((imm >> 5) & 0x1) as u16;
    let imm_4_0 = (imm & 0x1F) as u16;
    0b100_0_10_000_00000_01
        | (imm_5 << 12)
        | ((rd_compressed as u16) << 7)
        | (imm_4_0 << 2)
}

fn encode_c_lw(rd: Register, rs1: Register, offset: u32) -> u16 {
    // CL format: funct3=010, offset[5:3], rs1', offset[2|6], rd', op=00
    let rd_compressed = compress_reg_index(rd);
    let rs1_compressed = compress_reg_index(rs1);
    let offset_5_3 = ((offset >> 3) & 0x7) as u16;
    let offset_2 = ((offset >> 2) & 0x1) as u16;
    let offset_6 = ((offset >> 6) & 0x1) as u16;
    0b010_000_000_00_000_00
        | (offset_5_3 << 10)
        | ((rs1_compressed as u16) << 7)
        | (offset_2 << 6)
        | (offset_6 << 5)
        | ((rd_compressed as u16) << 2)
}

fn encode_c_lwsp(rd: Register, offset: u32) -> u16 {
    // CI format: funct3=010, offset[5], rd, offset[4:2|7:6], op=10
    let offset_5 = ((offset >> 5) & 0x1) as u16;
    let offset_4_2 = ((offset >> 2) & 0x7) as u16;
    let offset_7_6 = ((offset >> 6) & 0x3) as u16;
    0b010_0_00000_00000_10
        | (offset_5 << 12)
        | ((reg_to_u32(rd) as u16) << 7)
        | (offset_4_2 << 4)
        | (offset_7_6 << 2)
}

fn encode_c_sw(rs2: Register, rs1: Register, offset: u32) -> u16 {
    // CS format: funct3=110, offset[5:3], rs1', offset[2|6], rs2', op=00
    let rs1_compressed = compress_reg_index(rs1);
    let rs2_compressed = compress_reg_index(rs2);
    let offset_5_3 = ((offset >> 3) & 0x7) as u16;
    let offset_2 = ((offset >> 2) & 0x1) as u16;
    let offset_6 = ((offset >> 6) & 0x1) as u16;
    0b110_000_000_00_000_00
        | (offset_5_3 << 10)
        | ((rs1_compressed as u16) << 7)
        | (offset_2 << 6)
        | (offset_6 << 5)
        | ((rs2_compressed as u16) << 2)
}

fn encode_c_swsp(rs2: Register, offset: u32) -> u16 {
    // CSS format: funct3=110, offset[5:2|7:6], rs2, op=10
    let offset_5_2 = ((offset >> 2) & 0xF) as u16;
    let offset_7_6 = ((offset >> 6) & 0x3) as u16;
    0b110_000000_00000_10
        | (offset_5_2 << 9)
        | (offset_7_6 << 7)
        | ((reg_to_u32(rs2) as u16) << 2)
}

fn encode_c_beqz(rs1: Register, offset: i32) -> u16 {
    // CB format: funct3=110, offset[8|4:3], rs1', offset[7:6|2:1|5], op=01
    let rs1_compressed = compress_reg_index(rs1);
    let offset_8 = ((offset >> 8) & 0x1) as u16;
    let offset_4_3 = ((offset >> 3) & 0x3) as u16;
    let offset_7_6 = ((offset >> 6) & 0x3) as u16;
    let offset_2_1 = ((offset >> 1) & 0x3) as u16;
    let offset_5 = ((offset >> 5) & 0x1) as u16;
    0b110_000_000_00000_01
        | (offset_8 << 12)
        | (offset_4_3 << 10)
        | ((rs1_compressed as u16) << 7)
        | (offset_7_6 << 5)
        | (offset_2_1 << 3)
        | (offset_5 << 2)
}

fn encode_c_bnez(rs1: Register, offset: i32) -> u16 {
    // CB format: funct3=111, offset[8|4:3], rs1', offset[7:6|2:1|5], op=01
    let rs1_compressed = compress_reg_index(rs1);
    let offset_8 = ((offset >> 8) & 0x1) as u16;
    let offset_4_3 = ((offset >> 3) & 0x3) as u16;
    let offset_7_6 = ((offset >> 6) & 0x3) as u16;
    let offset_2_1 = ((offset >> 1) & 0x3) as u16;
    let offset_5 = ((offset >> 5) & 0x1) as u16;
    0b111_000_000_00000_01
        | (offset_8 << 12)
        | (offset_4_3 << 10)
        | ((rs1_compressed as u16) << 7)
        | (offset_7_6 << 5)
        | (offset_2_1 << 3)
        | (offset_5 << 2)
}

fn encode_c_j(offset: i32) -> u16 {
    // CJ format: funct3=101, offset[11|4|9:8|10|6|7|3:1|5], op=01
    let offset_11 = ((offset >> 11) & 0x1) as u16;
    let offset_4 = ((offset >> 4) & 0x1) as u16;
    let offset_9_8 = ((offset >> 8) & 0x3) as u16;
    let offset_10 = ((offset >> 10) & 0x1) as u16;
    let offset_6 = ((offset >> 6) & 0x1) as u16;
    let offset_7 = ((offset >> 7) & 0x1) as u16;
    let offset_3_1 = ((offset >> 1) & 0x7) as u16;
    let offset_5 = ((offset >> 5) & 0x1) as u16;
    0b101_00000000000_01
        | (offset_11 << 12)
        | (offset_4 << 11)
        | (offset_9_8 << 9)
        | (offset_10 << 8)
        | (offset_6 << 7)
        | (offset_7 << 6)
        | (offset_3_1 << 3)
        | (offset_5 << 2)
}

fn encode_c_jal(offset: i32) -> u16 {
    // CJ format: funct3=001, offset[11|4|9:8|10|6|7|3:1|5], op=01
    let offset_11 = ((offset >> 11) & 0x1) as u16;
    let offset_4 = ((offset >> 4) & 0x1) as u16;
    let offset_9_8 = ((offset >> 8) & 0x3) as u16;
    let offset_10 = ((offset >> 10) & 0x1) as u16;
    let offset_6 = ((offset >> 6) & 0x1) as u16;
    let offset_7 = ((offset >> 7) & 0x1) as u16;
    let offset_3_1 = ((offset >> 1) & 0x7) as u16;
    let offset_5 = ((offset >> 5) & 0x1) as u16;
    0b001_00000000000_01
        | (offset_11 << 12)
        | (offset_4 << 11)
        | (offset_9_8 << 9)
        | (offset_10 << 8)
        | (offset_6 << 7)
        | (offset_7 << 6)
        | (offset_3_1 << 3)
        | (offset_5 << 2)
}

fn encode_c_jr(rs1: Register) -> u16 {
    // CR format: funct4=1000, rs1, rs2=0, op=10
    0b1000_00000_00000_10 | ((reg_to_u32(rs1) as u16) << 7)
}

fn encode_c_jalr(rs1: Register) -> u16 {
    // CR format: funct4=1001, rs1, rs2=0, op=10
    0b1001_00000_00000_10 | ((reg_to_u32(rs1) as u16) << 7)
}

// ============================================================================
// Compressed Instruction Full Encoder (for explicit c.* instructions)
// ============================================================================

fn encode_compressed_inst(
    op: &CompressedOp,
    operands: &EvaluatedCompressedOperands,
    location: &Location,
) -> Result<u16> {
    match (op, operands) {
        (CompressedOp::CNop, EvaluatedCompressedOperands::None) => {
            Ok(0b000_0_00000_00000_01)
        }
        (CompressedOp::CEbreak, EvaluatedCompressedOperands::None) => {
            Ok(0b1001_00000_00000_10)
        }
        (CompressedOp::CAdd, EvaluatedCompressedOperands::CR { rd, rs2 }) => {
            Ok(encode_c_add(*rd, *rs2))
        }
        (CompressedOp::CMv, EvaluatedCompressedOperands::CR { rd, rs2 }) => {
            Ok(encode_c_mv(*rd, *rs2))
        }
        (
            CompressedOp::CSub,
            EvaluatedCompressedOperands::CA { rd_prime, rs2_prime },
        ) => Ok(encode_c_sub(*rd_prime, *rs2_prime)),
        (
            CompressedOp::CAnd,
            EvaluatedCompressedOperands::CA { rd_prime, rs2_prime },
        ) => Ok(encode_c_and(*rd_prime, *rs2_prime)),
        (
            CompressedOp::COr,
            EvaluatedCompressedOperands::CA { rd_prime, rs2_prime },
        ) => Ok(encode_c_or(*rd_prime, *rs2_prime)),
        (
            CompressedOp::CXor,
            EvaluatedCompressedOperands::CA { rd_prime, rs2_prime },
        ) => Ok(encode_c_xor(*rd_prime, *rs2_prime)),
        (CompressedOp::CAddi, EvaluatedCompressedOperands::CI { rd, imm }) => {
            if !fits_signed(*imm as i64, 6) {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.addi immediate {} out of range (must fit in 6-bit signed)",
                        imm
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_addi(*rd, *imm))
        }
        (CompressedOp::CLi, EvaluatedCompressedOperands::CI { rd, imm }) => {
            if !fits_signed(*imm as i64, 6) {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.li immediate {} out of range (must fit in 6-bit signed)",
                        imm
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_li(*rd, *imm))
        }
        (CompressedOp::CLui, EvaluatedCompressedOperands::CI { rd, imm }) => {
            if *rd == Register::X2 {
                return Err(AssemblerError::from_context(
                    "c.lui cannot use sp (x2) as destination".to_string(),
                    location.clone(),
                ));
            }
            if !fits_signed(*imm as i64, 6) {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.lui immediate {} out of range (must fit in 6-bit signed)",
                        imm
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_lui(*rd, *imm))
        }
        (
            CompressedOp::CAddi4spn,
            EvaluatedCompressedOperands::CIW { rd_prime, imm },
        ) => {
            if *imm == 0 || *imm % 4 != 0 || *imm < 0 || *imm > 1020 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.addi4spn immediate {} must be non-zero, multiple of 4, and 4-1020",
                        imm
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_addi4spn(*rd_prime, *imm as u32))
        }
        (
            CompressedOp::CAddi16sp,
            EvaluatedCompressedOperands::CI { rd: _, imm },
        ) => {
            if *imm == 0 || *imm % 16 != 0 || !fits_signed(*imm as i64, 10) {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.addi16sp immediate {} must be non-zero, multiple of 16, and fit in 10 bits",
                        imm
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_addi16sp(*imm))
        }
        (
            CompressedOp::CSlli,
            EvaluatedCompressedOperands::CI { rd, imm: shamt },
        ) => {
            if *shamt <= 0 || *shamt >= 32 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.slli shift amount {} out of range (must be 1-31)",
                        shamt
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_slli(*rd, *shamt as u32))
        }
        (
            CompressedOp::CSrli,
            EvaluatedCompressedOperands::CBImm { rd_prime, imm: shamt },
        ) => {
            if *shamt <= 0 || *shamt >= 32 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.srli shift amount {} out of range (must be 1-31)",
                        shamt
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_srli(*rd_prime, *shamt as u32))
        }
        (
            CompressedOp::CSrai,
            EvaluatedCompressedOperands::CBImm { rd_prime, imm: shamt },
        ) => {
            if *shamt <= 0 || *shamt >= 32 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.srai shift amount {} out of range (must be 1-31)",
                        shamt
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_srai(*rd_prime, *shamt as u32))
        }
        (
            CompressedOp::CAndi,
            EvaluatedCompressedOperands::CBImm { rd_prime, imm },
        ) => {
            if !fits_signed(*imm as i64, 6) {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.andi immediate {} out of range (must fit in 6-bit signed)",
                        imm
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_andi(*rd_prime, *imm))
        }
        (
            CompressedOp::CLw,
            EvaluatedCompressedOperands::CL { rd_prime, rs1_prime, offset },
        ) => {
            if *offset < 0 || *offset > 124 || *offset % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.lw offset {} must be 0-124 and 4-byte aligned",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_lw(*rd_prime, *rs1_prime, *offset as u32))
        }
        (
            CompressedOp::CLwsp,
            EvaluatedCompressedOperands::CIStackLoad { rd, offset },
        ) => {
            if *offset < 0 || *offset > 252 || *offset % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.lwsp offset {} must be 0-252 and 4-byte aligned",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_lwsp(*rd, *offset as u32))
        }
        (
            CompressedOp::CSw,
            EvaluatedCompressedOperands::CS { rs2_prime, rs1_prime, offset },
        ) => {
            if *offset < 0 || *offset > 124 || *offset % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.sw offset {} must be 0-124 and 4-byte aligned",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_sw(*rs2_prime, *rs1_prime, *offset as u32))
        }
        (
            CompressedOp::CSwsp,
            EvaluatedCompressedOperands::CSSStackStore { rs2, offset },
        ) => {
            if *offset < 0 || *offset > 252 || *offset % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.swsp offset {} must be 0-252 and 4-byte aligned",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_swsp(*rs2, *offset as u32))
        }
        (
            CompressedOp::CBeqz,
            EvaluatedCompressedOperands::CBBranch { rs1_prime, offset },
        ) => {
            if *offset < -256 || *offset >= 256 || *offset % 2 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.beqz offset {} must be -256 to 254 and even",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_beqz(*rs1_prime, *offset))
        }
        (
            CompressedOp::CBnez,
            EvaluatedCompressedOperands::CBBranch { rs1_prime, offset },
        ) => {
            if *offset < -256 || *offset >= 256 || *offset % 2 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.bnez offset {} must be -256 to 254 and even",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_bnez(*rs1_prime, *offset))
        }
        (
            CompressedOp::CJComp,
            EvaluatedCompressedOperands::CJOpnd { offset },
        ) => {
            if *offset < -2048 || *offset >= 2048 || *offset % 2 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.j offset {} must be -2048 to 2046 and even",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_j(*offset))
        }
        (
            CompressedOp::CJalComp,
            EvaluatedCompressedOperands::CJOpnd { offset },
        ) => {
            if *offset < -2048 || *offset >= 2048 || *offset % 2 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.jal offset {} must be -2048 to 2046 and even",
                        offset
                    ),
                    location.clone(),
                ));
            }
            Ok(encode_c_jal(*offset))
        }
        (CompressedOp::CJr, EvaluatedCompressedOperands::CRSingle { rs1 }) => {
            Ok(encode_c_jr(*rs1))
        }
        (
            CompressedOp::CJalr,
            EvaluatedCompressedOperands::CRSingle { rs1 },
        ) => Ok(encode_c_jalr(*rs1)),
        _ => Err(AssemblerError::from_context(
            format!("Invalid compressed instruction operands for {:?}", op),
            location.clone(),
        )),
    }
}

// Helper type for evaluated compressed operands
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
enum EvaluatedCompressedOperands {
    None,
    CR { rd: Register, rs2: Register },
    CRSingle { rs1: Register },
    CI { rd: Register, imm: i32 },
    CIStackLoad { rd: Register, offset: i32 },
    CSSStackStore { rs2: Register, offset: i32 },
    CIW { rd_prime: Register, imm: i32 },
    CL { rd_prime: Register, rs1_prime: Register, offset: i32 },
    CS { rs2_prime: Register, rs1_prime: Register, offset: i32 },
    CA { rd_prime: Register, rs2_prime: Register },
    CBImm { rd_prime: Register, imm: i32 },
    CBBranch { rs1_prime: Register, offset: i32 },
    CJOpnd { offset: i32 },
}

fn eval_compressed_operands(
    operands: &CompressedOperands,
    current_address: u32,
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    pointer: LinePointer,
) -> Result<EvaluatedCompressedOperands> {
    let refs = symbol_links.get_line_refs(pointer);

    match operands {
        CompressedOperands::None => Ok(EvaluatedCompressedOperands::None),
        CompressedOperands::CR { rd, rs2 } => {
            Ok(EvaluatedCompressedOperands::CR { rd: *rd, rs2: *rs2 })
        }
        CompressedOperands::CRSingle { rs1 } => {
            Ok(EvaluatedCompressedOperands::CRSingle { rs1: *rs1 })
        }
        CompressedOperands::CI { rd, imm } => {
            let val = eval_expr(
                imm,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let imm_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CI { rd: *rd, imm: imm_val })
        }
        CompressedOperands::CIStackLoad { rd, offset } => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CIStackLoad {
                rd: *rd,
                offset: offset_val,
            })
        }
        CompressedOperands::CSSStackStore { rs2, offset } => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CSSStackStore {
                rs2: *rs2,
                offset: offset_val,
            })
        }
        CompressedOperands::CIW { rd_prime, imm } => {
            let val = eval_expr(
                imm,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let imm_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CIW {
                rd_prime: *rd_prime,
                imm: imm_val,
            })
        }
        CompressedOperands::CL { rd_prime, rs1_prime, offset } => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CL {
                rd_prime: *rd_prime,
                rs1_prime: *rs1_prime,
                offset: offset_val,
            })
        }
        CompressedOperands::CS { rs2_prime, rs1_prime, offset } => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CS {
                rs2_prime: *rs2_prime,
                rs1_prime: *rs1_prime,
                offset: offset_val,
            })
        }
        CompressedOperands::CA { rd_prime, rs2_prime } => {
            Ok(EvaluatedCompressedOperands::CA {
                rd_prime: *rd_prime,
                rs2_prime: *rs2_prime,
            })
        }
        CompressedOperands::CBImm { rd_prime, imm } => {
            let val = eval_expr(
                imm,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let imm_val = match val {
                EvaluatedValue::Integer(i) => i,
                EvaluatedValue::Address(a) => a as i32,
            };
            Ok(EvaluatedCompressedOperands::CBImm {
                rd_prime: *rd_prime,
                imm: imm_val,
            })
        }
        CompressedOperands::CBBranch { rs1_prime, offset } => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val = match val {
                // For branches, if we get an address, compute PC-relative offset
                EvaluatedValue::Address(target_addr) => {
                    let current_pc = current_address as i64;
                    (target_addr as i64 - current_pc) as i32
                }
                // If it's an integer, use it directly as the offset
                EvaluatedValue::Integer(i) => i,
            };
            Ok(EvaluatedCompressedOperands::CBBranch {
                rs1_prime: *rs1_prime,
                offset: offset_val,
            })
        }
        CompressedOperands::CJOpnd { offset } => {
            let val = eval_expr(
                offset,
                current_address,
                refs,
                symbol_values,
                source,
                pointer,
            )?;
            let offset_val = match val {
                // For jumps, if we get an address, compute PC-relative offset
                EvaluatedValue::Address(target_addr) => {
                    let current_pc = current_address as i64;
                    (target_addr as i64 - current_pc) as i32
                }
                // If it's an integer, use it directly as the offset
                EvaluatedValue::Integer(i) => i,
            };
            Ok(EvaluatedCompressedOperands::CJOpnd { offset: offset_val })
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

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

fn is_compressed_reg(reg: Register) -> bool {
    matches!(
        reg,
        Register::X8
            | Register::X9
            | Register::X10
            | Register::X11
            | Register::X12
            | Register::X13
            | Register::X14
            | Register::X15
    )
}

fn compress_reg_index(reg: Register) -> u8 {
    (reg_to_u32(reg) - 8) as u8
}

fn fits_signed(value: i64, bits: u32) -> bool {
    let min = -(1i64 << (bits - 1));
    let max = (1i64 << (bits - 1)) - 1;
    value >= min && value <= max
}

fn split_offset_hi_lo(offset: i64) -> (i64, i64) {
    let lo = ((offset as i32) << 20) >> 20;
    let hi = ((offset as i32) - lo) >> 12;
    (hi as i64, lo as i64)
}

fn require_integer(
    val: EvaluatedValue,
    context: &str,
    location: &Location,
) -> Result<i64> {
    match val {
        EvaluatedValue::Integer(i) => Ok(i as i64),
        EvaluatedValue::Address(_) => Err(AssemblerError::from_context(
            format!("{} must be an Integer, got Address", context),
            location.clone(),
        )),
    }
}

fn require_address(
    val: EvaluatedValue,
    context: &str,
    location: &Location,
) -> Result<u32> {
    match val {
        EvaluatedValue::Address(a) => Ok(a),
        EvaluatedValue::Integer(_) => Err(AssemblerError::from_context(
            format!("{} must be an Address, got Integer", context),
            location.clone(),
        )),
    }
}

fn check_i_imm(imm: i64, location: &Location) -> Result<()> {
    if !fits_signed(imm, 12) {
        return Err(AssemblerError::from_context(
            format!(
                "Immediate {} out of range for I-type (must fit in 12-bit signed)",
                imm
            ),
            location.clone(),
        ));
    }
    Ok(())
}

fn check_b_imm(offset: i64, location: &Location) -> Result<()> {
    if offset % 2 != 0 {
        return Err(AssemblerError::from_context(
            format!("Branch offset {} must be even", offset),
            location.clone(),
        ));
    }
    if !fits_signed(offset, 13) {
        return Err(AssemblerError::from_context(
            format!(
                "Branch offset {} out of range (must fit in 13-bit signed)",
                offset
            ),
            location.clone(),
        ));
    }
    Ok(())
}

fn check_j_imm(offset: i64, location: &Location) -> Result<()> {
    if offset % 2 != 0 {
        return Err(AssemblerError::from_context(
            format!("Jump offset {} must be even", offset),
            location.clone(),
        ));
    }
    if !fits_signed(offset, 21) {
        return Err(AssemblerError::from_context(
            format!(
                "Jump offset {} out of range (must fit in 21-bit signed)",
                offset
            ),
            location.clone(),
        ));
    }
    Ok(())
}

fn check_u_imm(imm: u32, location: &Location) -> Result<()> {
    if imm > 0xFFFFF {
        return Err(AssemblerError::from_context(
            format!(
                "Immediate {} out of range for U-type (must fit in 20 bits)",
                imm
            ),
            location.clone(),
        ));
    }
    Ok(())
}
