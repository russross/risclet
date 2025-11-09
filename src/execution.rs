use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
use std::io::{self, Read, Write};
use std::rc::Rc;

use crossterm::tty::IsTty;

use crate::checkabi::CheckABI;
use crate::config::{Config, Mode};
use crate::error::{Result, RiscletError};
use crate::memory::{CpuState, MemoryManager, Segment};
use crate::riscv::{Field, Op, fields_to_string};
use crate::trace::{Effects, ExecutionTrace, MemoryValue, RegisterValue};

pub struct Machine {
    state: CpuState,
    memory: MemoryManager,
    trace: ExecutionTrace,
    pc_start: u32,
    pub global_pointer: u32,
    pub address_symbols: HashMap<u32, String>,
    pub other_symbols: HashMap<String, u32>,
    current_effect: Option<Effects>,
    reservation_set: Option<u32>,
    #[cfg(test)]
    stdin_data: Vec<u8>,
    #[cfg(test)]
    stdin_pos: usize,
    #[cfg(test)]
    stdout_buffer: Vec<u8>,
}

impl Machine {
    pub fn new(
        segments: Vec<Segment>,
        pc_start: u32,
        global_pointer: u32,
        address_symbols: HashMap<u32, String>,
        other_symbols: HashMap<String, u32>,
    ) -> Self {
        let mut memory = MemoryManager::new(segments);
        memory.reset();

        let mut state = CpuState::new(pc_start);
        state.reset(pc_start, memory.layout.stack_end);

        let trace = ExecutionTrace::new(memory.layout);

        Self {
            state,
            memory,
            trace,
            pc_start,
            global_pointer,
            address_symbols,
            other_symbols,
            current_effect: None,
            reservation_set: None,
            #[cfg(test)]
            stdin_data: Vec::new(),
            #[cfg(test)]
            stdin_pos: 0,
            #[cfg(test)]
            stdout_buffer: Vec::new(),
        }
    }

    pub fn builder() -> MachineBuilder {
        MachineBuilder::new()
    }

    pub fn for_testing() -> Self {
        MachineBuilder::new().with_flat_memory(1024 * 1024).build()
    }

    pub fn reset(&mut self) {
        self.memory.reset();
        self.state.reset(self.pc_start, self.memory.layout.stack_end);
        self.trace.clear();
        self.current_effect = None;
        self.reservation_set = None;
    }

    pub fn load(&mut self, addr: u32, size: u32) -> Result<Vec<u8>> {
        let raw = self.memory.load_raw(addr, size)?;
        if let Some(effects) = &mut self.current_effect {
            assert!(effects.mem_read.is_none());
            effects.mem_read =
                Some(MemoryValue { address: addr, value: raw.to_vec() });
        }
        Ok(raw.to_vec())
    }

    pub fn load_i8(&mut self, addr: u32) -> Result<i32> {
        let bytes = self.load(addr, 1)?;
        Ok(i8::from_le_bytes(bytes[..1].try_into().unwrap()) as i32)
    }

    pub fn load_u8(&mut self, addr: u32) -> Result<i32> {
        let bytes = self.load(addr, 1)?;
        Ok(u8::from_le_bytes(bytes[..1].try_into().unwrap()) as i32)
    }

    pub fn load_i16(&mut self, addr: u32) -> Result<i32> {
        let bytes = self.load(addr, 2)?;
        Ok(i16::from_le_bytes(bytes[..2].try_into().unwrap()) as i32)
    }

    pub fn load_u16(&mut self, addr: u32) -> Result<i32> {
        let bytes = self.load(addr, 2)?;
        Ok(u16::from_le_bytes(bytes[..2].try_into().unwrap()) as i32)
    }

    pub fn load_i32(&mut self, addr: u32) -> Result<i32> {
        let bytes = self.load(addr, 4)?;
        Ok(i32::from_le_bytes(bytes[..4].try_into().unwrap()))
    }

    pub fn load_instruction(&self, addr: u32) -> Result<(i32, u32)> {
        self.memory.load_instruction(addr)
    }

    pub fn store(&mut self, addr: u32, raw: &[u8]) -> Result<()> {
        if let Some(effects) = &mut self.current_effect
            && let Ok(old_val) = self.memory.load(addr, raw.len() as u32)
        {
            assert!(effects.mem_write.is_none());
            effects.mem_write = Some((
                MemoryValue { address: addr, value: old_val },
                MemoryValue { address: addr, value: raw.to_vec() },
            ));
        }
        self.memory.store(addr, raw)
    }

    pub fn get(&mut self, reg: usize) -> i32 {
        let val = self.state.get_reg(reg);
        if reg != 0 && self.current_effect.is_some() {
            let effects = self.current_effect.as_mut().unwrap();
            if !effects.reg_reads.iter().any(|r| r.register == reg) {
                effects
                    .reg_reads
                    .push(RegisterValue { register: reg, value: val });
            }
        }
        val
    }

    pub fn set(&mut self, reg: usize, value: i32) {
        // Don't record effects for writes to the zero register (x0),
        // since the RegisterFile silently ignores them
        if reg != 0
            && let Some(effects) = &mut self.current_effect
        {
            assert!(effects.reg_write.is_none());
            let old_val = self.state.get_reg(reg);
            effects.reg_write = Some((
                RegisterValue { register: reg, value: old_val },
                RegisterValue { register: reg, value },
            ));
        }
        self.state.set_reg(reg, value);
    }

    pub fn set_pc(&mut self, value: u32) -> Result<()> {
        let old_pc = self.state.pc();
        self.state.set_pc(value);
        if value & 1 != 0 {
            return Err(RiscletError::execution_error(format!(
                "bus error: pc addr={:x}",
                value
            )));
        }
        if let Some(effects) = &mut self.current_effect {
            effects.pc = (old_pc, value);
        }
        Ok(())
    }

    /// Extract syscall signature for display before ecall execution
    /// Returns a string like "write(1, 0x1234, 5)" or None if not a recognized syscall
    pub fn ecall_signature_for_display(
        &self,
        hex_mode: bool,
    ) -> Option<String> {
        let syscall_num = self.get_reg(17); // a7
        match syscall_num {
            63 => {
                // read syscall
                let fd = self.get_reg(10); // a0
                let buf_addr = self.get_reg(11) as u32; // a1
                let count = self.get_reg(12); // a2
                Some(if hex_mode {
                    format!("read({}, 0x{:x}, 0x{:x})", fd, buf_addr, count)
                } else {
                    format!("read({}, 0x{:x}, {})", fd, buf_addr, count)
                })
            }
            64 => {
                // write syscall
                let fd = self.get_reg(10); // a0
                let buf_addr = self.get_reg(11) as u32; // a1
                let count = self.get_reg(12); // a2
                Some(if hex_mode {
                    format!("write({}, 0x{:x}, 0x{:x})", fd, buf_addr, count)
                } else {
                    format!("write({}, 0x{:x}, {})", fd, buf_addr, count)
                })
            }
            93 => {
                // exit syscall
                let status = self.get_reg(10) & 0xff; // a0
                Some(format!("exit({})", status))
            }
            _ => None, // unsupported syscall
        }
    }

    pub fn execute_and_collect_effects(
        &mut self,
        instruction: &Rc<Instruction>,
    ) -> Effects {
        self.current_effect = Some(Effects::new(instruction));

        let exec_res = instruction.op.execute(self, instruction.length);

        let mut effects = self.current_effect.take().unwrap();

        if effects.pc == (0, 0) {
            let old_pc = self.state.pc();
            let new_pc = old_pc + instruction.length;
            self.state.set_pc(new_pc);
            effects.pc = (old_pc, new_pc);
        }

        if let Err(msg) = exec_res {
            effects.error(msg);
        }

        self.trace.add(effects.clone());
        self.current_effect = None;

        effects
    }

    pub fn set_most_recent_memory(
        &mut self,
        _sequence: &[Effects],
        _index: usize,
    ) {
    }

    pub fn most_recent_memory(&self) -> u32 {
        let (addr, _, _) = self.trace.set_most_recent_memory();
        addr
    }

    pub fn most_recent_data(&self) -> (u32, usize) {
        let (_, addr_size, _) = self.trace.set_most_recent_memory();
        addr_size
    }

    pub fn most_recent_stack(&self) -> (u32, usize) {
        let (_, _, addr_size) = self.trace.set_most_recent_memory();
        addr_size
    }

    pub fn apply(&mut self, effect: &Effects, is_forward: bool) {
        let (old_pc, new_pc) = effect.pc;
        self.set_pc(if is_forward { new_pc } else { old_pc })
            .expect("PC should be valid during replay");

        if let Some((old, new)) = &effect.reg_write {
            let write = if is_forward { new } else { old };
            self.set(write.register, write.value);
        }

        if let Some((old, new)) = &effect.mem_write {
            let store = if is_forward { new } else { old };
            self.store(store.address, &store.value)
                .expect("Memory should be valid during replay");
        }

        if let Some(output) = &effect.stdout {
            if is_forward {
                self.stdout_mut().extend(output);
            } else {
                let new_len = self.stdout().len() - output.len();
                self.stdout_mut().truncate(new_len);
            }
        }

        if let Some(input) = &effect.stdin {
            if is_forward {
                self.stdout_mut().extend(input);
            } else {
                let new_len = self.stdout().len() - input.len();
                self.stdout_mut().truncate(new_len);
            }
        }

        if let Some(frame) = effect.function_start {
            if is_forward {
                self.push_stack_frame(frame);
            } else {
                self.pop_stack_frame();
            }
        }

        if let Some(frame) = effect.function_end {
            if is_forward {
                self.pop_stack_frame();
            } else {
                self.push_stack_frame(frame);
            }
        }
    }

    pub fn text_start(&self) -> u32 {
        self.memory.layout.text_start
    }

    pub fn text_end(&self) -> u32 {
        self.memory.layout.text_end
    }

    pub fn data_start(&self) -> u32 {
        self.memory.layout.data_start
    }

    pub fn data_end(&self) -> u32 {
        self.memory.layout.data_end
    }

    pub fn stack_start(&self) -> u32 {
        self.memory.layout.stack_start
    }

    pub fn stack_end(&self) -> u32 {
        self.memory.layout.stack_end
    }

    pub fn pc(&self) -> u32 {
        self.state.pc()
    }

    pub fn stdout(&self) -> &[u8] {
        self.state.stdout()
    }

    pub fn stdout_mut(&mut self) -> &mut Vec<u8> {
        self.state.stdout_mut()
    }

    pub fn stdin(&self) -> &[u8] {
        self.state.stdin()
    }

    pub fn stdin_mut(&mut self) -> &mut Vec<u8> {
        self.state.stdin_mut()
    }

    pub fn stack_frames(&self) -> &[u32] {
        self.state.stack_frames()
    }

    pub fn push_stack_frame(&mut self, frame: u32) {
        self.state.push_stack_frame(frame);
    }

    pub fn pop_stack_frame(&mut self) {
        self.state.pop_stack_frame();
    }

    pub fn get_reg(&self, reg: usize) -> i32 {
        self.state.get_reg(reg)
    }

    pub fn current_effect_mut(&mut self) -> Option<&mut Effects> {
        self.current_effect.as_mut()
    }

    pub fn read_stdin(&mut self, buffer: &mut [u8]) -> Result<usize> {
        #[cfg(test)]
        {
            let available = self.stdin_data.len() - self.stdin_pos;
            let to_read = std::cmp::min(buffer.len(), available);
            buffer[..to_read].copy_from_slice(
                &self.stdin_data[self.stdin_pos..self.stdin_pos + to_read],
            );
            self.stdin_pos += to_read;
            Ok(to_read)
        }
        #[cfg(not(test))]
        {
            let mut handle = io::stdin().lock();
            handle.read(buffer).map_err(|e| {
                RiscletError::syscall_error(format!(
                    "read syscall error: {}",
                    e
                ))
            })
        }
    }

    pub fn write_stdout(&mut self, data: &[u8]) -> Result<()> {
        // Just record the data; printing is handled by the trace function
        // (which respects the execution mode: run/debug echo immediately,
        // trace mode prints only from effects report)
        self.state.stdout_mut().extend_from_slice(data);
        Ok(())
    }

    pub fn set_reservation(&mut self, addr: u32) {
        self.reservation_set = Some(addr);
    }

    pub fn check_and_clear_reservation(&mut self, addr: u32) -> bool {
        if self.reservation_set == Some(addr) {
            self.reservation_set = None;
            true
        } else {
            false
        }
    }

    pub fn clear_reservation(&mut self) {
        self.reservation_set = None;
    }

    #[cfg(test)]
    pub fn with_stdin(mut self, data: Vec<u8>) -> Self {
        self.stdin_data = data;
        self.stdin_pos = 0;
        self
    }

    #[cfg(test)]
    pub fn get_stdout_buffer(&self) -> &[u8] {
        &self.stdout_buffer
    }
}

pub struct MachineBuilder {
    segments: Vec<Segment>,
    pc_start: u32,
    global_pointer: u32,
    address_symbols: HashMap<u32, String>,
    other_symbols: HashMap<String, u32>,
}

impl MachineBuilder {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            pc_start: 0x1000,
            global_pointer: 0,
            address_symbols: HashMap::new(),
            other_symbols: HashMap::new(),
        }
    }

    pub fn with_segments(mut self, segments: Vec<Segment>) -> Self {
        self.segments = segments;
        self
    }

    pub fn with_entry_point(mut self, pc_start: u32) -> Self {
        self.pc_start = pc_start;
        self
    }

    pub fn with_global_pointer(mut self, gp: u32) -> Self {
        self.global_pointer = gp;
        self
    }

    pub fn with_address_symbols(
        mut self,
        symbols: HashMap<u32, String>,
    ) -> Self {
        self.address_symbols = symbols;
        self
    }

    pub fn with_other_symbols(mut self, symbols: HashMap<String, u32>) -> Self {
        self.other_symbols = symbols;
        self
    }

    pub fn with_flat_memory(mut self, size: u32) -> Self {
        self.segments =
            vec![Segment::new(0x1000, 0x1000 + size, true, true, Vec::new())];
        self.pc_start = 0x1000;
        self
    }

    pub fn build(self) -> Machine {
        Machine::new(
            self.segments,
            self.pc_start,
            self.global_pointer,
            self.address_symbols,
            self.other_symbols,
        )
    }
}

impl Default for MachineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Instruction {
    pub address: u32,
    pub op: Op,
    pub length: u32,
    pub pseudo_index: usize,
    pub verbose_fields: Vec<Field>,
    pub pseudo_fields: Vec<Field>,
}

pub fn add_local_labels(m: &mut Machine, instructions: &[Instruction]) {
    let mut branch_targets: HashSet<u32> = HashSet::new();
    for inst in instructions {
        if let Some(target) = inst.op.branch_target(inst.address) {
            branch_targets.insert(target);
        }
    }

    let mut next_label = 1;
    for inst in instructions {
        #[allow(clippy::map_entry)]
        if m.address_symbols.contains_key(&inst.address) {
            next_label = 1;
        } else if branch_targets.contains(&inst.address) {
            m.address_symbols.insert(inst.address, next_label.to_string());
            next_label += 1;
        }
    }
}

/// Helper function to print a single instruction with its effects
fn print_instruction_trace(
    effects: &Effects,
    _instruction: &Rc<Instruction>,
    disassembly: &str,
    hex_mode: bool,
) {
    let effect_lines = effects.report(hex_mode);

    if !effect_lines.is_empty() && !effect_lines[0].is_empty() {
        println!("{}{}", disassembly, effect_lines[0]);
    } else {
        println!("{}", disassembly);
    }

    for line in &effect_lines[1..] {
        println!("    {}", line);
    }
}

pub fn trace(
    m: &mut Machine,
    instructions: &[Rc<Instruction>],
    addresses: &HashMap<u32, usize>,
    config: &Config,
) -> Vec<Effects> {
    let mut abi = CheckABI::new(
        m.get_reg(2) as u32,
        m.text_start(),
        m.text_end(),
        m.data_start(),
        m.data_end(),
        m.stack_start(),
        m.stack_end(),
    );
    let mut sequence: Vec<Effects> = Vec::new();
    let mut i = 0;
    let mut prev_pseudo_index: Option<usize> = None;
    let mut pending_pseudo_effects: Vec<Effects> = Vec::new();
    let echo_in = matches!(config.mode, Mode::Debug) && !io::stdin().is_tty();

    for steps in 1..=config.max_steps {
        if i >= instructions.len() || instructions[i].address != m.pc() {
            let Some(&new_i) = addresses.get(&m.pc()) else {
                if let Some(effects) = sequence.last_mut() {
                    effects.error(RiscletError::execution_error(
                        "next instruction not found".to_string(),
                    ));
                }
                break;
            };
            i = new_i;
        }

        let instruction = &instructions[i];

        // Special handling for ecall in trace mode: need to print before execution,
        // but also need to flush any pending pseudo effects first (in pseudo-mode)
        let is_ecall = matches!(instruction.op, Op::Ecall);
        if is_ecall
            && matches!(config.mode, Mode::Trace)
            && !config.verbose_instructions
        {
            // In pseudo-mode with ecall: flush pending effects before printing ecall line
            if !pending_pseudo_effects.is_empty() {
                let merged_effects =
                    merge_pseudo_effects(&pending_pseudo_effects);
                let first_inst = &pending_pseudo_effects[0].instruction;
                let fields =
                    &instructions[addresses[&first_inst.address]].pseudo_fields;
                let disassembly = fields_to_string(
                    config,
                    fields,
                    first_inst.address,
                    m.global_pointer,
                    first_inst.length == 2,
                    None,
                    &m.address_symbols,
                );
                print_instruction_trace(
                    &merged_effects,
                    first_inst,
                    &disassembly,
                    config.hex_mode,
                );
                pending_pseudo_effects.clear();
            }

            // Now print ecall line with syscall signature before execution
            let fields = &instruction.verbose_fields;
            let disassembly = fields_to_string(
                config,
                fields,
                instruction.address,
                m.global_pointer,
                instruction.length == 2,
                None,
                &m.address_symbols,
            );
            if let Some(syscall_sig) =
                m.ecall_signature_for_display(config.hex_mode)
            {
                println!("{}{}", disassembly, syscall_sig);
            } else {
                println!("{}", disassembly);
            }
        } else if is_ecall
            && matches!(config.mode, Mode::Trace)
            && config.verbose_instructions
        {
            // In verbose-mode with ecall: just print ecall line before execution
            let fields = &instruction.verbose_fields;
            let disassembly = fields_to_string(
                config,
                fields,
                instruction.address,
                m.global_pointer,
                instruction.length == 2,
                None,
                &m.address_symbols,
            );
            if let Some(syscall_sig) =
                m.ecall_signature_for_display(config.hex_mode)
            {
                println!("{}{}", disassembly, syscall_sig);
            } else {
                println!("{}", disassembly);
            }
        }

        let mut effects = m.execute_and_collect_effects(instruction);
        i += 1;

        // Echo stdout for run and debug modes (trace mode handles printing itself)
        if !effects.terminate
            && matches!(config.mode, Mode::Run | Mode::Debug)
            && let Some(output) = &effects.stdout
        {
            let mut handle = io::stdout().lock();
            if let Err(e) = handle.write(output) {
                effects.error(RiscletError::io(format!(
                    "error echoing stdout: {}",
                    e
                )));
            }
        }

        // Echo stdin for debug mode only
        if !effects.terminate
            && echo_in
            && let Some(input) = &effects.stdin
        {
            let mut handle = io::stdout().lock();
            if let Err(e) = handle.write(input) {
                effects.error(RiscletError::io(format!(
                    "error echoing stdin: {}",
                    e
                )));
            }
        }

        // Perform ABI checking if enabled
        if !effects.terminate
            && config.check_abi
            && let Err(msg) =
                abi.check_instruction(m, instruction, &mut effects)
        {
            effects.error(RiscletError::abi_violation(msg));
        }

        // For trace mode, handle pseudo-instruction printing
        if matches!(config.mode, Mode::Trace) {
            if config.verbose_instructions {
                // In verbose mode, print each instruction immediately
                // For ecall, we already printed the instruction with syscall signature before execution,
                // so only print the follow-up effect lines (not the first line with syscall signature)
                if is_ecall {
                    let effect_lines = effects.report(config.hex_mode);
                    for line in &effect_lines[1..] {
                        println!("    {}", line);
                    }
                } else {
                    let fields = &instruction.verbose_fields;
                    let disassembly = fields_to_string(
                        config,
                        fields,
                        instruction.address,
                        m.global_pointer,
                        instruction.length == 2,
                        None,
                        &m.address_symbols,
                    );
                    print_instruction_trace(
                        &effects,
                        instruction,
                        &disassembly,
                        config.hex_mode,
                    );
                }
            } else {
                // In non-verbose mode, ecall is already handled in the before-execution section
                // (pending effects were flushed and ecall line was printed)
                // So we only need to print the follow-up effect lines for ecall
                if is_ecall {
                    let effect_lines = effects.report(config.hex_mode);
                    for line in &effect_lines[1..] {
                        println!("    {}", line);
                    }
                    prev_pseudo_index = Some(instruction.pseudo_index);
                } else {
                    // Normal pseudo-instruction accumulation for non-ecall
                    let current_pseudo_index = instruction.pseudo_index;

                    if prev_pseudo_index != Some(current_pseudo_index) {
                        // We've moved to a new pseudo-instruction; print the accumulated effects
                        if !pending_pseudo_effects.is_empty() {
                            let merged_effects =
                                merge_pseudo_effects(&pending_pseudo_effects);
                            let first_inst =
                                &pending_pseudo_effects[0].instruction;
                            let fields = &instructions
                                [addresses[&first_inst.address]]
                                .pseudo_fields;
                            let disassembly = fields_to_string(
                                config,
                                fields,
                                first_inst.address,
                                m.global_pointer,
                                first_inst.length == 2,
                                None,
                                &m.address_symbols,
                            );
                            print_instruction_trace(
                                &merged_effects,
                                first_inst,
                                &disassembly,
                                config.hex_mode,
                            );
                            pending_pseudo_effects.clear();
                        }
                        prev_pseudo_index = Some(current_pseudo_index);
                    }

                    pending_pseudo_effects.push(effects.clone());
                }
            }
        }

        let terminate = effects.terminate;
        sequence.push(effects);
        if terminate {
            break;
        }

        if steps == config.max_steps {
            if let Some(last) = sequence.last_mut()
                && last.other_message.is_none()
            {
                last.error(RiscletError::execution_error(format!(
                    "stopped after {} steps",
                    config.max_steps
                )));
            }
        } else if !matches!(config.mode, Mode::Debug) {
            sequence.clear();
        }
    }

    // Print any remaining pending pseudo effects at the end
    if matches!(config.mode, Mode::Trace)
        && !config.verbose_instructions
        && !pending_pseudo_effects.is_empty()
    {
        let merged_effects = merge_pseudo_effects(&pending_pseudo_effects);
        let first_inst = &pending_pseudo_effects[0].instruction;
        let fields =
            &instructions[addresses[&first_inst.address]].pseudo_fields;
        let disassembly = fields_to_string(
            config,
            fields,
            first_inst.address,
            m.global_pointer,
            first_inst.length == 2,
            None,
            &m.address_symbols,
        );
        print_instruction_trace(
            &merged_effects,
            first_inst,
            &disassembly,
            config.hex_mode,
        );
    }

    sequence
}

/// Merge effects from multiple instructions in a pseudo-sequence into a single effect
/// showing the combined result
fn merge_pseudo_effects(effects_list: &[Effects]) -> Effects {
    if effects_list.is_empty() {
        panic!("merge_pseudo_effects called with empty list");
    }

    let mut merged = effects_list[0].clone();

    // Keep only the final register write (from the last instruction that wrote a register)
    for effects in &effects_list[1..] {
        if effects.reg_write.is_some() {
            merged.reg_write = effects.reg_write.clone();
        }
        if effects.mem_read.is_some() {
            merged.mem_read = effects.mem_read.clone();
        }
        if effects.mem_write.is_some() {
            merged.mem_write = effects.mem_write.clone();
        }
        // Update PC to the final PC from the last instruction
        merged.pc.1 = effects.pc.1;
    }

    // Only adjust pc.0 for actual multi-instruction sequences.
    // For single instructions (including branches/jumps), keep the original PC values
    // so that branches and jumps correctly show as PC changes.
    if effects_list.len() > 1 {
        // Adjust pc.0 so that the sequential PC calculation (pc.0 + instruction.length)
        // equals the final PC, preventing the report() method from showing an "unexpected" PC change
        // for pseudo-instruction sequences that execute sequentially.
        merged.pc.0 = merged.pc.1 - merged.instruction.length;
    }

    merged
}
