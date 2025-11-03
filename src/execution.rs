use crate::execution_context::ExecutionContext;
use crate::linter::Linter;
use crate::memory::{CpuState, MemoryManager, Segment};
use crate::riscv::{Field, Op};
use crate::trace::{Effects, ExecutionTrace, MemoryValue, RegisterValue};
use crossterm::tty::IsTty;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::rc::Rc;

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

    pub fn load(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, String> {
        let raw = self.memory.load_raw(addr, size)?;
        if let Some(effects) = &mut self.current_effect {
            assert!(effects.mem_read.is_none());
            effects.mem_read =
                Some(MemoryValue { address: addr, value: raw.to_vec() });
        }
        Ok(raw.to_vec())
    }

    pub fn load_i8(&mut self, addr: u32) -> Result<i32, String> {
        let bytes = self.load(addr, 1)?;
        Ok(i8::from_le_bytes(bytes[..1].try_into().unwrap()) as i32)
    }

    pub fn load_u8(&mut self, addr: u32) -> Result<i32, String> {
        let bytes = self.load(addr, 1)?;
        Ok(u8::from_le_bytes(bytes[..1].try_into().unwrap()) as i32)
    }

    pub fn load_i16(&mut self, addr: u32) -> Result<i32, String> {
        let bytes = self.load(addr, 2)?;
        Ok(i16::from_le_bytes(bytes[..2].try_into().unwrap()) as i32)
    }

    pub fn load_u16(&mut self, addr: u32) -> Result<i32, String> {
        let bytes = self.load(addr, 2)?;
        Ok(u16::from_le_bytes(bytes[..2].try_into().unwrap()) as i32)
    }

    pub fn load_i32(&mut self, addr: u32) -> Result<i32, String> {
        let bytes = self.load(addr, 4)?;
        Ok(i32::from_le_bytes(bytes[..4].try_into().unwrap()))
    }

    pub fn load_instruction(&self, addr: u32) -> Result<(i32, u32), String> {
        self.memory.load_instruction(addr)
    }

    pub fn store(&mut self, addr: u32, raw: &[u8]) -> Result<(), String> {
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
        if let Some(effects) = &mut self.current_effect {
            assert!(effects.reg_write.is_none());
            let old_val = self.state.get_reg(reg);
            effects.reg_write = Some((
                RegisterValue { register: reg, value: old_val },
                RegisterValue { register: reg, value },
            ));
        }
        self.state.set_reg(reg, value);
    }

    pub fn set_pc(&mut self, value: u32) -> Result<(), String> {
        let old_pc = self.state.pc();
        self.state.set_pc(value);
        if value & 1 != 0 {
            return Err(format!("bus error: pc addr={:x}", value));
        }
        if let Some(effects) = &mut self.current_effect {
            effects.pc = (old_pc, value);
        }
        Ok(())
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

    pub fn read_stdin(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
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
            handle.read(buffer).map_err(|e| format!("read syscall error: {}", e))
        }
    }

    pub fn write_stdout(&mut self, data: &[u8]) -> Result<(), String> {
        #[cfg(test)]
        {
            self.stdout_buffer.extend_from_slice(data);
            Ok(())
        }
        #[cfg(not(test))]
        {
            let mut handle = io::stdout().lock();
            handle
                .write_all(data)
                .map_err(|e| format!("write syscall error: {}", e))
        }
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

impl ExecutionContext for Machine {
    fn read_register(&mut self, reg: usize) -> i32 {
        self.get(reg)
    }

    fn write_register(&mut self, reg: usize, value: i32) {
        self.set(reg, value);
    }

    fn read_memory(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, String> {
        self.load(addr, size)
    }

    fn write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        self.store(addr, data)
    }

    fn read_pc(&self) -> u32 {
        self.pc()
    }

    fn write_pc(&mut self, pc: u32) -> Result<(), String> {
        self.set_pc(pc)
    }

    fn current_effects(&mut self) -> Option<&mut Effects> {
        self.current_effect_mut()
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

pub fn trace(
    m: &mut Machine,
    instructions: &[Rc<Instruction>],
    addresses: &HashMap<u32, usize>,
    lint: bool,
    max_steps: usize,
    mode: &str,
) -> Vec<Effects> {
    let mut linter = Linter::new(m.get_reg(2) as u32);
    let mut sequence: Vec<Effects> = Vec::new();
    let mut i = 0;
    let echo_in =
        [/* "run", */ "debug"].contains(&mode) && !io::stdin().is_tty();

    for steps in 1..=max_steps {
        if i >= instructions.len() || instructions[i].address != m.pc() {
            let Some(&new_i) = addresses.get(&m.pc()) else {
                if let Some(effects) = sequence.last_mut() {
                    effects.error("next instruction not found".to_string());
                }
                break;
            };
            i = new_i;
        }

        let instruction = &instructions[i];
        let mut effects = m.execute_and_collect_effects(instruction);
        i += 1;

        if !effects.terminate
            && ["run", "debug"].contains(&mode)
            && let Some(output) = &effects.stdout
        {
            let mut handle = io::stdout().lock();
            if let Err(e) = handle.write(output) {
                effects.error(format!("error echoing stdout: {}", e));
            }
        }

        if !effects.terminate
            && echo_in
            && let Some(input) = &effects.stdin
        {
            let mut handle = io::stdout().lock();
            if let Err(e) = handle.write(input) {
                effects.error(format!("error echoing stdin: {}", e));
            }
        }

        if !effects.terminate
            && lint
            && let Err(msg) =
                linter.check_instruction(m, instruction, &mut effects)
        {
            effects.error(msg);
        }

        let terminate = effects.terminate;
        sequence.push(effects);
        if terminate {
            break;
        }

        if steps == max_steps {
            if let Some(last) = sequence.last_mut()
                && last.other_message.is_none()
            {
                last.error(format!("stopped after {} steps", max_steps));
            }
        } else if mode != "debug" {
            sequence.clear();
        }
    }

    sequence
}
