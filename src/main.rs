use ast::{Line, Segment, Source, SourceFile};
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

mod ast;
mod error;
mod parser;
mod symbols;
mod tokenizer;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file> [file...]", args[0]);
        std::process::exit(1);
    }

    let files: Vec<String> = args[1..].to_vec();

    match process_files(files) {
        Ok(mut source) => {
            // Resolve symbols
            if let Err(e) = symbols::resolve_symbols(&mut source) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            compute_offsets(&mut source);
            // Dump the parsed Source to stdout
            println!("{}", source);
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
                                    guess_line_size(&new_line.content)?;

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

fn compute_offsets(source: &mut Source) {
    let mut global_text_offset: u64 = 0;
    let mut global_data_offset: u64 = 0;
    let mut global_bss_offset: u64 = 0;

    for source_file in &mut source.files {
        // Track the starting offset for this file in each segment
        let file_text_start = global_text_offset;
        let file_data_start = global_data_offset;
        let file_bss_start = global_bss_offset;

        // Track offsets within this file (continuing from global offsets)
        let mut text_offset: u64 = global_text_offset;
        let mut data_offset: u64 = global_data_offset;
        let mut bss_offset: u64 = global_bss_offset;

        for line in &mut source_file.lines {
            let current_offset = match line.segment {
                Segment::Text => text_offset,
                Segment::Data => data_offset,
                Segment::Bss => bss_offset,
            };

            // For .balign, compute actual size based on offset
            if let ast::LineContent::Directive(ast::Directive::Balign(_expr)) =
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

fn guess_line_size(content: &ast::LineContent) -> Result<u64, String> {
    match content {
        ast::LineContent::Instruction(inst) => match inst {
            ast::Instruction::Pseudo(_) => Ok(4),
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
            ast::Directive::Byte(exprs) => Ok(exprs.len() as u64 * 1),
            ast::Directive::TwoByte(exprs) => Ok(exprs.len() as u64 * 2),
            ast::Directive::FourByte(exprs) => Ok(exprs.len() as u64 * 4),
            ast::Directive::EightByte(exprs) => Ok(exprs.len() as u64 * 8),
            ast::Directive::String(strings) => {
                let mut size = 0;
                for s in strings {
                    size += s.len() as u64;
                }
                Ok(size)
            }
            ast::Directive::Asciz(strings) => {
                let mut size = 0;
                for s in strings {
                    size += s.len() as u64 + 1; // +1 for null terminator
                }
                Ok(size)
            }
            _ => Ok(0), // Non-data directives like .text, .global
        },
    }
}
