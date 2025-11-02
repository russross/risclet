// layout.rs
//
// This module manages address layout information for the assembled program.
// It separates layout concerns (segment assignments, offsets, sizes) from the AST,
// creating a clear data flow: Parsing → AST, Linking → SymbolLinks, Layout Computation → Layout.

use crate::ast::{
    Directive, Instruction, LineContent, LinePointer, PseudoOp, Segment, Source,
};
use crate::elf::compute_header_size;
use crate::symbols::{BUILTIN_FILE_NAME, SPECIAL_GLOBAL_POINTER};
use std::collections::HashMap;

/// Information about a line's position and size in the binary
///
/// This is stored in the Layout structure and represents where a particular
/// source line ends up in the assembled binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineLayout {
    /// Which segment this line belongs to (text, data, or bss)
    pub segment: Segment,
    /// Offset within the segment (in bytes)
    pub offset: u32,
    /// Size in bytes (guessed initially)
    pub size: u32,
}

/// Complete layout information for the assembled program
///
/// Created after symbol linking and updated during relaxation iterations.
/// This structure replaces the scattered layout fields that used to live in
/// Source, SourceFile, and Line.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Per-line layout information: maps each line to its position and size
    pub lines: HashMap<LinePointer, LineLayout>,

    /// ELF header size (depends on number of segments)
    pub header_size: u32,

    /// Total size of each segment
    pub text_size: u32,
    pub data_size: u32,
    pub bss_size: u32,

    /// Computed segment start addresses in the final binary
    /// These are calculated from the nominal text_start and layout sizes
    pub text_start: u32,
    pub data_start: u32,
    pub bss_start: u32,
}

impl Layout {
    /// Create a new empty layout
    pub fn new() -> Self {
        Layout {
            lines: HashMap::new(),
            header_size: 0,
            text_size: 0,
            data_size: 0,
            bss_size: 0,
            text_start: 0,
            data_start: 0,
            bss_start: 0,
        }
    }

    /// Get layout info for a specific line (panics if not found)
    pub fn get(&self, pointer: LinePointer) -> &LineLayout {
        self.lines.get(&pointer).unwrap()
    }

    /// Set layout info for a specific line
    pub fn set(&mut self, pointer: LinePointer, layout: LineLayout) {
        self.lines.insert(pointer, layout);
    }

    /// Set segment start addresses based on nominal text_start
    ///
    /// This computes the concrete segment start addresses in the final binary:
    /// - **text_start**: Adjusted to account for ELF header before text segment
    /// - **data_start**: Aligned to 4K boundary after text segment
    /// - **bss_start**: Immediately after data segment
    pub fn set_segment_addresses(&mut self, nominal_text_start: u32) {
        // Adjust text_start to account for ELF header
        self.text_start = nominal_text_start + self.header_size;

        // Align data_start to 4K boundary after text segment
        let text_end = self.text_start + self.text_size;
        self.data_start = (text_end + 4095) & !(4096 - 1);

        // BSS starts immediately after data
        self.bss_start = self.data_start + self.data_size;
    }

    /// Compute the concrete address of a line in the final binary
    ///
    /// Given a line pointer, returns the absolute address in the executable where
    /// this line's content will reside, accounting for segment base address and
    /// offset within the segment.
    pub fn get_line_address(&self, pointer: LinePointer) -> u32 {
        let line_layout = self.get(pointer);
        let segment_start = match line_layout.segment {
            Segment::Text => self.text_start,
            Segment::Data => self.data_start,
            Segment::Bss => self.bss_start,
        };
        segment_start + line_layout.offset
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

// ==============================================================================
// Layout Computation Functions
// ==============================================================================

/// Compute the initial size estimate for a line of code/data.
///
/// This provides conservative estimates used during layout computation:
/// - Compressed instructions: 2 bytes
/// - Pseudo-instructions: 8 bytes (worst case before relaxation)
/// - Regular instructions: 4 bytes
/// - Labels: 0 bytes
/// - Directives: varies based on operands
///
/// During relaxation, actual sizes may shrink (e.g., pseudo → 4 bytes, instructions → 2 bytes)
pub fn guess_line_size(content: &LineContent) -> u32 {
    (match content {
        LineContent::Instruction(inst) => match inst {
            // Compressed instructions are always 2 bytes
            Instruction::Compressed(_, _) => 2,

            // Pseudo-instructions that expand to 2 base instructions (8 bytes)
            Instruction::Pseudo(pseudo) => match pseudo {
                PseudoOp::Li(_, _) => 8,                // lui + addi
                PseudoOp::La(_, _) => 8,                // auipc + addi
                PseudoOp::Call(_) => 8,                 // auipc + jalr
                PseudoOp::Tail(_) => 8,                 // auipc + jalr
                PseudoOp::LoadGlobal(_, _, _) => 8,     // auipc + load
                PseudoOp::StoreGlobal(_, _, _, _) => 8, // auipc + store
            },
            // All other instructions start at 4 bytes (may relax to 2 with auto-relaxation)
            _ => 4,
        },
        LineContent::Label(_) => 0,
        LineContent::Directive(dir) => match dir {
            Directive::Space(_expr) => 0,
            Directive::Balign(_expr) => 0,
            Directive::Byte(exprs) => exprs.len(),
            Directive::TwoByte(exprs) => exprs.len() * 2,
            Directive::FourByte(exprs) => exprs.len() * 4,
            Directive::String(strings) => strings.iter().map(|s| s.len()).sum(),
            Directive::Asciz(strings) => {
                strings.iter().map(|s| s.len() + 1).sum()
            }
            _ => 0, // Non-data directives like .text, .global
        },
    }) as u32
}

/// Compute offsets for all lines based on the Layout structure.
///
/// This is the NEW version that works with Layout instead of mutating Source.
/// It reads from `layout.lines` (which contains sizes set by earlier passes)
/// and writes back the computed offsets.
///
/// The segment for each line is computed on-the-fly by tracking directives.
pub fn compute_offsets(source: &Source, layout: &mut Layout) {
    let mut global_text_offset: u32 = 0;
    let mut global_data_offset: u32 = 0;
    let mut global_bss_offset: u32 = 0;

    // Track current segment as we iterate through lines
    let mut current_segment = Segment::Text;

    // Check if there's a builtin symbols file (last file)
    if source.files.is_empty() {
        return;
    }
    let has_builtin = source
        .files
        .last()
        .map(|f| f.file == BUILTIN_FILE_NAME)
        .unwrap_or(false);

    for (file_index, source_file) in source.files.iter().enumerate() {
        let is_builtin_file =
            has_builtin && file_index == source.files.len() - 1;

        // For builtin file, we use special hardcoded offsets (not cumulative)
        // This file contains __global_pointer$ at data segment offset 2048
        if is_builtin_file {
            for (line_index, line) in source_file.lines.iter().enumerate() {
                let pointer = LinePointer { file_index, line_index };

                // Compute segment based on directives
                if let LineContent::Directive(directive) = &line.content {
                    match directive {
                        Directive::Text => current_segment = Segment::Text,
                        Directive::Data => current_segment = Segment::Data,
                        Directive::Bss => current_segment = Segment::Bss,
                        _ => {}
                    }
                }

                let size = guess_line_size(&line.content);

                // Special handling: __global_pointer$ label is at offset 2048 in data segment
                let offset = if let LineContent::Label(name) = &line.content {
                    if name == SPECIAL_GLOBAL_POINTER { 2048 } else { 0 }
                } else {
                    0
                };

                layout.set(
                    pointer,
                    LineLayout { segment: current_segment, offset, size },
                );
            }
            continue;
        }

        // Track offsets within this file (continuing from global offsets)
        let mut text_offset: u32 = global_text_offset;
        let mut data_offset: u32 = global_data_offset;
        let mut bss_offset: u32 = global_bss_offset;

        for (line_index, line) in source_file.lines.iter().enumerate() {
            let pointer = LinePointer { file_index, line_index };

            // Update current segment based on directives
            if let LineContent::Directive(directive) = &line.content {
                match directive {
                    Directive::Text => current_segment = Segment::Text,
                    Directive::Data => current_segment = Segment::Data,
                    Directive::Bss => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            // Get the size: use layout if available (which may have been updated during encoding),
            // otherwise fall back to guess
            let size = layout.get(pointer).size;

            // Compute offset based on current segment
            let offset = match current_segment {
                Segment::Text => text_offset,
                Segment::Data => data_offset,
                Segment::Bss => bss_offset,
            };

            // Update layout with computed offset and segment
            layout.set(
                pointer,
                LineLayout { segment: current_segment, offset, size },
            );

            // Advance offset in the appropriate segment
            match current_segment {
                Segment::Text => text_offset += size,
                Segment::Data => data_offset += size,
                Segment::Bss => bss_offset += size,
            }
        }

        // Update global offsets to continue in the next file
        global_text_offset = text_offset;
        global_data_offset = data_offset;
        global_bss_offset = bss_offset;
    }

    // Update layout segment sizes (total across all files)
    layout.text_size = global_text_offset;
    layout.data_size = global_data_offset;
    layout.bss_size = global_bss_offset;

    // Update header size estimate
    // Segments: .text, .riscv.attributes, and optionally .data/.bss
    let has_data_or_bss = layout.data_size > 0 || layout.bss_size > 0;
    let num_segments = if has_data_or_bss { 3 } else { 2 };
    layout.header_size = compute_header_size(num_segments) as u32;
}

/// Create initial layout after symbol linking is complete.
///
/// This function is called after parsing and symbol linking but before the
/// relaxation loop. It initializes the Layout with:
/// - Initial size guesses for each line
/// - Segment assignments based on directives
/// - Computed offsets for each line
pub fn create_initial_layout(source: &Source) -> Layout {
    let mut layout = Layout::new();

    // Track current segment as we iterate through lines
    let mut current_segment = Segment::Text;

    // Set initial size guesses and segment for all lines
    for (file_index, file) in source.files.iter().enumerate() {
        for (line_index, line) in file.lines.iter().enumerate() {
            let pointer = LinePointer { file_index, line_index };

            // Update current segment based on directives
            if let LineContent::Directive(directive) = &line.content {
                match directive {
                    Directive::Text => current_segment = Segment::Text,
                    Directive::Data => current_segment = Segment::Data,
                    Directive::Bss => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            let size = guess_line_size(&line.content);

            layout.set(
                pointer,
                LineLayout {
                    segment: current_segment,
                    offset: 0, // Will be computed by compute_offsets
                    size,
                },
            );
        }
    }

    // Compute initial offsets and segment sizes
    compute_offsets(source, &mut layout);

    layout
}
