// error.rs
//
// This file defines the RiscletError type for the RISC-V assembler and simulator.
// Assembly errors include source location; runtime errors do not.

use std::fmt;
use std::fs;
use std::io::{self, BufRead};
use std::result::Result as StdResult;

use crate::ast::{LinePointer, Location, Source};
use crate::symbols::BUILTIN_FILE_NAME;

/// An error type for the RISC-V assembler and simulator.
/// Assembly errors carry source location for error reporting with context.
/// Runtime errors do not have source locations.
#[derive(Debug, Clone)]
pub enum RiscletError {
    /// Assembly-time errors with source location
    Assembly { location: Location, message: String },
    /// File I/O errors
    Io(String),
    /// ELF binary format errors
    Elf(String),
    /// Runtime memory access violations
    MemoryAccess(String),
    /// Runtime execution errors
    Execution(String),
    /// Syscall errors
    Syscall(String),
    /// Invalid or unsupported instructions during execution
    InvalidInstruction(String),
    /// ABI constraint violations during execution
    AbiViolation(String),
    /// UI/debugger errors
    Ui(String),
    /// Internal errors (should not happen in normal operation)
    Internal(String),
    /// Program exit via syscall (control flow, not an error)
    Exit(i32),
}

impl RiscletError {
    // Assembly error constructors
    pub fn from_context(message: String, location: Location) -> Self {
        RiscletError::Assembly { location, message }
    }

    pub fn from_source_pointer(
        message: String,
        source: &Source,
        pointer: LinePointer,
    ) -> Self {
        let location = source.files[pointer.file_index].lines
            [pointer.line_index]
            .location
            .clone();
        RiscletError::Assembly { location, message }
    }

    // File I/O error constructors
    pub fn io(message: String) -> Self {
        RiscletError::Io(message)
    }

    pub fn elf(message: String) -> Self {
        RiscletError::Elf(message)
    }

    // Simulator-specific error constructors
    pub fn memory_access_error(message: String) -> Self {
        RiscletError::MemoryAccess(message)
    }

    pub fn execution_error(message: String) -> Self {
        RiscletError::Execution(message)
    }

    pub fn syscall_error(message: String) -> Self {
        RiscletError::Syscall(message)
    }

    pub fn invalid_instruction_error(message: String) -> Self {
        RiscletError::InvalidInstruction(message)
    }

    pub fn abi_violation(message: String) -> Self {
        RiscletError::AbiViolation(message)
    }

    pub fn ui(message: String) -> Self {
        RiscletError::Ui(message)
    }

    pub fn internal(message: String) -> Self {
        RiscletError::Internal(message)
    }

    /// Get the error message without source context
    pub fn message(&self) -> String {
        match self {
            RiscletError::Assembly { message, .. } => message.clone(),
            RiscletError::Io(msg) => msg.clone(),
            RiscletError::Elf(msg) => msg.clone(),
            RiscletError::MemoryAccess(msg) => msg.clone(),
            RiscletError::Execution(msg) => msg.clone(),
            RiscletError::Syscall(msg) => msg.clone(),
            RiscletError::InvalidInstruction(msg) => msg.clone(),
            RiscletError::AbiViolation(msg) => msg.clone(),
            RiscletError::Ui(msg) => msg.clone(),
            RiscletError::Internal(msg) => msg.clone(),
            RiscletError::Exit(code) => format!("exit({})", code),
        }
    }

    /// Format error with source context for assembly errors
    pub fn with_source_context(&self) -> String {
        match self {
            RiscletError::Assembly { location, message } => {
                // Special handling for builtin file - don't try to read it
                if location.file == BUILTIN_FILE_NAME {
                    return format!("Error at {}: {}", location, message);
                }

                let file = fs::File::open(&location.file);
                if let Ok(file) = file {
                    let reader = io::BufReader::new(file);
                    let lines: Vec<String> = reader
                        .lines()
                        .collect::<StdResult<_, _>>()
                        .unwrap_or_default();
                    let line_num = location.line;
                    let start = line_num.saturating_sub(3);
                    let end = (line_num + 3).min(lines.len());
                    let mut context = String::new();
                    for (i, line) in
                        lines.iter().enumerate().take(end).skip(start)
                    {
                        let marker =
                            if i + 1 == line_num { ">>> " } else { "    " };
                        context.push_str(&format!(
                            "{}{:4}: {}\n",
                            marker,
                            i + 1,
                            line
                        ));
                    }
                    format!("Error at {}: {}\n{}", location, message, context)
                } else {
                    format!(
                        "Error at {}: {} (could not read source file)",
                        location, message
                    )
                }
            }
            _ => self.message(),
        }
    }
}

impl From<io::Error> for RiscletError {
    fn from(e: io::Error) -> Self {
        RiscletError::Io(format!("IO error: {}", e))
    }
}

impl From<RiscletError> for String {
    fn from(e: RiscletError) -> Self {
        e.with_source_context()
    }
}

impl fmt::Display for RiscletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.with_source_context())
    }
}

// Type alias for Result with RiscletError
pub type Result<T> = StdResult<T, RiscletError>;
