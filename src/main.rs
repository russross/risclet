use ast::{Line, Segment, Source, SourceFile};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Write};

mod assembler;
mod ast;
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
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(format!("Usage: {} [options] <file.s> [file.s...]

Options:
  -o <file>        Write output to <file> (default: a.out)
  -t <address>     Set text start address (default: 0x10000)
  -v, --verbose    Show detailed assembly output
  -h, --help       Show this help message", args[0]));
    }

    let mut input_files = Vec::new();
    let mut output_file = "a.out".to_string();
    let mut text_start = 0x10000i64;
    let mut verbose = false;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
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
                return Err(format!("Usage: {} [options] <file.s> [file.s...]

Options:
  -o <file>        Write output to <file> (default: a.out)
  -t <address>     Set text start address (default: 0x10000)
  -v, --verbose    Show detailed assembly output
  -h, --help       Show this help message", args[0]));
            }
            arg => {
                if arg.starts_with('-') {
                    return Err(format!("Error: unknown option: {}", arg));
                }
                input_files.push(arg.to_string());
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
    })
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
            // Resolve symbols (create symbol table with pointers to definitions)
            if let Err(e) = symbols::resolve_symbols(&mut source) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }

            // Converge: repeatedly compute offsets, evaluate expressions, and encode
            // until line sizes stabilize. Returns the final encoded segments.
            let (text_bytes, data_bytes, bss_size) =
                match assembler::converge_and_encode(&mut source, config.text_start) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Error during convergence and encoding: {}", e);
                        std::process::exit(1);
                    }
                };

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
