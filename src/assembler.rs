// assembler.rs
//
// Core assembly pipeline functions shared between main.rs and tests

use crate::ast::Source;
use crate::encoder::{Relax, encode_source};
use crate::error::{AssemblerError, Result};
use crate::expressions::{SymbolValues, eval_symbol_values};
use crate::layout::{Layout, compute_offsets};
use crate::symbols::SymbolLinks;

/// Callback trait for per-iteration convergence dumps
///
/// Allows main.rs to inject debug dump logic into the convergence loop
/// without coupling this module to dump implementation details.
#[allow(clippy::too_many_arguments)]
pub trait ConvergenceCallback {
    /// Called after symbol evaluation, before encoding
    fn on_values_computed(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        symbol_values: &SymbolValues,
        layout: &Layout,
    );

    /// Called after encoding
    fn on_code_generated(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        symbol_values: &SymbolValues,
        layout: &Layout,
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
        _: &SymbolValues,
        _: &Layout,
    ) {
    }
    fn on_code_generated(
        &self,
        _: usize,
        _: bool,
        _: &Source,
        _: &SymbolValues,
        _: &Layout,
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
    layout: &mut Layout,
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
        compute_offsets(source, layout);

        if show_progress {
            eprintln!(
                "  {:4}  {:5}  {:6}  {:6}",
                pass_number,
                layout.text_size,
                layout.data_size,
                layout.bss_size
            );
        }

        // Step 2: Compute segment addresses and calculate all symbol values upfront
        layout.set_segment_addresses(text_start);
        let symbol_values = eval_symbol_values(source, symbol_links, layout)?;

        // Callback: after symbol values computed
        callback.on_values_computed(
            pass_number,
            false,
            source,
            &symbol_values,
            layout,
        );

        // Step 3: Encode everything and update line sizes
        // Track if any size changed
        let mut any_changed = false;

        // Encode and collect results
        let encode_result = encode_source(
            source,
            &symbol_values,
            symbol_links,
            layout,
            relax,
            &mut any_changed,
        );

        let (text_bytes, data_bytes, bss_size) = encode_result?;

        // Callback: after code generated
        callback.on_code_generated(
            pass_number,
            !any_changed, // is_final if no changes
            source,
            &symbol_values,
            layout,
            &text_bytes,
            &data_bytes,
        );

        // Step 4: Check convergence
        if !any_changed {
            // Converged! Call final value callback
            callback.on_values_computed(
                pass_number,
                true,
                source,
                &symbol_values,
                layout,
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
