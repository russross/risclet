pub mod memory;
pub mod riscv;
pub mod ui;
pub mod elf;
pub mod linter;
pub mod trace;
pub mod execution;
pub mod io_abstraction;
pub mod execution_context;
pub mod memory_interface;
pub mod linter_context;
pub mod decoder;
pub mod test_utils;

use self::execution::{Machine, Instruction, add_local_labels, trace};
use self::trace::Effects;
use self::ui::*;
use self::elf::*;
use self::riscv::{Op, get_pseudo_sequence, fields_to_string};
use std::collections::HashMap;
use std::cmp::min;
use std::fmt;
use std::rc::Rc;

const MAX_STEPS_DEFAULT: usize = 100000000;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();

    let mut mode = String::from("debug");
    let mut executable = String::from("a.out");
    let mut lint = String::from("true");

    let mut usage = false;
    let mut i = 1;
    let mut max_steps = MAX_STEPS_DEFAULT;
    while i < args.len() {
        match args[i].as_str() {
            "-m" | "--mode" => {
                i += 1;
                if i < args.len() {
                    mode = args[i].clone();
                    if !["run", "dasm", "debug"].contains(&mode.as_str()) {
                        eprintln!("invalid mode");
                        usage = true;
                    }
                } else {
                    eprintln!("missing argument for {}", args[i]);
                    usage = true;
                }
            }
            "-e" | "--executable" => {
                i += 1;
                if i < args.len() {
                    executable = args[i].clone();
                } else {
                    eprintln!("missing argument for {}", args[i]);
                    usage = true;
                }
            }
            "-l" | "--lint" => {
                i += 1;
                if i < args.len() {
                    lint = args[i].clone();
                    if !["true", "false"].contains(&lint.as_str()) {
                        eprintln!("invalid value for lint");
                        usage = true;
                    }
                } else {
                    eprintln!("missing argument for {}", args[i]);
                    usage = true;
                }
            }
            "-s" | "--steps" => {
                i += 1;
                if i < args.len() {
                    if let Ok(steps) = args[i].parse::<usize>() {
                        max_steps = steps;
                    } else {
                        eprintln!("{} with invalid number of steps {}", args[i - 1], args[i]);
                        usage = true;
                    };
                } else {
                    eprintln!("missing argument for {}", args[i]);
                    usage = true;
                }
            }
            "-v" | "--version" => {
                println!("0.3.11");
                std::process::exit(0);
            }
            "-h" | "--help" => usage = true,
            _ => usage = true,
        }
        i += 1;
    }
    if usage {
        eprintln!("Usage: risclet [options]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -e, --executable <path>            Path of executable to run (default a.out)");
        eprintln!("  -l, --lint <true|false>            Apply strict ABI and other checks (default true)");
        eprintln!("  -m, --mode <run|dasm|debug>        Simulator Mode (default debug)");
        eprintln!("  -s, --steps <count>                Maximum steps to run (default {})", MAX_STEPS_DEFAULT);
        eprintln!("  -v, --version                      Print version number");
        eprintln!("  -h, --help                         Show this help");
        std::process::exit(1);
    }

    let mut m = load_elf(&executable)?;

    let mut instructions = Vec::new();
    let mut pc = m.text_start();
    while pc < m.text_end() {
        let (inst, length) = m.load_instruction(pc)?;
        let instruction = Instruction {
            address: pc,
            op: Op::new(inst),
            length,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        };
        instructions.push(instruction);
        pc += length;
    }
    let mut addresses = HashMap::new();
    for (index, instruction) in instructions.iter().enumerate() {
        addresses.insert(instruction.address, index);
    }
    add_local_labels(&mut m, &instructions);

    let mut pseudo_addresses = HashMap::new();
    {
        let mut i = 0;
        let mut j = 0;
        while i < instructions.len() {
            let n = if let Some((n, fields)) = get_pseudo_sequence(&instructions[i..], &m.address_symbols) {
                instructions[i].pseudo_fields = fields;
                n
            } else {
                instructions[i].pseudo_fields = instructions[i].op.to_pseudo_fields();
                1
            };
            for inst in &mut instructions[i..i + n] {
                inst.verbose_fields = inst.op.to_fields();
                inst.pseudo_index = j;
            }
            pseudo_addresses.insert(j, i);
            i += n;
            j += 1;
        }
    }

    if mode == "dasm" {
        let mut prev = usize::MAX;
        for instruction in &instructions {
            if instruction.pseudo_index == prev {
                continue;
            } else {
                prev = instruction.pseudo_index;
            }
            println!(
                "{}",
                fields_to_string(
                    &instruction.pseudo_fields,
                    instruction.address,
                    m.global_pointer,
                    instruction.length == 2,
                    false,
                    false,
                    false,
                    None,
                    &m.address_symbols
                )
            );
        }
        return Ok(());
    }

    let instructions: Vec<Rc<Instruction>> = instructions.into_iter().map(Rc::new).collect();

    let sequence = trace(&mut m, &instructions, &addresses, lint == "true", max_steps, &mode);

    if mode == "debug" {
        m.reset();
        m.set_most_recent_memory(&sequence, 0);
        let mut tui = Tui::new(m, instructions, addresses, pseudo_addresses, sequence)?;
        tui.main_loop()?;
        return Ok(());
    }

    if let Some(effects) = sequence.last() {
        if let (Op::Ecall, Some(msg)) = (&effects.instruction.op, &effects.other_message)
            && msg.starts_with("exit(") && msg.ends_with(")") {
                let n: i32 = msg[5..msg.len() - 1].parse().unwrap();
                std::process::exit(n);
            }

        if let Some(msg) = &effects.other_message {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    }
    eprintln!("program ended unexpectedly");
    std::process::exit(1);
}
