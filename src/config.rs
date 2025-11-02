// config.rs
//
// Configuration and CLI argument parsing for the RISC-V assembler

use crate::dump;
use std::env;

/// Complete configuration for the assembler
pub struct Config {
    pub input_files: Vec<String>,
    pub output_file: String,
    pub text_start: u32,
    pub verbose: bool,
    pub dump: dump::DumpConfig,
    pub relax: Relax,
}

/// Relaxation settings for instruction optimization
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relax {
    /// Enable GP-relative la optimization
    pub gp: bool,
    /// Enable call/tail pseudo-instruction optimization
    pub pseudo: bool,
    /// Enable automatic RV32C compressed encoding
    pub compressed: bool,
}

impl Relax {
    /// Create a new Relax configuration with all optimizations enabled
    pub fn all() -> Self {
        Relax { gp: true, pseudo: true, compressed: true }
    }

    /// Create a new Relax configuration with all optimizations disabled
    pub fn none() -> Self {
        Relax { gp: false, pseudo: false, compressed: false }
    }
}

/// Parse command-line arguments and return a Config object
pub fn process_cli_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(print_help(&args[0]));
    }

    let mut input_files = Vec::new();
    let mut output_file = "a.out".to_string();
    let mut text_start = 0x10000u32;
    let mut verbose = false;
    let mut dump_config = dump::DumpConfig::new();
    let mut relax = Relax::all();
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        // Handle --dump-* options
        if arg.starts_with("--dump-") {
            if arg.starts_with("--dump-ast") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_ast = Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-symbols") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_symbols = Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-values") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_values = Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-code") {
                let spec_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_code = Some(dump::parse_dump_spec(spec_str)?);
            } else if arg.starts_with("--dump-elf") {
                let parts_str = if arg.contains('=') {
                    arg.split('=').nth(1).unwrap_or("")
                } else {
                    ""
                };
                dump_config.dump_elf = Some(dump::parse_elf_parts(parts_str)?);
            } else {
                return Err(format!("Error: unknown option: {}", arg));
            }
        } else {
            match arg.as_str() {
                "-o" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("Error: -o requires an argument".to_string());
                    }
                    output_file = args[i].clone();
                }
                "-t" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("Error: -t requires an argument".to_string());
                    }
                    text_start = parse_address(&args[i])?;
                }
                "-v" | "--verbose" => {
                    verbose = true;
                }
                "--no-relax" => {
                    relax = Relax::none();
                }
                "--relax-gp" => {
                    relax.gp = true;
                }
                "--no-relax-gp" => {
                    relax.gp = false;
                }
                "--relax-pseudo" => {
                    relax.pseudo = true;
                }
                "--no-relax-pseudo" => {
                    relax.pseudo = false;
                }
                "--relax-compressed" => {
                    relax.compressed = true;
                }
                "--no-relax-compressed" => {
                    relax.compressed = false;
                }
                "-h" | "--help" => {
                    return Err(print_help(&args[0]));
                }
                _ => {
                    if arg.starts_with('-') {
                        return Err(format!("Error: unknown option: {}", arg));
                    }
                    input_files.push(arg.to_string());
                }
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        return Err("Error: no input files specified".to_string());
    }

    Ok(Config { input_files, output_file, text_start, verbose, dump: dump_config, relax })
}

/// Print help message
fn print_help(program_name: &str) -> String {
    format!(
        "Usage: {} [options] <file.s> [file.s...]

Options:
    -o <file>            Write output to <file> (default: a.out)
    -t <address>         Set text start address (default: 0x10000)
    -v, --verbose        Show input statistics and convergence progress
    --no-relax           Disable all relaxations
    --relax-gp           Enable GP-relative 'la' optimization (default: on)
    --no-relax-gp        Disable GP-relative 'la' optimization
    --relax-pseudo       Enable 'call'/'tail' pseudo-instruction optimization (default: on)
    --no-relax-pseudo    Disable 'call'/'tail' pseudo-instruction optimization
    --relax-compressed   Enable automatic RV32C compressed encoding (default: on)
    --no-relax-compressed Disable automatic RV32C compressed encoding
    -h, --help           Show this help message

Output Behavior:
  By default, successful assembly produces no output
  Use -v to see input statistics and convergence progress during assembly.
  Use --dump-* options for detailed inspections (AST, symbols, code, ELF) - disables output file.

Debug Dump Options:
  --dump-ast[=PASSES[:FILES]]     Dump AST after parsing (s-expression format)
  --dump-symbols[=PASSES[:FILES]] Dump after symbol linking with references
  --dump-values[=PASSES[:FILES]]  Dump symbol values for specific passes/files
  --dump-code[=PASSES[:FILES]]    Dump generated code for specific passes/files
  --dump-elf[=PARTS]              Dump detailed ELF info

  PASSES syntax:
    (empty)   Final pass only (default)
    N         Specific pass (e.g., 1, 2)
    N-M       Range (e.g., 1-3)
    N-        From N to end (e.g., 1- for all passes)
    -M        From start to M (e.g., -2 for first two)
    *         All passes

  FILES syntax:
    (empty)   All files (default)
    *         All files
    file1.s,file2.s  Specific files (comma-separated)

  PARTS syntax (for --dump-elf):
    (empty)   All parts (default)
    headers   ELF and program headers
    sections  Section headers
    symbols   Symbol table
    (comma-separated for multiple, e.g., headers,symbols)

Examples:
  ./assembler program.s                        # Silent on success
  ./assembler -v program.s                     # Show input stats and convergence progress
  ./assembler --dump-code program.s            # Dump generated code (no stats)
  ./assembler -v --dump-code program.s         # Show stats AND code dump
  ./assembler --dump-elf=headers,symbols prog.s # Dump ELF metadata

Note: When any --dump-* option is used, no output file is generated.",
        program_name
    )
}

/// Parse an address string (decimal or hex with 0x prefix)
fn parse_address(s: &str) -> Result<u32, String> {
    if let Some(hex) = s.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
            .map_err(|_| format!("Error: invalid hex address: {}", s))
    } else {
        s.parse::<u32>().map_err(|_| format!("Error: invalid address: {}", s))
    }
}
