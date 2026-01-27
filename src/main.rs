// Risclet: A RISC-V simulator and assembler

// Shared modules
mod elf;
mod error;

// Simulator modules
mod checkabi;
mod decoder;
mod elf_loader;
mod execution;
mod isa_tests;
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
mod expressions;
mod layout;
mod parser;
mod symbols;
mod tokenizer;

// Test modules
#[cfg(test)]
mod checkabi_tests;
#[cfg(test)]
mod encoder_tests;
#[cfg(test)]
mod expressions_tests;
#[cfg(test)]
mod parser_tests;
#[cfg(test)]
mod riscv_tests;
#[cfg(test)]
mod symbols_tests;
#[cfg(test)]
mod tokenizer_tests;

use crate::assembler::{assemble_and_save, assemble_files};
use crate::config::{Mode, parse_cli_args};
use crate::elf_loader::ElfInput;
use crate::simulator::run_simulator;

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
            if let Err(e) = assemble_and_save(&config) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }

        Mode::Run | Mode::Debug | Mode::Disassemble | Mode::Trace => {
            // Check if we have .s files to assemble first
            if !config.input_files.is_empty() {
                // We have .s files - assemble them in-memory, then run simulator
                let elf_bytes = match assemble_files(&config) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("{}", e);
                        std::process::exit(1);
                    }
                };

                // Pass in-memory ELF to simulator
                if let Err(e) =
                    run_simulator(&config, ElfInput::Bytes(&elf_bytes))
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            } else {
                // No .s files - load executable and run simulator
                if let Err(e) =
                    run_simulator(&config, ElfInput::File(&config.executable))
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Mode::Default => {
            // This mode should no longer be used, but keep for compatibility
            eprintln!("Error: Default mode is deprecated");
            std::process::exit(1);
        }
    }
}

// Re-export main APIs (for compatibility with any code that imports from this crate)
pub use crate::elf_loader::load_elf;
pub use execution::{Instruction, Machine, add_local_labels, trace};
pub use riscv::{Op, fields_to_string, get_pseudo_sequence};
pub use trace::Effects;
pub use ui::Tui;
