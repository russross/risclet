// assembler.rs
//
// Core assembly pipeline functions shared between main.rs and tests

use crate::ast::{self, Directive, LineContent, Segment, Source};
use crate::encoder::encode_source_with_size_tracking;
use crate::expressions;

/// Compute offsets for all lines in the source
///
/// This assigns each line an offset within its segment based on the
/// current size guesses for all preceding lines.
pub fn compute_offsets(source: &mut Source) {
    let mut global_text_offset: i64 = 0;
    let mut global_data_offset: i64 = 0;
    let mut global_bss_offset: i64 = 0;

    for source_file in &mut source.files {
        // Track the starting offset for this file in each segment
        let file_text_start = global_text_offset;
        let file_data_start = global_data_offset;
        let file_bss_start = global_bss_offset;

        // Track offsets within this file (continuing from global offsets)
        let mut text_offset: i64 = global_text_offset;
        let mut data_offset: i64 = global_data_offset;
        let mut bss_offset: i64 = global_bss_offset;

        for line in &mut source_file.lines {
            let current_offset = match line.segment {
                Segment::Text => text_offset,
                Segment::Data => data_offset,
                Segment::Bss => bss_offset,
            };

            // For .balign, compute actual size based on offset
            if let LineContent::Directive(Directive::Balign(_expr)) = &line.content {
                let align = 8; // Placeholder: should evaluate expr
                let padding = (align - (current_offset % align)) % align;
                line.size = padding;
            }

            line.offset = current_offset;

            // Advance offset
            let advance = line.size;
            match line.segment {
                Segment::Text => text_offset += advance,
                Segment::Data => data_offset += advance,
                Segment::Bss => bss_offset += advance,
            }
        }

        // Update source_file sizes (size contributed by this file in each segment)
        source_file.text_size = text_offset - file_text_start;
        source_file.data_size = data_offset - file_data_start;
        source_file.bss_size = bss_offset - file_bss_start;

        // Update global offsets to continue in the next file
        global_text_offset = text_offset;
        global_data_offset = data_offset;
        global_bss_offset = bss_offset;
    }

    // Update source sizes (total across all files)
    source.text_size = global_text_offset;
    source.data_size = global_data_offset;
    source.bss_size = global_bss_offset;
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
pub fn converge_and_encode(
    source: &mut Source,
    text_start: i64,
) -> Result<(Vec<u8>, Vec<u8>, i64), String> {
    const MAX_ITERATIONS: usize = 10;

    for _iteration in 0..MAX_ITERATIONS {
        // Step 1: Calculate addresses based on current size guesses
        compute_offsets(source);

        // Step 2 & 3: Calculate symbol values and evaluate expressions
        let mut eval_context =
            expressions::new_evaluation_context(source.clone(), text_start);

        // Evaluate all line symbols to populate the expression evaluation context
        for file in &source.files {
            for line in &file.lines {
                // Ignore errors - some expressions may not be evaluable yet
                let _ = expressions::evaluate_line_symbols(line, &mut eval_context);
            }
        }

        // Step 4: Encode everything and update line sizes
        // Track if any size changed
        let mut any_changed = false;

        // Encode and collect results
        let encode_result = encode_source_with_size_tracking(
            source,
            &mut eval_context,
            &mut any_changed,
        );

        // Step 5: Check convergence
        if !any_changed {
            // Converged! Return the encoded result
            return encode_result.map_err(|e| format!("Encode error: {}", e));
        }

        // Sizes changed, discard encoded data and loop again
        // (The encoder already updated source.lines[].size)
    }

    Err(format!(
        "Failed to converge after {} iterations - possible cyclic size dependencies",
        MAX_ITERATIONS
    ))
}

/// Initial size guess for a line before convergence
///
/// Returns a conservative estimate that will be refined during convergence.
pub fn guess_line_size(content: &ast::LineContent) -> Result<i64, String> {
    use ast::{Instruction, PseudoOp};

    match content {
        ast::LineContent::Instruction(inst) => match inst {
            // Pseudo-instructions that expand to 2 base instructions (8 bytes)
            Instruction::Pseudo(pseudo) => match pseudo {
                PseudoOp::Li(_,_) => Ok(8),  // lui + addiw (worst case)
                PseudoOp::La(_, _) => Ok(8), // auipc + addi
                PseudoOp::Call(_) => Ok(8),  // auipc + jalr
                PseudoOp::Tail(_) => Ok(8),  // auipc + jalr
                PseudoOp::LoadGlobal(_, _, _) => Ok(8), // auipc + load
                PseudoOp::StoreGlobal(_, _, _, _) => Ok(8), // auipc + store
            },
            // All other instructions are 4 bytes
            _ => Ok(4),
        },
        ast::LineContent::Label(_) => Ok(0),
        ast::LineContent::Directive(dir) => match dir {
            ast::Directive::Space(_expr) => {
                // Placeholder: in later phases, evaluate expression
                Ok(0)
            }
            ast::Directive::Balign(_expr) => {
                // Size computed in compute_offsets
                Ok(0)
            }
            ast::Directive::Byte(exprs) => Ok(exprs.len() as i64),
            ast::Directive::TwoByte(exprs) => Ok(exprs.len() as i64 * 2),
            ast::Directive::FourByte(exprs) => Ok(exprs.len() as i64 * 4),
            ast::Directive::EightByte(exprs) => Ok(exprs.len() as i64 * 8),
            ast::Directive::String(strings) => {
                let mut size = 0;
                for s in strings {
                    size += s.len() as i64;
                }
                Ok(size)
            }
            ast::Directive::Asciz(strings) => {
                let mut size = 0;
                for s in strings {
                    size += s.len() as i64 + 1; // +1 for null terminator
                }
                Ok(size)
            }
            _ => Ok(0), // Non-data directives like .text, .global
        },
    }
}
