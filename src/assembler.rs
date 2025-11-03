// assembler.rs
//
// Core assembly pipeline: relaxation loop, file input, and statistics

use crate::ast::{Line, LineContent, Location, Source, SourceFile};
use crate::config::Config;
use crate::dump::{dump_ast, dump_code, dump_elf, dump_symbols, dump_values};
use crate::elf_builder::ElfBuilder;
use crate::encoder::encode;
use crate::error::{AssemblerError, Result};
use crate::expressions::eval_symbol_values;
use crate::layout::Layout;
use crate::parser::parse;
use crate::symbols::{SymbolLinks, create_builtin_symbols_file, link_symbols};
use crate::tokenizer::tokenize;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::PermissionsExt;

/// Main assembly driver - processing flow with output dump checkpoints
pub fn drive_assembler(config: &Config) -> Result<()> {
    // ========================================================================
    // Phase 1: Parse source files into AST
    // ========================================================================
    let source = &process_files(&config.input_files)?;

    // Checkpoint: dump AST if requested
    if should_dump_phase(config, Phase::Parse) {
        dump_ast(config, source);
        if is_terminal_phase(config, Phase::Parse) {
            println!("\n(No output file generated)");
            return Ok(());
        }
        println!(); // Separator between phase dumps
    }

    // ========================================================================
    // Phase 2: Link symbols (connect symbol uses to their definitions)
    // ========================================================================
    let symbol_links = &link_symbols(source)?;

    // Checkpoint: dump symbol linking if requested
    if should_dump_phase(config, Phase::SymbolLinking) {
        dump_symbols(config, source, symbol_links);
        if is_terminal_phase(config, Phase::SymbolLinking) {
            println!("\n(No output file generated)");
            return Ok(());
        }
        println!(); // Separator between phase dumps
    }

    // Create initial layout after symbol linking
    let mut layout = Layout::new(source);

    // ========================================================================
    // Phase 3: Relaxation - iteratively compute offsets and encode until stable
    // ========================================================================

    // Show input statistics before relaxation loop if verbose
    if config.verbose {
        print_input_statistics(source, symbol_links);
    }

    let (text_bytes, data_bytes, _bss_size) = relaxation_loop(config, source, symbol_links, &mut layout)?;

    // Checkpoint: after relaxation, check if we should exit before ELF generation
    if should_dump_phase(config, Phase::Relaxation) && is_terminal_phase(config, Phase::Relaxation) {
        println!("\n(No output file generated)");
        return Ok(());
    }

    // ========================================================================
    // Phase 4: Generate ELF binary
    // ========================================================================

    let mut elf_builder = ElfBuilder::new(&layout, text_bytes, data_bytes);

    // Build symbol table
    elf_builder.build_symbol_table(source, symbol_links);

    // Checkpoint: dump ELF if requested
    if should_dump_phase(config, Phase::Elf) {
        dump_elf(config, &elf_builder);
        if is_terminal_phase(config, Phase::Elf) {
            println!("\n(No output file generated)");
            return Ok(());
        }
        println!(); // Separator (though this won't be reached for ELF dumps currently)
    }

    // ========================================================================
    // Output: Write ELF binary to file and display summary
    // ========================================================================

    // If any dump options were used, we skip writing the output file
    if config.dump.has_dumps() {
        println!("\n(No output file generated)");
        return Ok(());
    }

    // Find entry point (_start symbol is required for executables)
    let entry_point = {
        if let Some(g) = symbol_links.global_symbols.iter().find(|g| g.symbol == "_start") {
            let pointer = g.definition_pointer;
            Ok(layout.get_line_address(pointer))
        } else {
            Err(AssemblerError::no_context("_start symbol not defined".to_string()))
        }
    }?;

    let elf_bytes = elf_builder.build(entry_point);

    // Write to output file
    let mut file = File::create(&config.output_file).map_err(|e| AssemblerError::no_context(e.to_string()))?;
    file.write_all(&elf_bytes).map_err(|e| AssemblerError::no_context(e.to_string()))?;

    // Set executable permissions (0755)
    let metadata = file.metadata().map_err(|e| AssemblerError::no_context(e.to_string()))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&config.output_file, permissions)
        .map_err(|e| AssemblerError::no_context(e.to_string()))?;

    // Default: silent on success

    Ok(())
}

/// Iterate until line sizes and offsets are stable
///
/// This function repeatedly:
/// 1. Computes addresses/offsets based on current size guesses
/// 2. Computes symbol values (addresses of labels)
/// 3. Evaluates expressions (which depend on symbol values/offsets)
/// 4. Encodes/generates code, which determines actual instruction sizes
/// 5. Checks if any sizes changed from the guess
///
/// Loops until there are no size changes or max iterations reached.
/// Returns Ok((text, data, bss_size)) with the final encoding.
pub fn relaxation_loop(
    config: &Config,
    source: &Source,
    symbol_links: &SymbolLinks,
    layout: &mut Layout,
) -> Result<(Vec<u8>, Vec<u8>, u32)> {
    const MAX_ITERATIONS: usize = 10;

    if config.verbose {
        eprintln!("Relaxation:");
        eprintln!("  Pass   Text    Data     BSS");
        eprintln!("  ----  -----  ------  ------");
    }

    for iteration in 0..MAX_ITERATIONS {
        let pass_number = iteration + 1;

        // Step 1: Calculate addresses based on current size guesses
        layout.update_addresses(source);

        if config.verbose {
            eprintln!("  {:4}  {:5}  {:6}  {:6}", pass_number, layout.text_size, layout.data_size, layout.bss_size);
        }

        // Step 2: Compute segment addresses and calculate all symbol values upfront
        layout.set_segment_addresses(config.text_start);
        let symbol_values = eval_symbol_values(source, symbol_links, layout)?;

        // Dump symbol values if requested
        if let Some(ref spec) = config.dump.dump_values {
            dump_values(pass_number, false, source, layout, spec);
        }

        // Step 3: Encode everything and update line sizes
        // Encode and collect results
        let (any_changed, text_bytes, data_bytes, bss_size) =
            encode(config, source, symbol_links, &symbol_values, layout)?;

        // Dump generated code if requested
        if let Some(ref spec) = config.dump.dump_code {
            dump_code(
                pass_number,
                !any_changed, // is_final if no changes
                source,
                layout,
                &text_bytes,
                &data_bytes,
                spec,
            );
        }

        // Step 4: Check for size changes
        if !any_changed {
            // Dump final symbol values if requested
            if let Some(ref spec) = config.dump.dump_values {
                dump_values(pass_number, true, source, layout, spec);
            }
            if config.verbose {
                eprintln!("  Converged after {} pass{}", pass_number, if pass_number == 1 { "" } else { "es" });
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

// Assembly phases - each phase has a checkpoint where we can dump and optionally exit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Parse,         // After parsing source files into AST
    SymbolLinking, // After linking all symbols
    Relaxation,    // During/after code generation and relaxation
    Elf,           // After ELF generation
}

// Helper: Check if we should dump at this phase
fn should_dump_phase(config: &Config, phase: Phase) -> bool {
    match phase {
        Phase::Parse => config.dump.dump_ast.is_some(),
        Phase::SymbolLinking => config.dump.dump_symbols.is_some(),
        Phase::Relaxation => config.dump.dump_values.is_some() || config.dump.dump_code.is_some(),
        Phase::Elf => config.dump.dump_elf.is_some(),
    }
}

// Helper: Check if this is the last phase we need to execute (early exit after dump)
fn is_terminal_phase(config: &Config, phase: Phase) -> bool {
    // If this phase has a dump option, check if any later phases also have dump options
    if !should_dump_phase(config, phase) {
        return false;
    }

    match phase {
        Phase::Parse => {
            !should_dump_phase(config, Phase::SymbolLinking)
                && !should_dump_phase(config, Phase::Relaxation)
                && !should_dump_phase(config, Phase::Elf)
        }
        Phase::SymbolLinking => !should_dump_phase(config, Phase::Relaxation) && !should_dump_phase(config, Phase::Elf),
        Phase::Relaxation => !should_dump_phase(config, Phase::Elf),
        Phase::Elf => {
            // ELF is always terminal if we're dumping it
            true
        }
    }
}

/// Read and parse input files into AST
pub fn process_files(files: &[String]) -> Result<Source> {
    let mut source = Source { files: Vec::new() };

    for file_path in files {
        let source_file = process_file(file_path)?;
        source.files.push(source_file);
    }

    // Add builtin symbols file (provides __global_pointer$ definition)
    source.files.push(create_builtin_symbols_file());

    Ok(source)
}

/// Process a single source file
fn process_file(file_path: &str) -> Result<SourceFile> {
    let file = File::open(file_path)
        .map_err(|e| AssemblerError::no_context(format!("could not open file '{}': {}", file_path, e)))?;
    let reader = io::BufReader::new(file);

    let mut lines: Vec<Line> = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result
            .map_err(|e| AssemblerError::no_context(format!("could not read file '{}': {}", file_path, e)))?;
        if line.trim().is_empty() {
            continue;
        }

        let location = Location { file: file_path.to_string(), line: line_num + 1 };

        let tokens = tokenize(&line).map_err(|e| AssemblerError::from_context(e, location.clone()))?;

        if !tokens.is_empty() {
            let parsed_lines = parse(&tokens, file_path.to_string(), line_num + 1)?;

            for parsed_line in parsed_lines {
                // Segment and size will be set in the layout phase
                lines.push(parsed_line);
            }
        }
    }

    Ok(SourceFile { file: file_path.to_string(), lines })
}

/// Print input statistics during verbose assembly
pub fn print_input_statistics(source: &Source, symbol_links: &SymbolLinks) {
    let mut total_lines = 0;
    let mut total_labels = 0;
    let mut total_instructions = 0;
    let mut total_directives = 0;

    for file in &source.files {
        for line in &file.lines {
            total_lines += 1;
            match &line.content {
                LineContent::Label(_) => total_labels += 1,
                LineContent::Instruction(_) => total_instructions += 1,
                LineContent::Directive(_) => total_directives += 1,
            }
        }
    }

    let num_globals = symbol_links.global_symbols.len();

    // Don't count the builtin file
    let num_files = source.files.len().saturating_sub(1);

    eprintln!("Input:");
    eprintln!("  Files:        {}", num_files);
    eprintln!("  Lines:        {}", total_lines);
    eprintln!("  Labels:       {}", total_labels);
    eprintln!("  Instructions: {}", total_instructions);
    eprintln!("  Directives:   {}", total_directives);
    if num_globals > 0 {
        eprintln!("  Globals:      {}", num_globals);
    }
    eprintln!();
}
