use crate::riscv::{
    Op, RA, SP, ZERO, get_funct3, get_funct7, get_imm_b, get_imm_i, get_imm_j, get_imm_s, get_imm_u, get_rd, get_rs1,
    get_rs2,
};

pub struct InstructionDecoder;

impl InstructionDecoder {
    pub fn decode(inst: i32) -> Op {
        if (inst & 0x3) != 0x3 {
            return Self::decode_compressed(inst);
        }

        let opcode = inst & 0x7f;

        match opcode {
            0x33 => Self::decode_r_type(inst),
            0x13 => Self::decode_i_type(inst),
            0x63 => Self::decode_branches(inst),
            0x6f => Op::Jal { rd: get_rd(inst), offset: get_imm_j(inst) },
            0x67 => {
                let funct3 = get_funct3(inst);
                if funct3 == 0 {
                    Op::Jalr { rd: get_rd(inst), rs1: get_rs1(inst), offset: get_imm_i(inst) }
                } else {
                    Op::Unimplemented { inst, note: format!("jalr with unknown funct3 value of {}", funct3) }
                }
            }
            0x03 => Self::decode_load(inst),
            0x23 => Self::decode_store(inst),
            0x2f => Self::decode_atomic(inst),
            0x37 => Op::Lui { rd: get_rd(inst), imm: get_imm_u(inst) },
            0x17 => Op::Auipc { rd: get_rd(inst), imm: get_imm_u(inst) },
            0x0f => Op::Fence,
            0x73 if inst == 0x00000073 => Op::Ecall,
            0x73 if inst == 0x00100073 => Op::Ebreak,
            _ => Op::Unimplemented {
                inst,
                note: format!("disassembler found unknown instruction opcode 0x{:x}", opcode),
            },
        }
    }

    fn decode_branches(inst: i32) -> Op {
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
            _ => Op::Unimplemented { inst, note: format!("branch instruction of unknown type {}", funct3) },
        }
    }

    fn decode_load(inst: i32) -> Op {
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
            _ => Op::Unimplemented { inst, note: format!("load instruction of unknown type {}", funct3) },
        }
    }

    fn decode_store(inst: i32) -> Op {
        let funct3 = get_funct3(inst);
        let rs1 = get_rs1(inst);
        let rs2 = get_rs2(inst);
        let offset = get_imm_s(inst);

        match funct3 {
            0 => Op::Sb { rs1, rs2, offset },
            1 => Op::Sh { rs1, rs2, offset },
            2 => Op::Sw { rs1, rs2, offset },
            _ => Op::Unimplemented { inst, note: format!("store instruction of unknown type {}", funct3) },
        }
    }

    fn decode_i_type(inst: i32) -> Op {
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
            _ => Op::Unimplemented { inst, note: format!("alu immediate of unknown type {}", funct3) },
        }
    }

    fn decode_r_type(inst: i32) -> Op {
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
                note: format!("alu instruction of unknown type {} subtype {}", funct3, funct7),
            },
        }
    }

    fn decode_atomic(inst: i32) -> Op {
        let funct5 = (inst >> 27) & 0x1f;
        let aq = ((inst >> 26) & 1) != 0;
        let rl = ((inst >> 25) & 1) != 0;
        let rs2 = get_rs2(inst);
        let rs1 = get_rs1(inst);
        let funct3 = get_funct3(inst);
        let rd = get_rd(inst);

        // Only support .W variants for RV32
        if funct3 != 0x2 {
            return Op::Unimplemented {
                inst,
                note: format!("atomic instruction with unsupported width funct3={}", funct3),
            };
        }

        match funct5 {
            0x02 => Op::LrW { rd, rs1, aq, rl },
            0x03 => Op::ScW { rd, rs1, rs2, aq, rl },
            0x01 => Op::AmoswapW { rd, rs1, rs2, aq, rl },
            0x00 => Op::AmoaddW { rd, rs1, rs2, aq, rl },
            0x04 => Op::AmoxorW { rd, rs1, rs2, aq, rl },
            0x0c => Op::AmoandW { rd, rs1, rs2, aq, rl },
            0x08 => Op::AmoorW { rd, rs1, rs2, aq, rl },
            0x10 => Op::AmominW { rd, rs1, rs2, aq, rl },
            0x14 => Op::AmomaxW { rd, rs1, rs2, aq, rl },
            0x18 => Op::AmominuW { rd, rs1, rs2, aq, rl },
            0x1c => Op::AmomaxuW { rd, rs1, rs2, aq, rl },
            _ => Op::Unimplemented { inst, note: format!("unknown atomic operation funct5={}", funct5) },
        }
    }

    fn decode_compressed(inst: i32) -> Op {
        use crate::riscv::{
            get_c_addi4spn_imm, get_c_addi16sp_imm, get_c_beqz_bnez_imm, get_c_funct3, get_c_j_jal_imm,
            get_c_li_addi_addiw_andi_imm, get_c_lui_imm, get_c_lw_sw_imm, get_c_lwsp_imm, get_c_op, get_c_rd_rs1,
            get_c_rs1_prime, get_c_rs2, get_c_rs2_prime, get_c_slli_srli_srai_imm, get_c_swsp_imm,
        };

        let op = get_c_op(inst);
        let funct3 = get_c_funct3(inst);

        match (op, funct3) {
            (0, 0) => {
                let rd = get_c_rs2_prime(inst);
                let imm = get_c_addi4spn_imm(inst);
                if rd == 0 && imm == 0 {
                    Op::Unimplemented { inst, note: String::from("Illegal compressed instruction at (0, 0)") }
                } else if imm == 0 {
                    Op::Unimplemented { inst, note: String::from("C.ADDI4SPN with imm=0 is reserved") }
                } else {
                    Op::Addi { rd, rs1: SP, imm }
                }
            }
            (0, 1) => Op::Unimplemented { inst, note: String::from("C.FLD is not supported") },
            (0, 2) => {
                let rd = get_c_rs2_prime(inst);
                let rs1 = get_c_rs1_prime(inst);
                let imm = get_c_lw_sw_imm(inst);
                Op::Lw { rd, rs1, offset: imm }
            }
            (0, 3) => Op::Unimplemented { inst, note: String::from("C.LD is not supported in RV32") },
            (0, 4) => Op::Unimplemented { inst, note: String::from("Reserved compressed instruction at (0, 4)") },
            (0, 5) => Op::Unimplemented { inst, note: String::from("C.FSD is not supported") },
            (0, 6) => {
                let rs2 = get_c_rs2_prime(inst);
                let rs1 = get_c_rs1_prime(inst);
                let imm = get_c_lw_sw_imm(inst);
                Op::Sw { rs1, rs2, offset: imm }
            }
            (0, 7) => Op::Unimplemented { inst, note: String::from("C.SD is not supported in RV32") },

            (1, 0) => {
                let rd = get_c_rd_rs1(inst);
                let imm = get_c_li_addi_addiw_andi_imm(inst);
                Op::Addi { rd, rs1: rd, imm }
            }
            (1, 1) => {
                let offset = get_c_j_jal_imm(inst);
                Op::Jal { rd: RA, offset }
            }
            (1, 2) => {
                let rd = get_c_rd_rs1(inst);
                let imm = get_c_li_addi_addiw_andi_imm(inst);
                Op::Addi { rd, rs1: ZERO, imm }
            }
            (1, 3) => {
                let rd = get_c_rd_rs1(inst);
                if rd == 2 {
                    let imm = get_c_addi16sp_imm(inst);
                    if imm == 0 {
                        Op::Unimplemented { inst, note: String::from("C.ADDI16SP with imm=0 is reserved") }
                    } else {
                        Op::Addi { rd: SP, rs1: SP, imm }
                    }
                } else {
                    let imm = get_c_lui_imm(inst);
                    if imm == 0 {
                        Op::Unimplemented { inst, note: String::from("C.LUI with imm=0 is reserved") }
                    } else {
                        Op::Lui { rd, imm }
                    }
                }
            }
            (1, 4) => {
                let funct2 = (inst >> 10) & 0x3;
                let rd = get_c_rs1_prime(inst);
                match funct2 {
                    0 => {
                        let shamt = get_c_slli_srli_srai_imm(inst);
                        Op::Srli { rd, rs1: rd, shamt }
                    }
                    1 => {
                        let shamt = get_c_slli_srli_srai_imm(inst);
                        Op::Srai { rd, rs1: rd, shamt }
                    }
                    2 => {
                        let imm = get_c_li_addi_addiw_andi_imm(inst);
                        Op::Andi { rd, rs1: rd, imm }
                    }
                    3 => {
                        let rs2 = get_c_rs2_prime(inst);
                        let bit12 = (inst >> 12) & 0x1;
                        let funct = (inst >> 5) & 0x3;
                        match (bit12, funct) {
                            (0, 0) => Op::Sub { rd, rs1: rd, rs2 },
                            (0, 1) => Op::Xor { rd, rs1: rd, rs2 },
                            (0, 2) => Op::Or { rd, rs1: rd, rs2 },
                            (0, 3) => Op::And { rd, rs1: rd, rs2 },
                            (1, 0) | (1, 1) => {
                                Op::Unimplemented { inst, note: "C.SUBW/C.ADDW are not supported in RV32".to_string() }
                            }
                            _ => Op::Unimplemented {
                                inst,
                                note: "Reserved compressed instruction at (1, 4)".to_string(),
                            },
                        }
                    }
                    _ => unreachable!(),
                }
            }
            (1, 5) => {
                let offset = get_c_j_jal_imm(inst);
                Op::Jal { rd: ZERO, offset }
            }
            (1, 6) => {
                let rs1 = get_c_rs1_prime(inst);
                let offset = get_c_beqz_bnez_imm(inst);
                Op::Beq { rs1, rs2: ZERO, offset }
            }
            (1, 7) => {
                let rs1 = get_c_rs1_prime(inst);
                let offset = get_c_beqz_bnez_imm(inst);
                Op::Bne { rs1, rs2: ZERO, offset }
            }

            (2, 0) => {
                let rd = get_c_rd_rs1(inst);
                let shamt = get_c_slli_srli_srai_imm(inst);
                Op::Slli { rd, rs1: rd, shamt }
            }
            (2, 1) => Op::Unimplemented { inst, note: String::from("C.FLDSP is not supported") },
            (2, 2) => {
                let rd = get_c_rd_rs1(inst);
                let imm = get_c_lwsp_imm(inst);
                if rd == 0 {
                    Op::Unimplemented { inst, note: String::from("C.LWSP with rd=0 is reserved") }
                } else {
                    Op::Lw { rd, rs1: SP, offset: imm }
                }
            }
            (2, 3) => Op::Unimplemented { inst, note: String::from("C.LDSP is not supported in RV32") },
            (2, 4) => {
                let rd = get_c_rd_rs1(inst);
                let rs2 = get_c_rs2(inst);
                let bit12 = (inst >> 12) & 0x1;

                match (bit12, rd, rs2) {
                    (0, 0, 0) => Op::Unimplemented { inst, note: String::from("C.JR with rd=0 is reserved") },
                    (0, _, 0) => Op::Jalr { rd: ZERO, rs1: rd, offset: 0 },
                    (0, _, _) => Op::Add { rd, rs1: ZERO, rs2 },
                    (1, 0, 0) => Op::Ebreak,
                    (1, _, 0) => Op::Jalr { rd: RA, rs1: rd, offset: 0 },
                    (_, _, _) => Op::Add { rd, rs1: rd, rs2 },
                }
            }
            (2, 5) => Op::Unimplemented { inst, note: String::from("C.FSDSP is not supported") },
            (2, 6) => {
                let rs2 = get_c_rs2(inst);
                let imm = get_c_swsp_imm(inst);
                Op::Sw { rs1: SP, rs2, offset: imm }
            }
            (2, 7) => Op::Unimplemented { inst, note: String::from("C.SDSP is not supported in RV32") },

            _ => unreachable!(),
        }
    }
}
