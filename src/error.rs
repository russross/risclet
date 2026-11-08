// error.rs
//
// This file defines the RiscletError type for the RISC-V assembler and simulator.
// Assembly errors retain source snapshots; runtime diagnostics carry labeled
// instruction sites that resolve to source snapshots or disassembly.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::result::Result as StdResult;

use crate::ast::{LinePointer, Location, Source};
use crate::symbols::BUILTIN_FILE_NAME;

// Source snapshots describe the exact input that produced these instructions.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticSources {
    pub files: HashMap<String, Vec<String>>,
    pub locations: HashMap<u32, Location>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticSite {
    pub address: u32,
    pub label: String,
    pub listing: Option<ContextListing>,
}

impl DiagnosticSite {
    pub fn new(address: u32, label: impl Into<String>) -> Self {
        Self { address, label: label.into(), listing: None }
    }
}

pub const STRICT_URL: &str =
    "https://github.com/russross/risclet/blob/main/STRICT.md";
pub const SYNTAX_URL: &str =
    "https://github.com/russross/risclet/blob/main/SYNTAX.md";

// Listings hold already formatted rows so source and disassembly use the same
// marker while preserving their own numbering and numeric display conventions.
#[derive(Debug, Clone)]
pub struct ContextListing {
    pub heading: String,
    pub rows: Vec<(bool, String)>,
}

pub fn context_window(index: usize, length: usize) -> std::ops::Range<usize> {
    let start = index.saturating_sub(3).min(length.saturating_sub(7));
    start..(start + 7).min(length)
}

impl ContextListing {
    pub fn source(location: &Location, lines: &[String]) -> Self {
        let selected = location.line.saturating_sub(1);
        let rows = context_window(selected, lines.len())
            .map(|index| {
                (
                    index == selected,
                    format!("{:6}: {}", index + 1, lines[index]),
                )
            })
            .collect();
        Self {
            heading: format!("at {}:{}:", location.file, location.line),
            rows,
        }
    }
}

impl fmt::Display for ContextListing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.heading)?;
        for (selected, row) in &self.rows {
            writeln!(f, "{}{row}", if *selected { ">>> " } else { "    " })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub pc: u32,
    pub step: usize,
    pub registers: Vec<(usize, i32)>,
    pub location: Option<Location>,
    pub hex: bool,
}

#[derive(Debug, Clone)]
pub enum DiagnosticContext {
    Execution(Box<ExecutionContext>),
    Assembly(Location),
    Operation(String),
}

pub fn address(value: u32, hex: bool) -> String {
    if hex { format!("0x{value:x}") } else { value.to_string() }
}

pub fn register_value(value: i32, hex: bool) -> String {
    if hex && !(0..=9).contains(&value) {
        format!("0x{value:x}")
    } else {
        value.to_string()
    }
}

// Primary and related sites share a renderer in the CLI, trace, and debugger.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub context: DiagnosticContext,
    pub sites: Vec<DiagnosticSite>,
    pub help: Option<String>,
    pub reference: Option<&'static str>,
}

impl From<String> for Diagnostic {
    fn from(message: String) -> Self {
        Self {
            message,
            context: DiagnosticContext::Operation(
                "during program preparation".into(),
            ),
            sites: Vec::new(),
            help: None,
            reference: None,
        }
    }
}

impl From<&str> for Diagnostic {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

pub fn command_line_diagnostic(message: &str) -> Diagnostic {
    let message = message.strip_prefix("Error: ").unwrap_or(message);
    let mut diagnostic = Diagnostic::from(message);
    diagnostic.context = DiagnosticContext::Operation(
        "while parsing command-line arguments".into(),
    );
    diagnostic
}

pub fn report(error: &impl fmt::Display) {
    // Buffered guest output belongs before the diagnostic on the normal console.
    if let Err(cause) = io::stdout().flush() {
        let output_error =
            RiscletError::io(format!("cannot flush program output: {cause}"));
        eprintln!("\n{output_error}");
    }
    eprintln!("\n{error}");
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ERROR: {}", summary(&self.message))?;
        // For assembly errors the source listing itself supplies the location.
        let source_is_context =
            matches!(self.context, DiagnosticContext::Assembly(_))
                && self.sites.first().is_some_and(|site| {
                    site.label.is_empty() && site.listing.is_some()
                });
        match &self.context {
            DiagnosticContext::Execution(context) => {
                write!(
                    f,
                    "at pc:{} step {}",
                    address(context.pc, context.hex),
                    context.step
                )?;
                if let Some(location) = &context.location {
                    write!(f, " in {}:{}", location.file, location.line)?;
                }
                for (index, (register, value)) in
                    context.registers.iter().enumerate()
                {
                    write!(
                        f,
                        "{}{}={}",
                        if index == 0 { " with " } else { ", " },
                        crate::riscv::R[*register],
                        register_value(*value, context.hex)
                    )?;
                }
                writeln!(f)?;
            }
            DiagnosticContext::Assembly(location) => {
                if !source_is_context {
                    writeln!(f, "at {}:{}", location.file, location.line)?;
                }
            }
            DiagnosticContext::Operation(operation) => {
                writeln!(f, "{operation}")?
            }
        }
        for (index, site) in self.sites.iter().enumerate() {
            if index != 0 || !source_is_context {
                writeln!(f)?;
            }
            if !site.label.is_empty() {
                writeln!(f, "{}", site.label)?;
            }
            if let Some(listing) = &site.listing {
                write!(f, "{listing}")?;
            }
        }
        if let Some(help) = &self.help {
            writeln!(f)?;
            write_wrapped_help(f, help)?;
        }
        if let Some(reference) = self.reference {
            writeln!(f, "\n    (see {reference})")?;
        }
        Ok(())
    }
}

// Static guidance wraps with a hanging indent; source text remains untouched.
fn write_wrapped_help(f: &mut fmt::Formatter<'_>, help: &str) -> fmt::Result {
    let mut column = 6;
    let mut first = true;
    write!(f, "help: ")?;
    for word in help.split_whitespace() {
        if !first && column + 1 + word.chars().count() >= 80 {
            write!(f, "\n    ")?;
            column = 4;
            first = true;
        }
        if !first {
            write!(f, " ")?;
            column += 1;
        }
        write!(f, "{word}")?;
        column += word.chars().count();
        first = false;
    }
    writeln!(f)
}

fn summary(message: &str) -> String {
    let mut text = message.trim().to_string();
    if text.as_bytes().get(1).is_some_and(u8::is_ascii_lowercase)
        && let Some(first) = text.get_mut(..1)
    {
        first.make_ascii_lowercase();
    }
    text
}

/// An error type for the RISC-V assembler and simulator.
/// Assembly errors carry source location for error reporting with context.
/// Runtime diagnostics retain primary and related instruction sites.
#[derive(Debug, Clone)]
pub enum RiscletError {
    /// Assembly-time errors with source location
    Assembly {
        location: Location,
        message: String,
        source_lines: Option<Vec<String>>,
        reference: Option<&'static str>,
    },
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
    AbiViolation(Box<Diagnostic>),
    Diagnostic(Box<Diagnostic>),
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
        RiscletError::Assembly {
            location,
            message,
            source_lines: None,
            reference: None,
        }
    }

    pub fn syntax_error(message: String, location: Location) -> Self {
        RiscletError::Assembly {
            location,
            message,
            source_lines: None,
            reference: Some(SYNTAX_URL),
        }
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
        RiscletError::Assembly {
            location,
            message,
            source_lines: None,
            reference: None,
        }
    }

    // File I/O error constructors
    pub fn io(message: String) -> Self {
        RiscletError::Io(message)
    }

    pub fn io_at(
        operation: &str,
        path: &str,
        cause: impl fmt::Display,
    ) -> Self {
        let mut diagnostic =
            Diagnostic::from(format!("cannot {operation}: {cause}"));
        diagnostic.context =
            DiagnosticContext::Operation(format!("at file '{path}'"));
        Self::Diagnostic(Box::new(diagnostic))
    }

    pub fn explained(message: impl Into<String>, help: &str) -> Self {
        let mut diagnostic = Diagnostic::from(message.into());
        diagnostic.help = Some(help.into());
        Self::Diagnostic(Box::new(diagnostic))
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

    pub fn abi_violation(message: impl Into<Diagnostic>) -> Self {
        let mut diagnostic = message.into();
        diagnostic.reference = Some(STRICT_URL);
        RiscletError::AbiViolation(Box::new(diagnostic))
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
            RiscletError::AbiViolation(msg) | RiscletError::Diagnostic(msg) => {
                msg.to_string()
            }
            RiscletError::Ui(msg) => msg.clone(),
            RiscletError::Internal(msg) => msg.clone(),
            RiscletError::Exit(code) => format!("exit({})", code),
        }
    }

    /// Every subsystem uses the same opening lines and context listing format.
    pub fn with_source_context(&self) -> String {
        if let Self::AbiViolation(diagnostic) | Self::Diagnostic(diagnostic) =
            self
        {
            return diagnostic.to_string();
        }
        if matches!(self, Self::Exit(_)) {
            return self.message();
        }
        let mut diagnostic = Diagnostic::from(self.message());
        diagnostic.context = DiagnosticContext::Operation(
            match self {
                Self::Io(_) => "during file or stream I/O",
                Self::Elf(_) => "while preparing the ELF executable",
                Self::Ui(_) => "in the debugger terminal",
                Self::Internal(_) => "inside Risclet",
                _ => "during execution",
            }
            .into(),
        );
        if let Self::Assembly { location, source_lines, reference, .. } = self {
            diagnostic.context = DiagnosticContext::Assembly(location.clone());
            diagnostic.reference = *reference;
            if reference.is_some() {
                diagnostic.help = Some(
                    "Check the syntax reference for supported source forms."
                        .into(),
                );
            }
            let lines = source_lines.clone().or_else(|| {
                if location.file == BUILTIN_FILE_NAME {
                    return None;
                }
                fs::read_to_string(&location.file)
                    .ok()
                    .map(|text| text.lines().map(str::to_owned).collect())
            });
            if let Some(lines) = lines {
                let mut site = DiagnosticSite::new(0, "");
                site.listing = Some(ContextListing::source(location, &lines));
                diagnostic.sites.push(site);
            }
        }
        diagnostic.to_string()
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

#[cfg(test)]
mod tests {
    use super::{
        ContextListing, Diagnostic, DiagnosticContext, RiscletError,
        context_window,
    };
    use crate::ast::Location;

    // Window boundaries retain seven rows when the input has enough context.
    #[test]
    fn source_context_centers_and_keeps_twelve_column_prefix() {
        let lines: Vec<_> =
            (1..=12).map(|line| format!("\tline {line}")).collect();
        let listing = ContextListing::source(
            &Location { file: "input.s".into(), line: 7 },
            &lines,
        )
        .to_string();
        let rows: Vec<_> = listing.lines().skip(1).collect();
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[3], ">>>      7: \tline 7");
        assert_eq!(&rows[3][12..], "\tline 7");
        assert_eq!(context_window(0, 12), 0..7);
        assert_eq!(context_window(11, 12), 5..12);
        assert_eq!(context_window(0, 2), 0..2);
    }

    #[test]
    fn assembly_context_without_a_listing_still_shows_its_location() {
        let mut diagnostic = Diagnostic::from("missing source".to_string());
        diagnostic.context = DiagnosticContext::Assembly(Location {
            file: "input.s".into(),
            line: 7,
        });
        assert_eq!(
            diagnostic.to_string(),
            "ERROR: missing source\nat input.s:7\n"
        );
    }

    #[test]
    fn help_uses_hanging_indent_and_strict_reference() {
        let mut diagnostic =
            Diagnostic::from("Cannot read a saved value".to_string());
        diagnostic.help = Some("The callee may overwrite a caller-saved register. Keep values needed across calls in saved registers, or write them to the stack before the call and read them back afterward.".into());
        let text = RiscletError::abi_violation(diagnostic).to_string();
        assert!(text.starts_with(
            "ERROR: cannot read a saved value\nduring program preparation\n"
        ));
        let help =
            text.split("help: ").nth(1).unwrap().split("\n\n").next().unwrap();
        assert!(help.lines().count() > 1);
        assert!(help.lines().all(|line| line.chars().count() < 80));
        assert!(help.lines().skip(1).all(|line| line.starts_with("    ")));
        assert!(text.contains("\n    (see https://github.com/russross/risclet/blob/main/STRICT.md)"));
    }
}
