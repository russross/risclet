pub mod riscv;
pub mod ui;
pub mod elf;

use self::riscv::*;
use self::ui::*;
use self::elf::*;
use crossterm::tty::IsTty;
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write as FmtWrite};
use std::io::{self, Read, Write};
use std::rc::Rc;

const STACK_SIZE: i64 = 8192;
const MAX_STEPS_DEFAULT: usize = 100000000;

pub struct Segment {
    start: i64,
    end: i64,
    mem: Vec<u8>,
    init: Vec<u8>,
    writeable: bool,
    executable: bool,
}

impl Segment {
    pub fn new(start: i64, end: i64, writeable: bool, executable: bool, init: Vec<u8>) -> Self {
        assert!(start > 0 && end > start);
        assert!(init.len() <= (end - start) as usize);
        Self { start, end, mem: Vec::new(), init, writeable, executable }
    }

    fn in_range(&self, addr: i64, size: i64) -> bool {
        addr >= self.start && addr + size <= self.end
    }

    fn reset(&mut self) {
        self.mem = self.init.clone();
        self.mem.resize((self.end - self.start) as usize, 0);
    }

    fn load(&self, addr: i64, size: i64, effects: &mut Option<Effects>) -> &[u8] {
        assert!(self.in_range(addr, size));
        let raw = &self.mem[(addr - self.start) as usize..(addr + size - self.start) as usize];
        if let Some(effects) = effects {
            assert!(effects.mem_read.is_none());
            effects.mem_read = Some(MemoryValue { address: addr, value: raw.to_vec() });
        }
        raw
    }

    fn store(&mut self, addr: i64, raw: &[u8], effects: &mut Option<Effects>) {
        assert!(self.in_range(addr, raw.len() as i64));
        let offset = (addr - self.start) as usize;
        if let Some(effects) = effects {
            assert!(effects.mem_write.is_none());
            let old_val = self.mem[offset..offset + raw.len()].to_vec();
            effects.mem_write = Some((
                MemoryValue { address: addr, value: old_val },
                MemoryValue { address: addr, value: raw.to_vec() },
            ));
        }
        self.mem[offset..offset + raw.len()].copy_from_slice(raw);
    }
}

pub struct Machine {
    segments: Vec<Segment>,
    pc_start: i64,
    global_pointer: i64,
    address_symbols: HashMap<i64, String>,
    other_symbols: HashMap<String, i64>,
    stack_start: i64,
    stack_end: i64,
    data_start: i64,
    data_end: i64,
    text_start: i64,
    text_end: i64,
    x: [i64; 32],
    pc: i64,
    stdout: Vec<u8>,
    stdin: Vec<u8>,
    stack_frames: Vec<i64>,
    effects: Option<Effects>,
    most_recent_memory: i64,
    most_recent_data: (i64, usize),  // (address, size)
    most_recent_stack: (i64, usize), // (address, size)
}

impl Machine {
    fn new(
        mut segments: Vec<Segment>,
        pc_start: i64,
        global_pointer: i64,
        address_symbols: HashMap<i64, String>,
        other_symbols: HashMap<String, i64>,
    ) -> Self {
        let mut stack_start = 0x100000 - STACK_SIZE;
        for segment in &segments {
            if segment.end + STACK_SIZE >= stack_start {
                stack_start = (segment.end + STACK_SIZE * 2 - 1) & (STACK_SIZE - 1);
            }
        }

        let stack_end = stack_start + STACK_SIZE;
        let mut data_start = stack_end - 8;
        let mut data_end = 0;
        let mut text_start = stack_end;
        let mut text_end = 0;

        for segment in &segments {
            if segment.executable {
                text_start = text_start.min(segment.start);
                text_end = text_end.max(segment.end);
            } else {
                data_start = data_start.min(segment.start);
                data_end = data_end.max(segment.end);
            }
        }

        if data_start == stack_end - 8 && data_end == 0 {
            data_start = 0;
        }

        segments.push(Segment::new(stack_start, stack_end, true, false, Vec::new()));

        let mut machine = Self {
            segments,
            pc_start,
            global_pointer,
            address_symbols,
            other_symbols,
            stack_start,
            stack_end,
            data_start,
            data_end,
            text_start,
            text_end,
            x: [0; 32],
            pc: pc_start,
            stdout: Vec::new(),
            stdin: Vec::new(),
            stack_frames: Vec::new(),
            effects: None,
            most_recent_memory: 0,
            most_recent_data: (0, 0),
            most_recent_stack: (0, 0),
        };

        machine.reset();
        machine
    }

    fn reset(&mut self) {
        for segment in &mut self.segments {
            segment.reset();
        }

        self.x = [0; 32];
        self.x[2] = self.stack_end;
        self.pc = self.pc_start;

        self.stdout.clear();
        self.stdin.clear();
        self.stack_frames.clear();
        self.effects = None;
    }

    fn set_most_recent_memory(&mut self, sequence: &[Effects], seq_i: usize) {
        self.most_recent_memory = if self.data_start > 0 { self.data_start } else { self.stack_end - 8 };
        self.most_recent_data = (self.data_start, 0);
        self.most_recent_stack = (self.stack_end - 8, 0);

        let mut stack = false;
        let mut data = false;

        for effect in sequence[..=seq_i].iter().rev() {
            let (address, value_len) = if let Some(read) = &effect.mem_read {
                (read.address, read.value.len())
            } else if let Some((_, write)) = &effect.mem_write {
                (write.address, write.value.len())
            } else {
                continue;
            };

            if !stack && address >= self.stack_start {
                self.most_recent_stack = (address, value_len);
                if !data {
                    self.most_recent_memory = address;
                }
                stack = true;
            }

            if !data && address < self.data_end {
                self.most_recent_data = (address, value_len);
                if !stack {
                    self.most_recent_memory = address;
                }
                data = true;
            }

            if stack && data {
                break;
            }
        }
    }

    fn load(&mut self, addr: i64, size: i64) -> Result<Vec<u8>, String> {
        for segment in &self.segments {
            if segment.in_range(addr, size) {
                let raw = segment.load(addr, size, &mut self.effects);
                return Ok(raw.to_vec());
            }
        }
        Err(format!("segfault: load addr=0x{:x} size={}", addr, size))
    }

    fn load_i8(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 1)?;
        Ok(i8::from_le_bytes(bytes[..1].try_into().unwrap()) as i64)
    }

    fn load_u8(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 1)?;
        Ok(u8::from_le_bytes(bytes[..1].try_into().unwrap()) as i64)
    }

    fn load_i16(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 2)?;
        Ok(i16::from_le_bytes(bytes[..2].try_into().unwrap()) as i64)
    }

    fn load_u16(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 2)?;
        Ok(u16::from_le_bytes(bytes[..2].try_into().unwrap()) as i64)
    }

    fn load_i32(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 4)?;
        Ok(i32::from_le_bytes(bytes[..4].try_into().unwrap()) as i64)
    }

    fn load_u32(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 4)?;
        Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()) as i64)
    }

    fn load_i64(&mut self, addr: i64) -> Result<i64, String> {
        let bytes = self.load(addr, 8)?;
        Ok(i64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn load_instruction(&self, addr: i64) -> Result<(i32, i64), String> {
        for segment in &self.segments {
            if !segment.in_range(addr, 2) || !segment.executable {
                continue;
            }

            // start by reading 16 bits
            let raw = segment.load(addr, 2, &mut None);
            let half = i16::from_le_bytes(raw.try_into().unwrap());

            if (half & 0b11) != 0b11 {
                // If compressed, expand to 32-bit and return with length 2
                return Ok((half as i32, 2));
            } else if segment.in_range(addr, 4) {
                // If not compressed, read full 32-bit instruction
                let raw = segment.load(addr, 4, &mut None);
                return Ok((i32::from_le_bytes(raw.try_into().unwrap()), 4));
            } else {
                return Err(format!("partial instruction at end of segment addr=0x{:x}", addr));
            }
        }
        Err(format!("segfault: instruction fetch addr=0x{:x}", addr))
    }

    fn store(&mut self, addr: i64, raw: &[u8]) -> Result<(), String> {
        let size = raw.len() as i64;
        for segment in &mut self.segments {
            if segment.in_range(addr, size) && segment.writeable {
                segment.store(addr, raw, &mut self.effects);
                return Ok(());
            }
        }
        Err(format!("segfault: store addr=0x{:x} size={}", addr, size))
    }

    fn get(&mut self, reg: usize) -> i64 {
        if reg != 0 && self.effects.is_some() {
            let effects = self.effects.as_mut().unwrap();
            if !effects.reg_reads.iter().any(|r| r.register == reg) {
                effects.reg_reads.push(RegisterValue { register: reg, value: self.x[reg] });
            }
        }
        self.x[reg]
    }

    fn get32(&mut self, reg: usize) -> i32 {
        self.get(reg) as i32
    }

    fn set(&mut self, reg: usize, value: i64) {
        // zero register never changes
        if reg != 0 {
            if let Some(effects) = &mut self.effects {
                assert!(effects.reg_write.is_none());
                effects.reg_write =
                    Some((RegisterValue { register: reg, value: self.x[reg] }, RegisterValue { register: reg, value }));
            }
            self.x[reg] = value;
        }
    }

    fn set32(&mut self, reg: usize, value: i32) {
        self.set(reg, value as i64);
    }

    fn set_pc(&mut self, value: i64) -> Result<(), String> {
        let old_pc = self.pc;
        self.pc = value;
        if self.pc & 1 != 0 {
            return Err(format!("bus error: pc addr={}", self.pc));
        }
        if let Some(effects) = &mut self.effects {
            effects.pc = (old_pc, self.pc);
        }
        Ok(())
    }

    fn execute_and_collect_effects(&mut self, instruction: &Rc<Instruction>) -> Effects {
        // trace the effects
        self.effects = Some(Effects::new(instruction));

        // execute the instruction
        let exec_res = instruction.op.execute(self, instruction.length);

        // reclaim the effects
        let mut effects = self.effects.take().unwrap();

        // default pc update (no error possible)
        if effects.pc == (0, 0) {
            let old_pc = self.pc;
            self.pc = old_pc + instruction.length;
            effects.pc = (old_pc, self.pc);
        }

        if let Err(msg) = exec_res {
            effects.error(msg);
        }

        effects
    }

    fn apply(&mut self, effect: &Effects, is_forward: bool) {
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
                self.stdout.extend(output);
            } else {
                let new_len = self.stdout.len() - output.len();
                self.stdout.truncate(new_len);
            }
        }

        if let Some(input) = &effect.stdin {
            // echo input
            if is_forward {
                self.stdout.extend(input);
            } else {
                let new_len = self.stdout.len() - input.len();
                self.stdout.truncate(new_len);
            }
        }

        if let Some(frame) = effect.function_start {
            if is_forward {
                self.stack_frames.push(frame);
            } else {
                self.stack_frames.pop();
            }
        }

        if let Some(frame) = effect.function_end {
            if is_forward {
                self.stack_frames.pop();
            } else {
                self.stack_frames.push(frame);
            }
        }
    }
}

pub struct Instruction {
    address: i64,
    op: Op,
    length: i64,
    pseudo_index: usize,
    verbose_fields: Vec<Field>,
    pseudo_fields: Vec<Field>,
}

struct MemoryValue {
    address: i64,
    value: Vec<u8>,
}

struct RegisterValue {
    register: usize,
    value: i64,
}

pub struct Effects {
    instruction: Rc<Instruction>,

    // pairs are (old_value, new_value)
    pc: (i64, i64),
    reg_reads: Vec<RegisterValue>,
    reg_write: Option<(RegisterValue, RegisterValue)>,
    mem_read: Option<MemoryValue>,
    mem_write: Option<(MemoryValue, MemoryValue)>,
    stdin: Option<Vec<u8>>,
    stdout: Option<Vec<u8>>,
    other_message: Option<String>,
    terminate: bool,
    function_start: Option<i64>,
    function_end: Option<i64>,
}

impl Effects {
    fn new(instruction: &Rc<Instruction>) -> Self {
        Effects {
            instruction: instruction.clone(),
            pc: (0, 0),
            reg_reads: Vec::new(),
            reg_write: None,
            mem_read: None,
            mem_write: None,
            stdin: None,
            stdout: None,
            other_message: None,
            terminate: false,
            function_start: None,
            function_end: None,
        }
    }

    fn error(&mut self, msg: String) {
        self.other_message = Some(msg);
        self.terminate = true;
    }

    fn report(&self, hex_mode: bool) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some((_, RegisterValue { register: rd, value: val })) = self.reg_write {
            if hex_mode {
                parts.push(format!("{} <- 0x{:x}", R[rd], val));
            } else {
                parts.push(format!("{} <- {}", R[rd], val));
            }
        }
        if self.pc.1 != self.pc.0 + self.instruction.length {
            if hex_mode {
                parts.push(format!("pc <- 0x{:x}", self.pc.1));
            } else {
                parts.push(format!("pc <- {}", self.pc.1));
            }
        }

        let mut lines = vec![parts.join(", ")];

        if let Some(msg) = &self.other_message {
            lines.push(msg.clone());
        }
        if let Some(stdin) = &self.stdin {
            let msg = String::from_utf8_lossy(stdin).into_owned();
            lines.push(format!("{:?}", msg));
        }
        if let Some(stdout) = &self.stdout {
            let msg = String::from_utf8_lossy(stdout).into_owned();
            lines.push(format!("{:?}", msg));
        }

        lines
    }
}

fn add_local_labels(m: &mut Machine, instructions: &[Instruction]) {
    // find local branch targets
    let mut branch_targets: HashSet<i64> = HashSet::new();
    for inst in instructions {
        if let Some(target) = inst.op.branch_target(inst.address) {
            branch_targets.insert(target);
        }
    }

    // add numbered local labels to the symbol table
    let mut next_label = 1;
    for inst in instructions {
        // reset numbers at the start of a new function
        #[allow(clippy::map_entry)]
        if m.address_symbols.contains_key(&inst.address) {
            next_label = 1;
        } else if branch_targets.contains(&inst.address) {
            m.address_symbols.insert(inst.address, next_label.to_string());
            next_label += 1;
        }
    }
}

fn trace(
    m: &mut Machine,
    instructions: &[Rc<Instruction>],
    addresses: &HashMap<i64, usize>,
    lint: bool,
    max_steps: usize,
    mode: &str,
) -> Vec<Effects> {
    let mut linter = Linter::new(m.x[2]);
    let mut sequence: Vec<Effects> = Vec::new();
    let mut i = 0;
    let echo_in = [/* "run", */ "debug"].contains(&mode) && !io::stdin().is_tty();

    for steps in 1..=max_steps {
        if i >= instructions.len() || instructions[i].address != m.pc {
            let Some(&new_i) = addresses.get(&m.pc) else {
                if let Some(effects) = sequence.last_mut() {
                    effects.error("next instruction not found".to_string());
                }
                break;
            };
            i = new_i;
        }

        // execute the instruction
        let instruction = &instructions[i];
        let mut effects = m.execute_and_collect_effects(instruction);
        i += 1;

        // echo the output?
        if !effects.terminate && ["run", "debug"].contains(&mode) {
            if let Some(output) = &effects.stdout {
                let mut handle = io::stdout().lock();
                if let Err(e) = handle.write(output) {
                    effects.error(format!("error echoing stdout: {}", e));
                }
            }
        }

        // echo the input?
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

struct FunctionRegisters {
    at_entry: [Option<usize>; 32],
    valid: [bool; 32],
    save_only: [bool; 32],
    at_entry_sp: i64,
}

struct ValueInMemory {
    n: usize,
    size: usize,
}

struct Linter {
    memory: HashMap<i64, ValueInMemory>,

    stack: Vec<FunctionRegisters>,
    at_entry: [Option<usize>; 32],
    at_entry_sp: i64,

    registers: [Option<usize>; 32],
    valid: [bool; 32],
    save_only: [bool; 32],
    next_n: usize,
}

impl Linter {
    fn new(at_entry_sp: i64) -> Self {
        let mut at_entry = [None; 32];
        for (n, elt) in at_entry.iter_mut().enumerate() {
            *elt = Some(n);
        }
        let mut valid = [false; 32];
        valid[0] = true;
        valid[2] = true;
        let save_only = [false; 32];
        let registers = at_entry;

        Self {
            memory: HashMap::new(),
            stack: Vec::new(),
            at_entry,
            at_entry_sp,
            registers,
            valid,
            save_only,
            next_n: 32,
        }
    }

    fn new_n(&mut self) -> usize {
        self.next_n += 1;
        self.next_n - 1
    }

    fn check_instruction(
        &mut self,
        m: &Machine,
        instruction: &Rc<Instruction>,
        effects: &mut Effects,
    ) -> Result<(), String> {
        // start with checks applicable to all instructions
        // this allows us to make basic assumptions later

        // first check that all input registers are valid values
        for read in &effects.reg_reads {
            let x = read.register;
            if !self.valid[x] || self.registers[x].is_none() {
                return Err(format!("{} is uninitialized", R[x]));
            }

            // save-only values can be moved to other registers
            // or written to memory but nothing else
            if self.save_only[x] {
                match &effects.mem_write {
                    Some((_, MemoryValue { value: store_val, .. })) if store_val.len() == 8 => {
                        // 64-bit write to memory is okay
                    }

                    _ => {
                        return Err(format!(
                            "the value in {} can only be saved to memory; it is not a valid input",
                            R[x]
                        ));
                    }
                }
            }
        }

        // next record register write
        if let Some((_, write)) = &effects.reg_write {
            let x = write.register;
            self.valid[x] = true;
            self.save_only[x] = false;

            // mv clones a value
            if matches!(instruction.op, Op::Addi { rd: 1..32, rs1: 1..32, imm: 0 }) {
                assert!(effects.reg_reads.len() == 1);
                self.registers[x] = self.registers[effects.reg_reads[0].register];
            } else {
                self.registers[x] = Some(self.new_n());
            }

            // sp must be aligned on 16-byte address
            if x == 2 && m.x[x] & 0xf != 0 {
                return Err("sp must always be a multiple of 16".to_string());
            }
        }

        // special per-instruction cases
        match instruction.op {
            // function call
            Op::Jal { rd: 1..32, .. } | Op::Jalr { rd: 1..32, .. } => {
                let op_name = if matches!(instruction.op, Op::Jal { .. }) { "jal" } else { "jalr" };

                // must use ra for return address
                let Some((_, RegisterValue { register: RA, .. })) = effects.reg_write else {
                    return Err(format!("{} did not use ra for return address", op_name));
                };

                // must call named function
                let (_, target_pc) = effects.pc;
                if !m.address_symbols.contains_key(&target_pc) {
                    return Err(format!("{} to unlabeled address", op_name));
                }
                let name = &m.address_symbols[&target_pc];

                // push caller register context
                self.stack.push(FunctionRegisters {
                    at_entry: self.at_entry,
                    valid: self.valid,
                    save_only: self.save_only,
                    at_entry_sp: self.at_entry_sp,
                });

                // update context for callee
                self.at_entry_sp = m.x[SP];

                // capture the stack start in the Effect for the tui
                effects.function_start = Some(m.x[SP]);

                // invalidate t registers
                for &x in &T_REGS {
                    self.registers[x] = None;
                    self.valid[x] = false;
                }

                let mut arg_count = 8;
                let args_sym = format!("{}_args", name);
                if let Some(&count) = m.other_symbols.get(&args_sym) {
                    // we have an argument count
                    assert!((0..8).contains(&count));
                    arg_count = count as usize;

                    // make sure func args are all valid values
                    for &x in A_REGS.iter().take(arg_count) {
                        if !self.valid[x] {
                            return Err(format!("argument in {} is uninitialized", R[x]));
                        }
                    }
                    for &x in A_REGS.iter().skip(arg_count) {
                        self.registers[x] = None;
                        self.valid[x] = false;
                    }
                } else {
                    // no argument count, so assume all a registers are args
                    for &x in A_REGS.iter().take(arg_count) {
                        self.valid[x] = self.registers[x].is_some();
                    }
                }

                // make sure all s registers have a number
                self.save_only = [false; 32];
                for &x in &S_REGS {
                    if self.registers[x].is_none() {
                        self.registers[x] = Some(self.new_n());
                    }
                    self.valid[x] = true;
                    self.save_only[x] = true;
                }

                // record the registers at function entry time
                self.at_entry = self.registers;
            }

            // function return
            Op::Jalr { rd: ZERO, rs1: RA, offset: 0 } => {
                // ra, gp, and tp must match what they were at call time
                for x in [1, 3, 4] {
                    if self.registers[x] != self.at_entry[x] {
                        return Err(format!("{} is not same value as when function called", R[x]));
                    }
                }

                // s registers must be same as at call time
                for &x in &S_REGS {
                    if self.registers[x] != self.at_entry[x] {
                        return Err(format!("{} is not same value as when function called", R[x]));
                    }
                }

                // sp must have the same address, but not necessarily the same value number
                if m.x[2] != self.at_entry_sp {
                    return Err("sp is not same value as when function called".to_string());
                }

                // record sp at function exit in Effects for the tui
                effects.function_end = Some(m.x[2]);

                // pop previous function context
                if let Some(FunctionRegisters { at_entry, valid, save_only, at_entry_sp }) = self.stack.pop() {
                    self.at_entry = at_entry;
                    self.valid = valid;
                    self.save_only = save_only;
                    self.at_entry_sp = at_entry_sp;
                } else {
                    return Err("ret with no stack frame to return to".to_string());
                }

                // invalidate t and a1+ registers
                for &x in &T_REGS {
                    self.registers[x] = None;
                    self.valid[x] = false;
                }
                for &x in A_REGS.iter().skip(1) {
                    self.registers[x] = None;
                    self.valid[x] = false;
                }
            }

            // stores
            Op::Sb { .. } | Op::Sh { .. } | Op::Sw { .. } | Op::Sd { .. } => {
                let Some((_, write)) = &effects.mem_write else {
                    return Err("store instruction with no memory write".to_string());
                };

                let addr = write.address;
                let size = write.value.len();

                // insist on aligned writes
                // partial register writes count as new values
                // since re-reading them does not restore a full register value
                let (alignment, n) = match instruction.op {
                    Op::Sb { .. } => (0, self.new_n()),
                    Op::Sh { .. } => (1, self.new_n()),
                    Op::Sw { .. } => (3, self.new_n()),
                    Op::Sd { rs2, .. } => (7, self.registers[rs2].unwrap()),
                    _ => unreachable!(),
                };

                if addr & alignment != 0 {
                    return Err(format!("{}-byte memory write at unaligned address 0x{:x}", alignment + 1, addr));
                }

                // record the memory write
                for address in addr..addr + size as i64 {
                    self.memory.insert(address, ValueInMemory { n, size });
                }
            }

            // loads
            Op::Lb { rd, .. }
            | Op::Lh { rd, .. }
            | Op::Lw { rd, .. }
            | Op::Ld { rd, .. }
            | Op::Lbu { rd, .. }
            | Op::Lhu { rd, .. }
            | Op::Lwu { rd, .. } => {
                let Some(read) = &effects.mem_read else {
                    return Err("load instruction with no memory read".to_string());
                };

                let addr = read.address;
                let size = read.value.len();

                // insist on aligned reads
                // partial register reads count as new values
                // since they do not restore a full register value
                let alignment = match instruction.op {
                    Op::Lb { .. } | Op::Lbu { .. } => 0,
                    Op::Lh { .. } | Op::Lhu { .. } => 1,
                    Op::Lw { .. } | Op::Lwu { .. } => 3,
                    Op::Ld { .. } => 7,
                    _ => unreachable!(),
                };
                if addr & alignment != 0 {
                    return Err(format!("{}-byte memory read from unaligned address 0x{:x}", alignment + 1, addr));
                }

                // we accept two kinds of reads:
                // 1. a value that has not been written (recorded as a new number)
                // 2. a single value that is the same size as when written
                let n = if let Some(mem_val) = self.memory.get(&addr) {
                    let n = mem_val.n;

                    for address in addr..addr + size as i64 {
                        match self.memory.get(&address) {
                            None => return Err("reading data that was only partially written".to_string()),
                            Some(ValueInMemory { n: mem_n, size: mem_size }) => {
                                if *mem_n != n {
                                    return Err("reading data from multiple writes".to_string());
                                }
                                if *mem_size != size {
                                    return Err("reading data with different size than when written".to_string());
                                }
                            }
                        }
                    }
                    n
                } else {
                    // record this value in memory as we verify that no bytes
                    // already have a value number
                    let n = self.new_n();
                    for address in addr..addr + size as i64 {
                        if self.memory.contains_key(&address) {
                            return Err("reading data that is only partially from a previous write".to_string());
                        }
                        self.memory.insert(address, ValueInMemory { n, size });
                    }
                    n
                };

                self.registers[rd] = Some(n);
            }

            // reads and writes
            Op::Ecall => {
                // write syscall
                if let Some(read) = &effects.mem_read {
                    let addr = read.address;
                    let size = read.value.len();

                    // only allow byte values from memory
                    for address in addr..addr + size as i64 {
                        if let Some(val) = self.memory.get(&address) {
                            if val.size != 1 {
                                return Err("write syscall on non-byte data".to_string());
                            }
                        }
                    }
                }

                // read syscall
                if let Some((_, write)) = &effects.mem_write {
                    let addr = write.address;
                    let size = write.value.len();

                    for address in addr..addr + size as i64 {
                        // do not allow overwrite of non-byte data
                        if let Some(val) = self.memory.get(&address) {
                            if val.size != 1 {
                                return Err("read syscall overwriting non-byte data".to_string());
                            }
                        }

                        // record data as individual bytes
                        let n = self.new_n();
                        self.memory.insert(address, ValueInMemory { n, size: 1 });
                    }
                }
            }

            _ => {}
        }

        Ok(())
    }
}

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

    // load the program from disk and form the
    // simulated address space and cpu
    let mut m = load_elf(&executable)?;

    // disassemble the entire text segment
    let mut instructions = Vec::new();
    let mut pc = m.text_start;
    while pc < m.text_end {
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

    // find pseudo-instructions that combine two or more real instructions
    // pseudo_addresses: pseudo index => verbose index
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

    // convert to Rc<Instruction> so Effects can reference entries
    let instructions: Vec<Rc<Instruction>> = instructions.into_iter().map(Rc::new).collect();

    // trace the entire execution
    // for run mode, have pre_trace echo output as it goes
    // so inputs and outputs are correctly interleved
    let sequence = trace(&mut m, &instructions, &addresses, lint == "true", max_steps, &mode);

    // debug
    if mode == "debug" {
        m.reset();
        m.set_most_recent_memory(&sequence, 0);
        let mut tui = Tui::new(m, instructions, addresses, pseudo_addresses, sequence)?;
        tui.main_loop()?;
        return Ok(());
    }

    // should have ended with exit(0)
    if let Some(effects) = sequence.last() {
        if let (Op::Ecall, Some(msg)) = (&effects.instruction.op, &effects.other_message) {
            if msg.starts_with("exit(") && msg.ends_with(")") {
                let n: i32 = msg[5..msg.len() - 1].parse().unwrap();
                std::process::exit(n);
            }
        }

        if let Some(msg) = &effects.other_message {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    }
    eprintln!("program ended unexpectedly");
    std::process::exit(1);
}
