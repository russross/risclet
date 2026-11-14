use std::rc::Rc;

use crate::assembler::{AssemblyOutput, assemble};
use crate::config::{Config, Mode};
use crate::elf::ElfFile;
use crate::execution::{Instruction, Machine};
use crate::riscv::{Op, fields_to_string};
use std::collections::HashMap;

const GNU_ENCODINGS: &[u8] = include_bytes!("test_binaries/bitmanip_encodings");
const QEMU_RESULTS: &[u8] = include_bytes!("test_binaries/bitmanip_results");
const GNU_SOURCE: &str = include_str!("test_binaries/src/bitmanip_encodings.s");

fn assemble_text(source: &str, compressed: bool) -> Result<Vec<u8>, String> {
    let mut config = Config::for_mode(Mode::Assemble);
    config.relax.compressed = compressed;
    let source = format!(".global _start\n_start:\n{source}\n");
    let AssemblyOutput::Elf(bytes, _) =
        assemble(&mut config, vec![("test.s".into(), source)])
            .map_err(|error| error.to_string())?
    else {
        panic!("unexpected dump")
    };
    let file = ElfFile::parse(&bytes).unwrap();
    let text = file
        .section_headers
        .iter()
        .position(|section| section.sh_flags & 4 != 0)
        .unwrap();
    Ok(file.section_data(text).unwrap().to_vec())
}

// GNU's closed fixture includes every mnemonic, with compression disabled for
// ordinary instructions. This checks parser, layout, and encoder together.
#[test]
fn instruction_encodings_match_gnu_and_disassembly_reassembles() {
    let source = GNU_SOURCE
        .lines()
        .filter(|line| {
            !line.starts_with(".option")
                && !line.starts_with(".global")
                && *line != "_start:"
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(assemble_text(&source, false).unwrap(), GNU_ENCODINGS);
    let mut config = Config::for_mode(Mode::Disassemble);
    config.show_addresses = false;
    config.show_encoding = false;
    let symbols = HashMap::new();
    let mut source = String::new();
    let mut offset = 0;
    while offset < GNU_ENCODINGS.len() {
        let length = if GNU_ENCODINGS[offset] & 3 == 3 { 4 } else { 2 };
        let mut raw = [0; 4];
        raw[..length].copy_from_slice(&GNU_ENCODINGS[offset..offset + length]);
        let word = u32::from_le_bytes(raw);
        let op = Op::new(word as i32);
        assert!(!matches!(op, Op::Unimplemented { .. }), "{word:08x}");
        for fields in
            [op.to_fields(), op.to_pseudo_fields(), op.to_encoding_fields(word)]
        {
            let line = fields_to_string(
                &config,
                &fields,
                0x10000 + offset as u32,
                0,
                true,
                None,
                &symbols,
            );
            let actual = assemble_text(line.trim(), false).unwrap();
            // Expanded disassembly uses full-width operations; encoded spelling
            // preserves the exact original representation, including aliases.
            if fields == op.to_encoding_fields(word) {
                assert_eq!(
                    actual,
                    GNU_ENCODINGS[offset..offset + length],
                    "{line}"
                );
            } else {
                let mut bytes = [0; 4];
                bytes.copy_from_slice(&actual);
                assert_eq!(Op::new(i32::from_le_bytes(bytes)), op, "{line}");
            }
        }
        source.push_str(&fields_to_string(
            &config,
            &op.to_encoding_fields(word),
            0x10000 + offset as u32,
            0,
            true,
            None,
            &symbols,
        ));
        source.push('\n');
        offset += length;
    }
    assert_eq!(assemble_text(&source, false).unwrap(), GNU_ENCODINGS);
}

// Expected results come from QEMU, independently of Risclet's implementation.
// Aliasing and discarded destinations also exercise trace recording and replay.
#[test]
fn execution_and_replay_match_qemu_for_boundaries_and_bit_patterns() {
    assert_eq!(QEMU_RESULTS.len() % 16, 0);
    let mut machine = Machine::for_testing();
    for record in QEMU_RESULTS.as_chunks::<16>().0 {
        let words: Vec<_> = record
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect();
        let [encoding, a, b, expected] = words[..] else { unreachable!() };
        for destination in [0, 10, 11, 12] {
            let encoding = (encoding & !(31 << 7)) | (destination << 7);
            machine.set_pc(0x10000).unwrap();
            machine.set(10, 0x13579);
            machine.set(11, a as i32);
            machine.set(12, b as i32);
            let old = machine.get_reg(destination as usize);
            let op = Op::new(encoding as i32);
            let instruction = Rc::new(Instruction {
                address: 0x10000,
                verbose_fields: op.to_fields(),
                pseudo_fields: op.to_pseudo_fields(),
                op,
                length: 4,
                encoding,
                pseudo_index: 0,
            });
            let effects = machine.execute_and_collect_effects(&instruction);
            assert!(effects.other_message().is_none(), "{encoding:08x}");
            let result = if destination == 0 { 0 } else { expected };
            assert_eq!(
                machine.get_reg(destination as usize) as u32,
                result,
                "{encoding:08x} a={a:08x} b={b:08x}"
            );
            assert_eq!(machine.pc(), 0x10004);
            assert_eq!(effects.reg_write.is_some(), destination != 0);
            machine.apply(&effects, false);
            assert_eq!(machine.get_reg(destination as usize), old);
            assert_eq!(machine.pc(), 0x10000);
            machine.apply(&effects, true);
            assert_eq!(machine.get_reg(destination as usize) as u32, result);
        }
    }
}

// Every Zcb form has one ordinary counterpart, with exact register and offset
// constraints. Ordinary forms outside those constraints retain four bytes.
#[test]
fn zcb_relaxation_matches_explicit_encodings_and_preserves_fallbacks() {
    for (ordinary, compressed) in [
        ("lbu a0, 3(a1)", "c.lbu a0, 3(a1)"),
        ("lh a0, 2(a1)", "c.lh a0, 2(a1)"),
        ("lhu a0, 2(a1)", "c.lhu a0, 2(a1)"),
        ("sb a0, 3(a1)", "c.sb a0, 3(a1)"),
        ("sh a0, 2(a1)", "c.sh a0, 2(a1)"),
        ("zext.b a0, a0", "c.zext.b a0"),
        ("andi a0, a0, 255", "c.zext.b a0"),
        ("sext.b a0, a0", "c.sext.b a0"),
        ("zext.h a0, a0", "c.zext.h a0"),
        ("sext.h a0, a0", "c.sext.h a0"),
        ("not a0, a0", "c.not a0"),
        ("xori a0, a0, -1", "c.not a0"),
        ("mul a0, a0, a1", "c.mul a0, a1"),
        ("mul a0, a1, a0", "c.mul a0, a1"),
    ] {
        assert_eq!(
            assemble_text(ordinary, true).unwrap(),
            assemble_text(compressed, false).unwrap(),
            "{ordinary}"
        );
        assert_eq!(assemble_text(ordinary, false).unwrap().len(), 4);
    }
    for source in [
        "lbu a0, 4(a1)",
        "lbu a0, -1(a1)",
        "lh a0, 1(a1)",
        "lhu a0, 4(a1)",
        "sb a0, 0(t0)",
        "sh t0, 0(a0)",
        "sext.b a0, a1",
        "sext.h t0, t0",
        "zext.h a0, a1",
        "andi a0, a1, 255",
        "xori a0, a1, -1",
        "mul a0, a1, a2",
        "mul t0, t0, a0",
    ] {
        assert_eq!(assemble_text(source, true).unwrap().len(), 4, "{source}");
    }
}

#[test]
fn invalid_operands_and_rv64_or_reserved_encodings_are_rejected() {
    for name in ["rori", "bclri", "bexti", "binvi", "bseti"] {
        for amount in ["-1", "32", "0x800", "_start"] {
            assert!(
                assemble_text(&format!("{name} a0, a1, {amount}"), false)
                    .is_err()
            );
        }
        for amount in [0, 31] {
            assert!(
                assemble_text(&format!("{name} a0, a1, {amount}"), false)
                    .is_ok()
            );
        }
    }
    for source in [
        "clz a0",
        "clz a0, a1, a2",
        "zip a0, 1",
        "c.sext.b t0",
        "c.mul a0, t0",
        "c.lbu a0, 4(a1)",
        "c.lh a0, 1(a1)",
        "c.lhu a0, -2(a1)",
        "c.sb t0, 0(a1)",
        "c.sh a0, 0(t0)",
        "c.zext.w a0",
        "cm.push {ra}, -16",
    ] {
        assert!(assemble_text(source, false).is_err(), "{source}");
    }
    for encoding in [
        0x6205d513, 0x6b85d513, 0x64059513, 0x4805d51b, 0x8c40, 0x9000, 0x9c71,
        0x9c79, 0x9c7d,
    ] {
        assert!(
            matches!(Op::new(encoding), Op::Unimplemented { .. }),
            "{encoding:08x}"
        );
    }
}

// Compressed instructions retain two-byte PC progress and scalar effects.
#[test]
fn zcb_execution_records_and_replays_register_and_memory_effects() {
    for (index, expected, stored) in [
        (0, Some(255), None),
        (1, Some(-128), None),
        (2, Some(65408), None),
        (3, None, Some([0x11, 0x22, 0x80, 0x78])),
        (4, None, Some([0x11, 0x22, 0x78, 0x56])),
        (5, Some(0x78), None),
        (6, Some(0x78), None),
        (7, Some(0x5678), None),
        (8, Some(0x5678), None),
        (9, Some(!0x12345678), None),
        (10, Some(0x12345678_i32.wrapping_mul(0x20000)), None),
    ] {
        let mut machine = Machine::for_testing();
        machine.set_pc(0x10000).unwrap();
        machine.set(10, 0x12345678);
        machine.set(11, 0x20000);
        let initial = [0x11, 0x22, 0x80, 0xff];
        machine.store(0x20000, &initial).unwrap();
        let offset = 140 + index * 2;
        let encoding = u16::from_le_bytes(
            GNU_ENCODINGS[offset..offset + 2].try_into().unwrap(),
        ) as u32;
        let op = Op::new(encoding as i32);
        let instruction = Rc::new(Instruction {
            address: 0x10000,
            verbose_fields: op.to_encoding_fields(encoding),
            pseudo_fields: op.to_pseudo_fields(),
            op,
            length: 2,
            encoding,
            pseudo_index: 0,
        });
        let effects = machine.execute_and_collect_effects(&instruction);
        assert!(effects.other_message().is_none());
        assert_eq!(machine.pc(), 0x10002);
        assert_eq!(machine.get_reg(10), expected.unwrap_or(0x12345678));
        assert_eq!(
            machine.load(0x20000, 4).unwrap(),
            stored.unwrap_or(initial)
        );
        assert_eq!(effects.mem_write.is_some(), stored.is_some());
        assert_eq!(effects.mem_read.is_some(), index < 3);
        assert!(!effects.report(false, &machine.io).is_empty());

        // Rewinding restores the destination and every overwritten byte.
        machine.apply(&effects, false);
        assert_eq!(machine.get_reg(10), 0x12345678);
        assert_eq!(machine.load(0x20000, 4).unwrap(), initial);
        machine.apply(&effects, true);
        assert_eq!(machine.get_reg(10), expected.unwrap_or(0x12345678));
        assert_eq!(
            machine.load(0x20000, 4).unwrap(),
            stored.unwrap_or(initial)
        );
    }
}
