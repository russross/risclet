use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Colors};

use super::MemoryPane::{Data, Stack, Text};
use super::{
    MemoryVisibility, Screen, Tui, calc_range, call_entries, function_colors,
    function_entries, memory_color, memory_layout, stack_regions,
};
use crate::config::{Config, Mode};
use crate::execution::{Instruction, Machine};
use crate::memory::Segment;
use crate::riscv::{Op, RA, SP, ZERO};
use crate::trace::{
    Effects, FrameChange, MemoryValue, MemoryWrite, RegisterWrite, SyscallInfo,
};

const ALL: MemoryVisibility =
    MemoryVisibility { stack: true, data: true, text: true };

// Use mixed instruction lengths and a partial final memory row. The first two
// instructions share a source pseudoinstruction but remain distinct in memory.
fn debugger(data: bool) -> Tui {
    let mut instructions = Vec::new();
    let mut bytes = Vec::new();
    for (index, length) in [2, 4, 4, 2, 4, 2].into_iter().enumerate() {
        let encoding: u32 = if length == 2 { 1 } else { 0x13 };
        let op = if index == 5 {
            Op::Jal { rd: RA, offset: -6 }
        } else {
            Op::new(encoding as i32)
        };
        instructions.push(Rc::new(Instruction {
            address: 0x1000 + bytes.len() as u32,
            encoding,
            length,
            pseudo_index: index.saturating_sub(1),
            verbose_fields: op.to_fields(),
            pseudo_fields: op.to_pseudo_fields(),
            op,
        }));
        bytes.extend_from_slice(&encoding.to_le_bytes()[..length as usize]);
    }
    let mut segments = vec![Segment::new(
        0x1000,
        0x1000 + bytes.len() as u32,
        false,
        true,
        bytes,
    )];
    if data {
        segments.push(Segment::new(
            0x2000,
            0x2011,
            true,
            false,
            b"abcdefghijklmnopq".to_vec(),
        ));
    }

    // Calls identify functions; labels can name internal blocks or data.
    let symbols = HashMap::from([
        (0x1000, "main".to_owned()),
        (0x1002, "1".to_owned()),
        (0x100a, "worker".to_owned()),
        (0x2000, "message".to_owned()),
    ]);
    let machine = Machine::builder()
        .with_segments(segments)
        .with_address_symbols(symbols)
        .build();
    let addresses = instructions
        .iter()
        .enumerate()
        .map(|(i, inst)| (inst.address, i))
        .collect();
    let mut pseudo_addresses = HashMap::new();
    for (i, inst) in instructions.iter().enumerate() {
        pseudo_addresses.entry(inst.pseudo_index).or_insert(i);
    }
    let sequence = instructions
        .iter()
        .enumerate()
        .map(|(i, inst)| {
            let mut effect = Effects::new(inst);
            effect.pc =
                (inst.address, instructions.get(i + 1).unwrap_or(inst).address);
            effect
        })
        .collect();
    Tui::for_screen(
        machine,
        instructions,
        addresses,
        pseudo_addresses,
        sequence,
        &Config::for_mode(Mode::Debug),
    )
}

fn key(tui: &mut Tui, code: KeyCode) {
    assert!(
        !tui.handle_key(KeyEvent::new(code, KeyModifiers::NONE), 12).unwrap()
    );
}

// Replace a displayed operation and its trace preview together.
fn set_operation(tui: &mut Tui, index: usize, op: Op, target: u32) {
    let old = &tui.instructions[index];
    let instruction = Rc::new(Instruction {
        address: old.address,
        encoding: old.encoding,
        length: old.length,
        pseudo_index: old.pseudo_index,
        verbose_fields: op.to_fields(),
        pseudo_fields: op.to_pseudo_fields(),
        op,
    });
    let mut effects = Effects::new(&instruction);
    effects.pc = (instruction.address, target);
    tui.instructions[index] = instruction;
    tui.sequence[index] = effects;
}

#[test]
fn branch_lines_mark_one_way_control_flow_in_both_listing_modes() {
    for verbose in [false, true] {
        for (index, op, target, expected) in [
            (0, Op::Beq { rs1: ZERO, rs2: ZERO, offset: 6 }, 0x1006, true),
            (2, Op::Jal { rd: ZERO, offset: -6 }, 0x1000, true),
            (0, Op::Jal { rd: RA, offset: 6 }, 0x1006, false),
            (0, Op::Jal { rd: ZERO, offset: 10 }, 0x100a, true),
            (3, Op::Bne { rs1: RA, rs2: ZERO, offset: -10 }, 0x1000, true),
            (0, Op::Beq { rs1: ZERO, rs2: ZERO, offset: 6 }, 0x1002, false),
            (0, Op::Jalr { rd: ZERO, rs1: RA, offset: 0 }, 0x1006, false),
            (0, Op::Jalr { rd: ZERO, rs1: SP, offset: 0 }, 0x100a, true),
            (0, Op::Jalr { rd: ZERO, rs1: RA, offset: 4 }, 0x1006, true),
            (0, Op::Jalr { rd: RA, rs1: SP, offset: 0 }, 0x1006, false),
            (0, Op::Jalr { rd: SP, rs1: RA, offset: 0 }, 0x1006, false),
            (0, Op::Jalr { rd: ZERO, rs1: SP, offset: 0 }, 0x1000, false),
            (0, Op::Jalr { rd: ZERO, rs1: SP, offset: 0 }, 0x1002, false),
            (0, Op::Jalr { rd: ZERO, rs1: SP, offset: 0 }, 0x2000, false),
        ] {
            let mut tui = debugger(false);
            tui.config.verbose_instructions = verbose;
            tui.machine.address_symbols.insert(0x1002, "loop".to_owned());
            set_operation(&mut tui, index, op, target);
            tui.sequence_index = index;
            let (screen, _) = tui.render_screen(79, 24);
            let has_line =
                (0..screen.len()).any(|row| line(&screen, row).contains("┌──"));
            assert_eq!(
                has_line, expected,
                "verbose={verbose}, target={target:x}"
            );
            assert_eq!(
                memory_color(&tui.text_colors, 0x1002, 0..0),
                tui.pastels[0]
            );
        }
    }
}

#[test]
fn call_entries_combine_static_and_observed_destinations() {
    let mut tui = debugger(false);
    set_operation(&mut tui, 0, Op::Auipc { rd: RA, imm: 0 }, 0x1002);
    set_operation(&mut tui, 1, Op::Jalr { rd: RA, rs1: RA, offset: 6 }, 0x1006);
    tui.sequence[1].reg_write =
        Some(RegisterWrite { register: RA, old_value: 0, new_value: 0x1006 });
    set_operation(&mut tui, 2, Op::Jalr { rd: RA, rs1: SP, offset: 0 }, 0x100c);
    tui.sequence[2].reg_write =
        Some(RegisterWrite { register: RA, old_value: 0, new_value: 0x100a });
    assert_eq!(
        call_entries(0x1000, &tui.instructions, &tui.sequence),
        vec![0x1000, 0x1006, 0x100a, 0x100c]
    );

    // Tail jumps and invalid call destinations do not introduce entries.
    set_operation(
        &mut tui,
        1,
        Op::Jalr { rd: ZERO, rs1: RA, offset: 6 },
        0x1006,
    );
    set_operation(&mut tui, 5, Op::Jal { rd: RA, offset: 1 }, 0x1011);
    set_operation(
        &mut tui,
        2,
        Op::Jalr { rd: ZERO, rs1: SP, offset: 0 },
        0x100c,
    );
    assert_eq!(
        call_entries(0x1000, &tui.instructions, &tui.sequence),
        vec![0x1000]
    );
}

#[test]
fn unused_named_regions_respect_visitation_and_text_segments() {
    // Two segments span multiple visitation words, with a gap between them.
    let instructions: Vec<_> = (0..70)
        .map(|index| {
            let address = if index < 35 {
                0x1000 + index * 2
            } else {
                0x2000 + (index - 35) * 2
            };
            let op = Op::new(1);
            Rc::new(Instruction {
                address,
                encoding: 1,
                length: 2,
                pseudo_index: index as usize,
                verbose_fields: op.to_fields(),
                pseudo_fields: op.to_pseudo_fields(),
                op,
            })
        })
        .collect();
    let addresses = instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| (instruction.address, index))
        .collect();
    let symbols = [
        (0, "main"),
        (10, "internal"),
        (31, "library"),
        (33, "1"),
        (35, "second_segment"),
        (40, "visited"),
        (60, "unused_last"),
    ]
    .map(|(index, name)| (instructions[index].address, name.to_owned()));
    let mut machine = Machine::builder()
        .with_segments(vec![
            Segment::new(0x1000, 0x1046, false, true, vec![0; 70]),
            Segment::new(0x2000, 0x2046, false, true, vec![0; 70]),
        ])
        .with_address_symbols(HashMap::from(symbols))
        .build();

    // Numeric labels, non-instruction symbols, and non-code names add no entries.
    machine.address_symbols.insert(0x1001, "misaligned".to_owned());
    machine.address_symbols.insert(0x3000, "data".to_owned());
    let sequence: Vec<_> = [0, 10, 30, 40, 40]
        .into_iter()
        .map(|index| Effects::new(&instructions[index]))
        .collect();
    assert_eq!(
        function_entries(&machine, &instructions, &addresses, &sequence),
        [0, 31, 35, 60].map(|index| instructions[index].address)
    );

    // Without execution, every named code interval qualifies as unused.
    assert_eq!(
        function_entries(&machine, &instructions, &addresses, &[]),
        [0, 10, 31, 35, 40, 60].map(|index| instructions[index].address)
    );
}

fn line(screen: &Screen, row: usize) -> String {
    screen[row].iter().map(|&(ch, _)| ch).collect()
}

fn label_row(screen: &Screen, label: &str) -> Option<usize> {
    (0..screen.len()).find(|&row| line(screen, row).contains(label))
}

#[test]
fn memory_height_thresholds_and_growth() {
    for (height, expected) in [
        (16, vec![(Data, 14)]),
        (17, vec![(Stack, 7), (Data, 7)]),
        (23, vec![(Stack, 9), (Data, 11)]),
        (24, vec![(Stack, 7), (Data, 7), (Text, 6)]),
        (25, vec![(Stack, 7), (Data, 7), (Text, 7)]),
        (29, vec![(Stack, 8), (Data, 9), (Text, 8)]),
        (49, vec![(Stack, 13), (Data, 19), (Text, 13)]),
    ] {
        assert_eq!(memory_layout(80, height, ALL, false), expected);
    }
    assert_eq!(memory_layout(80, 16, ALL, true), vec![(Stack, 14)]);
    assert!(memory_layout(79, 50, ALL, false).is_empty());
}

#[test]
fn toggles_determine_eligibility_before_space_and_context() {
    for (visible, expected) in [
        (MemoryVisibility { text: false, ..ALL }, vec![(Stack, 9), (Data, 12)]),
        (
            MemoryVisibility { data: false, ..ALL },
            vec![(Stack, 10), (Text, 11)],
        ),
        (MemoryVisibility { stack: false, ..ALL }, vec![(Data, 12), (Text, 9)]),
        (
            MemoryVisibility { stack: false, data: false, text: true },
            vec![(Text, 22)],
        ),
    ] {
        assert_eq!(memory_layout(80, 24, visible, false), expected);
    }
    // A hidden/absent Data pane must never displace Stack on a short screen.
    let no_data = MemoryVisibility { data: false, ..ALL };
    assert_eq!(memory_layout(80, 16, no_data, false), vec![(Stack, 14)]);
    let no_stack = MemoryVisibility { stack: false, ..ALL };
    assert_eq!(memory_layout(80, 16, no_stack, true), vec![(Data, 14)]);
}

#[test]
fn every_toggle_combination_fits_and_grows_without_losing_rows() {
    for mask in 0..8 {
        let visible = MemoryVisibility {
            stack: mask & 1 != 0,
            data: mask & 2 != 0,
            text: mask & 4 != 0,
        };
        for recent_stack in [false, true] {
            let mut previous = Vec::new();
            for height in 3..=200 {
                let layout = memory_layout(80, height, visible, recent_stack);
                if mask == 0 {
                    assert!(layout.is_empty());
                    continue;
                }
                let content: u16 = layout.iter().map(|&(_, size)| size).sum();
                assert_eq!(content + layout.len() as u16 + 1, height);
                assert!(layout.iter().all(|&(_, size)| size > 0));
                if layout.len() == previous.len() {
                    for ((kind, size), (old_kind, old_size)) in
                        layout.iter().zip(&previous)
                    {
                        assert_eq!(kind, old_kind);
                        assert!(size >= old_size);
                    }
                }
                previous = layout;
            }
        }
    }
}

#[test]
fn actual_screen_preserves_borders_and_restores_text_after_resize() {
    let mut tui = debugger(true);
    assert!(tui.show_text);
    let (screen, source_height) = tui.render_screen(80, 24);
    assert_eq!(source_height, 17); // Four register rows and one separator.
    assert_eq!(label_row(&screen, " Stack "), Some(0));
    assert_eq!(label_row(&screen, " Data "), Some(8));
    assert_eq!(label_row(&screen, " Text "), Some(16));
    assert_eq!(screen[8][39].0, '├');
    assert_eq!(screen[16][79].0, '┤');
    assert_eq!(screen[23][39].0, '┴');

    let (small, _) = tui.render_screen(80, 23);
    assert_eq!(label_row(&small, " Text "), None);
    assert_eq!(label_row(&small, " Data "), Some(10));
    assert!(tui.show_text);
    let (large, _) = tui.render_screen(80, 25);
    assert_eq!(label_row(&large, " Text "), Some(16));
    key(&mut tui, KeyCode::Char('t'));
    assert_eq!(label_row(&tui.render_screen(80, 25).0, " Text "), None);
    key(&mut tui, KeyCode::Char('t'));
    assert!(label_row(&tui.render_screen(80, 25).0, " Text ").is_some());
}

#[test]
fn absent_data_and_narrow_screens_use_available_space() {
    let mut tui = debugger(false);
    let (screen, _) = tui.render_screen(80, 24);
    assert_eq!(label_row(&screen, " Data "), None);
    assert_eq!(label_row(&screen, " Text "), Some(11));
    for width in [5, 10, 39, 79] {
        let (screen, _) = tui.render_screen(width, 24);
        assert_eq!(label_row(&screen, " Text "), None);
        assert_eq!(screen[0].len(), width as usize);
    }
    for height in 3..=30 {
        for width in [5, 79, 80, 100] {
            let (screen, _) = tui.render_screen(width, height);
            assert_eq!(screen.len(), height as usize);
        }
    }
}

// Output remains content-capped. It receives the first extra row at 25,
// while disabling Registers releases their allocation to sufficiently long output.
#[test]
fn registers_and_output_keep_their_existing_growth_rules() {
    let mut tui = debugger(true);
    tui.machine.io.stdout = b"hello\n".to_vec();
    tui.sequence[0].extra_mut().syscall =
        Some(SyscallInfo::Write { fd: 1, buf_addr: 0, count: 6, data: 0..6 });
    tui.machine.apply(&tui.sequence[0], true);
    for (height, source, registers, output) in [
        (18, 16, None, None),
        (19, 12, Some(13), None),
        (23, 16, Some(17), None),
        (24, 12, Some(13), Some(18)),
        (25, 13, Some(14), Some(19)),
        (30, 18, Some(19), Some(24)),
    ] {
        let (screen, source_height) = tui.render_screen(80, height);
        assert_eq!(source_height, source);
        assert_eq!(label_row(&screen, " Registers "), registers);
        assert_eq!(label_row(&screen, " Output "), output);
    }
    tui.machine.reset();
    tui.machine.io.stdout = b"line\n".repeat(30);
    tui.sequence[0].extra_mut().syscall = Some(SyscallInfo::Write {
        fd: 1,
        buf_addr: 0,
        count: 150,
        data: 0..150,
    });
    tui.machine.apply(&tui.sequence[0], true);
    assert_eq!(tui.render_screen(80, 25).1, 12);
    tui.show_registers = false;
    let (screen, source) = tui.render_screen(80, 24);
    assert_eq!(source, 12);
    assert_eq!(label_row(&screen, " Output "), Some(13));
    tui.show_output = false;
    assert_eq!(tui.render_screen(80, 24).1, 22);
}

// Stream ranges preserve interleaving and colors across rewind and UTF-8 boundaries.
#[test]
fn output_colors_input_and_preserves_event_order() {
    let mut tui = debugger(true);
    tui.machine.io.stdout = b"prompt: \xc3\xa9\nend".to_vec();
    tui.machine.io.stdin = b"answer\n".to_vec();
    let mut effects = Vec::new();
    for syscall in [
        SyscallInfo::Write { fd: 1, buf_addr: 0, count: 8, data: 0..8 },
        SyscallInfo::Read { fd: 0, buf_addr: 0, count: 7, data: 0..7 },
        SyscallInfo::Write { fd: 1, buf_addr: 0, count: 1, data: 8..9 },
        SyscallInfo::Write { fd: 1, buf_addr: 0, count: 5, data: 9..14 },
    ] {
        let mut effect = tui.sequence[0].clone();
        effect.extra_mut().syscall = Some(syscall);
        tui.machine.apply(&effect, true);
        effects.push(effect);
    }
    let lines = tui.output_lines();
    let text: Vec<String> = lines
        .iter()
        .map(|line| line.iter().map(|&(ch, _)| ch).collect())
        .collect();
    assert_eq!(text, ["prompt: answer", "é", "end"]);
    assert!(lines[0][8..].iter().all(|&(_, input)| input));
    assert!(lines[1].iter().all(|&(_, input)| !input));

    // Verify the terminal screen uses color 39 only for input characters.
    let (screen, _) = tui.render_screen(80, 30);
    let row = label_row(&screen, " Output ").unwrap() + 1;
    let line = &screen[row];
    let start = line.iter().position(|&(ch, _)| ch == 'p').unwrap();
    assert_eq!(line[start + 8].1.foreground, Some(Color::AnsiValue(39)));
    assert_eq!(line[start].1, tui.normal_color);
    assert_eq!(line[start + 14].1, tui.normal_color);

    for effect in effects.iter().rev() {
        tui.machine.apply(effect, false);
    }
    assert!(tui.output_lines().is_empty());
    for effect in &effects {
        tui.machine.apply(effect, true);
    }
    assert_eq!(tui.output_lines(), lines);
}

fn assert_text_bytes(tui: &mut Tui, expected: Range<u32>) {
    let (screen, _) = tui.render_screen(80, 24);
    // Check hex and ASCII independently, including the partial final row.
    for address in 0x1000..0x1012 {
        let row_address = address - (address - 0x1000) % 8;
        let needle = format!("{row_address:06x}:");
        let row = (0..screen.len())
            .find(|&y| line(&screen, y).contains(&needle))
            .unwrap();
        let byte = ((address - 0x1000) % 8) as usize;
        let base =
            if address < 0x100a { tui.pastels[0] } else { tui.pastels[1] };
        let color = if expected.contains(&address) {
            Colors { foreground: base.background, background: base.foreground }
        } else {
            base
        };
        assert_eq!(screen[row][40 + 7 + 3 * byte].1, color, "hex {address:x}");
        assert_eq!(screen[row][40 + 31 + byte].1, color, "ASCII {address:x}");
    }
}

#[test]
fn text_tracks_execution_with_exact_lengths_and_ignores_source_cursor() {
    let mut tui = debugger(true);
    assert_text_bytes(&mut tui, 0x1000..0x1002);
    key(&mut tui, KeyCode::Down);
    assert_ne!(tui.cursor_index, 0);
    assert_text_bytes(&mut tui, 0x1000..0x1002);
    key(&mut tui, KeyCode::Right);
    assert_text_bytes(&mut tui, 0x1002..0x1006);
    key(&mut tui, KeyCode::Right);
    assert_text_bytes(&mut tui, 0x1006..0x100a); // Crosses the eight-byte row.
    key(&mut tui, KeyCode::Right);
    assert_text_bytes(&mut tui, 0x100a..0x100c); // New function color.
    key(&mut tui, KeyCode::Left);
    key(&mut tui, KeyCode::Char('v'));
    assert_text_bytes(&mut tui, 0x1006..0x100a);
}

#[test]
fn text_highlight_follows_execution_order_and_overlap() {
    let mut tui = debugger(true);
    // A jump can leave the previous instruction above the upcoming address.
    tui.sequence[0].instruction = tui.instructions[3].clone();
    key(&mut tui, KeyCode::Right);
    assert_text_bytes(&mut tui, 0x1002..0x1006);
    key(&mut tui, KeyCode::Left);
    assert_text_bytes(&mut tui, 0x100a..0x100c);

    // Repeated execution of the same address keeps the full highlight.
    tui.sequence[0].instruction = tui.instructions[1].clone();
    key(&mut tui, KeyCode::Right);
    assert_text_bytes(&mut tui, 0x1002..0x1006);
}

#[test]
fn stack_colors_preserve_callers_and_inactive_space() {
    let palette = [
        Colors::new(Color::Red, Color::Black),
        Colors::new(Color::Blue, Color::Black),
        Colors::new(Color::Green, Color::Black),
    ];
    let inactive = Colors::new(Color::Grey, Color::Black);
    let regions = stack_regions(&[0x3000, 0x2ff0], 0x2fe0, &palette, inactive);
    assert_eq!(memory_color(&regions, 0x2fdf, 0..0), inactive);
    assert_eq!(memory_color(&regions, 0x2fe0, 0..0), palette[2]);
    assert_eq!(memory_color(&regions, 0x2ff0, 0..0), palette[1]);
    assert_eq!(memory_color(&regions, 0x3000, 0..0), palette[0]);
    let returned = stack_regions(&[0x3000], 0x2ff0, &palette, inactive);
    assert_eq!(memory_color(&returned, 0x2ff0, 0..0), palette[1]);
    let same_sp = stack_regions(&[0x3000, 0x2ff0], 0x2ff0, &palette, inactive);
    assert_eq!(memory_color(&same_sp, 0x2ff0, 0..0), palette[1]);
    let duplicate =
        stack_regions(&[0x3000, 0x3000], 0x2ff0, &palette, inactive);
    assert_eq!(memory_color(&duplicate, 0x3000, 0..0), palette[0]);
    let no_frames = stack_regions(&[], 0x3000, &palette, inactive);
    assert_eq!(memory_color(&no_frames, 0x2fff, 0..0), inactive);
    assert_eq!(memory_color(&no_frames, 0x3000, 0..0), palette[0]);
}

#[test]
fn text_before_entries_stays_neutral_and_function_colors_wrap() {
    let tui = debugger(true);
    let colors = function_colors(&[], &tui.pastels, tui.normal_color);
    assert_eq!(memory_color(&colors, 0x1006, 0..0), tui.normal_color);

    // Entries cycle through the palette while any preceding text stays neutral.
    let colors = function_colors(
        &[0x1002, 0x1006, 0x100a],
        &tui.pastels[..2],
        tui.normal_color,
    );
    for (address, expected) in [
        (0x1000, tui.normal_color),
        (0x1002, tui.pastels[0]),
        (0x1006, tui.pastels[1]),
        (0x100b, tui.pastels[0]),
    ] {
        assert_eq!(memory_color(&colors, address, 0..0), expected);
    }
}

#[test]
fn data_keeps_partial_rows_and_recent_access_highlighting() {
    let mut tui = debugger(true);
    tui.sequence[0].mem_read =
        Some(MemoryValue { address: 0x2010, value: vec![b'q'] });
    key(&mut tui, KeyCode::Right);
    let (screen, _) = tui.render_screen(80, 24);
    let row = label_row(&screen, "002010:").unwrap();
    let base = memory_color(&tui.data_colors, 0x2010, 0..0);
    let inverted =
        Colors { foreground: base.background, background: base.foreground };
    assert_eq!(screen[row][47], ('7', inverted));
    assert_eq!(screen[row][48], ('1', inverted));
    assert_eq!(screen[row][71], ('q', inverted));
    assert_eq!(screen[row][72], (' ', tui.normal_color));
}

#[test]
fn upcoming_memory_access_takes_priority_and_rewinds() {
    for segment in [Data, Stack] {
        let mut tui = debugger(true);
        let address =
            if segment == Data { 0x2000 } else { tui.machine.stack_end() - 16 };
        if segment == Stack {
            tui.machine.set(SP, address as i32);
        }
        tui.sequence[0].mem_read =
            Some(MemoryValue { address, value: vec![0; 4] });
        tui.sequence[1].mem_write = Some(MemoryWrite {
            address: address + 2,
            old_value: vec![0; 4],
            new_value: vec![1; 4],
        });
        key(&mut tui, KeyCode::Right);

        // Only the upcoming store is highlighted, including overlapping bytes.
        let (screen, _) = tui.render_screen(80, 24);
        let row = label_row(&screen, &format!("{address:06x}:")).unwrap();
        let normal = screen[row][47].1;
        let full = screen[row][53].1.background.unwrap();
        assert_eq!(normal.background, Some(Color::AnsiValue(16)));
        assert_eq!(screen[row][56].1.background, Some(full));
        assert_eq!(screen[row][71].1, normal);
        assert_eq!(screen[row][73].1.background, Some(full));

        // Non-memory instructions preserve the most recent access highlight.
        for _ in 0..2 {
            key(&mut tui, KeyCode::Right);
            let (screen, _) = tui.render_screen(80, 24);
            let row = label_row(&screen, &format!("{address:06x}:")).unwrap();
            assert_eq!(screen[row][47].1, normal);
            assert_eq!(screen[row][53].1.background, Some(full));
        }

        // A later access removes the store highlight entirely.
        tui.sequence[4].mem_read =
            Some(MemoryValue { address: address + 6, value: vec![0; 2] });
        key(&mut tui, KeyCode::Right);
        let (screen, _) = tui.render_screen(80, 24);
        let row = label_row(&screen, &format!("{address:06x}:")).unwrap();
        assert_eq!(screen[row][47].1, normal);
        assert_eq!(screen[row][53].1, normal);
        assert_eq!(screen[row][65].1.background, Some(full));

        // Rewinding removes the store preview and restores the load preview.
        for _ in 0..4 {
            key(&mut tui, KeyCode::Left);
        }
        let (screen, _) = tui.render_screen(80, 24);
        let row = label_row(&screen, &format!("{address:06x}:")).unwrap();
        assert_eq!(screen[row][47].1.background, Some(full));
        assert_eq!(screen[row][53].1.background, Some(full));
        assert_ne!(screen[row][59].1.background, Some(full));
    }
}

#[test]
fn upcoming_access_selects_the_memory_pane_when_space_is_short() {
    let mut tui = debugger(true);
    tui.sequence[0].mem_read =
        Some(MemoryValue { address: 0x2000, value: vec![0; 1] });
    tui.sequence[1].mem_read = Some(MemoryValue {
        address: tui.machine.stack_end() - 16,
        value: vec![0; 1],
    });
    let (data, _) = tui.render_screen(80, 16);
    assert!(label_row(&data, "Data").is_some());
    assert!(label_row(&data, "Stack").is_none());

    // The upcoming stack access controls layout before that access executes.
    key(&mut tui, KeyCode::Right);
    let (stack, _) = tui.render_screen(80, 16);
    assert!(label_row(&stack, "Stack").is_some());
    assert!(label_row(&stack, "Data").is_none());
    key(&mut tui, KeyCode::Left);
    let (rewound, _) = tui.render_screen(80, 16);
    assert_eq!(rewound, data);
}

#[test]
fn released_stack_keeps_recent_access_in_dim_gray() {
    let mut tui = debugger(true);
    let address = tui.machine.stack_end() - 16;
    tui.machine.set(SP, address as i32);
    tui.sequence[0].mem_read = Some(MemoryValue { address, value: vec![0; 4] });
    let (active, _) = tui.render_screen(80, 24);
    let row = label_row(&active, &format!("{address:06x}:")).unwrap();
    assert_eq!(active[row][47].1.background, tui.pastels[0].foreground);

    // Releasing the frame preserves focus but gives its bytes inactive gray.
    key(&mut tui, KeyCode::Right);
    tui.machine.set(SP, tui.machine.stack_end() as i32);
    let (released, _) = tui.render_screen(80, 24);
    let row = label_row(&released, &format!("{address:06x}:")).unwrap();
    let dim = Colors::new(Color::AnsiValue(16), Color::AnsiValue(240));
    assert_eq!(released[row][47].1, dim);
    assert_eq!(released[row][71].1, dim);
    assert_eq!(released[row][59].1, tui.inactive_stack_color);
}

#[test]
fn stack_positions_one_instruction_earlier_and_keeps_its_fallback() {
    let mut tui = debugger(true);
    let address = tui.machine.stack_end() - 128;
    tui.sequence[0].mem_read = Some(MemoryValue { address, value: vec![0; 4] });
    let (preview, _) = tui.render_screen(80, 24);
    let row = label_row(&preview, &format!("{address:06x}:")).unwrap();
    let stack_label = label_row(&preview, "Stack").unwrap();
    assert_eq!(row, stack_label + 7);

    // After the load, an instruction without memory effects keeps the same
    // viewport and full highlight that were already shown in the preview.
    key(&mut tui, KeyCode::Right);
    let (after, _) = tui.render_screen(80, 24);
    assert_eq!(label_row(&after, &format!("{address:06x}:")), Some(row));
    assert_eq!(preview[row][47..79], after[row][47..79]);
}

#[test]
fn viewport_centers_context_and_shifts_at_segment_edges() {
    assert_eq!(calc_range(100, 50, 7), (47, 54));
    assert_eq!(calc_range(100, 0, 7), (0, 7));
    assert_eq!(calc_range(100, 99, 7), (93, 100));
    assert_eq!(calc_range(3, 0, 7), (-3, 4));
    assert_eq!(calc_range(3, 2, 7), (-1, 6));
}

// Long synthetic traces exercise placement independently of instruction decoding.
// Addresses are rows relative to the stack segment, just like viewport results.
fn stack_trace(rows: &[Option<u32>], index: usize) -> Tui {
    let mut tui = debugger(true);
    let base = tui.machine.stack_start();
    tui.sequence = rows
        .iter()
        .map(|row| {
            let mut effect = Effects::new(&tui.instructions[0]);
            effect.pc = (0x1000, 0x1000);
            effect.mem_read = row.map(|row| MemoryValue {
                address: base + row * 8,
                value: vec![0; 4],
            });
            effect
        })
        .collect();
    tui.sequence_index = index;
    tui.machine.set_most_recent_memory(&tui.sequence, index);
    tui
}

#[test]
fn array_scan_keeps_visible_values_stationary_in_both_directions() {
    let rows: Vec<_> = (100..110).map(Some).collect();
    let mut tui = stack_trace(&rows, 0);
    assert_eq!(tui.memory_viewport(Stack, 7), 100);
    for _ in 0..6 {
        key(&mut tui, KeyCode::Right);
        tui.machine.set_most_recent_memory(&tui.sequence, tui.sequence_index);
        assert_eq!(tui.memory_viewport(Stack, 7), 100);
    }
    key(&mut tui, KeyCode::Right);
    tui.machine.set_most_recent_memory(&tui.sequence, tui.sequence_index);
    assert_eq!(tui.memory_viewport(Stack, 7), 103);
    key(&mut tui, KeyCode::Left);
    tui.machine.set_most_recent_memory(&tui.sequence, tui.sequence_index);
    assert_eq!(tui.memory_viewport(Stack, 7), 103);
}

#[test]
fn dovetail_considers_two_future_instructions_before_one_past_instruction() {
    let mut tui = stack_trace(
        &[None, Some(98), Some(100), Some(101), Some(102), Some(103)],
        2,
    );
    assert_eq!(tui.memory_viewport(Stack, 5), 98);
}

#[test]
fn overflowing_future_still_allows_recent_past_values() {
    let mut tui = stack_trace(
        &[Some(97), Some(98), Some(99), Some(100), Some(130), Some(96)],
        3,
    );
    assert_eq!(tui.memory_viewport(Stack, 5), 97);
}

#[test]
fn copy_accesses_outrank_surrounding_regions_and_include_current_write() {
    let mut tui = stack_trace(&[Some(100), Some(101)], 0);
    let address = tui.machine.stack_start() + 104 * 8;
    tui.sequence[0].mem_write = Some(MemoryWrite {
        address,
        old_value: vec![0; 4],
        new_value: vec![1; 4],
    });
    assert_eq!(tui.memory_viewport(Stack, 5), 100);
}

#[test]
fn partial_terminal_value_uses_space_without_displacing_complete_values() {
    let mut tui = stack_trace(&[Some(100), Some(103)], 0);
    tui.sequence[1].mem_read.as_mut().unwrap().value = vec![0; 32];
    assert_eq!(tui.memory_viewport(Stack, 5), 100);
}

#[test]
fn unaligned_highlight_that_fills_pane_anchors_its_start() {
    let mut tui = stack_trace(&[Some(100)], 0);
    let value = tui.sequence[0].mem_read.as_mut().unwrap();
    value.address += 7;
    value.value = vec![0; 10];
    tui.machine.set_most_recent_memory(&tui.sequence, 0);
    assert_eq!(tui.memory_viewport(Stack, 3), 100);
    assert_eq!(tui.memory_viewport(Stack, 2), 100);
    assert_eq!(tui.memory_viewport(Stack, 2), 100);
}

#[test]
fn short_segments_lock_alignment_across_accesses() {
    let mut tui = debugger(true);
    for address in [0x2000, 0x2010, 0x2008] {
        tui.sequence[0].mem_read =
            Some(MemoryValue { address, value: vec![0] });
        tui.machine.set_most_recent_memory(&tui.sequence, 0);
        assert_eq!(tui.memory_viewport(Data, 8), -2);
        assert_eq!(tui.memory_viewport(Text, 8), 0);
    }
    let stack_rows =
        ((tui.machine.stack_end() - tui.machine.stack_start()) / 8) as u16;
    assert_eq!(tui.memory_viewport(Stack, stack_rows + 3), -3);
}

#[test]
fn startup_looks_ahead_once_and_counts_instructions_without_accesses() {
    let mut rows = vec![None; 1002];
    rows[1000] = Some(100);
    let mut tui = stack_trace(&rows, 0);
    assert_eq!(tui.memory_viewport(Stack, 7), 100);
    assert_eq!(tui.memory_viewport(Stack, 7), 100);

    rows[1000] = None;
    rows[1001] = Some(100);
    let mut tui = stack_trace(&rows, 0);
    let count = (tui.machine.stack_end() - tui.machine.stack_start()) / 8;
    assert_eq!(tui.memory_viewport(Stack, 7), i64::from(count) - 7);
    assert_eq!(tui.memory_viewport(Data, 2), 0);
}

#[test]
fn future_frame_boundaries_expand_the_region_around_its_access() {
    let mut tui = stack_trace(&[None, Some(100)], 0);
    let base = tui.machine.stack_start();
    let top = base + 104 * 8;
    tui.machine.set(SP, top as i32);
    tui.sequence[0].frame_change = Some(FrameChange::Enter(top));
    tui.sequence[0].reg_write = Some(RegisterWrite {
        register: SP,
        old_value: top as i32,
        new_value: (base + 99 * 8) as i32,
    });
    assert_eq!(tui.memory_viewport(Stack, 7), 99);
    assert!(tui.machine.stack_frames().is_empty());
    assert_eq!(tui.machine.get(SP), top as i32);
}

#[test]
fn past_access_uses_the_frame_that_existed_before_return() {
    let mut tui = stack_trace(&[Some(100), None], 1);
    let base = tui.machine.stack_start();
    let top = base + 104 * 8;
    tui.machine.set(SP, top as i32);
    tui.sequence[0].frame_change = Some(FrameChange::Leave(top));
    tui.sequence[0].reg_write = Some(RegisterWrite {
        register: SP,
        old_value: (base + 99 * 8) as i32,
        new_value: top as i32,
    });
    assert_eq!(tui.memory_viewport(Stack, 7), 99);
    assert!(tui.machine.stack_frames().is_empty());
}

#[test]
fn pane_height_changes_replan_even_when_highlight_stays_visible() {
    let mut tui = stack_trace(&[Some(100), Some(104), Some(108)], 0);
    assert_eq!(tui.memory_viewport(Stack, 5), 100);
    assert_eq!(tui.memory_viewport(Stack, 10), 100);
    assert_eq!(tui.memory_viewport(Stack, 3), 100);
}

#[test]
fn backward_scan_stops_at_five_hundred_instructions() {
    let mut rows = vec![None; 601];
    rows[600] = Some(100);
    rows[100] = Some(96);
    rows[99] = Some(95);
    let mut tui = stack_trace(&rows, 600);
    assert_eq!(tui.memory_viewport(Stack, 5), 96);

    rows[100] = None;
    let mut tui = stack_trace(&rows, 600);
    assert_eq!(tui.memory_viewport(Stack, 5), 100);
}

// Larger static segments leave room to distinguish value and region priorities.
fn static_region_debugger() -> Tui {
    let mut tui = debugger(true);
    tui.machine = Machine::new(
        vec![
            Segment::new(0x1000, 0x1400, false, true, vec![0; 1024]),
            Segment::new(0x2000, 0x2400, true, false, vec![0; 1024]),
        ],
        0x1000,
        0,
        HashMap::new(),
        HashMap::new(),
    );
    let color = tui.normal_color;
    tui.data_colors = vec![(0x2000 + 20 * 8, color), (0x2000 + 24 * 8, color)];
    tui.text_colors = vec![(0x1000 + 20 * 8, color), (0x1000 + 24 * 8, color)];
    tui.sequence.truncate(1);
    tui
}

#[test]
fn data_centers_complete_region_with_odd_spare_row_above() {
    let mut tui = static_region_debugger();
    tui.sequence[0].mem_read =
        Some(MemoryValue { address: 0x2000 + 21 * 8, value: vec![0; 4] });
    tui.machine.set_most_recent_memory(&tui.sequence, 0);
    assert_eq!(tui.memory_viewport(Data, 7), 19);

    // Expanding the window recomputes region placement rather than value centering.
    assert_eq!(tui.memory_viewport(Data, 8), 18);
}

#[test]
fn text_centers_function_and_clamps_at_segment_start() {
    let mut tui = static_region_debugger();
    let mut instruction = Effects::new(&tui.instructions[0]);
    let original = &tui.instructions[0];
    instruction.instruction = Rc::new(Instruction {
        address: 0x1000 + 21 * 8,
        encoding: original.encoding,
        length: original.length,
        pseudo_index: original.pseudo_index,
        verbose_fields: original.op.to_fields(),
        pseudo_fields: original.op.to_pseudo_fields(),
        op: original.op.clone(),
    });
    tui.sequence = vec![instruction];
    assert_eq!(tui.memory_viewport(Text, 8), 18);

    tui.sequence[0].instruction = tui.instructions[0].clone();
    tui.text_colors =
        vec![(0x1000, tui.normal_color), (0x1010, tui.normal_color)];
    assert_eq!(tui.memory_viewport(Text, 8), 0);
}

#[test]
fn earlier_regions_take_priority_when_both_cannot_fit() {
    let mut tui = static_region_debugger();
    let color = tui.normal_color;
    tui.data_colors =
        [0, 20, 24, 30].map(|row| (0x2000 + row * 8, color)).to_vec();
    tui.sequence[0].mem_read =
        Some(MemoryValue { address: 0x2000 + 23 * 8, value: vec![0; 4] });
    let mut next = Effects::new(&tui.instructions[1]);
    next.mem_read =
        Some(MemoryValue { address: 0x2000 + 24 * 8, value: vec![0; 4] });
    tui.sequence.push(next);
    tui.machine.set_most_recent_memory(&tui.sequence, 0);
    assert_eq!(tui.memory_viewport(Data, 7), 20);
}

#[test]
fn screen_resize_replans_even_if_memory_pane_height_is_unchanged() {
    let mut tui = stack_trace(&[Some(100), Some(104)], 0);
    tui.render_screen(80, 24);
    assert_eq!(tui.memory_viewport(Stack, 7), 100);
    tui.sequence[0].mem_read = None;
    key(&mut tui, KeyCode::Right);
    tui.render_screen(80, 24);
    assert_eq!(tui.memory_viewport(Stack, 7), 100);
    tui.render_screen(81, 24);
    assert_eq!(tui.memory_viewport(Stack, 7), 104);
}

#[test]
fn help_lists_all_toggles_and_dismisses_without_toggling() {
    let mut tui = debugger(true);
    key(&mut tui, KeyCode::Char('?'));
    let (screen, _) = tui.render_screen(80, 24);
    assert!(
        label_row(&screen, "(r)egisters, (o)utput, (s)tack, (d)ata, (t)ext")
            .is_some()
    );
    key(&mut tui, KeyCode::Char('t'));
    assert!(!tui.show_help);
    assert!(tui.show_text);
}
