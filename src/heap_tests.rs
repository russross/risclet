use std::rc::Rc;

use crate::execution::{Instruction, Machine};
use crate::memory::Segment;
use crate::riscv::{A0, Op};
use crate::trace::{Effects, SyscallInfo};

fn machine() -> Machine {
    Machine::builder()
        .with_segments(vec![
            Segment::new(0x1000, 0x1004, false, true, vec![0x73, 0, 0, 0]),
            Segment::new(0x2000, 0x2011, true, false, vec![]),
        ])
        .build()
}

fn brk(machine: &mut Machine, requested: u32) -> Effects {
    machine.set(17, 214);
    machine.set(A0, requested as i32);
    let op = Op::Ecall;
    let instruction = Rc::new(Instruction {
        address: machine.pc(),
        verbose_fields: op.to_fields(),
        pseudo_fields: op.to_pseudo_fields(),
        op,
        length: 4,
        encoding: 0x73,
        pseudo_index: 0,
    });
    machine.execute_and_collect_effects(&instruction)
}

// Byte-exact bounds keep the alignment gap and the unused page tail inaccessible.
#[test]
fn heap_bounds_permissions_and_failed_requests() {
    let mut m = machine();
    assert_eq!(m.heap_start(), 0x3000);
    let query = brk(&mut m, 0);
    assert!(query.heap_change().is_none());
    assert_eq!(m.get_reg(A0), 0x3000);
    assert!(m.load_u8(0x2011).is_err());
    assert!(m.load_u8(0x3000).is_err());

    let growth = brk(&mut m, 0x3005);
    assert!(growth.other_message().is_none());
    assert_eq!(m.load(0x3000, 5).unwrap(), [0; 5]);
    assert!(m.load_u8(0x3005).is_err());
    assert!(m.load(0x3004, 2).is_err());
    assert!(m.store(0x3004, &[1, 2]).is_err());
    assert!(m.load_instruction(0x3000).is_err());
    m.store(0x3000, b"hello").unwrap();

    for request in [0, 0x2fff, 0x3005, m.stack_start() + 1, u32::MAX] {
        let effect = brk(&mut m, request);
        assert!(effect.heap_change().is_none());
        assert_eq!(m.get_reg(A0), 0x3005);
        assert_eq!(m.load(0x3000, 5).unwrap(), b"hello");
    }
    assert_eq!(
        growth.report_effects(false, &m.io),
        ["brk(0x3005)", "a0 <- 0x3005"]
    );
    assert!(matches!(
        growth.syscall(),
        Some(SyscallInfo::Brk { requested: 0x3005, result: 0x3005 })
    ));
}

// Each resize reconstructs the same bytes after arbitrarily repeated undo/redo.
#[test]
fn shrink_regrow_replay_and_reset() {
    let mut m = machine();
    let growth = brk(&mut m, 0x3008);
    m.store(0x3000, b"abcdefgh").unwrap();
    let shrink = brk(&mut m, 0x3003);
    assert_eq!(shrink.heap_change().unwrap().removed, b"defgh");
    let regrowth = brk(&mut m, 0x3008);
    assert_eq!(m.load(0x3000, 8).unwrap(), b"abc\0\0\0\0\0");
    for _ in 0..2 {
        m.apply(&regrowth, false);
        assert!(m.load_u8(0x3003).is_err());
        m.apply(&shrink, false);
        assert_eq!(m.load(0x3000, 8).unwrap(), b"abcdefgh");
        m.apply(&shrink, true);
        m.apply(&regrowth, true);
        assert_eq!(m.load(0x3000, 8).unwrap(), b"abc\0\0\0\0\0");
    }
    m.reset();
    assert_eq!(m.heap_break(), 0x3000);
    assert!(m.load_u8(0x3000).is_err());
    m.apply(&growth, true);
    assert_eq!(m.load(0x3000, 8).unwrap(), [0; 8]);
    m.apply(&growth, false);
    assert!(m.load_u8(0x3000).is_err());
}

#[test]
fn heap_limit_and_image_without_data() {
    let mut m = machine();
    let limit = m.stack_start();
    assert_eq!(m.brk(limit), limit);
    m.store(limit - 1, &[42]).unwrap();
    assert!(m.load(limit - 1, 2).is_err());
    assert_eq!(m.brk(limit + 1), limit);
    assert_eq!(m.brk(m.heap_start()), m.heap_start());

    let mut m = Machine::builder()
        .with_segments(vec![Segment::new(
            0x1000,
            0x1004,
            false,
            true,
            vec![0x73, 0, 0, 0],
        )])
        .build();
    assert!(m.data_display_bounds().is_empty());
    assert_eq!(m.brk(0), 0x2000);
    m.brk(0x2001);
    assert_eq!(m.data_display_bounds(), 0x2000..0x2001);
}
