// config.rs
//
// Unified configuration and CLI argument parsing for risclet

use crate::dump;
use std::fs;

/// Operating mode for risclet
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Explicit assemble mode
    Assemble,
    /// Run mode: execute and exit
    Run,
    /// Debug mode: interactive TUI
    Debug,
    /// Disassemble mode: print disassembly and exit
    Disassemble,
    /// Trace mode: execute and print each instruction with effects
    Trace,
}

/// Informational requests do not require input discovery or execution.
pub enum CliAction {
    Execute(Box<Config>),
    Help(String),
    Version,
}

/// Complete unified configuration for risclet
#[derive(Clone)]
pub struct Config {
    // Mode
    pub mode: Mode,

    // Common options
    pub verbose: bool,
    pub max_steps: usize,

    // Simulator/debugger options
    pub executable: String,
    pub check_abi: bool,

    // Display options (for debug/disassemble modes)
    pub hex_mode: bool,
    pub show_addresses: bool,
    pub show_encoding: bool,
    pub verbose_instructions: bool,

    // Assembler-specific options
    pub input_files: Vec<String>,
    pub output_file: String,
    pub text_start: u32,
    pub dump: dump::DumpConfig,
    pub relax: Relax,
}

/// Relaxation settings for instruction optimization
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relax {
    /// GP-relative la optimization
    /// - None = auto-detect based on GP initialization
    /// - Some(true) = force enabled (--relax-gp)
    /// - Some(false) = force disabled (--no-relax-gp)
    pub gp: Option<bool>,
    /// Enable call/tail pseudo-instruction optimization
    pub pseudo: bool,
    /// Enable automatic RV32C compressed encoding
    pub compressed: bool,
}

impl Relax {
    /// Resolve the effective GP relaxation setting after auto-detection
    pub fn effective_gp(&self) -> bool {
        self.gp.unwrap_or(false)
    }
}

const MAX_STEPS_DEFAULT: usize = 100_000_000;
const TEXT_START_DEFAULT: u32 = 0x10000;
const OUTPUT_FILE_DEFAULT: &str = "a.out";
const EXECUTABLE_DEFAULT: &str = "a.out";

impl Config {
    /// Shared defaults, with listing preferences selected by command.
    pub fn for_mode(mode: Mode) -> Self {
        // Both listings show addresses; only disassembly shows encodings by default.
        let show_addresses = mode == Mode::Disassemble || mode == Mode::Trace;
        let show_encoding = mode == Mode::Disassemble;

        Config {
            mode,
            verbose: false,
            max_steps: MAX_STEPS_DEFAULT,
            executable: EXECUTABLE_DEFAULT.to_string(),
            check_abi: false,
            hex_mode: false,
            show_addresses,
            show_encoding,
            verbose_instructions: false,
            input_files: Vec::new(),
            output_file: OUTPUT_FILE_DEFAULT.to_string(),
            text_start: TEXT_START_DEFAULT,
            dump: dump::DumpConfig::new(),
            relax: Relax { gp: None, pseudo: true, compressed: false },
        }
    }
}

/// Parse command-line arguments - unified entry point
pub fn parse_cli_args(args: &[String]) -> Result<CliAction, String> {
    // Only the first argument selects a command; bare arguments default to run.
    let first = args.first().map(String::as_str);
    match first {
        Some("-h" | "--help" | "help") => {
            return Ok(CliAction::Help(print_main_help()));
        }
        Some("--version") => return Ok(CliAction::Version),
        _ => {}
    }
    let mode = match first {
        Some("assemble") => Mode::Assemble,
        Some("debug") => Mode::Debug,
        Some("disassemble") => Mode::Disassemble,
        Some("trace") => Mode::Trace,
        _ => Mode::Run,
    };
    let explicit = first.is_some_and(is_explicit_subcommand);
    let args = if explicit { &args[1..] } else { args };
    parse_options(args, mode, explicit)
}

fn is_explicit_subcommand(arg: &str) -> bool {
    matches!(arg, "assemble" | "run" | "debug" | "disassemble" | "trace")
}

fn option_value_after_equals(arg: &str) -> &str {
    arg.split_once('=').map_or("", |(_, value)| value)
}

fn parse_dump_option(arg: &str, config: &mut Config) -> Result<bool, String> {
    if !arg.starts_with("--dump-") {
        return Ok(false);
    }

    if arg.starts_with("--dump-ast") {
        let spec = dump::parse_dump_spec(option_value_after_equals(arg))?;
        config.dump.dump_ast = Some(spec);
    } else if arg.starts_with("--dump-symbols") {
        let spec = dump::parse_dump_spec(option_value_after_equals(arg))?;
        config.dump.dump_symbols = Some(spec);
    } else if arg.starts_with("--dump-values") {
        let spec = dump::parse_dump_spec(option_value_after_equals(arg))?;
        config.dump.dump_values = Some(spec);
    } else if arg.starts_with("--dump-code") {
        let spec = dump::parse_dump_spec(option_value_after_equals(arg))?;
        config.dump.dump_code = Some(spec);
    } else if arg.starts_with("--dump-elf") {
        let parts = dump::parse_elf_parts(option_value_after_equals(arg))?;
        config.dump.dump_elf = Some(parts);
    } else {
        return Err(format!("Error: unknown option: {}", arg));
    }

    Ok(true)
}

fn parse_relax_option(arg: &str, relax: &mut Relax) -> bool {
    match arg {
        "--relax" => {
            *relax = Relax { gp: Some(true), pseudo: true, compressed: true };
            true
        }
        "--no-relax" => {
            *relax =
                Relax { gp: Some(false), pseudo: false, compressed: false };
            true
        }
        "--relax-gp" => {
            relax.gp = Some(true);
            true
        }
        "--no-relax-gp" => {
            relax.gp = Some(false);
            true
        }
        "--relax-pseudo" => {
            relax.pseudo = true;
            true
        }
        "--no-relax-pseudo" => {
            relax.pseudo = false;
            true
        }
        "--relax-compressed" => {
            relax.compressed = true;
            true
        }
        "--no-relax-compressed" => {
            relax.compressed = false;
            true
        }
        _ => false,
    }
}

fn require_option_value(
    args: &[String],
    i: &mut usize,
    option: &str,
) -> Result<String, String> {
    *i += 1;
    if *i >= args.len() {
        return Err(format!("Error: {} requires an argument", option));
    }
    Ok(args[*i].clone())
}

/// Parse shared options once, then resolve explicit or inferred input.
fn parse_options(
    args: &[String],
    mode: Mode,
    explicit_command: bool,
) -> Result<CliAction, String> {
    let mut config = Config::for_mode(mode);
    let mut has_explicit_executable = false;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        // Relaxation is shared; inspection is available only in assemble mode.
        if parse_relax_option(arg, &mut config.relax) {
            i += 1;
            continue;
        }
        if arg.starts_with("--dump-") {
            if config.mode != Mode::Assemble {
                return Err("Error: dump options (--dump-*) are not allowed with simulator subcommands (run/debug/disassemble/trace)".to_string());
            }
            parse_dump_option(arg, &mut config)?;
            i += 1;
            continue;
        }
        // Options consume their values before positional input is interpreted.
        match (config.mode == Mode::Assemble, arg.as_str()) {
            (true, "-o") => {
                config.output_file = require_option_value(args, &mut i, "-o")?;
            }
            (false, "-e" | "--executable") => {
                config.executable =
                    require_option_value(args, &mut i, arg.as_str())?;
                has_explicit_executable = true;
            }
            (false, "--check-abi") => {
                config.check_abi = true;
            }
            (false, "--no-check-abi") => {
                config.check_abi = false;
            }
            (false, "-s" | "--steps") => {
                let value = require_option_value(args, &mut i, arg.as_str())?;
                config.max_steps = value.parse::<usize>().map_err(|_| {
                    format!("Error: invalid number of steps: {}", value)
                })?;
            }
            (false, "--show-encoding") => config.show_encoding = true,
            (false, "--no-show-encoding") => config.show_encoding = false,
            (false, "--hex") => config.hex_mode = true,
            (false, "--no-hex") => config.hex_mode = false,
            (false, "--show-addresses") => config.show_addresses = true,
            (false, "--no-show-addresses") => config.show_addresses = false,
            (false, "--verbose-instructions") => {
                config.verbose_instructions = true
            }
            (false, "--no-verbose-instructions") => {
                config.verbose_instructions = false
            }
            (_, "-v" | "--verbose") => config.verbose = true,
            (_, "-t") => {
                let value = require_option_value(args, &mut i, "-t")?;
                config.text_start = parse_address(&value)?;
            }
            (_, "-h" | "--help") => {
                // Help describes stable defaults, independent of preceding options.
                let defaults = Config::for_mode(config.mode.clone());
                let help = if config.mode == Mode::Assemble {
                    print_assemble_help(&defaults)
                } else if explicit_command {
                    print_simulator_help(&defaults)
                } else {
                    print_main_help()
                };
                return Ok(CliAction::Help(help));
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(format!("Error: unknown option: {}", arg));
                }
                if !explicit_command && is_explicit_subcommand(arg) {
                    return Err(format!(
                        "Error: subcommand '{}' must appear before options and file arguments",
                        arg
                    ));
                }
                config.input_files.push(arg.clone());
            }
        }
        i += 1;
    }

    resolve_input(&mut config, has_explicit_executable)?;
    Ok(CliAction::Execute(Box::new(config)))
}

/// Explicit input wins; discovery applies only when no input was supplied.
fn resolve_input(
    config: &mut Config,
    has_explicit_executable: bool,
) -> Result<(), String> {
    if has_explicit_executable && !config.input_files.is_empty() {
        return Err("Error: cannot specify both -e/--executable and positional file arguments"
            .to_string());
    }

    // Assemble accepts source paths regardless of extension and never loads ELF.
    if config.mode == Mode::Assemble {
        if config.input_files.is_empty() {
            config.input_files = find_assembly_files()?;
        }
        if config.input_files.is_empty() {
            return Err(
                "Error: no assembly files (*.s) found in current directory"
                    .to_string(),
            );
        }
        return Ok(());
    }

    // Determine input type and set appropriate fields
    if !config.input_files.is_empty() {
        // Check if all files are .s files or all are executables
        let s_files: Vec<_> =
            config.input_files.iter().filter(|f| f.ends_with(".s")).collect();
        let non_s_files: Vec<_> =
            config.input_files.iter().filter(|f| !f.ends_with(".s")).collect();

        if !s_files.is_empty() && !non_s_files.is_empty() {
            return Err(
                "Error: cannot mix .s files and executables as positional arguments"
                    .to_string(),
            );
        }

        if !s_files.is_empty() {
            // All are .s files - will assemble in memory
            // config.input_files stays as-is for assembler
        } else if non_s_files.len() == 1 {
            // Single executable
            config.executable = config.input_files[0].clone();
            config.input_files.clear();
        } else {
            // Multiple non-.s files
            return Err(
                "Error: can only specify one executable as a positional argument"
                    .to_string(),
            );
        }
    } else if !has_explicit_executable {
        // No files specified and no explicit executable - try auto-detection
        config.input_files = find_assembly_files()?;
        if config.input_files.is_empty()
            && fs::metadata(EXECUTABLE_DEFAULT).is_err()
        {
            return Err("Error: no assembly files (*.s) or a.out found in current directory".to_string());
        }
    }

    Ok(())
}

/// Discover source files in a stable order without choosing an executable.
fn find_assembly_files() -> Result<Vec<String>, String> {
    let mut asm_files = Vec::new();

    // Try to read current directory
    let entries = fs::read_dir(".")
        .map_err(|e| format!("Error reading current directory: {}", e))?;

    for entry in entries {
        if let Ok(entry) = entry
            && let Some(name) = entry.file_name().to_str()
            && name.ends_with(".s")
        {
            asm_files.push(name.to_string());
        }
    }

    asm_files.sort();
    Ok(asm_files)
}

/// Print main help message
fn print_main_help() -> String {
    let defaults = Config::for_mode(Mode::Run);

    format!(
        "Usage: risclet [subcommand] [files...] [options]

Default behavior (no subcommand):
  - With no arguments: auto-detects *.s files in current directory or a.out, then runs
  - With .s files: assembles them in-memory and runs
  - With executable: runs the executable
  Default subcommand is 'run'

Subcommands:
  assemble      Assemble RISC-V source files to executable on disk
  run           Run executable or .s files and exit (default if no subcommand)
  debug         Debug executable or .s files with interactive TUI
  disassemble   Disassemble executable or .s files
  trace         Execute and print each instruction with effects
  help, -h      Show this help message
  --version     Show version information

File Arguments:
  - One or more .s files: assembles in-memory, then runs/debugs/etc.
  - One executable (no .s extension): runs/debugs/disassembles that file
  - No files: auto-detects *.s files in current directory, or uses a.out
  - -e, --executable <path>: load that executable regardless of extension or nearby sources
  - assemble: treat explicit paths as source; with no paths, discover *.s files

Common Options:
  --check-abi / --no-check-abi  Enable ABI checking (default: {})
  -s, --steps <count>           Max execution steps (default: {})
  --hex / --no-hex              Display values in hexadecimal
  --show-addresses              Show addresses in disassembly
  --verbose-instructions        Show strict instructions (not pseudo)
  -h, --help                    Show this help

Assembler Options:
{}

Examples:
  risclet                          # Auto-detect *.s or a.out, run (default)
  risclet prog.s                   # Assemble and run prog.s
  risclet prog.s lib.s             # Assemble both files and run
  risclet a.out                    # Run a.out
  risclet run prog.s               # Assemble and run prog.s (exit after completion)
  risclet debug prog.s             # Assemble and debug prog.s with interactive TUI
  risclet trace a.out --check-abi  # Trace a.out with ABI checking
  risclet disassemble prog.s       # Assemble and disassemble
  risclet assemble -o prog prog.s  # Assemble to disk as 'prog'

Use 'risclet <subcommand> --help' for subcommand-specific help.",
        if defaults.check_abi { "true" } else { "false" },
        defaults.max_steps,
        assembler_options(&defaults)
    )
}

/// Print assembler help message
fn print_assemble_help(config: &Config) -> String {
    format!(
        "Usage: risclet assemble [options] [file.s...]

Inputs:
    With no files, assemble all *.s files in the current directory.
    Explicit paths are treated as source regardless of extension.

Options:
    -o <file>            Write output to <file> (default: {})
{}
    -h, --help           Show this help message

Output Behavior:
  By default, successful assembly produces no output
  Use -v to see input statistics and relaxation progress during assembly.
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
  risclet assemble program.s                        # Silent on success
  risclet assemble -v program.s                     # Show input stats and relaxation progress
  risclet assemble --dump-code program.s            # Dump generated code (no stats)
  risclet assemble -v --dump-code program.s         # Show stats AND code dump
  risclet assemble --dump-elf=headers,symbols prog.s # Dump ELF metadata

Note: When any --dump-* option is used, no output file is generated.",
        config.output_file,
        assembler_options(config)
    )
}

// Source assembly options have the same meaning in every command's help.
fn assembler_options(config: &Config) -> String {
    format!(
        "  -v, --verbose                 Show assembly statistics and relaxation progress
  -t <address>                  Set text start address (default: 0x{:x})
  --relax                       Enable all relaxations
  --no-relax                    Disable all relaxations
  --relax-gp / --no-relax-gp    GP-relative optimization (default: auto-detect)
  --relax-pseudo / --no-relax-pseudo    call/tail optimization (default: {})
  --relax-compressed / --no-relax-compressed    RV32C compression (default: {})",
        config.text_start,
        if config.relax.pseudo { "on" } else { "off" },
        if config.relax.compressed { "on" } else { "off" }
    )
}

/// Print simulator help message
fn print_simulator_help(config: &Config) -> String {
    let (mode_str, action) = match config.mode {
        Mode::Run => ("run", "Run"),
        Mode::Debug => ("debug", "Debug"),
        Mode::Disassemble => ("disassemble", "Disassemble"),
        Mode::Trace => ("trace", "Trace"),
        Mode::Assemble => ("assemble", "Assemble"),
    };

    let mut help =
        format!("Usage: risclet {} [files...] [options]\n\n", mode_str);

    help.push_str("File Arguments:\n");
    help.push_str("  One or more .s files          Assemble in-memory, then ");
    help.push_str(mode_str);
    help.push('\n');
    help.push_str("  One executable (no .s ext)    ");
    help.push_str(action);
    help.push_str(" the executable\n");
    help.push_str(
        "  No files                      Auto-detect *.s or use a.out\n",
    );
    help.push_str(
        "  -e, --executable <path>       Explicitly specify executable\n",
    );
    help.push('\n');

    help.push_str("Simulator Options:\n");
    help.push_str(&format!(
        "  --check-abi / --no-check-abi  Enable ABI checking (default: {})\n",
        if config.check_abi { "true" } else { "false" }
    ));
    help.push_str(&format!(
        "  -s, --steps <count>           Max execution steps (default: {})\n",
        config.max_steps
    ));

    if config.mode == Mode::Debug
        || config.mode == Mode::Disassemble
        || config.mode == Mode::Trace
    {
        help.push_str(
            "  --hex / --no-hex              Display values in hexadecimal\n",
        );
        help.push_str(&format!(
            "  --show-addresses              Show addresses in disassembly (default: {})\n",
            if config.show_addresses { "on" } else { "off" }
        ));
        help.push_str(
            "  --no-show-addresses           Hide addresses in disassembly\n",
        );
        help.push_str("  --verbose-instructions        Show strict instructions (not pseudo)\n");
        help.push_str(&format!(
            "  --no-verbose-instructions     Show pseudo-instructions (default: {})\n",
            if config.verbose_instructions {
                "on"
            } else {
                "off"
            }
        ));
    }

    if matches!(config.mode, Mode::Disassemble | Mode::Trace) {
        help.push_str(&format!(
            "  --show-encoding               Show original instruction hex (default: {})\n",
            if config.show_encoding { "on" } else { "off" }
        ));
        help.push_str(
            "  --no-show-encoding            Hide original instruction hex\n",
        );
    }

    help.push('\n');
    help.push_str("Assembler Options (when using .s files):\n");
    help.push_str(&assembler_options(config));
    help.push('\n');

    help.push('\n');
    help.push_str("Other:\n");
    help.push_str("  -h, --help                    Show this help\n");

    if config.mode == Mode::Debug {
        help.push_str("\nInteractive Controls (in debugger):\n");
        help.push_str("  Press '?' in the debugger for keyboard shortcuts\n");
        help.push_str("  Key toggles: x (hex), v (verbose), a (addresses), r/o/s/d/t (panels)\n");
    }

    help.push_str("\nExamples:\n");
    help.push_str(&format!(
        "  risclet {} prog.s             # Assemble and {}\n",
        mode_str, mode_str
    ));
    help.push_str(&format!(
        "  risclet {} a.out              # {} a.out\n",
        mode_str, action
    ));
    help.push_str(&format!(
        "  risclet {} -e binary          # {} using -e flag\n",
        mode_str, action
    ));
    if config.mode != Mode::Run {
        help.push_str(&format!(
            "  risclet {} prog.s --hex       # With hex display\n",
            mode_str
        ));
    }

    help
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

#[cfg(test)]
mod tests {
    use super::{CliAction, Config, Mode};

    // These cases exercise executable commands rather than informational output.
    fn parse_cli_args(args: &[String]) -> Result<Config, String> {
        match super::parse_cli_args(args)? {
            CliAction::Execute(config) => Ok(*config),
            _ => panic!("expected executable command"),
        }
    }

    #[test]
    fn parse_assemble_relax_enables_all_relaxations() {
        let args = vec![
            "assemble".to_string(),
            "--relax".to_string(),
            "prog.s".to_string(),
        ];
        let config = parse_cli_args(&args).expect("parse should succeed");

        assert_eq!(config.mode, Mode::Assemble);
        assert_eq!(config.relax.gp, Some(true));
        assert!(config.relax.pseudo);
        assert!(config.relax.compressed);
    }

    #[test]
    fn parse_simulator_relax_enables_all_relaxations_for_s_files() {
        let args = vec![
            "run".to_string(),
            "--relax".to_string(),
            "prog.s".to_string(),
        ];
        let config = parse_cli_args(&args).expect("parse should succeed");

        assert_eq!(config.mode, Mode::Run);
        assert_eq!(config.relax.gp, Some(true));
        assert!(config.relax.pseudo);
        assert!(config.relax.compressed);
    }

    #[test]
    fn parse_relax_can_be_overridden_by_no_relax() {
        let args = vec![
            "assemble".to_string(),
            "--relax".to_string(),
            "--no-relax".to_string(),
            "prog.s".to_string(),
        ];
        let config = parse_cli_args(&args).expect("parse should succeed");

        assert_eq!(config.relax.gp, Some(false));
        assert!(!config.relax.pseudo);
        assert!(!config.relax.compressed);
    }

    #[test]
    fn parse_rejects_subcommand_after_option() {
        let args =
            vec!["--hex".to_string(), "run".to_string(), "a.out".to_string()];

        let err = match parse_cli_args(&args) {
            Ok(_) => panic!("parse should fail"),
            Err(err) => err,
        };
        assert!(err.contains(
            "subcommand 'run' must appear before options and file arguments"
        ));
    }

    #[test]
    fn parse_rejects_assemble_after_option() {
        let args = vec![
            "--hex".to_string(),
            "assemble".to_string(),
            "prog.s".to_string(),
        ];

        let err = match parse_cli_args(&args) {
            Ok(_) => panic!("parse should fail"),
            Err(err) => err,
        };
        assert!(err.contains(
            "subcommand 'assemble' must appear before options and file arguments"
        ));
    }

    #[test]
    fn parse_allows_options_without_explicit_subcommand() {
        let args = vec!["--hex".to_string(), "a.out".to_string()];
        let config = parse_cli_args(&args).expect("parse should succeed");

        assert_eq!(config.mode, Mode::Run);
        assert!(config.hex_mode);
        assert_eq!(config.executable, "a.out");
        assert!(config.input_files.is_empty());
    }

    #[test]
    fn parse_short_v_as_verbose_not_version() {
        let args = vec!["-v".to_string(), "a.out".to_string()];
        let config = parse_cli_args(&args).expect("parse should succeed");

        assert_eq!(config.mode, Mode::Run);
        assert!(config.verbose);
        assert_eq!(config.executable, "a.out");
        assert!(config.input_files.is_empty());
    }
}
