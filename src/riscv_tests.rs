#[cfg(test)]
mod pseudo_instruction_tests {
    use crate::execution::Instruction;
    use crate::riscv::{get_pseudo_sequence, Field, Op, ZERO, RA, GP};
    use std::collections::HashMap;

    fn make_instruction(op: Op, address: u32) -> Instruction {
        Instruction {
            address,
            op,
            length: 4,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        }
    }

    fn check_pseudo_opcode(op: &Op, expected: &str) {
        let fields = op.to_pseudo_fields();
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, expected),
            _ => panic!("Expected opcode field for {}, got non-opcode field", expected),
        }
    }

    // Single-instruction pseudo-instruction tests (to_pseudo_fields)

    #[test]
    fn test_pseudo_nop() {
        let op = Op::Addi {
            rd: ZERO,
            rs1: ZERO,
            imm: 0,
        };
        check_pseudo_opcode(&op, "nop");
    }

    #[test]
    fn test_pseudo_li() {
        let op = Op::Addi {
            rd: 10,
            rs1: ZERO,
            imm: 42,
        };
        check_pseudo_opcode(&op, "li");
    }

    #[test]
    fn test_pseudo_mv() {
        let op = Op::Addi {
            rd: 11,
            rs1: 10,
            imm: 0,
        };
        check_pseudo_opcode(&op, "mv");
    }

    #[test]
    fn test_pseudo_ret() {
        let op = Op::Jalr {
            rd: ZERO,
            rs1: RA,
            offset: 0,
        };
        check_pseudo_opcode(&op, "ret");
    }

    #[test]
    fn test_pseudo_jr() {
        let op = Op::Jalr {
            rd: ZERO,
            rs1: 10,
            offset: 0,
        };
        check_pseudo_opcode(&op, "jr");
    }

    #[test]
    fn test_pseudo_jalr() {
        let op = Op::Jalr {
            rd: RA,
            rs1: 10,
            offset: 0,
        };
        check_pseudo_opcode(&op, "jalr");
    }

    #[test]
    fn test_pseudo_j() {
        let op = Op::Jal {
            rd: ZERO,
            offset: 100,
        };
        check_pseudo_opcode(&op, "j");
    }

    #[test]
    fn test_pseudo_jal() {
        let op = Op::Jal {
            rd: RA,
            offset: 100,
        };
        check_pseudo_opcode(&op, "jal");
    }

    #[test]
    fn test_pseudo_la_gp_relative() {
        let op = Op::Addi {
            rd: 10,
            rs1: GP,
            imm: 256,
        };
        check_pseudo_opcode(&op, "la");
    }

    #[test]
    fn test_pseudo_not() {
        let op = Op::Xori {
            rd: 10,
            rs1: 11,
            imm: -1,
        };
        check_pseudo_opcode(&op, "not");
    }

    #[test]
    fn test_pseudo_seqz() {
        let op = Op::Sltiu {
            rd: 10,
            rs1: 11,
            imm: 1,
        };
        check_pseudo_opcode(&op, "seqz");
    }

    #[test]
    fn test_pseudo_snez() {
        let op = Op::Sltu {
            rd: 10,
            rs1: ZERO,
            rs2: 11,
        };
        check_pseudo_opcode(&op, "snez");
    }

    #[test]
    fn test_pseudo_beqz_rs1_zero() {
        let op = Op::Beq {
            rs1: ZERO,
            rs2: 10,
            offset: 50,
        };
        check_pseudo_opcode(&op, "beqz");
    }

    #[test]
    fn test_pseudo_beqz_rs2_zero() {
        let op = Op::Beq {
            rs1: 10,
            rs2: ZERO,
            offset: 50,
        };
        check_pseudo_opcode(&op, "beqz");
    }

    #[test]
    fn test_pseudo_bnez_rs1_zero() {
        let op = Op::Bne {
            rs1: ZERO,
            rs2: 10,
            offset: 50,
        };
        check_pseudo_opcode(&op, "bnez");
    }

    #[test]
    fn test_pseudo_bnez_rs2_zero() {
        let op = Op::Bne {
            rs1: 10,
            rs2: ZERO,
            offset: 50,
        };
        check_pseudo_opcode(&op, "bnez");
    }

    #[test]
    fn test_pseudo_bltz() {
        let op = Op::Blt {
            rs1: 10,
            rs2: ZERO,
            offset: 50,
        };
        check_pseudo_opcode(&op, "bltz");
    }

    #[test]
    fn test_pseudo_bgez() {
        let op = Op::Bge {
            rs1: 10,
            rs2: ZERO,
            offset: 50,
        };
        check_pseudo_opcode(&op, "bgez");
    }

    #[test]
    fn test_pseudo_blez() {
        let op = Op::Bge {
            rs1: ZERO,
            rs2: 10,
            offset: 50,
        };
        check_pseudo_opcode(&op, "blez");
    }

    #[test]
    fn test_pseudo_bgtz() {
        let op = Op::Blt {
            rs1: ZERO,
            rs2: 10,
            offset: 50,
        };
        check_pseudo_opcode(&op, "bgtz");
    }

    #[test]
    fn test_pseudo_neg() {
        let op = Op::Sub {
            rd: 10,
            rs1: ZERO,
            rs2: 11,
        };
        check_pseudo_opcode(&op, "neg");
    }

    #[test]
    fn test_pseudo_sgtz() {
        let op = Op::Slt {
            rd: 10,
            rs1: ZERO,
            rs2: 11,
        };
        check_pseudo_opcode(&op, "sgtz");
    }

    #[test]
    fn test_pseudo_sltz() {
        let op = Op::Slt {
            rd: 10,
            rs1: 11,
            rs2: ZERO,
        };
        check_pseudo_opcode(&op, "sltz");
    }

    // Multi-instruction pseudo-instruction tests (get_pseudo_sequence)

    #[test]
    fn test_pseudo_sequence_la_pc_relative() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x1000 }, 0x1000);
        let inst2 = make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 0x234 }, 0x1004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "la"),
            _ => panic!("Expected 'la' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_call() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: RA, imm: 0x2000 }, 0x2000);
        let inst2 = make_instruction(Op::Jalr { rd: RA, rs1: RA, offset: 0x100 }, 0x2004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "call"),
            _ => panic!("Expected 'call' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_tail() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 6, imm: 0x3000 }, 0x3000);
        let inst2 = make_instruction(Op::Jalr { rd: ZERO, rs1: 6, offset: 0x50 }, 0x3004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "tail"),
            _ => panic!("Expected 'tail' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_lb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x4000 }, 0x4000);
        let inst2 = make_instruction(Op::Lb { rd: 10, rs1: 10, offset: 0x100 }, 0x4004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "lb"),
            _ => panic!("Expected 'lb' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_lh() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 11, imm: 0x5000 }, 0x5000);
        let inst2 = make_instruction(Op::Lh { rd: 11, rs1: 11, offset: 0x200 }, 0x5004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "lh"),
            _ => panic!("Expected 'lh' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_lw() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 12, imm: 0x6000 }, 0x6000);
        let inst2 = make_instruction(Op::Lw { rd: 12, rs1: 12, offset: 0x400 }, 0x6004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "lw"),
            _ => panic!("Expected 'lw' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_lbu() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 13, imm: 0x7000 }, 0x7000);
        let inst2 = make_instruction(Op::Lbu { rd: 13, rs1: 13, offset: 0x80 }, 0x7004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "lbu"),
            _ => panic!("Expected 'lbu' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_lhu() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 14, imm: 0x8000 }, 0x8000);
        let inst2 = make_instruction(Op::Lhu { rd: 14, rs1: 14, offset: 0x150 }, 0x8004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "lhu"),
            _ => panic!("Expected 'lhu' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_sb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 15, imm: 0x9000 }, 0x9000);
        let inst2 = make_instruction(Op::Sb { rs1: 15, rs2: 10, offset: 0x50 }, 0x9004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "sb"),
            _ => panic!("Expected 'sb' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_sh() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 16, imm: 0xa000 }, 0xa000);
        let inst2 = make_instruction(Op::Sh { rs1: 16, rs2: 11, offset: 0x100 }, 0xa004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "sh"),
            _ => panic!("Expected 'sh' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_sw() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 17, imm: 0xb000 }, 0xb000);
        let inst2 = make_instruction(Op::Sw { rs1: 17, rs2: 12, offset: 0x200 }, 0xb004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        match &fields[0] {
            Field::Opcode(name) => assert_eq!(*name, "sw"),
            _ => panic!("Expected 'sw' opcode"),
        }
    }

    #[test]
    fn test_pseudo_sequence_not_detected_with_label() {
        let mut symbols = HashMap::new();
        symbols.insert(0x5004, "label".to_string());

        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x5000 }, 0x5000);
        let inst2 = make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 0x200 }, 0x5004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_none(), "Should not merge when second instruction is labeled");
    }

    #[test]
    fn test_pseudo_sequence_not_detected_mismatched_registers_la() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x6000 }, 0x6000);
        // rd and rs1 don't match
        let inst2 = make_instruction(Op::Addi { rd: 11, rs1: 10, imm: 0x100 }, 0x6004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_none());
    }

    #[test]
    fn test_pseudo_sequence_not_detected_mismatched_registers_lb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x7000 }, 0x7000);
        // rd and rs1 don't match
        let inst2 = make_instruction(Op::Lb { rd: 11, rs1: 10, offset: 0x80 }, 0x7004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_none());
    }

    #[test]
    fn test_pseudo_sequence_not_detected_mismatched_registers_sb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x8000 }, 0x8000);
        // rd and rs1 don't match
        let inst2 = make_instruction(Op::Sb { rs1: 11, rs2: 5, offset: 0x50 }, 0x8004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_none());
    }

    #[test]
    fn test_pseudo_sequence_only_one_instruction() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x9000 }, 0x9000);

        let result = get_pseudo_sequence(&[inst1], &symbols);
        assert!(result.is_none());
    }

    #[test]
    fn test_pseudo_sequence_empty_list() {
        let symbols = HashMap::new();
        let result = get_pseudo_sequence(&[], &symbols);
        assert!(result.is_none());
    }
}
