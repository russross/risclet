// Simulator module for risclet
//
// Provides the RISC-V simulation and debugging functionality

use crate::config::{Config, Mode};
use crate::elf_loader::{ElfInput, load_elf_with_style};
use crate::error::{DiagnosticSources, Result, RiscletError, report};
use crate::execution::{Instruction, add_local_labels, trace};
use crate::riscv::{Op, get_pseudo_sequence};
use crate::ui::Tui;
use std::collections::HashMap;
use std::rc::Rc;

/// Run the simulator with the specified ELF input (file or bytes)
pub fn run_simulator(
    config: &Config,
    input: ElfInput,
    sources: DiagnosticSources,
) -> Result<i32> {
    let executable_name = match &input {
        ElfInput::File(name) => Some((*name).to_string()),
        ElfInput::Bytes(_) => None,
    };
    let mut m = load_elf_with_style(input, config.hex_mode)?;
    m.diagnostic_sources = sources;
    m.executable_name = executable_name;
    m.set_numeric_style(config.hex_mode);
    let mut instructions = Vec::new();
    let mut pc = m.text_start();
    while pc < m.text_end() {
        let (inst, length) = m.load_instruction(pc)?;
        let instruction = Instruction {
            address: pc,
            encoding: inst as u32,
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
                    instructions[i].op.to_pseudo_fields_with_symbols(
                        m.global_pointer,
                        &m.address_symbols,
                    );
                1
            };
            for inst in &mut instructions[i..i + n] {
                inst.verbose_fields = inst.op.to_encoding_fields(inst.encoding);
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
                if config.show_encoding {
                    println!(
                        "{}",
                        instruction.encoding_prefix(config).trim_end()
                    );
                }
                continue;
            } else {
                prev = instruction.pseudo_index;
            }

            // Listings omit trailing padding while trace reserves space for effects.
            println!(
                "{}",
                instruction
                    .listing(
                        config,
                        m.global_pointer,
                        &m.address_symbols,
                        config.verbose_instructions,
                    )
                    .trim_end()
            );
        }
        return Ok(0);
    }

    let instructions: Vec<Rc<Instruction>> =
        instructions.into_iter().map(Rc::new).collect();

    // Unified execution loop for run, debug, and trace modes
    let recording = trace(&mut m, &instructions, &addresses, config);
    let sequence = recording.effects;

    if config.mode == Mode::Debug {
        if sequence.is_empty() {
            return recording.outcome;
        }
        // The normal terminal retains the initial run's diagnostic after replay.
        if let Err(error) = &recording.outcome {
            report(error);
        }
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
        return Ok(0);
    }

    recording.outcome
}
