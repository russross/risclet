use ast::{Line, LinePointer, Segment, Source, SourceFile};
use encoder::Relax;
use error::AssemblerError;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::PermissionsExt;

mod assembler;
mod ast;
mod dump;
mod elf;
mod encoder;
mod encoder_compressed;
mod error;
mod expressions;
mod parser;
mod symbols;
mod tokenizer;

struct Config {
    input_files: Vec<String>,
    output_file: String,
    text_start: u32,
    verbose: bool,
    dump: dump::DumpConfig,
    relax: Relax,
}

fn process_cli_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(print_help(&args[0]));
    }

    let mut input_files = Vec::new();
    let mut output_file = "a.out".to_string();
    let mut text_start = 0x10000u32;
    let mut verbose = false;
    let mut dump_config = dump::DumpConfig::new();
    let mut relax = Relax::all();
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        // Handle --dump-* options
        if arg.starts_with("--dump-") {
            if arg.starts_with("--dump-ast") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_ast = Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-symbols") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_symbols =
                    Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-values") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_values =
                    Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-code") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_code = Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-elf") {
                let parts_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_elf = Some(dump::parse_elf_parts(parts_str)?);
            } else {
                return Err(format!("Error: unknown option: {}", arg));
            }
        } else {
            match arg.as_str() {
                "-o" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(
                            "Error: -o requires an argument".to_string()
                        );
                    }
                    output_file = args[i].clone();
                }
                "-t" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(
                            "Error: -t requires an argument".to_string()
                        );
                    }
                    text_start = parse_address(&args[i])?;
                }
                "-v" | "--verbose" => {
                    verbose = true;
                }
                "--no-relax" => {
                    relax = Relax::none();
                }
                "--relax-gp" => {
                    relax.gp = true;
                }
                "--no-relax-gp" => {
                    relax.gp = false;
                }
                "--relax-pseudo" => {
                    relax.pseudo = true;
                }
                "--no-relax-pseudo" => {
                    relax.pseudo = false;
                }
                "--relax-compressed" => {
                    relax.compressed = true;
                }
                "--no-relax-compressed" => {
                    relax.compressed = false;
                }
                "-h" | "--help" => {
                    return Err(print_help(&args[0]));
                }
                _ => {
                    if arg.starts_with('-') {
                        return Err(format!("Error: unknown option: {}", arg));
                    }
                    input_files.push(arg.to_string());
                }
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        return Err("Error: no input files specified".to_string());
    }

    Ok(Config {
        input_files,
        output_file,
        text_start,
        verbose,
        dump: dump_config,
        relax,
    })
}

fn print_help(program_name: &str) -> String {
    format!("Usage: {} [options] <file.s> [file.s...]

Options:
    -o <file>            Write output to <file> (default: a.out)
    -t <address>         Set text start address (default: 0x10000)
    -v, --verbose        Show input statistics and convergence progress
    --no-relax           Disable all relaxations
    --relax-gp           Enable GP-relative 'la' optimization (default: on)
    --no-relax-gp        Disable GP-relative 'la' optimization
    --relax-pseudo       Enable 'call'/'tail' pseudo-instruction optimization (default: on)
    --no-relax-pseudo    Disable 'call'/'tail' pseudo-instruction optimization
    --relax-compressed   Enable automatic RV32C compressed encoding (default: on)
    --no-relax-compressed Disable automatic RV32C compressed encoding
    -h, --help           Show this help message

Output Behavior:
  By default, successful assembly produces no output
  Use -v to see input statistics and convergence progress during assembly.
  Use --dump-* options for detailed inspections (AST, symbols, code, ELF) - disables output file.

Debug Dump Options:
  --dump-ast[=PASSES[:FILES]]     Dump AST after parsing (s-expression format)
  --dump-symbols[=PASSES[:FILES]] Dump after symbol resolution with references
  --dump-values[=PASSES[:FILES]]  Dump symbol values for specific passes/files
  --dump-code[=PASSES[:FILES]]    Dump generated code for specific passes/files
  --dump-elf[=PARTS]              Dump detailed ELF info

  PASSES syntax:
    (empty)   Final pass only (default)
    N         Specific pass (e.g., 1, 2)
    N-M       Range (e.g., 1-3)
    N-        From N to end (e.g., 1- for all passes)
    -M        From start to M (e.g., -2 for first two)
    *         All passes

  FILES syntax:
    (empty)   All files (default)
    *         All files
    file1.s,file2.s  Specific files (comma-separated)

  PARTS syntax (for --dump-elf):
    (empty)   All parts (default)
    headers   ELF and program headers
    sections  Section headers
    symbols   Symbol table
    (comma-separated for multiple, e.g., headers,symbols)

Examples:
  ./assembler program.s                        # Silent on success
  ./assembler -v program.s                     # Show input stats and convergence progress
  ./assembler --dump-code program.s            # Dump generated code (no stats)
  ./assembler -v --dump-code program.s         # Show stats AND code dump
  ./assembler --dump-elf=headers,symbols prog.s # Dump ELF metadata

Note: When any --dump-* option is used, no output file is generated.",
        program_name)
}

fn parse_address(s: &str) -> Result<u32, String> {
    if let Some(hex) = s.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
            .map_err(|_| format!("Error: invalid hex address: {}", s))
    } else {
        s.parse::<u32>().map_err(|_| format!("Error: invalid address: {}", s))
    }
}

fn main() {
    let config = match process_cli_args() {
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
    Parse,            // After parsing source files into AST
    SymbolResolution, // After resolving all symbols
    Convergence,      // During/after code generation convergence
    Elf,              // After ELF generation
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
        eval_context: &mut expressions::EvaluationContext,
    ) {
        if let Some(ref spec) = self.dump_config.dump_values {
            dump::dump_values(pass, is_final, source, eval_context, spec);
        }
    }

    fn on_code_generated(
        &self,
        pass: usize,
        is_final: bool,
        source: &Source,
        eval_context: &mut expressions::EvaluationContext,
        text_bytes: &[u8],
        data_bytes: &[u8],
    ) {
        if let Some(ref spec) = self.dump_config.dump_code {
            dump::dump_code(
                pass,
                is_final,
                source,
                eval_context,
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
        Phase::SymbolResolution => config.dump.dump_symbols.is_some(),
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
            !should_dump_phase(config, Phase::SymbolResolution)
                && !should_dump_phase(config, Phase::Convergence)
                && !should_dump_phase(config, Phase::Elf)
        }
        Phase::SymbolResolution => {
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
    // Phase 2: Resolve symbols (create references from uses to definitions)
    // ========================================================================
    let symbols = symbols::resolve_symbols(&mut source)?;

    // Checkpoint: dump symbol resolution if requested
    if should_dump_phase(&config, Phase::SymbolResolution) {
        dump::dump_symbols(&source, config.dump.dump_symbols.as_ref().unwrap());
        if is_terminal_phase(&config, Phase::SymbolResolution) {
            println!("\n(No output file generated)");
            return Ok(());
        }
        println!(); // Separator between phase dumps
    }

    // ========================================================================
    // Phase 3: Convergence - iteratively compute offsets and encode until stable
    // ========================================================================

    // Show input statistics before convergence if verbose
    if config.verbose {
        print_input_statistics(&source);
    }

    let (text_bytes, data_bytes, bss_size) =
        if should_dump_phase(&config, Phase::Convergence) {
            // Use callback-based convergence with dump support
            let dump_callback = DumpCallback { dump_config: &config.dump };
            assembler::converge_and_encode(
                &mut source,
                &symbols,
                config.text_start,
                &config.relax,
                &dump_callback,
                config.verbose,
            )?
        } else {
            // Use standard convergence with verbose stats if requested
            assembler::converge_and_encode(
                &mut source,
                &symbols,
                config.text_start,
                &config.relax,
                &assembler::NoOpCallback,
                config.verbose,
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

    // Create evaluation context for symbol values and ELF generation
    let mut eval_context = expressions::new_evaluation_context(
        source.clone(),
        symbols.clone(),
        config.text_start,
    );

    // Evaluate all symbols to populate the context
    for (file_index, file) in source.files.iter().enumerate() {
        for (line_index, line) in file.lines.iter().enumerate() {
            let pointer = LinePointer { file_index, line_index };
            let _ = expressions::evaluate_line_symbols(
                line,
                &pointer,
                &mut eval_context,
            );
        }
    }

    // Build ELF binary
    let has_data = !data_bytes.is_empty();
    let has_bss = bss_size > 0;

    let mut elf_builder = elf::ElfBuilder::new(
        eval_context.text_start as u32,
        source.header_size as u32,
    );
    elf_builder.set_segments(
        text_bytes.clone(),
        data_bytes.clone(),
        bss_size,
        eval_context.data_start as u32,
        eval_context.bss_start as u32,
    );

    // Build symbol table
    elf::build_symbol_table(
        &source,
        &symbols,
        &mut elf_builder,
        eval_context.text_start as u32,
        eval_context.data_start as u32,
        eval_context.bss_start as u32,
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
            source.global_symbols.iter().find(|g| g.symbol == "_start")
        {
            let line = &source.files[g.definition_pointer.file_index].lines
                [g.definition_pointer.line_index];
            Ok((eval_context.text_start + line.offset) as u64)
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

fn print_input_statistics(source: &Source) {
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

    let num_globals = source.global_symbols.len();

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
    let mut source = Source {
        files: Vec::new(),
        header_size: 0,
        text_size: 0,
        data_size: 0,
        bss_size: 0,
        global_symbols: Vec::new(),
    };

    for file_path in &files {
        let source_file = process_file(file_path)?;
        source.text_size += source_file.text_size;
        source.data_size += source_file.data_size;
        source.bss_size += source_file.bss_size;
        source.files.push(source_file);
    }

    // Add builtin symbols file (provides __global_pointer$ definition)
    source.files.push(ast::create_builtin_symbols_file());

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

    let mut current_segment = Segment::Text;
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
                // Update segment if directive changes it
                if let ast::LineContent::Directive(ref dir) =
                    parsed_line.content
                {
                    match dir {
                        ast::Directive::Text => current_segment = Segment::Text,
                        ast::Directive::Data => current_segment = Segment::Data,
                        ast::Directive::Bss => current_segment = Segment::Bss,
                        _ => {}
                    }
                }

                // Assign segment and set size
                let mut new_line = parsed_line;
                new_line.segment = current_segment;
                new_line.size = assembler::guess_line_size(&new_line.content);
                lines.push(new_line);
            }
        }
    }

    Ok(SourceFile {
        file: file_path.to_string(),
        lines,
        text_size: 0, // Will be set in compute_offsets
        data_size: 0,
        bss_size: 0,
        local_symbols: Vec::new(),
    })
}
