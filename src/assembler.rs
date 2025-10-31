// assembler.rs
//
// Core assembly pipeline functions shared between main.rs and tests

use crate::ast::{self, Instruction, LinePointer, PseudoOp, Source};
use crate::encoder::{Relax, encode_source};
use crate::error::{AssemblerError, Result};
use crate::expressions;
use crate::symbols::SymbolLinks;

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
    symbol_links: &SymbolLinks,
    layout: &mut crate::layout::Layout,
    text_start: u32,
    relax: &Relax,
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
        crate::layout::compute_offsets(source, layout);

        if show_progress {
            eprintln!(
                "  {:4}  {:5}  {:6}  {:6}",
                pass_number,
                layout.text_size,
                layout.data_size,
                layout.bss_size
            );
        }

        // Step 2 & 3: Calculate symbol values and evaluate expressions
        let mut eval_context = expressions::new_evaluation_context(
            source.clone(),
            symbol_links.clone(),
            layout.clone(),
            text_start,
        );

        // Evaluate all line symbols to populate the expression evaluation context
        for (file_index, file) in source.files.iter().enumerate() {
            for (line_index, line) in file.lines.iter().enumerate() {
                let pointer = LinePointer { file_index, line_index };
                expressions::evaluate_line_symbols(
                    line,
                    &pointer,
                    &mut eval_context,
                )?;
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
        let encode_result =
            encode_source(source, &mut eval_context, layout, relax, &mut any_changed);

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
