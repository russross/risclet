use crate::memory::{MemoryManager, CpuState, Segment};
use crate::riscv::{Op, Field};
use crate::linter::Linter;
use crate::trace::{ExecutionTrace, Effects, MemoryValue, RegisterValue};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use crossterm::tty::IsTty;

pub struct Machine {
    state: CpuState,
    memory: MemoryManager,
    trace: ExecutionTrace,
    pc_start: i64,
    pub global_pointer: i64,
    pub address_symbols: HashMap<i64, String>,
    pub other_symbols: HashMap<String, i64>,
    current_effect: Option<Effects>,
}

impl Machine {
    pub fn new(
        segments: Vec<Segment>,
        pc_start: i64,
        global_pointer: i64,
        address_symbols: HashMap<i64, String>,
        other_symbols: HashMap<String, i64>,
    ) -> Self {
        let mut memory = MemoryManager::new(segments);
        memory.reset();
        
        let mut state = CpuState::new(pc_start);
        state.reset(pc_start, memory.layout.stack_end);

        let trace = ExecutionTrace::new(memory.layout);

        let machine = Self {
            state,
            memory,
            trace,
            pc_start,
            global_pointer,
            address_symbols,
            other_symbols,
            current_effect: None,
        };

        machine
    }

    pub fn reset(&mut self) {
        self.memory.reset();
        self.state.reset(self.pc_start, self.memory.layout.stack_end);
        self.trace.clear();
        self.current_effect = None;
    }

    pub fn load(&mut self, addr: i64, size: i64) -> Result<Vec<u8>, String> {
        let raw = self.memory.load_raw(addr, size)?;
        if let Some(effects) = &mut self.current_effect {
            assert!(effects.mem_read.is_none());
            effects.mem_read = Some(MemoryValue { address: addr, value: raw.to_vec() });
        }
        Ok(raw.to_vec())
    }

    pub fn load_i8(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 1)?;
        Ok(i8::from_le_bytes(bytes[..1].try_into().unwrap()) as i64)
    }

    pub fn load_u8(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 1)?;
        Ok(u8::from_le_bytes(bytes[..1].try_into().unwrap()) as i64)
    }

    pub fn load_i16(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 2)?;
        Ok(i16::from_le_bytes(bytes[..2].try_into().unwrap()) as i64)
    }

    pub fn load_u16(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 2)?;
        Ok(u16::from_le_bytes(bytes[..2].try_into().unwrap()) as i64)
    }

    pub fn load_i32(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 4)?;
        Ok(i32::from_le_bytes(bytes[..4].try_into().unwrap()) as i64)
    }

    pub fn load_u32(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 4)?;
        Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()) as i64)
    }

    pub fn load_i64(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 8)?;
        Ok(i64::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn load_instruction(&self, addr: i64) -> Result<(i32, i64), String> {
        self.memory.load_instruction(addr)
    }

    pub fn store(&mut self, addr: i64, raw: &[u8]) -> Result<(), String> {
        if let Some(effects) = &mut self.current_effect {
            if let Ok(old_val) = self.memory.load(addr, raw.len() as i64) {
                assert!(effects.mem_write.is_none());
                effects.mem_write = Some((
                    MemoryValue { address: addr, value: old_val },
                    MemoryValue { address: addr, value: raw.to_vec() },
                ));
            }
        }
        self.memory.store(addr, raw)
    }

    pub fn get(&mut self, reg: usize) -> i64 {
        let val = self.state.get_reg(reg);
        if reg != 0 && self.current_effect.is_some() {
            let effects = self.current_effect.as_mut().unwrap();
            if !effects.reg_reads.iter().any(|r| r.register == reg) {
                effects.reg_reads.push(RegisterValue { register: reg, value: val });
            }
        }
        val
    }

    pub fn get32(&mut self, reg: usize) -> i32 {
        self.get(reg) as i32
    }

    pub fn set(&mut self, reg: usize, value: i64) {
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

    pub fn set32(&mut self, reg: usize, value: i32) {
        self.set(reg, value as i64);
    }

    pub fn set_pc(&mut self, value: i64) -> Result<(), String> {
        let old_pc = self.state.pc();
        self.state.set_pc(value);
        if self.state.pc() & 1 != 0 {
            return Err(format!("bus error: pc addr={}", self.state.pc()));
        }
        if let Some(effects) = &mut self.current_effect {
            effects.pc = (old_pc, self.state.pc());
        }
        Ok(())
    }

    pub fn execute_and_collect_effects(&mut self, instruction: &Rc<Instruction>) -> Effects {
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

    pub fn set_most_recent_memory(&mut self, _sequence: &[Effects], _index: usize) {
    }

    pub fn most_recent_memory(&self) -> i64 {
        let (addr, _, _) = self.trace.set_most_recent_memory();
        addr
    }

    pub fn most_recent_data(&self) -> (i64, usize) {
        let (_, addr_size, _) = self.trace.set_most_recent_memory();
        addr_size
    }

    pub fn most_recent_stack(&self) -> (i64, usize) {
        let (_, _, addr_size) = self.trace.set_most_recent_memory();
        addr_size
    }

    pub fn apply(&mut self, effect: &Effects, is_forward: bool) {
        let (old_pc, new_pc) = effect.pc;
        self.set_pc(if is_forward { new_pc } else { old_pc }).expect("PC should be valid during replay");

        if let Some((old, new)) = &effect.reg_write {
            let write = if is_forward { new } else { old };
            self.set(write.register, write.value);
        }

        if let Some((old, new)) = &effect.mem_write {
            let store = if is_forward { new } else { old };
            self.store(store.address, &store.value).expect("Memory should be valid during replay");
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

    pub fn text_start(&self) -> i64 {
        self.memory.layout.text_start
    }

    pub fn text_end(&self) -> i64 {
        self.memory.layout.text_end
    }

    pub fn data_start(&self) -> i64 {
        self.memory.layout.data_start
    }

    pub fn data_end(&self) -> i64 {
        self.memory.layout.data_end
    }

    pub fn stack_start(&self) -> i64 {
        self.memory.layout.stack_start
    }

    pub fn stack_end(&self) -> i64 {
        self.memory.layout.stack_end
    }

    pub fn pc(&self) -> i64 {
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

    pub fn stack_frames(&self) -> &[i64] {
        self.state.stack_frames()
    }

    pub fn push_stack_frame(&mut self, frame: i64) {
        self.state.push_stack_frame(frame);
    }

    pub fn pop_stack_frame(&mut self) {
        self.state.pop_stack_frame();
    }

    pub fn get_reg(&self, reg: usize) -> i64 {
        self.state.get_reg(reg)
    }

    pub fn current_effect_mut(&mut self) -> Option<&mut Effects> {
        self.current_effect.as_mut()
    }
}

pub struct Instruction {
    pub address: i64,
    pub op: Op,
    pub length: i64,
    pub pseudo_index: usize,
    pub verbose_fields: Vec<Field>,
    pub pseudo_fields: Vec<Field>,
}

pub fn add_local_labels(m: &mut Machine, instructions: &[Instruction]) {
    let mut branch_targets: HashSet<i64> = HashSet::new();
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
    addresses: &HashMap<i64, usize>,
    lint: bool,
    max_steps: usize,
    mode: &str,
) -> Vec<Effects> {
    let mut linter = Linter::new(m.get_reg(2));
    let mut sequence: Vec<Effects> = Vec::new();
    let mut i = 0;
    let echo_in = [/* "run", */ "debug"].contains(&mode) && !io::stdin().is_tty();

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

        if !effects.terminate && ["run", "debug"].contains(&mode) {
            if let Some(output) = &effects.stdout {
                let mut handle = io::stdout().lock();
                if let Err(e) = handle.write(output) {
                    effects.error(format!("error echoing stdout: {}", e));
                }
            }
        }

        if !effects.terminate && echo_in {
            if let Some(input) = &effects.stdin {
                let mut handle = io::stdout().lock();
                if let Err(e) = handle.write(input) {
                    effects.error(format!("error echoing stdin: {}", e));
                }
            }
        }

        if !effects.terminate && lint {
            if let Err(msg) = linter.check_instruction(m, instruction, &mut effects) {
                effects.error(msg);
            }
        }

        let terminate = effects.terminate;
        sequence.push(effects);
        if terminate {
            break;
        }

        if steps == max_steps {
            if let Some(last) = sequence.last_mut() {
                if last.other_message.is_none() {
                    last.error(format!("stopped after {} steps", max_steps));
                }
            }
        } else if mode != "debug" {
            sequence.clear();
        }
    }

    sequence
}
