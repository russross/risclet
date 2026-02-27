// config.rs
//
// Unified configuration and CLI argument parsing for risclet

use crate::dump;

/// Operating mode for risclet
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Default mode: auto-assemble *.s files or load a.out, then debug
    Default,
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
    /// Create default config for assemble mode
    pub fn assemble_default() -> Self {
        Config {
            mode: Mode::Assemble,
            verbose: false,
            max_steps: MAX_STEPS_DEFAULT,
            executable: EXECUTABLE_DEFAULT.to_string(),
            check_abi: false,
            hex_mode: false,
            show_addresses: false,
            verbose_instructions: false,
            input_files: Vec::new(),
            output_file: OUTPUT_FILE_DEFAULT.to_string(),
            text_start: TEXT_START_DEFAULT,
            dump: dump::DumpConfig::new(),
            relax: Relax { gp: None, pseudo: true, compressed: false },
        }
    }

    /// Create default config for simulator modes (run, debug, disassemble, trace)
    pub fn simulator_default(mode: Mode) -> Self {
        // show_addresses defaults to true for disassemble and trace modes
        let show_addresses = mode == Mode::Disassemble || mode == Mode::Trace;

        Config {
            mode,
            verbose: false,
            max_steps: MAX_STEPS_DEFAULT,
            executable: EXECUTABLE_DEFAULT.to_string(),
            check_abi: false,
            hex_mode: false,
            show_addresses,
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
pub fn parse_cli_args(args: &[String]) -> Result<Config, String> {
    if args.is_empty() {
        // No arguments: auto-detect *.s files or a.out, debug mode (default)
        return parse_default_mode(&[]);
    }

    // Check for subcommands
    match args[0].as_str() {
        "assemble" => parse_assemble_mode(&args[1..]),
        "run" => parse_simulator_mode(&args[1..], Mode::Run),
        "debug" => parse_simulator_mode(&args[1..], Mode::Debug),
        "disassemble" => parse_simulator_mode(&args[1..], Mode::Disassemble),
        "trace" => parse_simulator_mode(&args[1..], Mode::Trace),
        "-h" | "--help" | "help" => Err(print_main_help()),
        "-v" | "--version" => {
            println!("risclet 0.4.3");
            std::process::exit(0);
        }
        _ => {
            // No recognized subcommand - treat as run mode with positional args
            // This allows: risclet foo.s, risclet a.out, risclet --hex, etc.
            parse_simulator_mode(args, Mode::Run)
        }
    }
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
        "--no-relax" => {
            *relax = Relax { gp: Some(false), pseudo: false, compressed: false };
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

/// Parse arguments for assemble mode
fn parse_assemble_mode(args: &[String]) -> Result<Config, String> {
    let mut config = Config::assemble_default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        // Handle --dump-* options
        if parse_dump_option(arg, &mut config)? {
        } else {
            match arg.as_str() {
                "-o" => {
                    config.output_file =
                        require_option_value(args, &mut i, "-o")?;
                }
                "-t" => {
                    let value = require_option_value(args, &mut i, "-t")?;
                    config.text_start = parse_address(&value)?;
                }
                "-v" | "--verbose" => {
                    config.verbose = true;
                }
                "-h" | "--help" => {
                    return Err(print_assemble_help(&config));
                }
                _ => {
                    if parse_relax_option(arg, &mut config.relax) {
                        i += 1;
                        continue;
                    }
                    if arg.starts_with('-') {
                        return Err(format!("Error: unknown option: {}", arg));
                    }
                    config.input_files.push(arg.to_string());
                }
            }
        }
        i += 1;
    }

    if config.input_files.is_empty() {
        return Err("Error: no input files specified".to_string());
    }

    Ok(config)
}

/// Parse arguments for simulator modes (run, debug, disassemble, trace)
fn parse_simulator_mode(args: &[String], mode: Mode) -> Result<Config, String> {
    let mut config = Config::simulator_default(mode);
    let mut has_explicit_executable = false;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "-e" | "--executable" => {
                config.executable =
                    require_option_value(args, &mut i, arg.as_str())?;
                has_explicit_executable = true;
            }
            "--check-abi" => {
                config.check_abi = true;
            }
            "--no-check-abi" => {
                config.check_abi = false;
            }
            "-s" | "--steps" => {
                let value =
                    require_option_value(args, &mut i, arg.as_str())?;
                config.max_steps = value.parse::<usize>().map_err(|_| {
                    format!("Error: invalid number of steps: {}", value)
                })?;
            }
            "--hex" => config.hex_mode = true,
            "--no-hex" => config.hex_mode = false,
            "--show-addresses" => config.show_addresses = true,
            "--no-show-addresses" => config.show_addresses = false,
            "--verbose-instructions" => config.verbose_instructions = true,
            "--no-verbose-instructions" => config.verbose_instructions = false,
            "-v" | "--verbose" => config.verbose = true,
            "-t" => {
                let value = require_option_value(args, &mut i, "-t")?;
                config.text_start = parse_address(&value)?;
            }
            "-h" | "--help" => {
                return Err(print_simulator_help(&config));
            }
            _ => {
                if parse_relax_option(arg, &mut config.relax) {
                    i += 1;
                    continue;
                } else if arg.starts_with("--dump-") {
                    return Err("Error: dump options (--dump-*) are not allowed with simulator subcommands (run/debug/disassemble/trace)".to_string());
                } else if arg.starts_with('-') {
                    return Err(format!("Error: unknown option: {}", arg));
                } else {
                    // Positional argument: could be .s file or executable
                    config.input_files.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    // Validate: cannot have both -e and positional file arguments
    if has_explicit_executable && !config.input_files.is_empty() {
        return Err("Error: cannot specify both -e/--executable and positional file arguments"
            .to_string());
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
    } else if config.executable == EXECUTABLE_DEFAULT {
        // No files specified and no explicit executable - try auto-detection
        config.input_files = find_assembly_files()?;
        if config.input_files.is_empty() {
            config.executable = EXECUTABLE_DEFAULT.to_string();
        }
    }

    Ok(config)
}

/// Parse default mode: auto-detect *.s files or a.out, default to debug mode
fn parse_default_mode(args: &[String]) -> Result<Config, String> {
    // Default mode is now Debug mode with file auto-detection
    // Just delegate to parse_simulator_mode with Mode::Debug
    parse_simulator_mode(args, Mode::Debug)
}

/// Find assembly files in current directory, or check for a.out
fn find_assembly_files() -> Result<Vec<String>, String> {
    use std::fs;

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

    if !asm_files.is_empty() {
        asm_files.sort();
        return Ok(asm_files);
    }

    // No .s files found, check for a.out
    if fs::metadata("a.out").is_ok() {
        // Return empty vec to signal we should just debug a.out
        return Ok(Vec::new());
    }

    Err("Error: no assembly files (*.s) or a.out found in current directory"
        .to_string())
}

/// Print main help message
fn print_main_help() -> String {
    let defaults = Config::simulator_default(Mode::Debug);

    format!(
        "Usage: risclet [subcommand] [files...] [options]

Default behavior (no subcommand):
  - With no arguments: auto-detects *.s files in current directory or a.out, then debugs
  - With .s files: assembles them in-memory and debugs
  - With executable: debugs the executable
  Default subcommand is 'debug'

Subcommands:
  assemble      Assemble RISC-V source files to executable on disk
  run           Run executable or .s files and exit (default if no subcommand)
  debug         Debug executable or .s files with interactive TUI
  disassemble   Disassemble executable or .s files
  trace         Execute and print each instruction with effects
  help, -h      Show this help message
  -v, --version Show version information

File Arguments:
  - One or more .s files: assembles in-memory, then runs/debugs/etc.
  - One executable (no .s extension): runs/debugs/disassembles that file
  - No files: auto-detects *.s files in current directory, or uses a.out

Common Options (all modes):
  --check-abi / --no-check-abi  Enable ABI checking (default: {})
  -s, --steps <count>           Max execution steps (default: {})
  --hex / --no-hex              Display values in hexadecimal
  --show-addresses              Show addresses in disassembly
  --verbose-instructions        Show strict instructions (not pseudo)
  -h, --help                    Show this help

Assembler Options (when using .s files):
  -v, --verbose                 Show assembly statistics
  -t <address>                  Set text start address (default: 0x{:x})
  --no-relax                    Disable all relaxations
  --relax-gp / --no-relax-gp    GP-relative optimization (default: auto-detect)
  --relax-pseudo / --no-relax-pseudo    call/tail optimization
  --relax-compressed / --no-relax-compressed    RV32C compression

Examples:
  risclet                          # Auto-detect *.s or a.out, debug (default)
  risclet prog.s                   # Assemble and debug prog.s
  risclet prog.s lib.s             # Assemble both files and debug
  risclet a.out                    # Debug a.out
  risclet run prog.s               # Assemble and run prog.s (exit after completion)
  risclet trace a.out --check-abi  # Trace a.out with ABI checking
  risclet disassemble prog.s       # Assemble and disassemble
  risclet assemble -o prog prog.s  # Assemble to disk as 'prog'

Use 'risclet <subcommand> --help' for subcommand-specific help.",
        if defaults.check_abi { "true" } else { "false" },
        defaults.max_steps,
        defaults.text_start
    )
}

/// Print assembler help message
fn print_assemble_help(config: &Config) -> String {
    format!(
        "Usage: risclet assemble [options] <file.s> [file.s...]

Options:
    -o <file>            Write output to <file> (default: {})
    -t <address>         Set text start address (default: 0x{:x})
    -v, --verbose        Show input statistics and relaxation progress
    --no-relax           Disable all relaxations
    --relax-gp           Enable GP-relative 'la' optimization (default: auto)
    --no-relax-gp        Disable GP-relative 'la' optimization
    --relax-pseudo       Enable 'call'/'tail' pseudo-instruction optimization (default: {})
    --no-relax-pseudo    Disable 'call'/'tail' pseudo-instruction optimization
    --relax-compressed   Enable automatic RV32C compressed encoding (default: {})
    --no-relax-compressed Disable automatic RV32C compressed encoding
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
        config.text_start,
        if config.relax.pseudo { "on" } else { "off" },
        if config.relax.compressed { "on" } else { "off" }
    )
}

/// Print simulator help message
fn print_simulator_help(config: &Config) -> String {
    let mode_str = match config.mode {
        Mode::Run => "run",
        Mode::Debug => "debug",
        Mode::Disassemble => "disassemble",
        Mode::Trace => "trace",
        _ => "simulator",
    };

    let mut help =
        format!("Usage: risclet {} [files...] [options]\n\n", mode_str);

    help.push_str("File Arguments:\n");
    help.push_str("  One or more .s files          Assemble in-memory, then ");
    help.push_str(mode_str);
    help.push('\n');
    help.push_str("  One executable (no .s ext)    ");
    help.push_str(match config.mode {
        Mode::Run => "Run",
        Mode::Debug => "Debug",
        Mode::Disassemble => "Disassemble",
        Mode::Trace => "Trace",
        _ => "Run",
    });
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

    help.push('\n');
    help.push_str("Assembler Options (when using .s files):\n");
    help.push_str("  -v, --verbose                 Show assembly statistics\n");
    help.push_str(&format!(
        "  -t <address>                  Set text start address (default: 0x{:x})\n",
        config.text_start
    ));
    help.push_str("  --no-relax                    Disable all relaxations\n");
    help.push_str("  --relax-gp / --no-relax-gp    GP-relative optimization (default: auto-detect)\n");
    help.push_str(&format!(
        "  --relax-pseudo / --no-relax-pseudo    call/tail optimization (default: {})\n",
        if config.relax.pseudo { "on" } else { "off" }
    ));
    help.push_str(&format!(
        "  --relax-compressed / --no-relax-compressed    RV32C compression (default: {})\n",
        if config.relax.compressed { "on" } else { "off" }
    ));

    help.push('\n');
    help.push_str("Other:\n");
    help.push_str("  -h, --help                    Show this help\n");

    if config.mode == Mode::Debug {
        help.push_str("\nInteractive Controls (in debugger):\n");
        help.push_str("  Press '?' in the debugger for keyboard shortcuts\n");
        help.push_str("  Key toggles: x (hex), v (verbose), a (addresses), r/o/s/d (panels)\n");
    }

    help.push_str("\nExamples:\n");
    help.push_str(&format!(
        "  risclet {} prog.s             # Assemble and {}\n",
        mode_str, mode_str
    ));
    help.push_str(&format!(
        "  risclet {} a.out              # {} a.out\n",
        mode_str,
        match config.mode {
            Mode::Run => "Run",
            Mode::Debug => "Debug",
            Mode::Disassemble => "Disassemble",
            Mode::Trace => "Trace",
            _ => "Run",
        }
    ));
    help.push_str(&format!(
        "  risclet {} -e binary          # {} using -e flag\n",
        mode_str,
        match config.mode {
            Mode::Run => "Run",
            Mode::Debug => "Debug",
            Mode::Disassemble => "Disassemble",
            Mode::Trace => "Trace",
            _ => "Run",
        }
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
