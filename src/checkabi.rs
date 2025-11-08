use std::collections::HashMap;
use std::rc::Rc;

use crate::execution::{Instruction, Machine};
use crate::riscv::{A_REGS, Op, R, RA, S_REGS, SP, T_REGS, ZERO};
use crate::trace::{Effects, MemoryValue, RegisterValue};

struct FunctionRegisters {
    at_entry: [Option<usize>; 32],
    valid: [bool; 32],
    save_only: [bool; 32],
    at_entry_sp: u32,
}

struct ValueInMemory {
    n: usize,
    size: usize,
}

pub struct CheckABI {
    memory: HashMap<u32, ValueInMemory>,

    stack: Vec<FunctionRegisters>,
    at_entry: [Option<usize>; 32],
    at_entry_sp: u32,

    registers: [Option<usize>; 32],
    valid: [bool; 32],
    save_only: [bool; 32],
    next_n: usize,
}

impl CheckABI {
    pub fn new(at_entry_sp: u32) -> Self {
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
                return Err(format!("Cannot use uninitialized {}", R[x]));
            }

            // save-only values can be moved to other registers
            // or written to memory but nothing else
            if self.save_only[x] {
                match &effects.mem_write {
                    Some((_, MemoryValue { value: store_val, .. }))
                        if store_val.len() == 8 =>
                    {
                        // 64-bit write to memory is okay
                    }

                    _ => {
                        return Err(format!(
                            "{} can only be stored, not used as input",
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
            if matches!(
                instruction.op,
                Op::Addi { rd: 1..32, rs1: 1..32, imm: 0 }
            ) {
                assert!(effects.reg_reads.len() == 1);
                self.registers[x] =
                    self.registers[effects.reg_reads[0].register];
            } else {
                self.registers[x] = Some(self.new_n());
            }

            // sp must be aligned on 16-byte address
            if x == 2 && m.get_reg(x) & 0xf != 0 {
                return Err("Stack pointer must be 16-byte aligned".to_string());
            }
        }

        // special per-instruction cases
        match instruction.op {
            // function call
            Op::Jal { rd: 1..32, .. } | Op::Jalr { rd: 1..32, .. } => {
                // must use ra for return address
                let Some((_, RegisterValue { register: RA, .. })) =
                    effects.reg_write
                else {
                    return Err(
                        "Return address must be stored in ra".to_string()
                    );
                };

                // must call named function
                let (_, target_pc) = effects.pc;
                if !m.address_symbols.contains_key(&target_pc) {
                    return Err("Cannot jump to unlabeled address".to_string());
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
                self.at_entry_sp = m.get_reg(SP) as u32;

                // capture the stack start in the Effect for the tui
                effects.function_start = Some(m.get_reg(SP) as u32);

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
                            return Err(format!(
                                "Function argument {} is uninitialized",
                                R[x]
                            ));
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
                        return Err(format!(
                            "{} must be preserved across function call",
                            R[x]
                        ));
                    }
                }

                // s registers must be same as at call time
                for &x in &S_REGS {
                    if self.registers[x] != self.at_entry[x] {
                        return Err(format!(
                            "{} must be preserved across function call",
                            R[x]
                        ));
                    }
                }

                // sp must have the same address, but not necessarily the same value number
                if m.get_reg(2) as u32 != self.at_entry_sp {
                    return Err("Stack pointer must be restored before return"
                        .to_string());
                }

                // record sp at function exit in Effects for the tui
                effects.function_end = Some(m.get_reg(2) as u32);

                // pop previous function context
                if let Some(FunctionRegisters {
                    at_entry,
                    valid,
                    save_only,
                    at_entry_sp,
                }) = self.stack.pop()
                {
                    self.at_entry = at_entry;
                    self.valid = valid;
                    self.save_only = save_only;
                    self.at_entry_sp = at_entry_sp;
                } else {
                    return Err("Unexpected return: no matching function call"
                        .to_string());
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
            Op::Sb { .. } | Op::Sh { .. } | Op::Sw { .. } => {
                let Some((_, write)) = &effects.mem_write else {
                    return Err(
                        "store instruction with no memory write".to_string()
                    );
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
                    _ => unreachable!(),
                };

                if addr & alignment != 0 {
                    return Err(format!(
                        "Unaligned {}-byte memory write at 0x{:x}",
                        alignment + 1,
                        addr
                    ));
                }

                // record the memory write
                for address in addr..addr + (size as u32) {
                    self.memory.insert(address, ValueInMemory { n, size });
                }
            }

            // loads
            Op::Lb { rd, .. }
            | Op::Lh { rd, .. }
            | Op::Lw { rd, .. }
            | Op::Lbu { rd, .. }
            | Op::Lhu { rd, .. } => {
                let Some(read) = &effects.mem_read else {
                    return Err(
                        "load instruction with no memory read".to_string()
                    );
                };

                let addr = read.address;
                let size = read.value.len();

                // insist on aligned reads
                // partial register reads count as new values
                // since they do not restore a full register value
                let alignment = match instruction.op {
                    Op::Lb { .. } | Op::Lbu { .. } => 0,
                    Op::Lh { .. } | Op::Lhu { .. } => 1,
                    Op::Lw { .. } => 3,
                    _ => unreachable!(),
                };
                if addr & alignment != 0 {
                    return Err(format!(
                        "Unaligned {}-byte memory read at 0x{:x}",
                        alignment + 1,
                        addr
                    ));
                }

                // we accept two kinds of reads:
                // 1. a value that has not been written (recorded as a new number)
                // 2. a single value that is the same size as when written
                let n = if let Some(mem_val) = self.memory.get(&addr) {
                    let n = mem_val.n;

                    for address in addr..addr + (size as u32) {
                        match self.memory.get(&address) {
                            None => return Err(
                                "Cannot read: incomplete write before this read"
                                    .to_string(),
                            ),
                            Some(ValueInMemory {
                                n: mem_n,
                                size: mem_size,
                            }) => {
                                if *mem_n != n {
                                    return Err(
                                        "Cannot read: data spans multiple separate writes"
                                            .to_string(),
                                    );
                                }
                                if *mem_size != size {
                                    return Err("Read size mismatches original write size".to_string());
                                }
                            }
                        }
                    }
                    n
                } else {
                    // record this value in memory as we verify that no bytes
                    // already have a value number
                    let n = self.new_n();
                    for address in addr..addr + (size as u32) {
                        if self.memory.contains_key(&address) {
                            return Err("Cannot read: overlaps partially with previous write".to_string());
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
                    for address in addr..addr + (size as u32) {
                        if let Some(val) = self.memory.get(&address)
                            && val.size != 1
                        {
                            return Err(
                                "Syscall write requires byte-level data"
                                    .to_string(),
                            );
                        }
                    }
                }

                // read syscall
                if let Some((_, write)) = &effects.mem_write {
                    let addr = write.address;
                    let size = write.value.len();

                    for address in addr..addr + (size as u32) {
                        // do not allow overwrite of non-byte data
                        if let Some(val) = self.memory.get(&address)
                            && val.size != 1
                        {
                            return Err(
                                "Syscall read would overwrite non-byte data"
                                    .to_string(),
                            );
                        }

                        // record data as individual bytes
                        let n = self.new_n();
                        self.memory
                            .insert(address, ValueInMemory { n, size: 1 });
                    }
                }
            }

            _ => {}
        }

        Ok(())
    }
}
