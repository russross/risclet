use ast::{Line, Source, SourceFile};
use config::Config;
use error::AssemblerError;
use layout::Layout;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::PermissionsExt;

mod assembler;
mod ast;
mod config;
mod dump;
mod elf;
mod encoder;
mod encoder_compressed;
mod error;
mod expressions;
mod layout;
mod parser;
mod symbols;
mod tokenizer;

#[cfg(test)]
mod encoder_compressed_tests;
#[cfg(test)]
mod encoder_tests;
#[cfg(test)]
mod expressions_tests;
#[cfg(test)]
mod parser_tests;
#[cfg(test)]
mod symbols_tests;
#[cfg(test)]
mod tokenizer_tests;




fn main() {
    let config = match config::process_cli_args() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = main_process(config) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

// Assembly phases - each phase has a checkpoint where we can dump and optionally exit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Parse,         // After parsing source files into AST
    SymbolLinking, // After linking all symbols
    Convergence,   // During/after code generation convergence
    Elf,           // After ELF generation
}

// Callback for convergence dumps - implements assembler::ConvergenceCallback
struct DumpCallback<'a> {
    dump_config: &'a dump::DumpConfig,
}

impl<'a> assembler::ConvergenceCallback for DumpCallback<'a> {
    fn on_values_computed(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        symbol_values: &expressions::SymbolValues,
        layout: &Layout,
    ) {
        if let Some(ref spec) = self.dump_config.dump_values {
            dump::dump_values(
                pass,
                is_final,
                source,
                symbol_values,
                layout,
                spec,
            );
        }
    }

    fn on_code_generated(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        symbol_values: &expressions::SymbolValues,
        layout: &Layout,
        text_bytes: &[u8],
        data_bytes: &[u8],
    ) {
        if let Some(ref spec) = self.dump_config.dump_code {
            dump::dump_code(
                pass,
                is_final,
                source,
                symbol_values,
                layout,
                text_bytes,
                data_bytes,
                spec,
            );
        }
    }
}

fn main_process(config: Config) -> Result<(), AssemblerError> {
    drive_assembler(config)
}

// Helper: Check if we should dump at this phase
fn should_dump_phase(config: &Config, phase: Phase) -> bool {
    match phase {
        Phase::Parse => config.dump.dump_ast.is_some(),
        Phase::SymbolLinking => config.dump.dump_symbols.is_some(),
        Phase::Convergence => {
            config.dump.dump_values.is_some() || config.dump.dump_code.is_some()
        }
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
                && !should_dump_phase(config, Phase::Convergence)
                && !should_dump_phase(config, Phase::Elf)
        }
        Phase::SymbolLinking => {
            !should_dump_phase(config, Phase::Convergence)
                && !should_dump_phase(config, Phase::Elf)
        }
        Phase::Convergence => !should_dump_phase(config, Phase::Elf),
        Phase::Elf => {
            // ELF is always terminal if we're dumping it
            true
        }
    }
}

// Main assembly driver - unified flow with checkpoints
fn drive_assembler(config: Config) -> Result<(), AssemblerError> {
    // ========================================================================
    // Phase 1: Parse source files into AST
    // ========================================================================
    let mut source = process_files(config.input_files.clone())?;

    // Checkpoint: dump AST if requested
    if should_dump_phase(&config, Phase::Parse) {
        dump::dump_ast(&source, config.dump.dump_ast.as_ref().unwrap());
        if is_terminal_phase(&config, Phase::Parse) {
            println!("\n(No output file generated)");
            return Ok(());
        }
        println!(); // Separator between phase dumps
    }

    // ========================================================================
    // Phase 2: Link symbols (connect symbol uses to their definitions)
    // ========================================================================
    let symbol_links = symbols::link_symbols(&source)?;

    // Checkpoint: dump symbol linking if requested
    if should_dump_phase(&config, Phase::SymbolLinking) {
        dump::dump_symbols(
            &source,
            &symbol_links,
            config.dump.dump_symbols.as_ref().unwrap(),
        );
        if is_terminal_phase(&config, Phase::SymbolLinking) {
            println!("\n(No output file generated)");
            return Ok(());
        }
        println!(); // Separator between phase dumps
    }

    // Create initial layout after symbol linking
    let mut layout = layout::create_initial_layout(&source);

    // ========================================================================
    // Phase 3: Convergence - iteratively compute offsets and encode until stable
    // ========================================================================

    // Show input statistics before convergence if verbose
    if config.verbose {
        print_input_statistics(&source, &symbol_links);
    }

    let (text_bytes, data_bytes, bss_size) =
        if should_dump_phase(&config, Phase::Convergence) {
            // Use callback-based convergence with dump support
            let dump_callback = DumpCallback { dump_config: &config.dump };
            assembler::converge_and_encode(
                &mut source,
                &symbol_links,
                &mut layout,
                &config,
                &dump_callback,
            )?
        } else {
            // Use standard convergence with verbose stats if requested
            assembler::converge_and_encode(
                &mut source,
                &symbol_links,
                &mut layout,
                &config,
                &assembler::NoOpCallback,
            )?
        };

    // Checkpoint: after convergence, check if we should exit before ELF generation
    if should_dump_phase(&config, Phase::Convergence)
        && is_terminal_phase(&config, Phase::Convergence)
    {
        println!("\n(No output file generated)");
        return Ok(());
    }

    // ========================================================================
    // Phase 4: Generate ELF binary
    // ========================================================================

    // Build ELF binary
    let has_data = !data_bytes.is_empty();
    let has_bss = bss_size > 0;

    let mut elf_builder =
        elf::ElfBuilder::new(layout.text_start, layout.header_size as u32);
    elf_builder.set_segments(
        text_bytes.clone(),
        data_bytes.clone(),
        bss_size,
        layout.data_start,
        layout.bss_start,
    );

    // Build symbol table
    elf::build_symbol_table(
        &source,
        &symbol_links,
        &layout,
        &mut elf_builder,
        layout.text_start,
        layout.data_start,
        layout.bss_start,
        has_data,
        has_bss,
    );

    // Checkpoint: dump ELF if requested
    if should_dump_phase(&config, Phase::Elf) {
        dump::dump_elf(
            &elf_builder,
            &source,
            config.dump.dump_elf.as_ref().unwrap(),
        );
        if is_terminal_phase(&config, Phase::Elf) {
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
        if let Some(g) =
            symbol_links.global_symbols.iter().find(|g| g.symbol == "_start")
        {
            let pointer = g.definition_pointer.clone();
            Ok(layout.get_line_address(&pointer) as u64)
        } else {
            Err(AssemblerError::no_context(
                "_start symbol not defined".to_string(),
            ))
        }
    }?;

    let elf_bytes = elf_builder.build(entry_point as u32);

    // Write to output file
    let mut file = File::create(&config.output_file)
        .map_err(|e| AssemblerError::no_context(e.to_string()))?;
    file.write_all(&elf_bytes)
        .map_err(|e| AssemblerError::no_context(e.to_string()))?;

    // Set executable permissions (0755)
    let metadata = file
        .metadata()
        .map_err(|e| AssemblerError::no_context(e.to_string()))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&config.output_file, permissions)
        .map_err(|e| AssemblerError::no_context(e.to_string()))?;

    // Default: silent on success (Unix style)
    // Verbose mode only shows stats/progress during convergence, not detailed listing

    Ok(())
}

fn print_input_statistics(
    source: &Source,
    symbol_links: &symbols::SymbolLinks,
) {
    let mut total_lines = 0;
    let mut total_labels = 0;
    let mut total_instructions = 0;
    let mut total_directives = 0;

    for file in &source.files {
        for line in &file.lines {
            total_lines += 1;
            match &line.content {
                ast::LineContent::Label(_) => total_labels += 1,
                ast::LineContent::Instruction(_) => total_instructions += 1,
                ast::LineContent::Directive(_) => total_directives += 1,
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

fn process_files(files: Vec<String>) -> Result<Source, error::AssemblerError> {
    let mut source = Source { files: Vec::new() };

    for file_path in &files {
        let source_file = process_file(file_path)?;
        source.files.push(source_file);
    }

    // Add builtin symbols file (provides __global_pointer$ definition)
    source.files.push(symbols::create_builtin_symbols_file());

    Ok(source)
}

fn process_file(file_path: &str) -> Result<SourceFile, AssemblerError> {
    let file = File::open(file_path).map_err(|e| {
        AssemblerError::no_context(format!(
            "could not open file '{}': {}",
            file_path, e
        ))
    })?;
    let reader = io::BufReader::new(file);

    let mut lines: Vec<Line> = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| {
            AssemblerError::no_context(format!(
                "could not read file '{}': {}",
                file_path, e
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }

        let location =
            ast::Location { file: file_path.to_string(), line: line_num };

        let tokens = tokenizer::tokenize(&line)
            .map_err(|e| AssemblerError::from_context(e, location.clone()))?;

        if !tokens.is_empty() {
            let parsed_lines =
                parser::parse(&tokens, file_path.to_string(), line_num + 1)?;

            for parsed_line in parsed_lines {
                // Segment and size will be set in the layout phase
                lines.push(parsed_line);
            }
        }
    }

    Ok(SourceFile { file: file_path.to_string(), lines })
}
