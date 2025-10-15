use ast::{Line, Segment, Source, SourceFile};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Write};

mod assembler;
mod ast;
mod dump;
mod elf;
mod encoder;
mod error;
mod expressions;
mod parser;
mod symbols;
mod tokenizer;

struct Config {
    input_files: Vec<String>,
    output_file: String,
    text_start: i64,
    verbose: bool,
    dump: dump::DumpConfig,
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(print_help(&args[0]));
    }

    let mut input_files = Vec::new();
    let mut output_file = "a.out".to_string();
    let mut text_start = 0x10000i64;
    let mut verbose = false;
    let mut dump_config = dump::DumpConfig::new();
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        // Handle --dump-* options
        if arg.starts_with("--dump-") {
            if arg == "--dump-ast" {
                dump_config.dump_ast = true;
            } else if arg == "--dump-symbols" {
                dump_config.dump_symbols = true;
            } else if arg.starts_with("--dump-values") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_values = Some(dump::parse_dump_spec(spec_str)?);
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
                        return Err("Error: -o requires an argument".to_string());
                    }
                    output_file = args[i].clone();
                }
                "-t" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("Error: -t requires an argument".to_string());
                    }
                    text_start = parse_address(&args[i])?;
                }
                "-v" | "--verbose" => {
                    verbose = true;
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
    })
}

fn print_help(program_name: &str) -> String {
    format!("Usage: {} [options] <file.s> [file.s...]

Options:
  -o <file>            Write output to <file> (default: a.out)
  -t <address>         Set text start address (default: 0x10000)
  -v, --verbose        Show detailed assembly output
  -h, --help           Show this help message

Debug Dump Options:
  --dump-ast                      Dump AST after parsing (s-expression format)
  --dump-symbols                  Dump after symbol resolution with references
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
  --dump-values=1-              Dump values for all passes, all files
  --dump-code=2:prog.s          Dump code for pass 2, prog.s only
  --dump-elf=headers,symbols    Dump ELF headers and symbol table

Note: When any --dump-* option is used, no output file is generated.",
        program_name)
}

fn parse_address(s: &str) -> Result<i64, String> {
    if let Some(hex) = s.strip_prefix("0x") {
        i64::from_str_radix(hex, 16)
            .map_err(|_| format!("Error: invalid hex address: {}", s))
    } else {
        s.parse::<i64>()
            .map_err(|_| format!("Error: invalid address: {}", s))
    }
}

fn main() {
    let config = match parse_args() {
        Ok(cfg) => cfg,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    let files = config.input_files.clone();

    match process_files(files) {
        Ok(mut source) => {
            // Dump AST if requested
            if config.dump.dump_ast {
                dump::dump_ast(&source);
                if !config.dump.dump_symbols
                    && config.dump.dump_values.is_none()
                    && config.dump.dump_code.is_none()
                    && config.dump.dump_elf.is_none()
                {
                    println!("\n(No output file generated)");
                    return;
                }
                println!();
            }

            // Resolve symbols (create symbol table with pointers to definitions)
            if let Err(e) = symbols::resolve_symbols(&mut source) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            // Dump symbol resolution if requested
            if config.dump.dump_symbols {
                dump::dump_symbols(&source);
                if config.dump.dump_values.is_none()
                    && config.dump.dump_code.is_none()
                    && config.dump.dump_elf.is_none()
                {
                    println!("\n(No output file generated)");
                    return;
                }
                println!();
            }

            // Converge: repeatedly compute offsets, evaluate expressions, and encode
            // until line sizes stabilize. Returns the final encoded segments.
            // If dump options are enabled, pass them to enable per-pass dumping.
            let (text_bytes, data_bytes, bss_size) = if config.dump.dump_values.is_some() || config.dump.dump_code.is_some() {
                // Use dump-aware version
                match converge_and_encode_with_dumps(
                    &mut source,
                    config.text_start,
                    &config.dump,
                ) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Error during convergence and encoding: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Use standard version (no dump overhead)
                match assembler::converge_and_encode(&mut source, config.text_start) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Error during convergence and encoding: {}", e);
                        std::process::exit(1);
                    }
                }
            };

            // If dump options were used, we're done (no ELF generation)
            if config.dump.has_dumps() {
                // Create evaluation context for final ELF dump if needed
                if config.dump.dump_elf.is_some() {
                    let mut eval_context =
                        expressions::new_evaluation_context(source.clone(), config.text_start);

                    // Evaluate all line symbols
                    for file in &source.files {
                        for line in &file.lines {
                            let _ = expressions::evaluate_line_symbols(line, &mut eval_context);
                        }
                    }

                    // Generate ELF binary (for dumping purposes)
                    let has_data = !data_bytes.is_empty();
                    let has_bss = bss_size > 0;

                    let mut elf_builder = elf::ElfBuilder::new(config.text_start as u64);
                    elf_builder.set_segments(
                        text_bytes.clone(),
                        data_bytes.clone(),
                        bss_size as u64,
                        eval_context.data_start as u64,
                        eval_context.bss_start as u64,
                    );

                    // Build symbol table
                    elf::build_symbol_table(
                        &source,
                        &mut elf_builder,
                        config.text_start as u64,
                        eval_context.data_start as u64,
                        eval_context.bss_start as u64,
                        has_data,
                        has_bss,
                    );

                    // Dump ELF
                    dump::dump_elf(&elf_builder, &source, config.dump.dump_elf.as_ref().unwrap());
                }

                println!("\n(No output file generated)");
                return;
            }

            // Normal path: generate ELF and write to file
            // Create evaluation context with final addresses for display
            let mut eval_context =
                expressions::new_evaluation_context(source.clone(), config.text_start);

            // Evaluate all line symbols for display
            for file in &source.files {
                for line in &file.lines {
                    if let Err(e) = expressions::evaluate_line_symbols(
                        line,
                        &mut eval_context,
                    ) {
                        eprintln!(
                            "Warning: Failed to evaluate symbols in line: {}",
                            e
                        );
                    }
                }
            }

            // Generate ELF binary
            let has_data = !data_bytes.is_empty();
            let has_bss = bss_size > 0;

            let mut elf_builder = elf::ElfBuilder::new(config.text_start as u64);
            elf_builder.set_segments(
                text_bytes.clone(),
                data_bytes.clone(),
                bss_size as u64,
                eval_context.data_start as u64,
                eval_context.bss_start as u64,
            );

            // Build symbol table
            elf::build_symbol_table(
                &source,
                &mut elf_builder,
                config.text_start as u64,
                eval_context.data_start as u64,
                eval_context.bss_start as u64,
                has_data,
                has_bss,
            );

            // Find entry point (_start symbol if present, otherwise use text_start)
            let entry_point = source
                .global_symbols
                .iter()
                .find(|g| g.symbol == "_start")
                .map(|g| {
                    let line = &source.files[g.definition_pointer.file_index].lines
                        [g.definition_pointer.line_index];
                    config.text_start as u64 + line.offset as u64
                })
                .unwrap_or(config.text_start as u64);

            let elf_bytes = elf_builder.build(entry_point);

            // Write ELF binary to output file
            match File::create(&config.output_file) {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(&elf_bytes) {
                        eprintln!("Error writing {}: {}", config.output_file, e);
                        std::process::exit(1);
                    }
                    // Minimal output by default
                    if config.verbose {
                        eprintln!("Generated: {}", config.output_file);
                    }
                }
                Err(e) => {
                    eprintln!("Error creating {}: {}", config.output_file, e);
                    std::process::exit(1);
                }
            }

            // Show summary or detailed output
            if config.verbose {
                dump_source_with_values(&source, &mut eval_context, &text_bytes, &data_bytes, bss_size);
            } else {
                // Just show a one-line summary
                println!("{}: text={} data={} bss={} total={}",
                    config.output_file,
                    source.text_size,
                    source.data_size,
                    bss_size,
                    elf_bytes.len()
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

// Convergence function with per-pass dump support
fn converge_and_encode_with_dumps(
    source: &mut Source,
    text_start: i64,
    dump_config: &dump::DumpConfig,
) -> Result<(Vec<u8>, Vec<u8>, i64), String> {
    const MAX_ITERATIONS: usize = 10;

    for iteration in 0..MAX_ITERATIONS {
        let pass_number = iteration + 1;

        // Step 1: Calculate addresses based on current size guesses
        assembler::compute_offsets(source);

        // Step 2 & 3: Calculate symbol values and evaluate expressions
        let mut eval_context = expressions::new_evaluation_context(source.clone(), text_start);

        // Evaluate all line symbols to populate the expression evaluation context
        for file in &source.files {
            for line in &file.lines {
                // Ignore errors - some expressions may not be evaluable yet
                let _ = expressions::evaluate_line_symbols(line, &mut eval_context);
            }
        }

        // Dump symbol values if requested for this pass
        if let Some(ref spec) = dump_config.dump_values {
            let is_final = false; // We don't know yet if this will be the final pass
            dump::dump_values(pass_number, is_final, source, &mut eval_context, spec);
        }

        // Step 4: Encode everything and update line sizes
        // Track if any size changed
        let mut any_changed = false;

        // Encode and collect results
        let encode_result = encoder::encode_source_with_size_tracking(
            source,
            &mut eval_context,
            &mut any_changed,
        );

        // Get the encoded bytes for code dump
        let (text_bytes, data_bytes, bss_size) = match encode_result {
            Ok(result) => result,
            Err(e) => return Err(format!("Encode error: {}", e)),
        };

        // Dump code generation if requested for this pass
        if let Some(ref spec) = dump_config.dump_code {
            let is_final = !any_changed; // If nothing changed, this will be the final pass
            dump::dump_code(
                pass_number,
                is_final,
                source,
                &mut eval_context,
                &text_bytes,
                &data_bytes,
                spec,
            );
        }

        // Step 5: Check convergence
        if !any_changed {
            // Converged! Dump final pass if requested
            if let Some(ref spec) = dump_config.dump_values {
                dump::dump_values(pass_number, true, source, &mut eval_context, spec);
            }

            return Ok((text_bytes, data_bytes, bss_size));
        }

        // Sizes changed, discard encoded data and loop again
        // (The encoder already updated source.lines[].size)
    }

    Err(format!(
        "Failed to converge after {} iterations - possible cyclic size dependencies",
        MAX_ITERATIONS
    ))
}

fn process_files(files: Vec<String>) -> Result<Source, String> {
    let mut source = Source {
        files: Vec::new(),
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

    Ok(source)
}

fn process_file(file_path: &str) -> Result<SourceFile, String> {
    let file = File::open(file_path)
        .map_err(|e| format!("Error opening {}: {}", file_path, e))?;
    let reader = io::BufReader::new(file);

    let mut current_segment = Segment::Text;
    let mut lines: Vec<Line> = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result
            .map_err(|e| format!("Error reading {}: {}", file_path, e))?;
        if line.trim().is_empty() {
            continue;
        }

        match tokenizer::tokenize(&line) {
            Ok(tokens) => {
                if !tokens.is_empty() {
                    match parser::parse(
                        &tokens,
                        file_path.to_string(),
                        (line_num + 1) as u32,
                    ) {
                        Ok(parsed_lines) => {
                            for parsed_line in parsed_lines {
                                // Update segment if directive changes it
                                if let ast::LineContent::Directive(ref dir) =
                                    parsed_line.content
                                {
                                    match dir {
                                        ast::Directive::Text => {
                                            current_segment = Segment::Text
                                        }
                                        ast::Directive::Data => {
                                            current_segment = Segment::Data
                                        }
                                        ast::Directive::Bss => {
                                            current_segment = Segment::Bss
                                        }
                                        _ => {}
                                    }
                                }

                                // Assign segment and set size
                                let mut new_line = parsed_line;
                                new_line.segment = current_segment.clone();
                                new_line.size =
                                    assembler::guess_line_size(&new_line.content)?;

                                lines.push(new_line);
                            }
                        }
                        Err(e) => {
                            return Err(format!(
                                "Parse error in {} on line {}: {}",
                                file_path,
                                line_num + 1,
                                e
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!(
                    "Tokenize error in {} on line {}: {}",
                    file_path,
                    line_num + 1,
                    e
                ));
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


fn get_encoded_bytes(line: &Line, text_bytes: &[u8], data_bytes: &[u8]) -> Vec<u8> {
    if line.size == 0 {
        return Vec::new();
    }

    let offset = line.offset as usize;
    let size = line.size as usize;

    match line.segment {
        Segment::Text => {
            if offset + size <= text_bytes.len() {
                text_bytes[offset..offset + size].to_vec()
            } else {
                Vec::new()
            }
        }
        Segment::Data => {
            if offset + size <= data_bytes.len() {
                data_bytes[offset..offset + size].to_vec()
            } else {
                Vec::new()
            }
        }
        Segment::Bss => {
            // BSS segment has no encoded bytes (zero-initialized)
            Vec::new()
        }
    }
}

fn dump_source_with_values(
    source: &Source,
    eval_context: &mut expressions::EvaluationContext,
    text_bytes: &[u8],
    data_bytes: &[u8],
    bss_size: i64,
) {
    println!(
        "Source (text: {}, data: {}, bss: {})",
        source.text_size, source.data_size, bss_size
    );

    for (file_index, file) in source.files.iter().enumerate() {
        println!(
            "SourceFile: {} (text: {}, data: {}, bss: {})",
            file.file, file.text_size, file.data_size, file.bss_size
        );

        for line in file.lines.iter() {
            // Calculate absolute address
            let abs_addr = line.offset + get_segment_base(line.segment.clone(), eval_context);

            // Get encoded bytes for this line
            let encoded_bytes = get_encoded_bytes(line, text_bytes, data_bytes);
            let bytes_str = if !encoded_bytes.is_empty() {
                encoded_bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                String::new()
            };

            // Format with absolute address and encoded bytes
            if !bytes_str.is_empty() {
                print!("  {:08x}: {:20} {}", abs_addr, bytes_str, line.content);
            } else {
                print!("  {:08x}: {:20} {}", abs_addr, "", line.content);
            }

            // Collect and show evaluated expression values inline
            let expr_values = collect_expression_values(line, eval_context);
            if !expr_values.is_empty() {
                print!("  # ");
                for (i, val_str) in expr_values.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", val_str);
                }
            }

            println!();
        }

        // Show exported symbols for this file
        let exported: Vec<_> = source
            .global_symbols
            .iter()
            .filter(|g| g.definition_pointer.file_index == file_index)
            .collect();

        if !exported.is_empty() {
            println!("  Exported symbols:");
            for global in exported {
                println!(
                    "    {} -> [{}:{}]",
                    global.symbol,
                    global.definition_pointer.file_index,
                    global.definition_pointer.line_index
                );
            }
        }
    }
}

fn collect_expression_values(
    line: &ast::Line,
    eval_context: &mut expressions::EvaluationContext,
) -> Vec<String> {
    let mut values = Vec::new();

    // Helper to format an evaluated expression value
    let mut format_value = |expr: &ast::Expression| -> String {
        match expressions::eval_expr(expr, line, eval_context) {
            Ok(value) => match value.value_type {
                expressions::ValueType::Integer => format!("{}", value.value),
                expressions::ValueType::Address => {
                    format!("{:#x}", value.value)
                }
            },
            Err(_) => "ERROR".to_string(),
        }
    };

    match &line.content {
        ast::LineContent::Label(_label) => {
            // Show the address value of this label
            let addr = line.offset
                + get_segment_base(line.segment.clone(), eval_context);
            values.push(format!("{:#x}", addr));
        }
        ast::LineContent::Directive(dir) => match dir {
            ast::Directive::Equ(_, expr) => {
                values.push(format_value(expr));
            }
            ast::Directive::Byte(exprs)
            | ast::Directive::TwoByte(exprs)
            | ast::Directive::FourByte(exprs)
            | ast::Directive::EightByte(exprs) => {
                for expr in exprs.iter() {
                    values.push(format_value(expr));
                }
            }
            ast::Directive::Space(expr) => {
                values.push(format_value(expr));
            }
            ast::Directive::Balign(expr) => {
                values.push(format_value(expr));
            }
            _ => {}
        },
        ast::LineContent::Instruction(inst) => {
            // Show expressions in instruction operands
            let exprs = extract_instruction_expressions(inst);
            for expr in exprs.iter() {
                values.push(format_value(expr));
            }
        }
    }

    values
}

fn get_segment_base(
    segment: ast::Segment,
    eval_context: &expressions::EvaluationContext,
) -> i64 {
    match segment {
        ast::Segment::Text => eval_context.text_start,
        ast::Segment::Data => eval_context.data_start,
        ast::Segment::Bss => eval_context.bss_start,
    }
}

fn extract_instruction_expressions(
    inst: &ast::Instruction,
) -> Vec<&ast::Expression> {
    let mut exprs = Vec::new();

    match inst {
        ast::Instruction::RType(..) => {}
        ast::Instruction::IType(_, _, _, expr) => {
            exprs.push(expr.as_ref());
        }
        ast::Instruction::BType(_, _, _, expr) => {
            exprs.push(expr.as_ref());
        }
        ast::Instruction::UType(_, _, expr) => {
            exprs.push(expr.as_ref());
        }
        ast::Instruction::JType(_, _, expr) => {
            exprs.push(expr.as_ref());
        }
        ast::Instruction::LoadStore(_, _, expr, _) => {
            exprs.push(expr.as_ref());
        }
        ast::Instruction::Special(_) => {}
        ast::Instruction::Pseudo(pseudo) => match pseudo {
            ast::PseudoOp::La(_, expr)
            | ast::PseudoOp::LoadGlobal(_, _, expr)
            | ast::PseudoOp::StoreGlobal(_, _, expr, _)
            | ast::PseudoOp::Li(_, expr)
            | ast::PseudoOp::Call(expr)
            | ast::PseudoOp::Tail(expr) => {
                exprs.push(expr.as_ref());
            }
        },
    }

    exprs
}
