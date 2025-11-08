// Simulator module for risclet
//
// Provides the RISC-V simulation and debugging functionality

use crate::config::{Config, Mode};
use crate::elf_loader::{ElfInput, load_elf};
use crate::error::{Result, RiscletError};
use crate::execution::{Instruction, add_local_labels, trace};
use crate::riscv::{Op, fields_to_string, get_pseudo_sequence};
use crate::ui::Tui;
use std::collections::HashMap;
use std::rc::Rc;

/// Run the simulator with the specified ELF input (file or bytes)
pub fn run_simulator(config: &Config, input: ElfInput) -> Result<()> {
    let mut m = load_elf(input)?;
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
            if !config.verbose_instructions && instruction.pseudo_index == prev
            {
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
                    config,
                    fields,
                    instruction.address,
                    m.global_pointer,
                    instruction.length == 2,
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
        if let Some(effects) = sequence.last()
            && let Some(error) = &effects.other_message
        {
            match error {
                RiscletError::Exit(code) => {
                    std::process::exit(*code);
                }
                _ => {
                    eprintln!("{}", error);
                    std::process::exit(1);
                }
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
            config,
        )
        .map_err(RiscletError::ui)?;
        tui.main_loop().map_err(RiscletError::ui)?;
        return Ok(());
    }

    if let Some(effects) = sequence.last()
        && let Some(error) = &effects.other_message
    {
        match error {
            RiscletError::Exit(code) => {
                std::process::exit(*code);
            }
            _ => {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        }
    }
    eprintln!("program ended unexpectedly");
    std::process::exit(1);
}
