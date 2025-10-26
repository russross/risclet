// assembler.rs
//
// Core assembly pipeline functions shared between main.rs and tests

use crate::ast::{
    self, Directive, Instruction, LineContent, PseudoOp, Segment, Source,
};
use crate::elf::compute_header_size;
use crate::encoder::encode_source_with_size_tracking;
use crate::error::{AssemblerError, Result};
use crate::expressions;

/// Compute offsets for all lines in the source
///
/// This assigns each line an offset within its segment based on the
/// current size guesses for all preceding lines.
pub fn compute_offsets(source: &mut Source) {
    let mut global_text_offset: u32 = 0;
    let mut global_data_offset: u32 = 0;
    let mut global_bss_offset: u32 = 0;

    for source_file in &mut source.files {
        // Track the starting offset for this file in each segment
        let file_text_start = global_text_offset;
        let file_data_start = global_data_offset;
        let file_bss_start = global_bss_offset;

        // Track offsets within this file (continuing from global offsets)
        let mut text_offset: u32 = global_text_offset;
        let mut data_offset: u32 = global_data_offset;
        let mut bss_offset: u32 = global_bss_offset;

        for line in &mut source_file.lines {
            let current_offset = match line.segment {
                Segment::Text => text_offset,
                Segment::Data => data_offset,
                Segment::Bss => bss_offset,
            };

            // For .balign, compute actual size based on offset
            if let LineContent::Directive(Directive::Balign(_expr)) =
                &line.content
            {
                let align = 8; // Placeholder: should evaluate expr
                let padding = (align - (current_offset % align)) % align;
                line.size = padding;
            }

            line.offset = current_offset;

            // Advance offset
            let advance = line.size;
            match line.segment {
                Segment::Text => {
                    text_offset = text_offset.wrapping_add(advance)
                }
                Segment::Data => {
                    data_offset = data_offset.wrapping_add(advance)
                }
                Segment::Bss => bss_offset = bss_offset.wrapping_add(advance),
            }
        }

        // Update source_file sizes (size contributed by this file in each segment)
        source_file.text_size = text_offset.wrapping_sub(file_text_start);
        source_file.data_size = data_offset.wrapping_sub(file_data_start);
        source_file.bss_size = bss_offset.wrapping_sub(file_bss_start);

        // Update global offsets to continue in the next file
        global_text_offset = text_offset;
        global_data_offset = data_offset;
        global_bss_offset = bss_offset;
    }

    // Update source sizes (total across all files)
    source.text_size = global_text_offset;
    source.data_size = global_data_offset;
    source.bss_size = global_bss_offset;

    // Update header size estimate
    // Segments: .text, .riscv.attributes, and optionally .data/.bss
    let has_data_or_bss = source.data_size > 0 || source.bss_size > 0;
    let num_segments = if has_data_or_bss { 3 } else { 2 };
    source.header_size = compute_header_size(num_segments) as u32;
}

/// Callback trait for per-iteration convergence dumps
///
/// Allows main.rs to inject debug dump logic into the convergence loop
/// without coupling this module to dump implementation details.
pub trait ConvergenceCallback {
    /// Called after symbol evaluation, before encoding
    fn on_values_computed(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        eval_context: &mut expressions::EvaluationContext,
    );

    /// Called after encoding
    fn on_code_generated(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        eval_context: &mut expressions::EvaluationContext,
        text_bytes: &[u8],
        data_bytes: &[u8],
    );
}

/// No-op callback for production use (no debug dumps)
pub struct NoOpCallback;

impl ConvergenceCallback for NoOpCallback {
    fn on_values_computed(
        &self,
        _: usize,
        _: bool,
        _: &Source,
        _: &mut expressions::EvaluationContext,
    ) {
    }
    fn on_code_generated(
        &self,
        _: usize,
        _: bool,
        _: &Source,
        _: &mut expressions::EvaluationContext,
        _: &[u8],
        _: &[u8],
    ) {
    }
}

/// Converge line sizes and offsets by iterating until stable
///
/// This function repeatedly:
/// 1. Computes addresses/offsets based on current size guesses
/// 2. Computes symbol values (addresses of labels)
/// 3. Evaluates expressions (which depend on symbol values/offsets)
/// 4. Encodes/generates code, which determines actual instruction sizes
/// 5. Checks if any sizes changed from the guess
///
/// Loops until convergence (no size changes) or max iterations reached.
/// Returns Ok((text, data, bss_size)) on convergence with the final encoding.
///
/// The optional `callback` parameter allows injection of debug dump logic
/// at specific points in the convergence loop.
///
/// If `show_progress` is true, prints convergence progress to stderr.
pub fn converge_and_encode<C: ConvergenceCallback>(
    source: &mut Source,
    text_start: u32,
    callback: &C,
    show_progress: bool,
) -> Result<(Vec<u8>, Vec<u8>, u32)> {
    const MAX_ITERATIONS: usize = 10;

    if show_progress {
        eprintln!("Convergence:");
        eprintln!("  Pass   Text    Data     BSS");
        eprintln!("  ----  -----  ------  ------");
    }

    for iteration in 0..MAX_ITERATIONS {
        let pass_number = iteration + 1;

        // Step 1: Calculate addresses based on current size guesses
        compute_offsets(source);

        if show_progress {
            eprintln!(
                "  {:4}  {:5}  {:6}  {:6}",
                pass_number,
                source.text_size,
                source.data_size,
                source.bss_size
            );
        }

        // Step 2 & 3: Calculate symbol values and evaluate expressions
        let mut eval_context =
            expressions::new_evaluation_context(source.clone(), text_start);

        // Evaluate all line symbols to populate the expression evaluation context
        for file in &source.files {
            for line in &file.lines {
                expressions::evaluate_line_symbols(line, &mut eval_context)?;
            }
        }

        // Callback: after symbol values computed
        callback.on_values_computed(
            pass_number,
            false,
            source,
            &mut eval_context,
        );

        // Step 4: Encode everything and update line sizes
        // Track if any size changed
        let mut any_changed = false;

        // Encode and collect results
        let encode_result = encode_source_with_size_tracking(
            source,
            &mut eval_context,
            &mut any_changed,
        );

        let (text_bytes, data_bytes, bss_size) = encode_result?;

        // Callback: after code generated
        callback.on_code_generated(
            pass_number,
            !any_changed, // is_final if no changes
            source,
            &mut eval_context,
            &text_bytes,
            &data_bytes,
        );

        // Step 5: Check convergence
        if !any_changed {
            // Converged! Call final value callback
            callback.on_values_computed(
                pass_number,
                true,
                source,
                &mut eval_context,
            );
            if show_progress {
                eprintln!(
                    "  Converged after {} pass{}",
                    pass_number,
                    if pass_number == 1 { "" } else { "es" }
                );
            }
            return Ok((text_bytes, data_bytes, bss_size));
        }

        // Sizes changed, discard encoded data and loop again
        // (The encoder already updated source.lines[].size)
    }

    Err(AssemblerError::no_context(format!(
        "Failed to converge after {} iterations - possible cyclic size dependencies",
        MAX_ITERATIONS
    )))
}

/// Initial size guess for a line before convergence
///
/// Returns a conservative estimate that will be refined during convergence.
pub fn guess_line_size(content: &ast::LineContent) -> u32 {
    (match content {
        ast::LineContent::Instruction(inst) => match inst {
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
        ast::LineContent::Label(_) => 0,
        ast::LineContent::Directive(dir) => match dir {
            ast::Directive::Space(_expr) => 0,
            ast::Directive::Balign(_expr) => 0,
            ast::Directive::Byte(exprs) => exprs.len(),
            ast::Directive::TwoByte(exprs) => exprs.len() * 2,
            ast::Directive::FourByte(exprs) => exprs.len() * 4,
            ast::Directive::String(strings) => {
                strings.iter().map(|s| s.len()).sum()
            }
            ast::Directive::Asciz(strings) => {
                strings.iter().map(|s| s.len() + 1).sum()
            }
            _ => 0, // Non-data directives like .text, .global
        },
    }) as u32
}
