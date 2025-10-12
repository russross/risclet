// error.rs
//
// This file defines the AssemblerError type for the RISC-V assembler.
// It provides error handling with location and source context.

use std::fmt;
use std::fs;
use std::io::{self, BufRead};
use crate::ast::{Location};

/// An error type for the assembler, including location and message, with context formatting.
#[derive(Debug, Clone)]
pub struct AssemblerError {
    pub location: Location,
    pub message: String,
}

impl AssemblerError {
    pub fn from_context(message: String, location: Location) -> Self {
        AssemblerError { location, message }
    }

    pub fn with_source_context(&self) -> String {
        let file = fs::File::open(&self.location.file);
        if let Ok(file) = file {
            let reader = io::BufReader::new(file);
            let lines: Vec<String> = reader.lines().collect::<Result<_, _>>().unwrap_or_default();
            let line_num = self.location.line as usize;
            let start = if line_num > 3 { line_num - 3 } else { 0 };
            let end = (line_num + 3).min(lines.len());
            let mut context = String::new();
            for i in start..end {
                let marker = if i + 1 == line_num { ">>> " } else { "    " };
                context.push_str(&format!("{}{:4}: {}\n", marker, i + 1, lines[i]));
            }
            format!("Error at {}: {}\n{}", self.location, self.message, context)
        } else {
            format!("Error at {}: {} (could not read source file)", self.location, self.message)
        }
    }
}

impl fmt::Display for AssemblerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.with_source_context())
    }
}
