// Risclet: A RISC-V simulator and assembler
//
// Unified command-line interface for both tools

// Simulator modules
mod decoder;
mod elf_loader;
mod execution;
mod execution_context;
mod isa_tests;
mod linter;
mod memory;
mod riscv;
mod simulator;
mod test_utils;
mod trace;
mod ui;

// Assembler modules
mod assembler;
mod ast;
mod config;
mod dump;
mod elf_builder;
mod encoder;
mod error;
mod expressions;
mod layout;
mod parser;
mod symbols;
mod tokenizer;

// Test modules
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

use assembler::{assemble_to_memory, drive_assembler};
use config::{Mode, parse_cli_args};
use simulator::{run_simulator, run_simulator_from_memory};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse CLI arguments using unified parser
    let config = match parse_cli_args(&args[1..]) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // Dispatch based on mode
    match config.mode {
        Mode::Assemble => {
            if let Err(e) = drive_assembler(&config) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }

        Mode::Run | Mode::Debug | Mode::Disassemble => {
            if let Err(e) = run_simulator(&config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        Mode::Default => {
            // Default mode: assemble if needed, then debug
            if !config.input_files.is_empty() {
                // We have .s files - assemble them in-memory
                let elf_bytes = match assemble_to_memory(&config) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("{}", e);
                        std::process::exit(1);
                    }
                };

                // Pass in-memory ELF to simulator
                if let Err(e) = run_simulator_from_memory(&config, &elf_bytes) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                // No .s files - just debug a.out
                if let Err(e) = run_simulator(&config) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

// Re-export main APIs (for compatibility with any code that imports from this crate)
pub use elf_loader::*;
pub use execution::{Instruction, Machine, add_local_labels, trace};
pub use riscv::{Op, fields_to_string, get_pseudo_sequence};
pub use trace::Effects;
pub use ui::*;
