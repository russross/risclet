use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Colors};

use super::MemoryPane::{Data, Stack, Text};
use super::{
    MemoryVisibility, Screen, Tui, calc_range, function_colors, memory_color,
    memory_layout, stack_regions,
};
use crate::config::{Config, Mode};
use crate::execution::{Instruction, Machine};
use crate::memory::Segment;
use crate::riscv::Op;
use crate::trace::{Effects, MemoryValue};

const ALL: MemoryVisibility =
    MemoryVisibility { stack: true, data: true, text: true };

// Use mixed instruction lengths and a partial final memory row. The first two
// instructions share a source pseudoinstruction but remain distinct in memory.
fn debugger(data: bool) -> Tui {
    let mut instructions = Vec::new();
    let mut bytes = Vec::new();
    for (index, length) in [2, 4, 4, 2, 4, 2].into_iter().enumerate() {
        let encoding: u32 = if length == 2 { 1 } else { 0x13 };
        let op = Op::new(encoding as i32);
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

    // Named text symbols change colors; numeric and non-text symbols do not.
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
        &Config::simulator_default(Mode::Debug),
    )
}

fn key(tui: &mut Tui, code: KeyCode) {
    assert!(
        !tui.handle_key(KeyEvent::new(code, KeyModifiers::NONE), 12).unwrap()
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
    tui.sequence[0].extra_mut().stdout = Some(b"hello\n".to_vec());
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
    tui.sequence[0].extra_mut().stdout = Some(b"line\n".repeat(30));
    tui.machine.apply(&tui.sequence[0], true);
    assert_eq!(tui.render_screen(80, 25).1, 12);
    tui.show_registers = false;
    let (screen, source) = tui.render_screen(80, 24);
    assert_eq!(source, 12);
    assert_eq!(label_row(&screen, " Output "), Some(13));
    tui.show_output = false;
    assert_eq!(tui.render_screen(80, 24).1, 22);
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
fn unnamed_text_stays_neutral_and_function_colors_wrap() {
    let tui = debugger(true);
    let no_names = HashMap::from([(0x1002, "1".to_owned())]);
    let colors = function_colors(
        &no_names,
        &tui.instructions,
        &tui.pastels,
        tui.normal_color,
    );
    assert_eq!(memory_color(&colors, 0x1006, 0..0), tui.normal_color);

    // An unlabeled prefix remains neutral; only instruction labels count.
    let names = HashMap::from([
        (0x1002, "first".to_owned()),
        (0x1006, "second".to_owned()),
        (0x100a, "third".to_owned()),
        (0x100b, "not_an_instruction".to_owned()),
    ]);
    let colors = function_colors(
        &names,
        &tui.instructions,
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
fn viewport_centers_context_and_shifts_at_segment_edges() {
    assert_eq!(calc_range(100, 50, 7), (47, 54));
    assert_eq!(calc_range(100, 0, 7), (0, 7));
    assert_eq!(calc_range(100, 99, 7), (93, 100));
    assert_eq!(calc_range(3, 0, 7), (-3, 4));
    assert_eq!(calc_range(3, 2, 7), (-1, 6));
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
