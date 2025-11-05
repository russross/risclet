// Simulator module for risclet
//
// Provides the RISC-V simulation and debugging functionality

use crate::config::{Config, Mode};
use crate::elf_loader::{load_elf, load_elf_from_memory};
use crate::execution::{Instruction, Machine, add_local_labels, trace};
use crate::riscv::{Op, fields_to_string, get_pseudo_sequence};
use crate::ui::Tui;
use std::collections::HashMap;
use std::rc::Rc;

pub fn run_simulator(config: &Config) -> Result<(), String> {
    let m = load_elf(&config.executable)?;
    run_simulator_impl(config, m)
}

pub fn run_simulator_from_memory(
    config: &Config,
    elf_bytes: &[u8],
) -> Result<(), String> {
    let m = load_elf_from_memory(elf_bytes)?;
    run_simulator_impl(config, m)
}

fn run_simulator_impl(config: &Config, mut m: Machine) -> Result<(), String> {
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
            let n = if let Some((n, fields)) =
                get_pseudo_sequence(&instructions[i..], &m.address_symbols)
            {
                instructions[i].pseudo_fields = fields;
                n
            } else {
                instructions[i].pseudo_fields =
                    instructions[i].op.to_pseudo_fields();
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

    if config.mode == Mode::Disassemble {
        let mut prev = usize::MAX;
        for instruction in &instructions {
            if instruction.pseudo_index == prev {
                continue;
            } else {
                prev = instruction.pseudo_index;
            }

            // Choose fields based on verbose_instructions setting
            let fields = if config.verbose_instructions {
                &instruction.verbose_fields
            } else {
                &instruction.pseudo_fields
            };

            println!(
                "{}",
                fields_to_string(
                    fields,
                    instruction.address,
                    m.global_pointer,
                    instruction.length == 2,
                    config.hex_mode,
                    config.verbose_instructions,
                    config.show_addresses,
                    None,
                    &m.address_symbols
                )
            );
        }
        return Ok(());
    }

    let instructions: Vec<Rc<Instruction>> =
        instructions.into_iter().map(Rc::new).collect();

    // Unified execution loop for run, debug, and trace modes
    let sequence = trace(&mut m, &instructions, &addresses, config);

    // Handle exit codes and errors from trace execution
    if config.mode == Mode::Trace {
        if let Some(effects) = sequence.last() {
            if let (Op::Ecall, Some(msg)) =
                (&effects.instruction.op, &effects.other_message)
                && msg.starts_with("exit(")
                && msg.ends_with(")")
            {
                let n: i32 = msg[5..msg.len() - 1].parse().unwrap();
                std::process::exit(n);
            }

            if let Some(msg) = &effects.other_message {
                eprintln!("{}", msg);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if config.mode == Mode::Debug || config.mode == Mode::Default {
        m.reset();
        m.set_most_recent_memory(&sequence, 0);
        let mut tui = Tui::new(
            m,
            instructions,
            addresses,
            pseudo_addresses,
            sequence,
            config.hex_mode,
            config.show_addresses,
            config.verbose_instructions,
        )?;
        tui.main_loop()?;
        return Ok(());
    }

    if let Some(effects) = sequence.last() {
        if let (Op::Ecall, Some(msg)) =
            (&effects.instruction.op, &effects.other_message)
            && msg.starts_with("exit(")
            && msg.ends_with(")")
        {
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
