// assembler.rs
//
// Core assembly pipeline functions shared between main.rs and tests

use crate::ast::Source;
use crate::config::Config;
use crate::dump;
use crate::encoder::encode_source;
use crate::error::{AssemblerError, Result};
use crate::expressions::eval_symbol_values;
use crate::layout::{Layout, compute_offsets};
use crate::symbols::SymbolLinks;

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
pub fn converge_and_encode(
    source: &mut Source,
    symbol_links: &SymbolLinks,
    layout: &mut Layout,
    config: &Config,
) -> Result<(Vec<u8>, Vec<u8>, u32)> {
    const MAX_ITERATIONS: usize = 10;

    if config.verbose {
        eprintln!("Convergence:");
        eprintln!("  Pass   Text    Data     BSS");
        eprintln!("  ----  -----  ------  ------");
    }

    for iteration in 0..MAX_ITERATIONS {
        let pass_number = iteration + 1;

        // Step 1: Calculate addresses based on current size guesses
        compute_offsets(source, layout);

        if config.verbose {
            eprintln!(
                "  {:4}  {:5}  {:6}  {:6}",
                pass_number,
                layout.text_size,
                layout.data_size,
                layout.bss_size
            );
        }

        // Step 2: Compute segment addresses and calculate all symbol values upfront
        layout.set_segment_addresses(config.text_start);
        let symbol_values = eval_symbol_values(source, symbol_links, layout)?;

        // Dump symbol values if requested
        if let Some(ref spec) = config.dump.dump_values {
            dump::dump_values(
                pass_number,
                false,
                source,
                &symbol_values,
                layout,
                spec,
            );
        }

        // Step 3: Encode everything and update line sizes
        // Track if any size changed
        let mut any_changed = false;

        // Encode and collect results
        let encode_result = encode_source(
            source,
            &symbol_values,
            symbol_links,
            layout,
            config,
            &mut any_changed,
        );

        let (text_bytes, data_bytes, bss_size) = encode_result?;

        // Dump generated code if requested
        if let Some(ref spec) = config.dump.dump_code {
            dump::dump_code(
                pass_number,
                !any_changed, // is_final if no changes
                source,
                &symbol_values,
                layout,
                &text_bytes,
                &data_bytes,
                spec,
            );
        }

        // Step 4: Check convergence
        if !any_changed {
            // Converged! Dump final symbol values if requested
            if let Some(ref spec) = config.dump.dump_values {
                dump::dump_values(
                    pass_number,
                    true,
                    source,
                    &symbol_values,
                    layout,
                    spec,
                );
            }
            if config.verbose {
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
