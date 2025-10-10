use ast::Line;
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

mod ast;
mod parser;
mod tokenizer;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file> [file...]", args[0]);
        std::process::exit(1);
    }

    let files: Vec<String> = args[1..].to_vec();

    let mut all_ast_lines: Vec<Line> = Vec::new();
    for file_path in &files {
        let file = match File::open(file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening {}: {}", file_path, e);
                std::process::exit(1);
            }
        };
        let reader = io::BufReader::new(file);
        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            match tokenizer::tokenize(&line) {
                Ok(tokens) => {
                    if !tokens.is_empty() {
                        match parser::parse(
                            &tokens,
                            file_path.clone(),
                            (line_num + 1) as u32,
                        ) {
                            Ok(lines) => {
                                for line in lines {
                                    all_ast_lines.push(line);
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "Parse error in {} on line {}: {}",
                                    file_path,
                                    line_num + 1,
                                    e
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Tokenize error in {} on line {}: {}",
                        file_path,
                        line_num + 1,
                        e
                    );
                    std::process::exit(1);
                }
            }
        }
    }

    // Dump all parsed AST lines to stdout
    for line in &all_ast_lines {
        println!("{}", line);
    }

    Ok(())
}


