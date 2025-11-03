// Risclet: A RISC-V simulator and assembler
//
// Unified command-line interface for both tools

use risclet::assembler;
use risclet::config;
use risclet::simulator::{self, SimulatorConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help(&args[0]);
        std::process::exit(1);
    }

    match args[1].as_str() {
        "assemble" => {
            // Forward assembler subcommand arguments to config parser
            let config = match config::process_cli_args() {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };

            if let Err(e) = assembler::drive_assembler(&config) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }

        "run" => {
            let sim_config = parse_simulator_args(&args[2..], "run");
            if let Err(e) = simulator::run_simulator(sim_config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        "disassemble" => {
            let sim_config = parse_simulator_args(&args[2..], "disassemble");
            if let Err(e) = simulator::run_simulator(sim_config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        "debug" => {
            let sim_config = parse_simulator_args(&args[2..], "debug");
            if let Err(e) = simulator::run_simulator(sim_config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }

        "-h" | "--help" | "help" => {
            print_help(&args[0]);
        }

        "-v" | "--version" => {
            println!("risclet 0.4.0");
        }

        _ => {
            eprintln!("Unknown subcommand: {}", args[1]);
            eprintln!();
            print_help(&args[0]);
            std::process::exit(1);
        }
    }
}

fn parse_simulator_args(args: &[String], mode: &str) -> SimulatorConfig {
    let mut config = SimulatorConfig {
        mode: mode.to_string(),
        ..Default::default()
    };

    let mut i = 0;
    let mut usage = false;

    while i < args.len() {
        match args[i].as_str() {
            "-e" | "--executable" => {
                i += 1;
                if i < args.len() {
                    config.executable = args[i].clone();
                } else {
                    eprintln!("missing argument for {}", args[i - 1]);
                    usage = true;
                }
            }
            "-l" | "--lint" => {
                i += 1;
                if i < args.len() {
                    config.lint = args[i].to_lowercase() == "true";
                } else {
                    eprintln!("missing argument for {}", args[i - 1]);
                    usage = true;
                }
            }
            "-s" | "--steps" => {
                i += 1;
                if i < args.len() {
                    if let Ok(steps) = args[i].parse::<usize>() {
                        config.max_steps = steps;
                    } else {
                        eprintln!("invalid number of steps: {}", args[i]);
                        usage = true;
                    }
                } else {
                    eprintln!("missing argument for {}", args[i - 1]);
                    usage = true;
                }
            }
            "-h" | "--help" => {
                print_simulator_help(mode);
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                usage = true;
            }
        }
        i += 1;
    }

    if usage {
        print_simulator_help(mode);
        std::process::exit(1);
    }

    config
}

fn print_help(program_name: &str) {
    eprintln!("Usage: {} <subcommand> [options]", program_name);
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  assemble      Assemble RISC-V source files to executable");
    eprintln!("  run           Run a RISC-V executable and exit");
    eprintln!("  disassemble   Disassemble a RISC-V executable");
    eprintln!("  debug         Debug a RISC-V executable with interactive TUI (default)");
    eprintln!("  help, -h      Show this help message");
    eprintln!("  -v, --version Show version information");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} assemble -o a.out prog.s", program_name);
    eprintln!("  {} run -e a.out", program_name);
    eprintln!("  {} disassemble -e a.out", program_name);
    eprintln!("  {} debug -e a.out", program_name);
}

fn print_simulator_help(mode: &str) {
    eprintln!("Usage: risclet {} [options]", mode);
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -e, --executable <path>       Path to executable (default: a.out)");
    eprintln!("  -l, --lint <true|false>       Enable ABI checks (default: true)");
    eprintln!("  -s, --steps <count>           Max execution steps (default: 100000000)");
    eprintln!("  -h, --help                    Show this help");
}
