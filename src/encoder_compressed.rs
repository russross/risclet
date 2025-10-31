// encoder_compressed.rs
//
// Compressed instruction encoding for RV32C extension
//
// This module handles encoding of 16-bit (2-byte) compressed instructions.
// All compressed instructions must be 2-byte aligned.
//
// Note: Binary literals use custom grouping to show instruction format structure
// (e.g., `0b010_0_00000_00000_01` shows [funct3]_[imm5]_[rd]_[imm4:0]_[op])
#![allow(clippy::unusual_byte_groupings)]

use crate::ast::{CompressedOp, CompressedOperands, Line, Register};
use crate::error::{AssemblerError, Result};
use crate::expressions::EvaluationContext;

/// Encode a compressed instruction to 2 bytes (16-bit) without expression evaluation
pub fn encode_compressed(
    op: &CompressedOp,
    operands: &CompressedOperands,
    location: &crate::ast::Location,
) -> Result<Vec<u8>> {
    let inst = encode_compressed_inst(op, operands, location, None)?;
    Ok(inst.to_le_bytes().to_vec())
}

/// Extract the immediate value from a literal expression
/// Expects expressions to have been evaluated to literals before encoding
fn extract_literal(expr: &crate::ast::Expression) -> i32 {
    match expr {
        crate::ast::Expression::Literal(val) => *val,
        _ => 0, // Fallback for non-evaluated expressions (shouldn't happen)
    }
}

/// Encode compressed instruction to u16
/// Expects all expressions in operands to have been pre-evaluated to Literal values
pub fn encode_compressed_inst(
    op: &CompressedOp,
    operands: &CompressedOperands,
    location: &crate::ast::Location,
    _eval_context: Option<(&Line, &mut EvaluationContext)>,
) -> Result<u16> {
    use CompressedOp::*;
    use CompressedOperands::*;

    match (op, operands) {
        // CR format: c.add rd, rs2
        // opcode: 1001 | rd[4:0] | rs2[4:0] | 10
        (CAdd, CR { rd, rs2 }) => {
            let rd_enc = reg_to_bits(*rd);
            let rs2_enc = reg_to_bits(*rs2);
            Ok(0b1001_0000_0000_0010 | (rd_enc << 7) | (rs2_enc << 2))
        }

        // CR format: c.mv rd, rs2
        // opcode: 1000 | rd[4:0] | rs2[4:0] | 10
        (CMv, CR { rd, rs2 }) => {
            let rd_enc = reg_to_bits(*rd);
            let rs2_enc = reg_to_bits(*rs2);
            Ok(0b1000_0000_0000_0010 | (rd_enc << 7) | (rs2_enc << 2))
        }

        // CR format single register: c.jr rs1
        // opcode: 1000 | rs1[4:0] | 00000 | 10
        (CJr, CRSingle { rs1 }) => {
            let rs1_enc = reg_to_bits(*rs1);
            Ok(0b1000_0000_0000_0010 | (rs1_enc << 7))
        }

        // CR format single register: c.jalr rs1
        // opcode: 1001 | rs1[4:0] | 00000 | 10
        (CJalr, CRSingle { rs1 }) => {
            let rs1_enc = reg_to_bits(*rs1);
            Ok(0b1001_0000_0000_0010 | (rs1_enc << 7))
        }

        // CI format: c.li rd, imm
        // opcode: 010 | imm[5] | rd[4:0] | imm[4:0] | 01
        // imm is 6-bit signed: -32 to 31
        (CLi, CI { rd, imm }) => {
            let rd_enc = reg_to_bits(*rd);
            let imm_val = extract_literal(imm);
            check_signed_imm(imm_val, 6, "c.li", location)?;
            let imm_bits = (imm_val as u16) & 0x3F;
            let imm_5 = (imm_bits >> 5) & 1;
            let imm_4_0 = imm_bits & 0x1F;
            Ok(0b010_0_00000_00000_01
                | (imm_5 << 12)
                | (rd_enc << 7)
                | (imm_4_0 << 2))
        }

        // CI format: c.addi rd, imm (rd is also rs1, rd != x0)
        // opcode: 000 | imm[5] | rd[4:0] | imm[4:0] | 01
        // imm is 6-bit signed: -32 to 31
        (CAddi, CI { rd, imm }) => {
            if *rd == Register::X0 {
                return Err(AssemblerError::from_context(
                    "c.addi cannot use x0 as destination".to_string(),
                    location.clone(),
                ));
            }
            let rd_enc = reg_to_bits(*rd);
            let imm_val = extract_literal(imm);
            check_signed_imm(imm_val, 6, "c.addi", location)?;
            let imm_bits = (imm_val as u16) & 0x3F;
            let imm_5 = (imm_bits >> 5) & 1;
            let imm_4_0 = imm_bits & 0x1F;
            Ok(0b000_0_00000_00000_01
                | (imm_5 << 12)
                | (rd_enc << 7)
                | (imm_4_0 << 2))
        }

        // CI format: c.addi16sp sp, imm
        // opcode: 011 | imm[9] | 00010 | imm[4:3,5,8:6] | 01
        // imm is 10-bit signed << 4, range [-512, 496] in multiples of 16
        (CAddi16sp, CI { rd, imm }) => {
            if *rd != Register::X2 {
                return Err(AssemblerError::from_context(
                    "c.addi16sp requires sp (x2) as rd".to_string(),
                    location.clone(),
                ));
            }
            let imm_val = extract_literal(imm);
            // Check range: -512 to 496 in multiples of 16
            if imm_val % 16 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.addi16sp immediate {} must be multiple of 16",
                        imm_val
                    ),
                    location.clone(),
                ));
            }
            check_signed_imm(imm_val, 11, "c.addi16sp", location)?; // Check 11-bit range for the original value
            let imm_bits = (imm_val as u16) & 0x3FF; // Keep as 10-bit immediate
            let imm_9 = (imm_bits >> 9) & 1;
            let imm_8 = (imm_bits >> 8) & 1;
            let imm_7_6 = (imm_bits >> 6) & 0x3;
            let imm_5 = (imm_bits >> 5) & 1;
            let imm_4 = (imm_bits >> 4) & 1;
            // Encoding: bits 15-13 = 011, bit 12 = imm[9], bits 11-7 = 00010 (sp)
            //           bit 6 = imm[4], bit 5 = imm[8], bit 4 = imm[5], bits 3-2 = imm[7:6], bits 1-0 = 01
            Ok(0b011_0_00010_00_0_00_01
                | (imm_9 << 12)
                | (imm_4 << 6)
                | (imm_8 << 5)
                | (imm_5 << 4)
                | (imm_7_6 << 2))
        }

        // CI format: c.slli rd, shamt
        // opcode: 000 | shamt[5] | rd[4:0] | shamt[4:0] | 10
        // shamt is 6-bit unsigned: 0 to 63 (but usually 1-31)
        (CSlli, CI { rd, imm }) => {
            if *rd == Register::X0 {
                return Err(AssemblerError::from_context(
                    "c.slli cannot use x0 as destination".to_string(),
                    location.clone(),
                ));
            }
            let rd_enc = reg_to_bits(*rd);
            let shamt_val = extract_literal(imm) as u32;
            if shamt_val > 63 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.slli shift amount {} out of range [0, 63]",
                        shamt_val
                    ),
                    location.clone(),
                ));
            }
            let shamt_bits = shamt_val as u16;
            let shamt_5 = (shamt_bits >> 5) & 1;
            let shamt_4_0 = shamt_bits & 0x1F;
            Ok(0b000_0_00000_00000_10
                | (shamt_5 << 12)
                | (rd_enc << 7)
                | (shamt_4_0 << 2))
        }

        // CI format stack-relative: c.lwsp rd, offset(sp)
        // opcode: 010 | offset[5] | rd[4:0] | offset[4:2,7:6] | 10
        // offset is in 4-byte increments, range [0, 252]
        (CLwsp, CIStackLoad { rd, offset }) => {
            if *rd == Register::X0 {
                return Err(AssemblerError::from_context(
                    "c.lwsp cannot use x0 as destination".to_string(),
                    location.clone(),
                ));
            }
            let rd_enc = reg_to_bits(*rd);
            let offset_val = extract_literal(offset) as u32;
            if offset_val > 252 || offset_val % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.lwsp offset {} must be 4-byte aligned in range [0, 252]",
                        offset_val
                    ),
                    location.clone(),
                ));
            }
            let offset_scaled = (offset_val >> 2) as u16;
            let offset_5 = (offset_scaled >> 5) & 1;
            let offset_4_2 = (offset_scaled >> 2) & 0x7;
            let offset_7_6 = (offset_scaled >> 7) & 0x3;
            Ok(0b010_0_00000_00000_10
                | (offset_5 << 12)
                | (rd_enc << 7)
                | (offset_4_2 << 4)
                | (offset_7_6 << 2))
        }

        // CSS format stack-relative: c.swsp rs2, offset(sp)
        // bits 15:13 = 110
        // bits 12:7 = offset[8:6,5:2] (split encoding of upper offset bits)
        // bits 6:2 = rs2[4:0] (5-bit register field)
        // bits 1:0 = 10 (opcode)
        // offset is in 4-byte increments, range [0, 252]
        (CSwsp, CSSStackStore { rs2, offset }) => {
            let rs2_enc = reg_to_bits(*rs2);
            let offset_val = extract_literal(offset) as u32;
            if offset_val > 252 || offset_val % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.swsp offset {} must be 4-byte aligned in range [0, 252]",
                        offset_val
                    ),
                    location.clone(),
                ));
            }
            let offset_scaled = (offset_val >> 2) as u16;
            let offset_5_2 = (offset_scaled & 0xF) as u16;
            let offset_8_6 = ((offset_scaled >> 4) & 0x7) as u16;
            // CSS format: bits 15:13=110, bits 12:7=offset[8:6,5:2], bits 6:2=rs2[4:0], bits 1:0=10
            Ok((0b110_u16 << 13)
                | (offset_8_6 << 10)
                | (offset_5_2 << 7)
                | (rs2_enc << 2)
                | 0b10)
        }

        // CIW format: c.addi4spn rd', imm
        // opcode: 000 | imm[9:6,2,5:3] | rd'[2:0] | 00
        // imm is 10-bit zero-extended << 2, range [0, 1020] in multiples of 4
        (CAddi4spn, CIW { rd_prime, imm }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let imm_val = extract_literal(imm) as u32;
            // Check range: 0 to 1020 in multiples of 4
            if imm_val > 1020 || imm_val % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.addi4spn immediate {} must be 4-byte aligned in range [0, 1020]",
                        imm_val
                    ),
                    location.clone(),
                ));
            }
            // CIW encoding: bits 15:13=000, bits 12:6=imm[8:2], bits 5:2=rd'[3:0], bits 1:0=00
            let imm_bits = (imm_val as u16) & 0x3FF;
            let imm_8_2 = (imm_bits >> 2) & 0x7F; // Extract bits 8:2
            Ok((imm_8_2 << 6) | (rd_enc << 2))
        }

        // CL format: c.lw rd', offset(rs1')
        // Layout: 010 | offset[5:3] | rs1'[2:0] | offset[2] | offset[6] | rd'[2:0] | 00
        // offset is in 4-byte increments, range [0, 124]
        // The offset is encoded as: offset[5:3] at bits 12-10, offset[2] at bit 6, offset[6] at bit 5
        (CLw, CL { rd_prime, rs1_prime, offset }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let rs1_enc = rs1_prime.compressed_encoding() as u16;
            let offset_val = extract_literal(offset) as u32;
            if offset_val > 124 || offset_val % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.lw offset {} must be 4-byte aligned in range [0, 124]",
                        offset_val
                    ),
                    location.clone(),
                ));
            }
            let offset_scaled = (offset_val >> 2) as u16;
            // Extract offset bits: offset = offset_scaled * 4
            // offset[5:3] = offset_scaled >> 1
            // offset[2] = offset_scaled & 1
            // offset[6] = 0 (always zero for valid range)
            let offset_5_3 = (offset_scaled >> 1) & 0x7;
            let offset_2 = offset_scaled & 1;
            Ok(0b010_000_000_00_000_00
                | (offset_5_3 << 10)
                | (rs1_enc << 7)
                | (offset_2 << 6)
                | (rd_enc << 2))
        }

        // CS format: c.sw rs2', offset(rs1')
        // Layout: 110 | offset[5:3] | rs1'[2:0] | offset[2] | offset[6] | rs2'[2:0] | 00
        // offset is in 4-byte increments, range [0, 124]
        // The offset is encoded the same way as CL format
        (CSw, CS { rs2_prime, rs1_prime, offset }) => {
            let rs2_enc = rs2_prime.compressed_encoding() as u16;
            let rs1_enc = rs1_prime.compressed_encoding() as u16;
            let offset_val = extract_literal(offset) as u32;
            if offset_val > 124 || offset_val % 4 != 0 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.sw offset {} must be 4-byte aligned in range [0, 124]",
                        offset_val
                    ),
                    location.clone(),
                ));
            }
            let offset_scaled = (offset_val >> 2) as u16;
            // Same offset extraction as CL
            let offset_5_3 = (offset_scaled >> 1) & 0x7;
            let offset_2 = offset_scaled & 1;
            Ok(0b110_000_000_00_000_00
                | (offset_5_3 << 10)
                | (rs1_enc << 7)
                | (offset_2 << 6)
                | (rs2_enc << 2))
        }

        // CA format: c.and, c.or, c.xor, c.sub
        // bits 15:10 = 100011 (funct6)
        // bits 9:7 = rd' (compressed register)
        // bits 6:5 = operation type (11=and, 10=or, 01=xor, 00=sub)
        // bits 4:2 = rs2' (compressed register)
        // bits 1:0 = 01 (opcode)
        (CAnd, CA { rd_prime, rs2_prime }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let rs2_enc = rs2_prime.compressed_encoding() as u16;
            Ok((0b100011_u16 << 10)
                | (rd_enc << 7)
                | (0b11_u16 << 5)
                | (rs2_enc << 2)
                | 0b01)
        }

        (COr, CA { rd_prime, rs2_prime }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let rs2_enc = rs2_prime.compressed_encoding() as u16;
            Ok((0b100011_u16 << 10)
                | (rd_enc << 7)
                | (0b10_u16 << 5)
                | (rs2_enc << 2)
                | 0b01)
        }

        (CXor, CA { rd_prime, rs2_prime }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let rs2_enc = rs2_prime.compressed_encoding() as u16;
            Ok((0b100011_u16 << 10)
                | (rd_enc << 7)
                | (0b01_u16 << 5)
                | (rs2_enc << 2)
                | 0b01)
        }

        (CSub, CA { rd_prime, rs2_prime }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let rs2_enc = rs2_prime.compressed_encoding() as u16;
            Ok((0b100011_u16 << 10) | (rd_enc << 7) | (rs2_enc << 2) | 0b01)
        }

        // CB format: c.srli, c.srai, c.andi
        // opcode: 100 | funct2 | imm[5] | rd'[2:0] | imm[4:0] | 01
        (CSrli, CBImm { rd_prime, imm }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let imm_val = extract_literal(imm) as u32;
            if imm_val > 63 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.srli shift amount {} out of range [0, 63]",
                        imm_val
                    ),
                    location.clone(),
                ));
            }
            let imm_bits = imm_val as u16;
            let imm_5 = (imm_bits >> 5) & 1;
            let imm_4_0 = imm_bits & 0x1F;
            Ok(0b100_0_00_000_00_000_01
                | (imm_5 << 12)
                | (rd_enc << 7)
                | (imm_4_0 << 2))
        }

        (CSrai, CBImm { rd_prime, imm }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let imm_val = extract_literal(imm) as u32;
            if imm_val > 63 {
                return Err(AssemblerError::from_context(
                    format!(
                        "c.srai shift amount {} out of range [0, 63]",
                        imm_val
                    ),
                    location.clone(),
                ));
            }
            let imm_bits = imm_val as u16;
            let imm_5 = (imm_bits >> 5) & 1;
            let imm_4_0 = imm_bits & 0x1F;
            Ok(0b100_0_01_000_00_000_01
                | (imm_5 << 12)
                | (rd_enc << 7)
                | (imm_4_0 << 2))
        }

        (CAndi, CBImm { rd_prime, imm }) => {
            let rd_enc = rd_prime.compressed_encoding() as u16;
            let imm_val = extract_literal(imm);
            check_signed_imm(imm_val, 6, "c.andi", location)?;
            let imm_bits = (imm_val as u16) & 0x3F;
            let imm_5 = (imm_bits >> 5) & 1;
            let imm_4_0 = imm_bits & 0x1F;
            Ok(0b100_1_10_000_00_000_01
                | (imm_5 << 12)
                | (rd_enc << 7)
                | (imm_4_0 << 2))
        }

        // CB format branches: c.beqz, c.bnez
        // opcode: 110 | offset[8|4:3] | rs1'[2:0] | offset[7:6|2:1|5] | 01
        // offset is 9-bit signed: -256 to 254, must be even
        (CBeqz, CBBranch { rs1_prime, offset }) => {
            let rs1_enc = rs1_prime.compressed_encoding() as u16;
            let offset_val = extract_literal(offset);
            check_compressed_branch_offset(offset_val, location)?;
            let offset_bits = ((offset_val as u16) & 0x1FF) as u16;
            let offset_8 = (offset_bits >> 8) & 1;
            let offset_7_6 = (offset_bits >> 6) & 0x3;
            let offset_5 = (offset_bits >> 5) & 1;
            let offset_4_3 = (offset_bits >> 3) & 0x3;
            let offset_2_1 = (offset_bits >> 1) & 0x3;
            Ok(0b110_000_000_00000_01
                | (offset_8 << 12)
                | (offset_4_3 << 10)
                | (rs1_enc << 7)
                | (offset_7_6 << 5)
                | (offset_2_1 << 3)
                | (offset_5 << 2))
        }

        (CBnez, CBBranch { rs1_prime, offset }) => {
            let rs1_enc = rs1_prime.compressed_encoding() as u16;
            let offset_val = extract_literal(offset);
            check_compressed_branch_offset(offset_val, location)?;
            let offset_bits = ((offset_val as u16) & 0x1FF) as u16;
            let offset_8 = (offset_bits >> 8) & 1;
            let offset_7_6 = (offset_bits >> 6) & 0x3;
            let offset_5 = (offset_bits >> 5) & 1;
            let offset_4_3 = (offset_bits >> 3) & 0x3;
            let offset_2_1 = (offset_bits >> 1) & 0x3;
            Ok(0b111_000_000_00000_01
                | (offset_8 << 12)
                | (offset_4_3 << 10)
                | (rs1_enc << 7)
                | (offset_7_6 << 5)
                | (offset_2_1 << 3)
                | (offset_5 << 2))
        }

        // CJ format: c.j offset
        // opcode: 101 | offset[11|4|9:8|10|6|7|3:1|5] | 01
        // offset is 12-bit signed: -2048 to 2046, must be even
        (CJComp, CJOpnd { offset }) => {
            let offset_val = extract_literal(offset);
            check_compressed_jump_offset(offset_val, location)?;
            let offset_bits = ((offset_val as u16) & 0xFFF) as u16;
            let offset_11 = (offset_bits >> 11) & 1;
            let offset_10 = (offset_bits >> 10) & 1;
            let offset_9_8 = (offset_bits >> 8) & 0x3;
            let offset_7 = (offset_bits >> 7) & 1;
            let offset_6 = (offset_bits >> 6) & 1;
            let offset_5 = (offset_bits >> 5) & 1;
            let offset_4 = (offset_bits >> 4) & 1;
            let offset_3_1 = (offset_bits >> 1) & 0x7;
            Ok(0b101_00000000000_01
                | (offset_11 << 12)
                | (offset_4 << 11)
                | (offset_9_8 << 9)
                | (offset_10 << 8)
                | (offset_6 << 7)
                | (offset_7 << 6)
                | (offset_3_1 << 3)
                | (offset_5 << 2))
        }

        // CJ format: c.jal offset (RV32C only)
        // Same encoding as c.j but with opcode 001
        (CJalComp, CJOpnd { offset }) => {
            let offset_val = extract_literal(offset);
            check_compressed_jump_offset(offset_val, location)?;
            let offset_bits = ((offset_val as u16) & 0xFFF) as u16;
            let offset_11 = (offset_bits >> 11) & 1;
            let offset_10 = (offset_bits >> 10) & 1;
            let offset_9_8 = (offset_bits >> 8) & 0x3;
            let offset_7 = (offset_bits >> 7) & 1;
            let offset_6 = (offset_bits >> 6) & 1;
            let offset_5 = (offset_bits >> 5) & 1;
            let offset_4 = (offset_bits >> 4) & 1;
            let offset_3_1 = (offset_bits >> 1) & 0x7;
            Ok(0b001_00000000000_01
                | (offset_11 << 12)
                | (offset_4 << 11)
                | (offset_9_8 << 9)
                | (offset_10 << 8)
                | (offset_6 << 7)
                | (offset_7 << 6)
                | (offset_3_1 << 3)
                | (offset_5 << 2))
        }

        // Special: c.nop
        // opcode: 000 | 0 | 00000 | 00000 | 01
        (CNop, None) => Ok(0b000_0_00000_00000_01),

        // Special: c.ebreak
        // opcode: 1001 | 00000 | 00000 | 10
        (CEbreak, None) => Ok(0b1001_00000_00000_10),

        // Unimplemented or unsupported combinations
        _ => Err(AssemblerError::from_context(
            format!(
                "Invalid/unimplemented compressed instruction encoding: {:?} {:?}",
                op, operands
            ),
            location.clone(),
        )),
    }
}

/// Convert register to 5-bit encoding
fn reg_to_bits(reg: Register) -> u16 {
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

/// Check if signed value fits in N bits
fn check_signed_imm(
    val: i32,
    bits: u32,
    inst: &str,
    location: &crate::ast::Location,
) -> Result<()> {
    let min = -(1i32 << (bits - 1));
    let max = (1i32 << (bits - 1)) - 1;
    if val < min || val > max {
        return Err(AssemblerError::from_context(
            format!(
                "{} immediate {} out of range [{}, {}]",
                inst, val, min, max
            ),
            location.clone(),
        ));
    }
    Ok(())
}

/// Check compressed branch offset (9-bit signed, must be even)
fn check_compressed_branch_offset(
    offset: i32,
    location: &crate::ast::Location,
) -> Result<()> {
    if offset % 2 != 0 {
        return Err(AssemblerError::from_context(
            format!(
                "Compressed branch offset {} must be 2-byte aligned",
                offset
            ),
            location.clone(),
        ));
    }
    if !(-256..=254).contains(&offset) {
        return Err(AssemblerError::from_context(
            format!(
                "Compressed branch offset {} out of range [-256, 254]",
                offset
            ),
            location.clone(),
        ));
    }
    Ok(())
}

/// Check compressed jump offset (12-bit signed, must be even)
fn check_compressed_jump_offset(
    offset: i32,
    location: &crate::ast::Location,
) -> Result<()> {
    if offset % 2 != 0 {
        return Err(AssemblerError::from_context(
            format!("Compressed jump offset {} must be 2-byte aligned", offset),
            location.clone(),
        ));
    }
    if !(-2048..=2046).contains(&offset) {
        return Err(AssemblerError::from_context(
            format!(
                "Compressed jump offset {} out of range [-2048, 2046]",
                offset
            ),
            location.clone(),
        ));
    }
    Ok(())
}
