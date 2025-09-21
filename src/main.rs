use std::env;
use std::fs::File;
use std::io::{self, BufRead};

mod ast;
mod lexer;
mod parser;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file> [file...]", args[0]);
        std::process::exit(1);
    }
    for file_path in &args[1..] {
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
            match lexer::tokenize(&line) {
                Ok(tokens) => {
                    if !tokens.is_empty() {
                        match parser::parse(&tokens, file_path.clone(), (line_num + 1) as u32) {
                            Ok(ast_lines) => {
                                for line in ast_lines {
                                    println!("{}", line);
                                }
                            }
                            Err(e) => {
                                println!("Parse error in {} on line {}: {}", file_path, line_num + 1, e);
                                let token_strs: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
                                println!("Tokens: {}", token_strs.join(" "));
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Tokenize error in {} on line {}: {}", file_path, line_num + 1, e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}