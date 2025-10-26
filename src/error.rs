// error.rs
//
// This file defines the AssemblerError type for the RISC-V assembler.
// It provides error handling with location and source context.

use crate::ast::{LinePointer, Location, Source};
use std::fmt;
use std::fs;
use std::io::{self, BufRead};

/// An error type for the assembler, including location and message, with context formatting.
#[derive(Debug, Clone)]
pub struct AssemblerError {
    pub location: Option<Location>,
    pub message: String,
}

impl AssemblerError {
    pub fn from_context(message: String, location: Location) -> Self {
        AssemblerError { location: Some(location), message }
    }

    pub fn from_source_pointer(
        message: String,
        source: &Source,
        pointer: &LinePointer,
    ) -> Self {
        let location = source.files[pointer.file_index].lines
            [pointer.line_index]
            .location
            .clone();
        AssemblerError { location: Some(location), message }
    }

    pub fn no_context(message: String) -> Self {
        AssemblerError { location: None, message }
    }

    pub fn with_source_context(&self) -> String {
        if let Some(location) = &self.location {
            let file = fs::File::open(&location.file);
            if let Ok(file) = file {
                let reader = io::BufReader::new(file);
                let lines: Vec<String> = reader
                    .lines()
                    .collect::<std::result::Result<_, _>>()
                    .unwrap_or_default();
                let line_num = location.line;
                let start = line_num.saturating_sub(3);
                let end = (line_num + 3).min(lines.len());
                let mut context = String::new();
                for (i, line) in lines.iter().enumerate().take(end).skip(start)
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
                format!("Error at {}: {}\n{}", location, self.message, context)
            } else {
                format!(
                    "Error at {}: {} (could not read source file)",
                    location, self.message
                )
            }
        } else {
            // No location, just return the message
            self.message.clone()
        }
    }
}

impl From<std::io::Error> for AssemblerError {
    fn from(e: std::io::Error) -> Self {
        AssemblerError::no_context(format!("IO error: {}", e))
    }
}

impl From<String> for AssemblerError {
    fn from(s: String) -> Self {
        AssemblerError::no_context(s)
    }
}

impl fmt::Display for AssemblerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.with_source_context())
    }
}

// Type alias for Result with AssemblerError
pub type Result<T> = std::result::Result<T, AssemblerError>;
