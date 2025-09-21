use std::env;
use std::fs::File;
use std::io::{self, BufRead};

mod ast;
mod lexer;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    let file = File::open(&args[1])?;
    let reader = io::BufReader::new(file);
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        match lexer::tokenize(&line) {
            Ok(tokens) => {
                if !tokens.is_empty() {
                    let token_strs: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
                    println!("{}: {}", line_num + 1, token_strs.join(" "));
                }
            }
            Err(e) => println!("{}: Error: {}", line_num + 1, e),
        }
    }
    Ok(())
}