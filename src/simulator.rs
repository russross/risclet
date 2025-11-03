// Simulator module for risclet
//
// Provides the RISC-V simulation and debugging functionality

use crate::elf_loader::load_elf;
use crate::execution::{Instruction, add_local_labels, trace};
use crate::riscv::{Op, fields_to_string, get_pseudo_sequence};
use crate::ui::Tui;
use std::collections::HashMap;
use std::rc::Rc;

const MAX_STEPS_DEFAULT: usize = 100000000;

#[derive(Debug, Clone)]
pub struct SimulatorConfig {
    pub mode: String,       // "debug", "run", or "disassemble"
    pub executable: String, // Path to executable
    pub lint: bool,         // Enable linting
    pub max_steps: usize,   // Maximum execution steps
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        SimulatorConfig {
            mode: "debug".to_string(),
            executable: "a.out".to_string(),
            lint: true,
            max_steps: MAX_STEPS_DEFAULT,
        }
    }
}

pub fn run_simulator(config: SimulatorConfig) -> Result<(), String> {
    let mut m = load_elf(&config.executable)?;

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

    if config.mode == "disassemble" {
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

    let instructions: Vec<Rc<Instruction>> =
        instructions.into_iter().map(Rc::new).collect();

    let sequence = trace(
        &mut m,
        &instructions,
        &addresses,
        config.lint,
        config.max_steps,
        &config.mode,
    );

    if config.mode == "debug" {
        m.reset();
        m.set_most_recent_memory(&sequence, 0);
        let mut tui =
            Tui::new(m, instructions, addresses, pseudo_addresses, sequence)?;
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
