use std::collections::HashMap;
use std::rc::Rc;
use crate::{Machine, Instruction, Effects, MemoryValue, RegisterValue, Op, R, RA, ZERO, SP, T_REGS, A_REGS, S_REGS};

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

pub struct Linter {
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
    pub fn new(at_entry_sp: i64) -> Self {
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

    pub fn check_instruction(
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
