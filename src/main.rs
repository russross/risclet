use assembler::drive_assembler;
use config::process_cli_args;

mod assembler;
mod ast;
mod config;
mod dump;
mod elf;
mod encoder;
mod error;
mod expressions;
mod layout;
mod parser;
mod symbols;
mod tokenizer;

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
    let config = match process_cli_args() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = drive_assembler(config) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
