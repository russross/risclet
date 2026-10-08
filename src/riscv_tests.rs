#[cfg(test)]
mod pseudo_instruction_tests {
    use crate::execution::{Instruction, Machine, add_local_labels};
    use crate::riscv::{Field, GP, Op, RA, ZERO, get_pseudo_sequence};
    use std::collections::HashMap;

    fn make_instruction(op: Op, address: u32) -> Instruction {
        Instruction {
            address,
            op,
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        }
    }

    fn check_pseudo_fields(op: &Op, expected: &[Field]) {
        assert_eq!(op.to_pseudo_fields(), expected);
    }

    // Single-instruction pseudo-instruction tests (to_pseudo_fields)

    #[test]
    fn test_pseudo_nop() {
        let op = Op::Addi { rd: ZERO, rs1: ZERO, imm: 0 };
        check_pseudo_fields(&op, &[Field::Opcode("nop")]);
    }

    #[test]
    fn test_pseudo_li() {
        let op = Op::Addi { rd: 10, rs1: ZERO, imm: 42 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("li"), Field::Reg(10), Field::Imm(42)],
        );
    }

    #[test]
    fn test_pseudo_mv() {
        let op = Op::Addi { rd: 11, rs1: 10, imm: 0 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("mv"), Field::Reg(11), Field::Reg(10)],
        );
    }

    #[test]
    fn test_pseudo_ret() {
        let op = Op::Jalr { rd: ZERO, rs1: RA, offset: 0 };
        check_pseudo_fields(&op, &[Field::Opcode("ret")]);
    }

    #[test]
    fn test_pseudo_jr() {
        let op = Op::Jalr { rd: ZERO, rs1: 10, offset: 0 };
        check_pseudo_fields(&op, &[Field::Opcode("jr"), Field::Reg(10)]);
    }

    #[test]
    fn test_pseudo_jalr() {
        let op = Op::Jalr { rd: RA, rs1: 10, offset: 0 };
        check_pseudo_fields(&op, &[Field::Opcode("jalr"), Field::Reg(10)]);
    }

    #[test]
    fn test_pseudo_j() {
        let op = Op::Jal { rd: ZERO, offset: 100 };
        check_pseudo_fields(&op, &[Field::Opcode("j"), Field::PCRelAddr(100)]);
    }

    #[test]
    fn test_pseudo_jal() {
        let op = Op::Jal { rd: RA, offset: 100 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("jal"), Field::PCRelAddr(100)],
        );
    }

    #[test]
    fn test_pseudo_la_gp_relative() {
        let op = Op::Addi { rd: 10, rs1: GP, imm: 256 };
        check_pseudo_fields(
            &op,
            &[
                Field::Opcode("addi"),
                Field::Reg(10),
                Field::Reg(GP),
                Field::Imm(256),
            ],
        );
        let symbols = HashMap::from([(0x1100, "data".to_string())]);
        assert_eq!(
            op.to_pseudo_fields_with_symbols(0x1000, &symbols),
            vec![Field::Opcode("la"), Field::Reg(10), Field::GPRelAddr(256)]
        );
        assert_eq!(
            op.to_pseudo_fields_with_symbols(0x2000, &symbols),
            op.to_fields()
        );
        // Updating GP itself must remain arithmetic, since la initializes it PC-relatively.
        let update = Op::Addi { rd: GP, rs1: GP, imm: 256 };
        assert_eq!(
            update.to_pseudo_fields_with_symbols(0x1000, &symbols),
            update.to_fields()
        );
    }

    #[test]
    fn test_pseudo_not() {
        let op = Op::Xori { rd: 10, rs1: 11, imm: -1 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("not"), Field::Reg(10), Field::Reg(11)],
        );
    }

    #[test]
    fn test_pseudo_seqz() {
        let op = Op::Sltiu { rd: 10, rs1: 11, imm: 1 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("seqz"), Field::Reg(10), Field::Reg(11)],
        );
    }

    #[test]
    fn test_pseudo_snez() {
        let op = Op::Sltu { rd: 10, rs1: ZERO, rs2: 11 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("snez"), Field::Reg(10), Field::Reg(11)],
        );
    }

    #[test]
    fn test_pseudo_beqz_rs1_zero() {
        let op = Op::Beq { rs1: ZERO, rs2: 10, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("beqz"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_beqz_rs2_zero() {
        let op = Op::Beq { rs1: 10, rs2: ZERO, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("beqz"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_bnez_rs1_zero() {
        let op = Op::Bne { rs1: ZERO, rs2: 10, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("bnez"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_bnez_rs2_zero() {
        let op = Op::Bne { rs1: 10, rs2: ZERO, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("bnez"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_bltz() {
        let op = Op::Blt { rs1: 10, rs2: ZERO, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("bltz"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_bgez() {
        let op = Op::Bge { rs1: 10, rs2: ZERO, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("bgez"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_blez() {
        let op = Op::Bge { rs1: ZERO, rs2: 10, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("blez"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_bgtz() {
        let op = Op::Blt { rs1: ZERO, rs2: 10, offset: 50 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("bgtz"), Field::Reg(10), Field::PCRelAddr(50)],
        );
    }

    #[test]
    fn test_pseudo_neg() {
        let op = Op::Sub { rd: 10, rs1: ZERO, rs2: 11 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("neg"), Field::Reg(10), Field::Reg(11)],
        );
    }

    #[test]
    fn test_pseudo_sgtz() {
        let op = Op::Slt { rd: 10, rs1: ZERO, rs2: 11 };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("sgtz"), Field::Reg(10), Field::Reg(11)],
        );
    }

    #[test]
    fn test_pseudo_sltz() {
        let op = Op::Slt { rd: 10, rs1: 11, rs2: ZERO };
        check_pseudo_fields(
            &op,
            &[Field::Opcode("sltz"), Field::Reg(10), Field::Reg(11)],
        );
    }

    // Multi-instruction pseudo-instruction tests (get_pseudo_sequence)

    #[test]
    fn test_pseudo_sequence_la_pc_relative() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x1000 }, 0x1000);
        let inst2 =
            make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 0x234 }, 0x1004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![Field::Opcode("la"), Field::Reg(10), Field::PCRelAddr(0x1234)]
        );
    }

    #[test]
    fn test_pseudo_sequence_call() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: RA, imm: 0x2000 }, 0x2000);
        let inst2 = make_instruction(
            Op::Jalr { rd: RA, rs1: RA, offset: 0x100 },
            0x2004,
        );

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![Field::Opcode("call"), Field::PCRelAddr(0x2100)]
        );
    }

    #[test]
    fn test_pseudo_sequence_tail() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 6, imm: 0x3000 }, 0x3000);
        let inst2 = make_instruction(
            Op::Jalr { rd: ZERO, rs1: 6, offset: 0x50 },
            0x3004,
        );

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![Field::Opcode("tail"), Field::PCRelAddr(0x3050)]
        );
    }

    #[test]
    fn test_pseudo_sequence_lb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x4000 }, 0x4000);
        let inst2 =
            make_instruction(Op::Lb { rd: 10, rs1: 10, offset: 0x100 }, 0x4004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![Field::Opcode("lb"), Field::Reg(10), Field::PCRelAddr(0x4100)]
        );
    }

    #[test]
    fn test_pseudo_sequence_lh() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 11, imm: 0x5000 }, 0x5000);
        let inst2 =
            make_instruction(Op::Lh { rd: 11, rs1: 11, offset: 0x200 }, 0x5004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![Field::Opcode("lh"), Field::Reg(11), Field::PCRelAddr(0x5200)]
        );
    }

    #[test]
    fn test_pseudo_sequence_lw() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 12, imm: 0x6000 }, 0x6000);
        let inst2 =
            make_instruction(Op::Lw { rd: 12, rs1: 12, offset: 0x400 }, 0x6004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![Field::Opcode("lw"), Field::Reg(12), Field::PCRelAddr(0x6400)]
        );
    }

    #[test]
    fn test_pseudo_sequence_lbu() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 13, imm: 0x7000 }, 0x7000);
        let inst2 =
            make_instruction(Op::Lbu { rd: 13, rs1: 13, offset: 0x80 }, 0x7004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![
                Field::Opcode("lbu"),
                Field::Reg(13),
                Field::PCRelAddr(0x7080)
            ]
        );
    }

    #[test]
    fn test_pseudo_sequence_lhu() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 14, imm: 0x8000 }, 0x8000);
        let inst2 = make_instruction(
            Op::Lhu { rd: 14, rs1: 14, offset: 0x150 },
            0x8004,
        );

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![
                Field::Opcode("lhu"),
                Field::Reg(14),
                Field::PCRelAddr(0x8150)
            ]
        );
    }

    #[test]
    fn test_pseudo_sequence_sb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 15, imm: 0x9000 }, 0x9000);
        let inst2 =
            make_instruction(Op::Sb { rs1: 15, rs2: 10, offset: 0x50 }, 0x9004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![
                Field::Opcode("sb"),
                Field::Reg(10),
                Field::PCRelAddr(0x9050),
                Field::Reg(15)
            ]
        );
    }

    #[test]
    fn test_pseudo_sequence_sh() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 16, imm: 0xa000 }, 0xa000);
        let inst2 = make_instruction(
            Op::Sh { rs1: 16, rs2: 11, offset: 0x100 },
            0xa004,
        );

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![
                Field::Opcode("sh"),
                Field::Reg(11),
                Field::PCRelAddr(0xa100),
                Field::Reg(16)
            ]
        );
    }

    #[test]
    fn test_pseudo_sequence_sw() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 17, imm: 0xb000 }, 0xb000);
        let inst2 = make_instruction(
            Op::Sw { rs1: 17, rs2: 12, offset: 0x200 },
            0xb004,
        );

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_some());
        let (count, fields) = result.unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fields,
            vec![
                Field::Opcode("sw"),
                Field::Reg(12),
                Field::PCRelAddr(0xb200),
                Field::Reg(17)
            ]
        );
    }

    #[test]
    fn test_pseudo_sequence_not_detected_with_label() {
        let mut symbols = HashMap::new();
        symbols.insert(0x5004, "label".to_string());

        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x5000 }, 0x5000);
        let inst2 =
            make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 0x200 }, 0x5004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(
            result.is_none(),
            "Should not merge when second instruction is labeled"
        );
    }

    #[test]
    fn test_pseudo_sequence_not_detected_mismatched_registers_la() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x6000 }, 0x6000);
        // rd and rs1 don't match
        let inst2 =
            make_instruction(Op::Addi { rd: 11, rs1: 10, imm: 0x100 }, 0x6004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_none());
    }

    #[test]
    fn test_pseudo_sequence_not_detected_mismatched_registers_lb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x7000 }, 0x7000);
        // rd and rs1 don't match
        let inst2 =
            make_instruction(Op::Lb { rd: 11, rs1: 10, offset: 0x80 }, 0x7004);

        let result = get_pseudo_sequence(&[inst1, inst2], &symbols);
        assert!(result.is_none());
    }

    #[test]
    fn test_pseudo_sequence_not_detected_mismatched_registers_sb() {
        let symbols = HashMap::new();
        let inst1 = make_instruction(Op::Auipc { rd: 10, imm: 0x8000 }, 0x8000);
        // rd and rs1 don't match
        let inst2 =
            make_instruction(Op::Sb { rs1: 11, rs2: 5, offset: 0x50 }, 0x8004);

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

    // Full operand checks cover aliases whose opcode alone cannot establish correctness.
    #[test]
    fn aliases_preserve_registers_and_constants() {
        for (op, fields) in [
            (
                Op::Add { rd: 10, rs1: ZERO, rs2: 11 },
                vec![Field::Opcode("mv"), Field::Reg(10), Field::Reg(11)],
            ),
            (
                Op::Lui { rd: 10, imm: i32::MIN },
                vec![Field::Opcode("li"), Field::Reg(10), Field::Imm(i32::MIN)],
            ),
            (
                Op::Sub { rd: 10, rs1: ZERO, rs2: 11 },
                vec![Field::Opcode("neg"), Field::Reg(10), Field::Reg(11)],
            ),
        ] {
            assert_eq!(op.to_pseudo_fields(), fields);
        }
    }

    #[test]
    fn sequences_preserve_wrapped_values_and_store_temporaries() {
        let symbols = HashMap::new();
        for (first, second, expected) in [
            (
                Op::Lui { rd: 10, imm: i32::MIN },
                Op::Addi { rd: 10, rs1: 10, imm: -1 },
                vec![Field::Opcode("li"), Field::Reg(10), Field::Imm(i32::MAX)],
            ),
            (
                Op::Auipc { rd: 10, imm: i32::MIN },
                Op::Addi { rd: 10, rs1: 10, imm: -1 },
                vec![
                    Field::Opcode("la"),
                    Field::Reg(10),
                    Field::PCRelAddr(i32::MAX),
                ],
            ),
            (
                Op::Auipc { rd: RA, imm: i32::MIN },
                Op::Jalr { rd: RA, rs1: RA, offset: -2 },
                vec![Field::Opcode("call"), Field::PCRelAddr(i32::MAX - 1)],
            ),
            (
                Op::Auipc { rd: 6, imm: i32::MIN },
                Op::Jalr { rd: ZERO, rs1: 6, offset: -2 },
                vec![Field::Opcode("tail"), Field::PCRelAddr(i32::MAX - 1)],
            ),
            (
                Op::Auipc { rd: 5, imm: i32::MIN },
                Op::Sw { rs1: 5, rs2: 10, offset: -1 },
                vec![
                    Field::Opcode("sw"),
                    Field::Reg(10),
                    Field::PCRelAddr(i32::MAX),
                    Field::Reg(5),
                ],
            ),
        ] {
            let pair = [
                make_instruction(first, 0x1000),
                make_instruction(second, 0x1004),
            ];
            assert_eq!(
                get_pseudo_sequence(&pair, &symbols),
                Some((2, expected))
            );
        }
    }

    // A discarded base, a gap, or an independently reachable instruction blocks merging.
    #[test]
    fn sequences_reject_invalid_bases_and_boundaries() {
        for first in
            [Op::Auipc { rd: ZERO, imm: 4096 }, Op::Lui { rd: ZERO, imm: 4096 }]
        {
            let pair = [
                make_instruction(first, 0x1000),
                make_instruction(
                    Op::Addi { rd: ZERO, rs1: ZERO, imm: 4 },
                    0x1004,
                ),
            ];
            assert!(get_pseudo_sequence(&pair, &HashMap::new()).is_none());
        }
        let mut pair = [
            make_instruction(Op::Lui { rd: 10, imm: 4096 }, 0x1000),
            make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 4 }, 0x1006),
        ];
        assert!(get_pseudo_sequence(&pair, &HashMap::new()).is_none());
        pair[1].address = 0x1004;
        assert!(
            get_pseudo_sequence(
                &pair,
                &HashMap::from([(0x1004, "entry".to_string())])
            )
            .is_none()
        );
        pair[0].op = Op::Auipc { rd: RA, imm: 4096 };
        pair[1].op = Op::Jalr { rd: RA, rs1: RA, offset: 1 };
        assert!(get_pseudo_sequence(&pair, &HashMap::new()).is_none());
        pair[0].op = Op::Auipc { rd: 6, imm: 4096 };
        pair[1].op = Op::Jalr { rd: ZERO, rs1: 6, offset: 1 };
        assert!(get_pseudo_sequence(&pair, &HashMap::new()).is_none());
    }

    // Both direct calls and statically resolvable indirect calls protect entry points.
    #[test]
    fn call_targets_block_merging_at_reachable_second_instructions() {
        for mut instructions in [
            vec![
                make_instruction(Op::Jal { rd: RA, offset: 8 }, 0x1000),
                make_instruction(Op::Lui { rd: 10, imm: 4096 }, 0x1004),
                make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 4 }, 0x1008),
            ],
            vec![
                make_instruction(Op::Auipc { rd: RA, imm: 0 }, 0x1000),
                make_instruction(
                    Op::Jalr { rd: RA, rs1: RA, offset: 13 },
                    0x1004,
                ),
                make_instruction(Op::Lui { rd: 10, imm: 4096 }, 0x1008),
                make_instruction(Op::Addi { rd: 10, rs1: 10, imm: 4 }, 0x100c),
            ],
        ] {
            let mut machine = Machine::for_testing();
            add_local_labels(&mut machine, &instructions);
            let second = instructions.pop().expect("pair second instruction");
            let first = instructions.pop().expect("pair first instruction");
            assert!(machine.address_symbols.contains_key(&second.address));
            assert!(
                get_pseudo_sequence(&[first, second], &machine.address_symbols)
                    .is_none()
            );
        }
    }

    #[test]
    fn address_operands_preserve_wrapping_and_numeric_labels() {
        for (pc, offset, expected) in [
            (0x1000, i32::MIN, ". + 2147483647 + 1"),
            (0xfffffff0, 32, ". - 2147483647 - 2147483617"),
            (0x1000, -16, ". - 16"),
        ] {
            assert_eq!(
                Field::PCRelAddr(offset).to_string(
                    pc,
                    0,
                    false,
                    false,
                    &HashMap::new()
                ),
                expected
            );
        }
        let symbols = HashMap::from([(0x1000, "0".to_string())]);
        assert_eq!(
            Field::GPRelAddr(0)
                .to_string(0x1004, 0x1000, false, false, &symbols),
            "0b"
        );
    }
}
