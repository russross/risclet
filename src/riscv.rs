use super::*;
use crate::decoder::InstructionDecoder;
use crate::execution_context::ExecutionContext;
use std::collections::HashMap;

pub fn get_funct3(inst: i32) -> i32 {
    (inst >> 12) & 0x07
}

pub fn get_rd(inst: i32) -> usize {
    ((inst >> 7) & 0x1f) as usize
}

pub fn get_rs1(inst: i32) -> usize {
    ((inst >> 15) & 0x1f) as usize
}

pub fn get_rs2(inst: i32) -> usize {
    ((inst >> 20) & 0x1f) as usize
}

pub fn get_imm_i(inst: i32) -> i32 {
    inst >> 20
}

pub fn get_imm_s(inst: i32) -> i32 {
    let mut imm = (inst >> 20) & !0x0000001f;
    imm |= (inst >> 7) & 0x0000001f;
    imm
}

pub fn get_imm_b(inst: i32) -> i32 {
    let mut imm = (inst >> 20) & !0x00000fff;
    imm |= (inst << 4) & 0x00000800;
    imm |= (inst >> 20) & 0x000007e0;
    imm |= (inst >> 7) & 0x0000001e;
    imm
}

pub fn get_imm_u(inst: i32) -> i32 {
    inst & !0x00000fff
}

pub fn get_imm_j(inst: i32) -> i32 {
    let mut imm = (inst >> 11) & !0x000fffff;
    imm |= inst & 0x000ff000;
    imm |= (inst >> 9) & 0x00000800;
    imm |= (inst >> 20) & 0x000007e0;
    imm |= (inst >> 20) & 0x0000001e;
    imm
}

pub fn get_funct7(inst: i32) -> i32 {
    inst >> 25
}

// Extract the opcode (lowest 2 bits) from a compressed instruction
pub fn get_c_op(inst: i32) -> i32 {
    inst & 0x3
}

// Extract the funct3 field (bits 15-13) from a compressed instruction
pub fn get_c_funct3(inst: i32) -> i32 {
    (inst >> 13) & 0x7
}

// Extract the rd'/rs1' field (bits 9-7) from a compressed instruction (x8-x15)
pub fn get_c_rs1_prime(inst: i32) -> usize {
    (((inst >> 7) & 0x7) + 8) as usize
}

// Extract the rs2' field (bits 4-2) from a compressed instruction (x8-x15)
pub fn get_c_rs2_prime(inst: i32) -> usize {
    (((inst >> 2) & 0x7) + 8) as usize
}

// Extract the 5-bit rd/rs1 field (bits 11-7) from a compressed instruction
pub fn get_c_rd_rs1(inst: i32) -> usize {
    ((inst >> 7) & 0x1f) as usize
}

// Extract the 5-bit rs2 field (bits 6-2) from a compressed instruction
pub fn get_c_rs2(inst: i32) -> usize {
    ((inst >> 2) & 0x1f) as usize
}

// Helper for sign extension from a specific bit position (width) within an i32
pub fn sign_extend(value: i32, width: u32) -> i32 {
    let shift = 32 - width;
    (value << shift) >> shift
}

macro_rules! define_immediate_decoders {
    (
        $($name:ident {
            mappings: [$(($src:expr, $dst:expr)),* $(,)?],
            signed: $signed:expr,
            width: $width:expr $(,)?
        }),* $(,)?
    ) => {
        $(
            pub fn $name(inst: i32) -> i32 {
                let mut imm = 0i32;
                $(imm |= ((inst >> $src) & 1) << $dst;)*
                if $signed {
                    sign_extend(imm, $width)
                } else {
                    imm
                }
            }
        )*
    };
}

define_immediate_decoders! {
    get_c_lwsp_imm {
        mappings: [(12, 5), (6, 4), (5, 3), (4, 2), (3, 7), (2, 6)],
        signed: false,
        width: 7
    },
    get_c_swsp_imm {
        mappings: [(12, 5), (11, 4), (10, 3), (9, 2), (8, 7), (7, 6)],
        signed: false,
        width: 7
    },
    get_c_lw_sw_imm {
        mappings: [(12, 5), (11, 4), (10, 3), (6, 2), (5, 6)],
        signed: false,
        width: 5
    },
    get_c_j_jal_imm {
        mappings: [
            (12, 11), (11, 4), (10, 9), (9, 8), (8, 10),
            (7, 6), (6, 7), (5, 3), (4, 2), (3, 1), (2, 5)
        ],
        signed: true,
        width: 12
    },
    get_c_beqz_bnez_imm {
        mappings: [(12, 8), (11, 4), (10, 3), (6, 7), (5, 6), (4, 2), (3, 1), (2, 5)],
        signed: true,
        width: 9
    },
    get_c_li_addi_addiw_andi_imm {
        mappings: [(12, 5), (6, 4), (5, 3), (4, 2), (3, 1), (2, 0)],
        signed: true,
        width: 6
    },
    get_c_lui_imm {
        mappings: [(12, 17), (6, 16), (5, 15), (4, 14), (3, 13), (2, 12)],
        signed: true,
        width: 18
    },
    get_c_addi16sp_imm {
        mappings: [(12, 9), (6, 4), (5, 6), (4, 8), (3, 7), (2, 5)],
        signed: true,
        width: 10
    },
    get_c_addi4spn_imm {
        mappings: [(12, 5), (11, 4), (10, 9), (9, 8), (8, 7), (7, 6), (6, 2), (5, 3)],
        signed: false,
        width: 8
    },
    get_c_slli_srli_srai_imm {
        mappings: [(12, 5), (6, 4), (5, 3), (4, 2), (3, 1), (2, 0)],
        signed: false,
        width: 6
    }
}

pub const R: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1",
    "a2", "a3", "a4", "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7",
    "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
];

pub const ZERO: usize = 0;
pub const RA: usize = 1;
pub const SP: usize = 2;
pub const GP: usize = 3;
pub const A0: usize = 10;
pub const A1: usize = 11;
pub const A2: usize = 12;

pub const A_REGS: [usize; 8] = [10, 11, 12, 13, 14, 15, 16, 17];
pub const T_REGS: [usize; 7] = [5, 6, 7, 28, 29, 30, 31];
pub const S_REGS: [usize; 12] = [8, 9, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27];

pub enum Op {
    // r-type
    Add { rd: usize, rs1: usize, rs2: usize },
    Sub { rd: usize, rs1: usize, rs2: usize },
    Sll { rd: usize, rs1: usize, rs2: usize },
    Slt { rd: usize, rs1: usize, rs2: usize },
    Sltu { rd: usize, rs1: usize, rs2: usize },
    Xor { rd: usize, rs1: usize, rs2: usize },
    Srl { rd: usize, rs1: usize, rs2: usize },
    Sra { rd: usize, rs1: usize, rs2: usize },
    Or { rd: usize, rs1: usize, rs2: usize },
    And { rd: usize, rs1: usize, rs2: usize },

    // i-type
    Addi { rd: usize, rs1: usize, imm: i32 },
    Slti { rd: usize, rs1: usize, imm: i32 },
    Sltiu { rd: usize, rs1: usize, imm: i32 },
    Xori { rd: usize, rs1: usize, imm: i32 },
    Ori { rd: usize, rs1: usize, imm: i32 },
    Andi { rd: usize, rs1: usize, imm: i32 },
    Slli { rd: usize, rs1: usize, shamt: i32 },
    Srli { rd: usize, rs1: usize, shamt: i32 },
    Srai { rd: usize, rs1: usize, shamt: i32 },

    // branch
    Beq { rs1: usize, rs2: usize, offset: i32 },
    Bne { rs1: usize, rs2: usize, offset: i32 },
    Blt { rs1: usize, rs2: usize, offset: i32 },
    Bge { rs1: usize, rs2: usize, offset: i32 },
    Bltu { rs1: usize, rs2: usize, offset: i32 },
    Bgeu { rs1: usize, rs2: usize, offset: i32 },

    // jump
    Jal { rd: usize, offset: i32 },
    Jalr { rd: usize, rs1: usize, offset: i32 },

    // load
    Lb { rd: usize, rs1: usize, offset: i32 },
    Lh { rd: usize, rs1: usize, offset: i32 },
    Lw { rd: usize, rs1: usize, offset: i32 },
    Lbu { rd: usize, rs1: usize, offset: i32 },
    Lhu { rd: usize, rs1: usize, offset: i32 },

    // store
    Sb { rs1: usize, rs2: usize, offset: i32 },
    Sh { rs1: usize, rs2: usize, offset: i32 },
    Sw { rs1: usize, rs2: usize, offset: i32 },

    // u-type
    Lui { rd: usize, imm: i32 },
    Auipc { rd: usize, imm: i32 },

    // misc
    Fence,
    Ecall,
    Ebreak,

    // m extension
    Mul { rd: usize, rs1: usize, rs2: usize },
    Mulh { rd: usize, rs1: usize, rs2: usize },
    Mulhsu { rd: usize, rs1: usize, rs2: usize },
    Mulhu { rd: usize, rs1: usize, rs2: usize },
    Div { rd: usize, rs1: usize, rs2: usize },
    Divu { rd: usize, rs1: usize, rs2: usize },
    Rem { rd: usize, rs1: usize, rs2: usize },
    Remu { rd: usize, rs1: usize, rs2: usize },

    // a extension - load reserved / store conditional
    LrW { rd: usize, rs1: usize, aq: bool, rl: bool },
    ScW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },

    // a extension - atomic memory operations
    AmoswapW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmoaddW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmoxorW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmoandW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmoorW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmominW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmomaxW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmominuW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },
    AmomaxuW { rd: usize, rs1: usize, rs2: usize, aq: bool, rl: bool },

    Unimplemented { inst: i32, note: String },
}

#[allow(dead_code)]
impl Op {
    pub fn new(inst: i32) -> Self {
        InstructionDecoder::decode(inst)
    }

    fn decode_branches(inst: i32) -> Self {
        let funct3 = get_funct3(inst);
        let rs1 = get_rs1(inst);
        let rs2 = get_rs2(inst);
        let offset = get_imm_b(inst);

        match funct3 {
            0 => Op::Beq { rs1, rs2, offset },
            1 => Op::Bne { rs1, rs2, offset },
            4 => Op::Blt { rs1, rs2, offset },
            5 => Op::Bge { rs1, rs2, offset },
            6 => Op::Bltu { rs1, rs2, offset },
            7 => Op::Bgeu { rs1, rs2, offset },
            _ => Op::Unimplemented {
                inst,
                note: format!("branch instruction of unknown type {}", funct3),
            },
        }
    }

    fn decode_load(inst: i32) -> Self {
        let funct3 = get_funct3(inst);
        let rd = get_rd(inst);
        let rs1 = get_rs1(inst);
        let offset = get_imm_i(inst);

        match funct3 {
            0 => Op::Lb { rd, rs1, offset },
            1 => Op::Lh { rd, rs1, offset },
            2 => Op::Lw { rd, rs1, offset },
            4 => Op::Lbu { rd, rs1, offset },
            5 => Op::Lhu { rd, rs1, offset },
            _ => Op::Unimplemented {
                inst,
                note: format!("load instruction of unknown type {}", funct3),
            },
        }
    }

    fn decode_store(inst: i32) -> Self {
        let funct3 = get_funct3(inst);
        let rs1 = get_rs1(inst);
        let rs2 = get_rs2(inst);
        let offset = get_imm_s(inst);

        match funct3 {
            0 => Op::Sb { rs1, rs2, offset },
            1 => Op::Sh { rs1, rs2, offset },
            2 => Op::Sw { rs1, rs2, offset },
            _ => Op::Unimplemented {
                inst,
                note: format!("store instruction of unknown type {}", funct3),
            },
        }
    }

    fn decode_i_type(inst: i32) -> Self {
        let funct3 = get_funct3(inst);
        let rd = get_rd(inst);
        let rs1 = get_rs1(inst);
        let imm = get_imm_i(inst);
        let shamt = imm & 0x1f;
        let imm_high = imm >> 5;

        match funct3 {
            0 => Op::Addi { rd, rs1, imm },
            1 => {
                if imm_high == 0 {
                    Op::Slli { rd, rs1, shamt }
                } else {
                    Op::Unimplemented {
                        inst,
                        note: format!(
                            "immediate mode alu instruction of type {} with unknown subtype {}",
                            funct3, imm_high
                        ),
                    }
                }
            }
            2 => Op::Slti { rd, rs1, imm },
            3 => Op::Sltiu { rd, rs1, imm },
            4 => Op::Xori { rd, rs1, imm },
            5 => match imm_high {
                0x00 => Op::Srli { rd, rs1, shamt },
                0x20 => Op::Srai { rd, rs1, shamt },
                _ => Op::Unimplemented {
                    inst,
                    note: format!(
                        "immediate mode alu instruction of type {} with unknown subtype {}",
                        funct3, imm_high
                    ),
                },
            },
            6 => Op::Ori { rd, rs1, imm },
            7 => Op::Andi { rd, rs1, imm },
            _ => Op::Unimplemented {
                inst,
                note: format!("alu immediate of unknown type {}", funct3),
            },
        }
    }

    fn decode_r_type(inst: i32) -> Self {
        let funct3 = get_funct3(inst);
        let funct7 = get_funct7(inst);
        let rd = get_rd(inst);
        let rs1 = get_rs1(inst);
        let rs2 = get_rs2(inst);

        match (funct7, funct3) {
            (0x00, 0x00) => Op::Add { rd, rs1, rs2 },
            (0x20, 0x00) => Op::Sub { rd, rs1, rs2 },
            (0x00, 0x01) => Op::Sll { rd, rs1, rs2 },
            (0x00, 0x02) => Op::Slt { rd, rs1, rs2 },
            (0x00, 0x03) => Op::Sltu { rd, rs1, rs2 },
            (0x00, 0x04) => Op::Xor { rd, rs1, rs2 },
            (0x00, 0x05) => Op::Srl { rd, rs1, rs2 },
            (0x20, 0x05) => Op::Sra { rd, rs1, rs2 },
            (0x00, 0x06) => Op::Or { rd, rs1, rs2 },
            (0x00, 0x07) => Op::And { rd, rs1, rs2 },

            (0x01, 0x00) => Op::Mul { rd, rs1, rs2 },
            (0x01, 0x01) => Op::Mulh { rd, rs1, rs2 },
            (0x01, 0x02) => Op::Mulhsu { rd, rs1, rs2 },
            (0x01, 0x03) => Op::Mulhu { rd, rs1, rs2 },
            (0x01, 0x04) => Op::Div { rd, rs1, rs2 },
            (0x01, 0x05) => Op::Divu { rd, rs1, rs2 },
            (0x01, 0x06) => Op::Rem { rd, rs1, rs2 },
            (0x01, 0x07) => Op::Remu { rd, rs1, rs2 },

            _ => Op::Unimplemented {
                inst,
                note: format!(
                    "alu instruction of unknown type {} subtype {}",
                    funct3, funct7
                ),
            },
        }
    }

    fn decode_compressed(inst: i32) -> Self {
        let op = get_c_op(inst);
        let funct3 = get_c_funct3(inst);

        match (op, funct3) {
            // C0 quadrant
            (0, 0) => {
                // C.ADDI4SPN
                let rd = get_c_rs2_prime(inst);
                let imm = get_c_addi4spn_imm(inst);
                if rd == 0 && imm == 0 {
                    Op::Unimplemented {
                        inst,
                        note: String::from(
                            "Illegal compressed instruction at (0, 0)",
                        ),
                    }
                } else if imm == 0 {
                    Op::Unimplemented {
                        inst,
                        note: String::from("C.ADDI4SPN with imm=0 is reserved"),
                    }
                } else {
                    Op::Addi { rd, rs1: SP, imm }
                }
            }
            (0, 1) => {
                // C.FLD (not supported)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.FLD is not supported"),
                }
            }
            (0, 2) => {
                // C.LW
                let rd = get_c_rs2_prime(inst);
                let rs1 = get_c_rs1_prime(inst);
                let imm = get_c_lw_sw_imm(inst);
                Op::Lw { rd, rs1, offset: imm }
            }
            (0, 3) => {
                // C.LD (RV64, not supported in RV32)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.LD is not supported in RV32"),
                }
            }
            (0, 4) => {
                // Reserved
                Op::Unimplemented {
                    inst,
                    note: String::from(
                        "Reserved compressed instruction at (0, 4)",
                    ),
                }
            }
            (0, 5) => {
                // C.FSD (not supported)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.FSD is not supported"),
                }
            }
            (0, 6) => {
                // C.SW
                let rs2 = get_c_rs2_prime(inst);
                let rs1 = get_c_rs1_prime(inst);
                let imm = get_c_lw_sw_imm(inst);
                Op::Sw { rs1, rs2, offset: imm }
            }
            (0, 7) => {
                // C.SD (RV64, not supported in RV32)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.SD is not supported in RV32"),
                }
            }

            // C1 quadrant
            (1, 0) => {
                // C.ADDI
                // note: rd == 0 => NOP, but we encode that as Addi anyway
                let rd = get_c_rd_rs1(inst);
                let imm = get_c_li_addi_addiw_andi_imm(inst);
                Op::Addi { rd, rs1: rd, imm }
            }
            (1, 1) => {
                // C.ADDIW - RV64 specific, not supported in RV32
                Op::Unimplemented {
                    inst,
                    note: String::from("C.ADDIW is not supported in RV32"),
                }
            }
            (1, 2) => {
                // C.LI
                // note: rd == 0 => hint
                let rd = get_c_rd_rs1(inst);
                let imm = get_c_li_addi_addiw_andi_imm(inst);
                Op::Addi { rd, rs1: ZERO, imm }
            }
            (1, 3) => {
                let rd = get_c_rd_rs1(inst);
                if rd == 2 {
                    // C.ADDI16SP
                    let imm = get_c_addi16sp_imm(inst);
                    if imm == 0 {
                        Op::Unimplemented {
                            inst,
                            note: String::from(
                                "C.ADDI16SP with imm=0 is reserved",
                            ),
                        }
                    } else {
                        Op::Addi { rd: SP, rs1: SP, imm }
                    }
                } else {
                    // C.LUI
                    let imm = get_c_lui_imm(inst);
                    if imm == 0 {
                        Op::Unimplemented {
                            inst,
                            note: String::from("C.LUI with imm=0 is reserved"),
                        }
                    } else {
                        // note: rd == 0 => hint
                        Op::Lui { rd, imm }
                    }
                }
            }
            (1, 4) => {
                // Various operations based on bits 11:10
                let funct2 = (inst >> 10) & 0x3;
                let rd = get_c_rs1_prime(inst);
                match funct2 {
                    0 => {
                        // C.SRLI
                        let shamt = get_c_slli_srli_srai_imm(inst);
                        Op::Srli { rd, rs1: rd, shamt }
                    }
                    1 => {
                        // C.SRAI
                        let shamt = get_c_slli_srli_srai_imm(inst);
                        Op::Srai { rd, rs1: rd, shamt }
                    }
                    2 => {
                        // C.ANDI
                        let imm = get_c_li_addi_addiw_andi_imm(inst);
                        Op::Andi { rd, rs1: rd, imm }
                    }
                    3 => {
                        // register-register instructions based on bits 6:5
                        let rs2 = get_c_rs2_prime(inst);
                        let bit12 = (inst >> 12) & 0x1;
                        let funct = (inst >> 5) & 0x3;
                        match (bit12, funct) {
                            (0, 0) => Op::Sub { rd, rs1: rd, rs2 },
                            (0, 1) => Op::Xor { rd, rs1: rd, rs2 },
                            (0, 2) => Op::Or { rd, rs1: rd, rs2 },
                            (0, 3) => Op::And { rd, rs1: rd, rs2 },
                            (1, 0) | (1, 1) => Op::Unimplemented {
                                inst,
                                note: "C.SUBW/C.ADDW are not supported in RV32"
                                    .to_string(),
                            },
                            _ => Op::Unimplemented {
                                inst,
                                note:
                                    "Reserved compressed instruction at (1, 4)"
                                        .to_string(),
                            },
                        }
                    }
                    _ => unreachable!(),
                }
            }
            (1, 5) => {
                // C.J
                let offset = get_c_j_jal_imm(inst);
                Op::Jal { rd: ZERO, offset }
            }
            (1, 6) => {
                // C.BEQZ
                let rs1 = get_c_rs1_prime(inst);
                let offset = get_c_beqz_bnez_imm(inst);
                Op::Beq { rs1, rs2: ZERO, offset }
            }
            (1, 7) => {
                // C.BNEZ
                let rs1 = get_c_rs1_prime(inst);
                let offset = get_c_beqz_bnez_imm(inst);
                Op::Bne { rs1, rs2: ZERO, offset }
            }

            // C2 quadrant
            (2, 0) => {
                // C.SLLI
                // note: rd == 0 => hint
                let rd = get_c_rd_rs1(inst);
                let shamt = get_c_slli_srli_srai_imm(inst);
                Op::Slli { rd, rs1: rd, shamt }
            }
            (2, 1) => {
                // C.FLDSP (not supported)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.FLDSP is not supported"),
                }
            }
            (2, 2) => {
                // C.LWSP
                let rd = get_c_rd_rs1(inst);
                let imm = get_c_lwsp_imm(inst);
                if rd == 0 {
                    Op::Unimplemented {
                        inst,
                        note: String::from("C.LWSP with rd=0 is reserved"),
                    }
                } else {
                    Op::Lw { rd, rs1: SP, offset: imm }
                }
            }
            (2, 3) => {
                // C.LDSP (RV64, not supported in RV32)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.LDSP is not supported in RV32"),
                }
            }
            (2, 4) => {
                let rd = get_c_rd_rs1(inst);
                let rs2 = get_c_rs2(inst);
                let bit12 = (inst >> 12) & 0x1;

                match (bit12, rd, rs2) {
                    (0, 0, 0) => Op::Unimplemented {
                        inst,
                        note: String::from("C.JR with rd=0 is reserved"),
                    },

                    // C.JR
                    (0, _, 0) => Op::Jalr { rd: ZERO, rs1: rd, offset: 0 },

                    // C.MV
                    // note: rd == 0 => hint
                    (0, _, _) => Op::Add { rd, rs1: ZERO, rs2 },

                    // C.EBREAK
                    (1, 0, 0) => Op::Ebreak,

                    // C.JALR
                    (1, _, 0) => Op::Jalr { rd: RA, rs1: rd, offset: 0 },

                    // C.ADD
                    // note: rd == 0 => hint
                    (_, _, _) => Op::Add { rd, rs1: rd, rs2 },
                }
            }
            (2, 5) => {
                // C.FSDSP (not supported)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.FSDSP is not supported"),
                }
            }
            (2, 6) => {
                // C.SWSP
                let rs2 = get_c_rs2(inst);
                let imm = get_c_swsp_imm(inst);
                Op::Sw { rs1: SP, rs2, offset: imm }
            }
            (2, 7) => {
                // C.SDSP (RV64, not supported in RV32)
                Op::Unimplemented {
                    inst,
                    note: String::from("C.SDSP is not supported in RV32"),
                }
            }

            // uncompressed instructions take a different decoding path
            _ => unreachable!(),
        }
    }

    pub fn execute(&self, m: &mut Machine, length: u32) -> Result<(), String> {
        match self {
            // r-type
            Op::Add { rd, rs1, rs2 } => {
                let val = m.get(*rs1).wrapping_add(m.get(*rs2));
                m.set(*rd, val);
            }
            Op::Sub { rd, rs1, rs2 } => {
                let val = m.get(*rs1).wrapping_sub(m.get(*rs2));
                m.set(*rd, val);
            }
            Op::Sll { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2) & 0x1f;
                let val = m.get(*rs1) << rs2_val;
                m.set(*rd, val);
            }
            Op::Slt { rd, rs1, rs2 } => {
                let val = if m.get(*rs1) < m.get(*rs2) { 1 } else { 0 };
                m.set(*rd, val);
            }
            Op::Sltu { rd, rs1, rs2 } => {
                let val = if (m.get(*rs1) as u32) < (m.get(*rs2) as u32) {
                    1
                } else {
                    0
                };
                m.set(*rd, val);
            }
            Op::Xor { rd, rs1, rs2 } => {
                let val = m.get(*rs1) ^ m.get(*rs2);
                m.set(*rd, val);
            }
            Op::Srl { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2) & 0x1f;
                let val = ((m.get(*rs1) as u32) >> rs2_val) as i32;
                m.set(*rd, val);
            }
            Op::Sra { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2) & 0x1f;
                let val = m.get(*rs1) >> rs2_val;
                m.set(*rd, val);
            }
            Op::Or { rd, rs1, rs2 } => {
                let val = m.get(*rs1) | m.get(*rs2);
                m.set(*rd, val);
            }
            Op::And { rd, rs1, rs2 } => {
                let val = m.get(*rs1) & m.get(*rs2);
                m.set(*rd, val);
            }

            // i-type
            Op::Addi { rd, rs1, imm } => {
                let val = m.get(*rs1).wrapping_add(*imm);
                m.set(*rd, val);
            }
            Op::Slti { rd, rs1, imm } => {
                let val = if m.get(*rs1) < *imm { 1 } else { 0 };
                m.set(*rd, val);
            }
            Op::Sltiu { rd, rs1, imm } => {
                let val =
                    if (m.get(*rs1) as u32) < (*imm as u32) { 1 } else { 0 };
                m.set(*rd, val);
            }
            Op::Xori { rd, rs1, imm } => {
                let val = m.get(*rs1) ^ *imm;
                m.set(*rd, val);
            }
            Op::Ori { rd, rs1, imm } => {
                let val = m.get(*rs1) | *imm;
                m.set(*rd, val);
            }
            Op::Andi { rd, rs1, imm } => {
                let val = m.get(*rs1) & *imm;
                m.set(*rd, val);
            }
            Op::Slli { rd, rs1, shamt } => {
                let val = m.get(*rs1) << *shamt;
                m.set(*rd, val);
            }
            Op::Srli { rd, rs1, shamt } => {
                let val = ((m.get(*rs1) as u32) >> *shamt) as i32;
                m.set(*rd, val);
            }
            Op::Srai { rd, rs1, shamt } => {
                let val = m.get(*rs1) >> *shamt;
                m.set(*rd, val);
            }

            // branch
            Op::Beq { rs1, rs2, offset } => {
                if m.get(*rs1) == m.get(*rs2) {
                    m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
                }
            }
            Op::Bne { rs1, rs2, offset } => {
                if m.get(*rs1) != m.get(*rs2) {
                    m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
                }
            }
            Op::Blt { rs1, rs2, offset } => {
                if m.get(*rs1) < m.get(*rs2) {
                    m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
                }
            }
            Op::Bge { rs1, rs2, offset } => {
                if m.get(*rs1) >= m.get(*rs2) {
                    m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
                }
            }
            Op::Bltu { rs1, rs2, offset } => {
                if (m.get(*rs1) as u32) < (m.get(*rs2) as u32) {
                    m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
                }
            }
            Op::Bgeu { rs1, rs2, offset } => {
                if (m.get(*rs1) as u32) >= (m.get(*rs2) as u32) {
                    m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
                }
            }

            // jump
            Op::Jal { rd, offset } => {
                let return_addr = m.pc().wrapping_add(length);
                m.set(*rd, return_addr as i32);
                m.set_pc((m.pc() as i32).wrapping_add(*offset) as u32)?;
            }
            Op::Jalr { rd, rs1, offset } => {
                let rs1_val = m.get(*rs1);
                let return_addr = m.pc().wrapping_add(length);
                m.set(*rd, return_addr as i32);
                m.set_pc(((rs1_val as u32).wrapping_add(*offset as u32)) & !1)?;
            }

            // load
            Op::Lb { rd, rs1, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let val = m.load_i8(effective_address)?;
                m.set(*rd, val);
            }
            Op::Lh { rd, rs1, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let val = m.load_i16(effective_address)?;
                m.set(*rd, val);
            }
            Op::Lw { rd, rs1, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let val = m.load_i32(effective_address)?;
                m.set(*rd, val);
            }
            Op::Lbu { rd, rs1, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let val = m.load_u8(effective_address)?;
                m.set(*rd, val);
            }
            Op::Lhu { rd, rs1, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let val = m.load_u16(effective_address)?;
                m.set(*rd, val);
            }

            // store
            Op::Sb { rs1, rs2, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let raw = (m.get(*rs2) as u8).to_le_bytes();
                m.store(effective_address, &raw)?;
            }
            Op::Sh { rs1, rs2, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let raw = (m.get(*rs2) as u16).to_le_bytes();
                m.store(effective_address, &raw)?;
            }
            Op::Sw { rs1, rs2, offset } => {
                let effective_address =
                    (m.get(*rs1) as u32).wrapping_add(*offset as u32);
                let raw = (m.get(*rs2) as u32).to_le_bytes();
                m.store(effective_address, &raw)?;
            }

            // u-type
            Op::Lui { rd, imm } => {
                m.set(*rd, *imm);
            }
            Op::Auipc { rd, imm } => {
                let result = (m.pc() as i32).wrapping_add(*imm);
                m.set(*rd, result);
            }

            // misc
            Op::Fence => {
                // treat fence as a no-op
            }
            Op::Ecall => {
                match m.get(17) {
                    63 => {
                        // read system call
                        m.current_effect_mut().unwrap().other_message =
                            Some(format!(
                                "read({}, 0x{:x}, {})",
                                m.get(10),
                                m.get(11),
                                m.get(12)
                            ));
                        let fd = m.get(A0);
                        let buf_addr = m.get(A1) as u32;
                        let count = m.get(A2);

                        if fd != 0 {
                            return Err(format!(
                                "read syscall: only stdin (fd 0) supported, not {fd}"
                            ));
                        }
                        if count < 0 {
                            return Err(format!(
                                "read syscall: invalid buffer size: {count}"
                            ));
                        }

                        // make a buffer and read from stdin
                        let mut read_buffer = vec![0; count as usize];
                        let n =
                            m.io_provider_mut().read_stdin(&mut read_buffer)?;
                        read_buffer.truncate(n);

                        m.store(buf_addr, &read_buffer)?;
                        m.set(A0, read_buffer.len() as i32);
                        m.stdin_mut().extend_from_slice(&read_buffer);
                        m.current_effect_mut().unwrap().stdin =
                            Some(read_buffer);
                    }
                    64 => {
                        // write system call
                        m.current_effect_mut().unwrap().other_message =
                            Some(format!(
                                "write({}, 0x{:x}, {})",
                                m.get(A0),
                                m.get(A1),
                                m.get(A2)
                            ));
                        let fd = m.get(A0);
                        let buf_addr = m.get(11) as u32;
                        let count = m.get(12);

                        if fd != 1 {
                            return Err(format!(
                                "write syscall: only stdout (fd 1) supported, not {fd}"
                            ));
                        }
                        if count < 0 {
                            return Err(format!(
                                "write syscall: invalid buffer size: {count}"
                            ));
                        }

                        let write_buffer = m.load(buf_addr, count as u32)?;
                        m.io_provider_mut().write_stdout(&write_buffer)?;
                        m.set(A0, write_buffer.len() as i32);
                        m.stdout_mut().extend_from_slice(&write_buffer);
                        m.current_effect_mut().unwrap().stdout =
                            Some(write_buffer);
                    }
                    93 => {
                        // exit system call
                        let status = m.get(A0) & 0xff;
                        return Err(format!("exit({})", status));
                    }
                    syscall => {
                        return Err(format!("unsupported syscall {syscall}"));
                    }
                }
            }
            Op::Ebreak => {
                return Err(String::from("ebreak"));
            }

            // m extension
            Op::Mul { rd, rs1, rs2 } => {
                let val = m.get(*rs1).wrapping_mul(m.get(*rs2));
                m.set(*rd, val);
            }
            Op::Mulh { rd, rs1, rs2 } => {
                let val =
                    ((m.get(*rs1) as i64 * m.get(*rs2) as i64) >> 32) as i32;
                m.set(*rd, val);
            }
            Op::Mulhsu { rd, rs1, rs2 } => {
                let val = ((m.get(*rs1) as i64 * (m.get(*rs2) as u32 as i64))
                    >> 32) as i32;
                m.set(*rd, val);
            }
            Op::Mulhu { rd, rs1, rs2 } => {
                let val = (((m.get(*rs1) as u32 as u64)
                    * (m.get(*rs2) as u32 as u64))
                    >> 32) as i32;
                m.set(*rd, val);
            }
            Op::Div { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2);
                let val = if rs2_val == 0 {
                    -1
                } else {
                    m.get(*rs1).wrapping_div(rs2_val)
                };
                m.set(*rd, val);
            }
            Op::Divu { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2) as u32;
                let val = if rs2_val == 0 {
                    -1
                } else {
                    ((m.get(*rs1) as u32).wrapping_div(rs2_val)) as i32
                };
                m.set(*rd, val);
            }
            Op::Rem { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2);
                let val = if rs2_val == 0 {
                    m.get(*rs1)
                } else {
                    m.get(*rs1).wrapping_rem(rs2_val)
                };
                m.set(*rd, val);
            }
            Op::Remu { rd, rs1, rs2 } => {
                let rs2_val = m.get(*rs2) as u32;
                let val = if rs2_val == 0 {
                    m.get(*rs1)
                } else {
                    ((m.get(*rs1) as u32).wrapping_rem(rs2_val)) as i32
                };
                m.set(*rd, val);
            }

            // a extension - load reserved
            Op::LrW { rd, rs1, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let val = m.load_i32(addr)?;
                m.set(*rd, val);
                m.set_reservation(addr);
            }

            // a extension - store conditional
            Op::ScW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                if m.check_and_clear_reservation(addr) {
                    let val = m.get(*rs2) as u32;
                    m.store(addr, &val.to_le_bytes())?;
                    m.set(*rd, 0); // Success
                } else {
                    m.set(*rd, 1); // Failure
                }
            }

            // a extension - atomic swap
            Op::AmoswapW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let new_val = m.get(*rs2) as u32;
                m.store(addr, &new_val.to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic add
            Op::AmoaddW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let new_val = old_val.wrapping_add(m.get(*rs2));
                m.store(addr, &(new_val as u32).to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic xor
            Op::AmoxorW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let new_val = old_val ^ m.get(*rs2);
                m.store(addr, &(new_val as u32).to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic and
            Op::AmoandW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let new_val = old_val & m.get(*rs2);
                m.store(addr, &(new_val as u32).to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic or
            Op::AmoorW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let new_val = old_val | m.get(*rs2);
                m.store(addr, &(new_val as u32).to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic min (signed)
            Op::AmominW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let rs2_val = m.get(*rs2);
                let new_val = if old_val < rs2_val { old_val } else { rs2_val };
                m.store(addr, &(new_val as u32).to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic max (signed)
            Op::AmomaxW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)?;
                let rs2_val = m.get(*rs2);
                let new_val = if old_val > rs2_val { old_val } else { rs2_val };
                m.store(addr, &(new_val as u32).to_le_bytes())?;
                m.set(*rd, old_val);
            }

            // a extension - atomic min (unsigned)
            Op::AmominuW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)? as u32;
                let rs2_val = m.get(*rs2) as u32;
                let new_val = if old_val < rs2_val { old_val } else { rs2_val };
                m.store(addr, &new_val.to_le_bytes())?;
                m.set(*rd, old_val as i32);
            }

            // a extension - atomic max (unsigned)
            Op::AmomaxuW { rd, rs1, rs2, aq: _, rl: _ } => {
                let addr = m.get(*rs1) as u32;
                let old_val = m.load_i32(addr)? as u32;
                let rs2_val = m.get(*rs2) as u32;
                let new_val = if old_val > rs2_val { old_val } else { rs2_val };
                m.store(addr, &new_val.to_le_bytes())?;
                m.set(*rd, old_val as i32);
            }

            Op::Unimplemented { inst, note } => {
                return Err(format!("inst: 0x{:x} note: {}", inst, note));
            }
        }
        Ok(())
    }

    pub fn execute_with_context(
        &self,
        ctx: &mut dyn ExecutionContext,
        length: u32,
    ) -> Result<(), String> {
        match self {
            // r-type
            Op::Add { rd, rs1, rs2 } => {
                let val = ctx
                    .read_register(*rs1)
                    .wrapping_add(ctx.read_register(*rs2));
                ctx.write_register(*rd, val);
            }
            Op::Sub { rd, rs1, rs2 } => {
                let val = ctx
                    .read_register(*rs1)
                    .wrapping_sub(ctx.read_register(*rs2));
                ctx.write_register(*rd, val);
            }
            Op::Sll { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2) & 0x1f;
                let val = ctx.read_register(*rs1) << rs2_val;
                ctx.write_register(*rd, val);
            }
            Op::Slt { rd, rs1, rs2 } => {
                let val = if ctx.read_register(*rs1) < ctx.read_register(*rs2) {
                    1
                } else {
                    0
                };
                ctx.write_register(*rd, val);
            }
            Op::Sltu { rd, rs1, rs2 } => {
                let val = if (ctx.read_register(*rs1) as u32)
                    < (ctx.read_register(*rs2) as u32)
                {
                    1
                } else {
                    0
                };
                ctx.write_register(*rd, val);
            }
            Op::Xor { rd, rs1, rs2 } => {
                let val = ctx.read_register(*rs1) ^ ctx.read_register(*rs2);
                ctx.write_register(*rd, val);
            }
            Op::Srl { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2) & 0x1f;
                let val = ((ctx.read_register(*rs1) as u32) >> rs2_val) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Sra { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2) & 0x1f;
                let val = ctx.read_register(*rs1) >> rs2_val;
                ctx.write_register(*rd, val);
            }
            Op::Or { rd, rs1, rs2 } => {
                let val = ctx.read_register(*rs1) | ctx.read_register(*rs2);
                ctx.write_register(*rd, val);
            }
            Op::And { rd, rs1, rs2 } => {
                let val = ctx.read_register(*rs1) & ctx.read_register(*rs2);
                ctx.write_register(*rd, val);
            }

            // i-type
            Op::Addi { rd, rs1, imm } => {
                let val = ctx.read_register(*rs1).wrapping_add(*imm);
                ctx.write_register(*rd, val);
            }
            Op::Slti { rd, rs1, imm } => {
                let val = if ctx.read_register(*rs1) < *imm { 1 } else { 0 };
                ctx.write_register(*rd, val);
            }
            Op::Sltiu { rd, rs1, imm } => {
                let val = if (ctx.read_register(*rs1) as u32) < (*imm as u32) {
                    1
                } else {
                    0
                };
                ctx.write_register(*rd, val);
            }
            Op::Xori { rd, rs1, imm } => {
                let val = ctx.read_register(*rs1) ^ *imm;
                ctx.write_register(*rd, val);
            }
            Op::Ori { rd, rs1, imm } => {
                let val = ctx.read_register(*rs1) | *imm;
                ctx.write_register(*rd, val);
            }
            Op::Andi { rd, rs1, imm } => {
                let val = ctx.read_register(*rs1) & *imm;
                ctx.write_register(*rd, val);
            }
            Op::Slli { rd, rs1, shamt } => {
                let val = ctx.read_register(*rs1) << *shamt;
                ctx.write_register(*rd, val);
            }
            Op::Srli { rd, rs1, shamt } => {
                let val = ((ctx.read_register(*rs1) as u32) >> *shamt) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Srai { rd, rs1, shamt } => {
                let val = ctx.read_register(*rs1) >> *shamt;
                ctx.write_register(*rd, val);
            }

            // branch
            Op::Beq { rs1, rs2, offset } => {
                if ctx.read_register(*rs1) == ctx.read_register(*rs2) {
                    ctx.write_pc(
                        (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                    )?;
                }
            }
            Op::Bne { rs1, rs2, offset } => {
                if ctx.read_register(*rs1) != ctx.read_register(*rs2) {
                    ctx.write_pc(
                        (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                    )?;
                }
            }
            Op::Blt { rs1, rs2, offset } => {
                if ctx.read_register(*rs1) < ctx.read_register(*rs2) {
                    ctx.write_pc(
                        (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                    )?;
                }
            }
            Op::Bge { rs1, rs2, offset } => {
                if ctx.read_register(*rs1) >= ctx.read_register(*rs2) {
                    ctx.write_pc(
                        (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                    )?;
                }
            }
            Op::Bltu { rs1, rs2, offset } => {
                if (ctx.read_register(*rs1) as u32)
                    < (ctx.read_register(*rs2) as u32)
                {
                    ctx.write_pc(
                        (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                    )?;
                }
            }
            Op::Bgeu { rs1, rs2, offset } => {
                if (ctx.read_register(*rs1) as u32)
                    >= (ctx.read_register(*rs2) as u32)
                {
                    ctx.write_pc(
                        (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                    )?;
                }
            }

            // jump
            Op::Jal { rd, offset } => {
                let return_addr = ctx.read_pc().wrapping_add(length);
                ctx.write_register(*rd, return_addr as i32);
                ctx.write_pc(
                    (ctx.read_pc() as i32).wrapping_add(*offset) as u32
                )?;
            }
            Op::Jalr { rd, rs1, offset } => {
                let rs1_val = ctx.read_register(*rs1);
                let return_addr = ctx.read_pc().wrapping_add(length);
                ctx.write_register(*rd, return_addr as i32);
                ctx.write_pc(
                    ((rs1_val as u32).wrapping_add(*offset as u32)) & !1,
                )?;
            }

            // load
            Op::Lb { rd, rs1, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let bytes = ctx.read_memory(effective_address, 1)?;
                let val =
                    i8::from_le_bytes(bytes[..1].try_into().unwrap()) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Lh { rd, rs1, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let bytes = ctx.read_memory(effective_address, 2)?;
                let val =
                    i16::from_le_bytes(bytes[..2].try_into().unwrap()) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Lw { rd, rs1, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let bytes = ctx.read_memory(effective_address, 4)?;
                let val = i32::from_le_bytes(bytes[..4].try_into().unwrap());
                ctx.write_register(*rd, val);
            }
            Op::Lbu { rd, rs1, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let bytes = ctx.read_memory(effective_address, 1)?;
                let val =
                    u8::from_le_bytes(bytes[..1].try_into().unwrap()) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Lhu { rd, rs1, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let bytes = ctx.read_memory(effective_address, 2)?;
                let val =
                    u16::from_le_bytes(bytes[..2].try_into().unwrap()) as i32;
                ctx.write_register(*rd, val);
            }

            // store
            Op::Sb { rs1, rs2, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let raw = (ctx.read_register(*rs2) as u8).to_le_bytes();
                ctx.write_memory(effective_address, &raw)?;
            }
            Op::Sh { rs1, rs2, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let raw = (ctx.read_register(*rs2) as u16).to_le_bytes();
                ctx.write_memory(effective_address, &raw)?;
            }
            Op::Sw { rs1, rs2, offset } => {
                let effective_address = (ctx.read_register(*rs1) as u32)
                    .wrapping_add(*offset as u32);
                let raw = (ctx.read_register(*rs2) as u32).to_le_bytes();
                ctx.write_memory(effective_address, &raw)?;
            }

            // u-type
            Op::Lui { rd, imm } => {
                ctx.write_register(*rd, *imm);
            }
            Op::Auipc { rd, imm } => {
                let result = (ctx.read_pc() as i32).wrapping_add(*imm);
                ctx.write_register(*rd, result);
            }

            // misc
            Op::Fence => {}
            Op::Ecall => {
                return Err(
                    "ecall execution via ExecutionContext not yet implemented"
                        .to_string(),
                );
            }
            Op::Ebreak => {
                return Err(String::from("ebreak"));
            }

            // m extension
            Op::Mul { rd, rs1, rs2 } => {
                let val = ctx
                    .read_register(*rs1)
                    .wrapping_mul(ctx.read_register(*rs2));
                ctx.write_register(*rd, val);
            }
            Op::Mulh { rd, rs1, rs2 } => {
                let val = ((ctx.read_register(*rs1) as i64
                    * ctx.read_register(*rs2) as i64)
                    >> 32) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Mulhsu { rd, rs1, rs2 } => {
                let val = ((ctx.read_register(*rs1) as i64
                    * (ctx.read_register(*rs2) as u32 as i64))
                    >> 32) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Mulhu { rd, rs1, rs2 } => {
                let val = (((ctx.read_register(*rs1) as u32 as u64)
                    * (ctx.read_register(*rs2) as u32 as u64))
                    >> 32) as i32;
                ctx.write_register(*rd, val);
            }
            Op::Div { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2);
                let val = if rs2_val == 0 {
                    -1
                } else {
                    ctx.read_register(*rs1).wrapping_div(rs2_val)
                };
                ctx.write_register(*rd, val);
            }
            Op::Divu { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2) as u32;
                let val = if rs2_val == 0 {
                    -1
                } else {
                    ((ctx.read_register(*rs1) as u32).wrapping_div(rs2_val))
                        as i32
                };
                ctx.write_register(*rd, val);
            }
            Op::Rem { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2);
                let val = if rs2_val == 0 {
                    ctx.read_register(*rs1)
                } else {
                    ctx.read_register(*rs1).wrapping_rem(rs2_val)
                };
                ctx.write_register(*rd, val);
            }
            Op::Remu { rd, rs1, rs2 } => {
                let rs2_val = ctx.read_register(*rs2) as u32;
                let val = if rs2_val == 0 {
                    ctx.read_register(*rs1)
                } else {
                    ((ctx.read_register(*rs1) as u32).wrapping_rem(rs2_val))
                        as i32
                };
                ctx.write_register(*rd, val);
            }

            // a extension - atomic operations not implemented for ExecutionContext
            Op::LrW { .. }
            | Op::ScW { .. }
            | Op::AmoswapW { .. }
            | Op::AmoaddW { .. }
            | Op::AmoxorW { .. }
            | Op::AmoandW { .. }
            | Op::AmoorW { .. }
            | Op::AmominW { .. }
            | Op::AmomaxW { .. }
            | Op::AmominuW { .. }
            | Op::AmomaxuW { .. } => {
                return Err(
                    "atomic operations not implemented for ExecutionContext"
                        .to_string(),
                );
            }

            Op::Unimplemented { inst, note } => {
                return Err(format!("inst: 0x{:x} note: {}", inst, note));
            }
        }
        Ok(())
    }

    pub fn to_fields(&self) -> Vec<Field> {
        match *self {
            // r-type
            Op::Add { rd, rs1, rs2 } => vec![
                Field::Opcode("add"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Sub { rd, rs1, rs2 } => vec![
                Field::Opcode("sub"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Sll { rd, rs1, rs2 } => vec![
                Field::Opcode("sll"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Slt { rd, rs1, rs2 } => vec![
                Field::Opcode("slt"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Sltu { rd, rs1, rs2 } => vec![
                Field::Opcode("sltu"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Xor { rd, rs1, rs2 } => vec![
                Field::Opcode("xor"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Srl { rd, rs1, rs2 } => vec![
                Field::Opcode("srl"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Sra { rd, rs1, rs2 } => vec![
                Field::Opcode("sra"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Or { rd, rs1, rs2 } => vec![
                Field::Opcode("or"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::And { rd, rs1, rs2 } => vec![
                Field::Opcode("and"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],

            // i-type instructions
            Op::Addi { rd, rs1, imm } => vec![
                Field::Opcode("addi"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Imm(imm),
            ],
            Op::Slti { rd, rs1, imm } => vec![
                Field::Opcode("slti"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Imm(imm),
            ],
            Op::Sltiu { rd, rs1, imm } => {
                vec![
                    Field::Opcode("sltiu"),
                    Field::Reg(rd),
                    Field::Reg(rs1),
                    Field::Imm(imm),
                ]
            }
            Op::Xori { rd, rs1, imm } => vec![
                Field::Opcode("xori"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Imm(imm),
            ],
            Op::Ori { rd, rs1, imm } => vec![
                Field::Opcode("ori"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Imm(imm),
            ],
            Op::Andi { rd, rs1, imm } => vec![
                Field::Opcode("andi"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Imm(imm),
            ],
            Op::Slli { rd, rs1, shamt } => {
                vec![
                    Field::Opcode("slli"),
                    Field::Reg(rd),
                    Field::Reg(rs1),
                    Field::Imm(shamt),
                ]
            }
            Op::Srli { rd, rs1, shamt } => {
                vec![
                    Field::Opcode("srli"),
                    Field::Reg(rd),
                    Field::Reg(rs1),
                    Field::Imm(shamt),
                ]
            }
            Op::Srai { rd, rs1, shamt } => {
                vec![
                    Field::Opcode("srai"),
                    Field::Reg(rd),
                    Field::Reg(rs1),
                    Field::Imm(shamt),
                ]
            }

            // branch
            Op::Beq { rs1, rs2, offset } => {
                vec![
                    Field::Opcode("beq"),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bne { rs1, rs2, offset } => {
                vec![
                    Field::Opcode("bne"),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Blt { rs1, rs2, offset } => {
                vec![
                    Field::Opcode("blt"),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bge { rs1, rs2, offset } => {
                vec![
                    Field::Opcode("bge"),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bltu { rs1, rs2, offset } => {
                vec![
                    Field::Opcode("bltu"),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bgeu { rs1, rs2, offset } => {
                vec![
                    Field::Opcode("bgeu"),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }

            // jump
            Op::Jal { rd, offset } => vec![
                Field::Opcode("jal"),
                Field::Reg(rd),
                Field::PCRelAddr(offset),
            ],
            Op::Jalr { rd, rs1, offset } => vec![
                Field::Opcode("jalr"),
                Field::Reg(rd),
                Field::Indirect(offset, rs1),
            ],

            // load
            Op::Lb { rd, rs1, offset } => vec![
                Field::Opcode("lb"),
                Field::Reg(rd),
                Field::Indirect(offset, rs1),
            ],
            Op::Lh { rd, rs1, offset } => vec![
                Field::Opcode("lh"),
                Field::Reg(rd),
                Field::Indirect(offset, rs1),
            ],
            Op::Lw { rd, rs1, offset } => vec![
                Field::Opcode("lw"),
                Field::Reg(rd),
                Field::Indirect(offset, rs1),
            ],
            Op::Lbu { rd, rs1, offset } => vec![
                Field::Opcode("lbu"),
                Field::Reg(rd),
                Field::Indirect(offset, rs1),
            ],
            Op::Lhu { rd, rs1, offset } => vec![
                Field::Opcode("lhu"),
                Field::Reg(rd),
                Field::Indirect(offset, rs1),
            ],

            // store
            Op::Sb { rs1, rs2, offset } => vec![
                Field::Opcode("sb"),
                Field::Reg(rs2),
                Field::Indirect(offset, rs1),
            ],
            Op::Sh { rs1, rs2, offset } => vec![
                Field::Opcode("sh"),
                Field::Reg(rs2),
                Field::Indirect(offset, rs1),
            ],
            Op::Sw { rs1, rs2, offset } => vec![
                Field::Opcode("sw"),
                Field::Reg(rs2),
                Field::Indirect(offset, rs1),
            ],

            // u-type
            Op::Lui { rd, imm } => {
                vec![Field::Opcode("lui"), Field::Reg(rd), Field::Imm(imm)]
            }
            Op::Auipc { rd, imm } => {
                vec![Field::Opcode("auipc"), Field::Reg(rd), Field::Imm(imm)]
            }

            // misc
            Op::Fence => vec![Field::Opcode("fence")],
            Op::Ecall => vec![Field::Opcode("ecall")],
            Op::Ebreak => vec![Field::Opcode("ebreak")],

            // m extension
            Op::Mul { rd, rs1, rs2 } => vec![
                Field::Opcode("mul"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Mulh { rd, rs1, rs2 } => vec![
                Field::Opcode("mulh"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Mulhsu { rd, rs1, rs2 } => {
                vec![
                    Field::Opcode("mulhsu"),
                    Field::Reg(rd),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                ]
            }
            Op::Mulhu { rd, rs1, rs2 } => {
                vec![
                    Field::Opcode("mulhu"),
                    Field::Reg(rd),
                    Field::Reg(rs1),
                    Field::Reg(rs2),
                ]
            }
            Op::Div { rd, rs1, rs2 } => vec![
                Field::Opcode("div"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Divu { rd, rs1, rs2 } => vec![
                Field::Opcode("divu"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Rem { rd, rs1, rs2 } => vec![
                Field::Opcode("rem"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],
            Op::Remu { rd, rs1, rs2 } => vec![
                Field::Opcode("remu"),
                Field::Reg(rd),
                Field::Reg(rs1),
                Field::Reg(rs2),
            ],

            // a extension - load reserved / store conditional
            Op::LrW { rd, rs1, .. } => vec![
                Field::Opcode("lr.w"),
                Field::Reg(rd),
                Field::Indirect(0, rs1),
            ],
            Op::ScW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("sc.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }

            // a extension - atomic memory operations
            Op::AmoswapW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amoswap.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmoaddW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amoadd.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmoxorW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amoxor.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmoandW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amoand.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmoorW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amoor.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmominW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amomin.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmomaxW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amomax.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmominuW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amominu.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }
            Op::AmomaxuW { rd, rs1, rs2, .. } => {
                vec![
                    Field::Opcode("amomaxu.w"),
                    Field::Reg(rd),
                    Field::Reg(rs2),
                    Field::Indirect(0, rs1),
                ]
            }

            // unknown instructions
            Op::Unimplemented { .. } => vec![Field::Opcode("???")],
        }
    }

    pub fn to_pseudo_fields(&self) -> Vec<Field> {
        match *self {
            Op::Addi { rd: ZERO, rs1: ZERO, imm: 0 } => {
                vec![Field::Opcode("nop")]
            }
            Op::Addi { rd, rs1: ZERO, imm } => {
                vec![Field::Opcode("li"), Field::Reg(rd), Field::Imm(imm)]
            }
            Op::Addi { rd, rs1, imm: 0 } => {
                vec![Field::Opcode("mv"), Field::Reg(rd), Field::Reg(rs1)]
            }
            Op::Jalr { rd: ZERO, rs1: RA, offset: 0 } => {
                vec![Field::Opcode("ret")]
            }
            Op::Jalr { rd: ZERO, rs1, offset: 0 } => {
                vec![Field::Opcode("jr"), Field::Reg(rs1)]
            }
            Op::Jalr { rd: RA, rs1, offset: 0 } => {
                vec![Field::Opcode("jalr"), Field::Reg(rs1)]
            }
            Op::Jal { rd: ZERO, offset } => {
                vec![Field::Opcode("j"), Field::PCRelAddr(offset)]
            }
            Op::Jal { rd: RA, offset } => {
                vec![Field::Opcode("jal"), Field::PCRelAddr(offset)]
            }
            Op::Addi { rd, rs1: GP, imm } => {
                vec![Field::Opcode("la"), Field::Reg(rd), Field::GPRelAddr(imm)]
            }
            Op::Xori { rd, rs1, imm: -1 } => {
                vec![Field::Opcode("not"), Field::Reg(rd), Field::Reg(rs1)]
            }
            Op::Sltiu { rd, rs1, imm: 1 } => {
                vec![Field::Opcode("seqz"), Field::Reg(rd), Field::Reg(rs1)]
            }
            Op::Sltu { rd, rs1: ZERO, rs2 } => {
                vec![Field::Opcode("snez"), Field::Reg(rd), Field::Reg(rs2)]
            }
            Op::Beq { rs1: ZERO, rs2, offset } => {
                vec![
                    Field::Opcode("beqz"),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Beq { rs1, rs2: ZERO, offset } => {
                vec![
                    Field::Opcode("beqz"),
                    Field::Reg(rs1),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bne { rs1: ZERO, rs2, offset } => {
                vec![
                    Field::Opcode("bnez"),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bne { rs1, rs2: ZERO, offset } => {
                vec![
                    Field::Opcode("bnez"),
                    Field::Reg(rs1),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Blt { rs1, rs2: ZERO, offset } => {
                vec![
                    Field::Opcode("bltz"),
                    Field::Reg(rs1),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bge { rs1, rs2: ZERO, offset } => {
                vec![
                    Field::Opcode("bgez"),
                    Field::Reg(rs1),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Bge { rs1: ZERO, rs2, offset } => {
                vec![
                    Field::Opcode("blez"),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Blt { rs1: ZERO, rs2, offset } => {
                vec![
                    Field::Opcode("bgtz"),
                    Field::Reg(rs2),
                    Field::PCRelAddr(offset),
                ]
            }
            Op::Sub { rd, rs1: ZERO, rs2 } => {
                vec![Field::Opcode("neg"), Field::Reg(rd), Field::Reg(rs2)]
            }
            Op::Slt { rd, rs1: ZERO, rs2 } => {
                vec![Field::Opcode("sgtz"), Field::Reg(rd), Field::Reg(rs2)]
            }
            Op::Slt { rd, rs1, rs2: ZERO } => {
                vec![Field::Opcode("sltz"), Field::Reg(rd), Field::Reg(rs1)]
            }

            // no matching pseudo-instruction
            _ => self.to_fields(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn to_string(
        &self,
        pc: u32,
        gp: u32,
        is_compressed: bool,
        hex: bool,
        verbose: bool,
        show_addresses: bool,
        arrow: Option<&str>,
        symbols: &HashMap<u32, String>,
    ) -> String {
        let fields =
            if verbose { self.to_fields() } else { self.to_pseudo_fields() };
        fields_to_string(
            &fields,
            pc,
            gp,
            is_compressed,
            hex,
            verbose,
            show_addresses,
            arrow,
            symbols,
        )
    }

    pub fn branch_target(&self, pc: u32) -> Option<u32> {
        match self {
            Self::Beq { offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            Self::Bne { offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            Self::Blt { offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            Self::Bge { offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            Self::Bltu { offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            Self::Bgeu { offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            Self::Jal { rd: ZERO, offset, .. } => {
                Some((pc as i32).wrapping_add(*offset) as u32)
            }
            _ => None,
        }
    }
}

pub fn get_pseudo_sequence(
    instructions: &[Instruction],
    symbols: &HashMap<u32, String>,
) -> Option<(usize, Vec<Field>)> {
    if instructions.len() < 2 {
        return None;
    }
    let (inst1, inst2) = (&instructions[0], &instructions[1]);

    // do not merge instructions if the second one is labeled
    if symbols.contains_key(&inst2.address) {
        return None;
    }

    match (&inst1.op, &inst2.op) {
        (
            Op::Auipc { rd: rd1, imm: imm1 },
            Op::Addi { rd: rd2, rs1: rs2, imm: imm2 },
        ) if rd1 == rd2 && rd2 == rs2 => Some((
            2,
            vec![
                Field::Opcode("la"),
                Field::Reg(*rd1),
                Field::PCRelAddr(imm1 + imm2),
            ],
        )),

        (Op::Auipc { rd: RA, imm }, Op::Jalr { rd: RA, rs1: RA, offset }) => {
            Some((
                2,
                vec![Field::Opcode("call"), Field::PCRelAddr(imm + offset)],
            ))
        }

        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fields_to_string(
    fields: &[Field],
    pc: u32,
    gp: u32,
    is_compressed: bool,
    hex: bool,
    verbose: bool,
    show_addresses: bool,
    arrow: Option<&str>,
    symbols: &HashMap<u32, String>,
) -> String {
    let addr_part = if !show_addresses {
        String::new()
    } else if hex {
        format!("0x{:5x} ", pc)
    } else {
        format!("{:>7} ", pc)
    };

    let mut label = if let Some(label) = symbols.get(&pc) {
        label.chars().collect()
    } else {
        Vec::new()
    };
    if label.len() > 14 {
        label.truncate(14);
        label.push('…');
    }
    if !label.is_empty() {
        label.push(':');
    }
    while label.len() < 16 {
        label.push(' ');
    }
    if let Some(overlay) = arrow {
        let mut chars: Vec<_> = overlay.chars().collect();
        label.truncate(label.len() - chars.len() - 1);
        label.append(&mut chars);
        label.push(' ');
    }
    let label: String = label.into_iter().collect();

    let mut inst = fields[0].to_string(pc, gp, hex, verbose, symbols);
    if verbose && is_compressed {
        inst.insert_str(0, "c.");
    }
    let operands = fields[1..]
        .iter()
        .map(|elt| elt.to_string(pc, gp, hex, verbose, symbols))
        .collect::<Vec<_>>()
        .join(", ");
    let disasm = format!("{:<8}{}", inst, operands);

    format!("{addr_part}{label:<16}{disasm:<48}")
}

pub enum Field {
    Opcode(&'static str),
    Reg(usize),
    Imm(i32),
    Indirect(i32, usize),
    PCRelAddr(i32),
    GPRelAddr(i32),
}

impl Field {
    pub fn to_string(
        &self,
        pc: u32,
        gp: u32,
        hex: bool,
        verbose: bool,
        symbols: &HashMap<u32, String>,
    ) -> String {
        match self {
            Field::Opcode(inst) => String::from(*inst),
            Field::Reg(reg) => String::from(R[*reg]),
            Field::Imm(i) if !hex || (0..=9).contains(i) => format!("{}", i),
            Field::Imm(i) => format!("0x{:x}", i),
            Field::Indirect(0, reg) if !verbose => format!("({})", R[*reg]),
            Field::Indirect(imm, reg) if hex => {
                format!("0x{:x}({})", imm, R[*reg])
            }
            Field::Indirect(imm, reg) => format!("{}({})", imm, R[*reg]),
            Field::PCRelAddr(offset) => {
                let addr = (pc as i32).wrapping_add(*offset) as u32;
                match symbols.get(&addr) {
                    Some(symbol) if !verbose => match symbol.parse::<u32>() {
                        Ok(num) if num > 0 => {
                            let suffix = if addr <= pc { "b" } else { "f" };
                            format!("{}{}", symbol, suffix)
                        }
                        _ => symbol.clone(),
                    },
                    _ => {
                        if !hex || (0..=9).contains(offset) {
                            format!("{}", offset)
                        } else {
                            format!("0x{:x}", offset)
                        }
                    }
                }
            }
            Field::GPRelAddr(offset) => {
                // gp-relative only applies to pseudo-instructions in !verbose mode
                // i.e., "la"
                let addr = (gp as i32).wrapping_add(*offset) as u32;
                match symbols.get(&addr) {
                    Some(symbol) => symbol.clone(),
                    _ => {
                        if !hex || (0..=9).contains(&{ *offset }) {
                            format!("{}", offset)
                        } else {
                            format!("0x{:x}", offset)
                        }
                    }
                }
            }
        }
    }
}
