// Risclet: A simple RISC-V simulator and assembler
//
// This library combines two tools:
// - The simulator: executes and debugs RISC-V ELF binaries
// - The assembler: assembles RISC-V source to executable ELF files

// Simulator modules
pub mod decoder;
pub mod elf_loader;
pub mod execution;
pub mod execution_context;
pub mod io_abstraction;
pub mod isa_tests;
pub mod linter;
pub mod linter_context;
pub mod memory;
pub mod memory_interface;
pub mod riscv;
pub mod simulator;
pub mod test_utils;
pub mod trace;
pub mod ui;

// Assembler modules
pub mod assembler;
pub mod ast;
pub mod config;
pub mod dump;
pub mod elf_builder;
pub mod encoder;
pub mod error;
pub mod expressions;
pub mod layout;
pub mod parser;
pub mod symbols;
pub mod tokenizer;

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

// Re-export main APIs
pub use execution::{Instruction, Machine, add_local_labels, trace};
pub use elf_loader::*;
pub use riscv::{Op, fields_to_string, get_pseudo_sequence};
pub use trace::Effects;
pub use ui::*;
