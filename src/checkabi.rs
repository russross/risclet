use std::rc::Rc;

use crate::execution::{Instruction, Machine};
use crate::riscv::{A_REGS, Op, R, RA, S_REGS, SP, T_REGS, ZERO};
use crate::trace::{Effects, MemoryValue, RegisterValue};

/// Size category for shadow memory tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShadowSize {
    Uninitialized, // Not yet written
    Byte,          // 1 byte
    HalfWord,      // 2 bytes
    Word,          // 4 bytes
}

impl ShadowSize {
    /// Convert from actual byte count to size category
    fn from_byte_count(bytes: usize) -> Self {
        match bytes {
            1 => ShadowSize::Byte,
            2 => ShadowSize::HalfWord,
            4 => ShadowSize::Word,
            _ => panic!("Invalid byte count for shadow size: {}", bytes),
        }
    }

    /// Encode as 2-bit value for shadow memory storage
    fn encode_bits(self) -> u64 {
        match self {
            ShadowSize::Uninitialized => 0,
            ShadowSize::Byte => 1,
            ShadowSize::HalfWord => 2,
            ShadowSize::Word => 3,
        }
    }

    /// Decode from 2-bit value in shadow memory
    fn from_encoded_bits(bits: u64) -> Self {
        match bits {
            0 => ShadowSize::Uninitialized,
            1 => ShadowSize::Byte,
            2 => ShadowSize::HalfWord,
            3 => ShadowSize::Word,
            _ => panic!("Invalid size encoding: {}", bits),
        }
    }
}

struct FunctionRegisters {
    at_entry: [Option<usize>; 32],
    valid: [bool; 32],
    save_only: [bool; 32],
    at_entry_sp: u32,
}

pub struct CheckABI {
    // Shadow memory for three segments: text, data, stack
    text_shadow: Vec<u64>,
    text_start: u32,

    data_shadow: Vec<u64>,
    data_start: u32,

    stack_shadow: Vec<u64>,
    stack_start: u32,

    stack: Vec<FunctionRegisters>,
    at_entry: [Option<usize>; 32],
    at_entry_sp: u32,

    registers: [Option<usize>; 32],
    valid: [bool; 32],
    save_only: [bool; 32],
    next_n: usize,
}

impl CheckABI {
    pub fn new(
        at_entry_sp: u32,
        text_start: u32,
        text_end: u32,
        data_start: u32,
        data_end: u32,
        stack_start: u32,
        stack_end: u32,
    ) -> Self {
        // Calculate sizes for each segment
        let text_size = (text_end - text_start) as usize;
        let data_size = if data_start < data_end {
            (data_end - data_start) as usize
        } else {
            0
        };
        let stack_size = (stack_end - stack_start) as usize;

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
            text_shadow: vec![0; text_size],
            text_start,
            data_shadow: vec![0; data_size],
            data_start,
            stack_shadow: vec![0; stack_size],
            stack_start,
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

    /// Encode value number and size into a u64 shadow memory entry
    /// Bottom 2 bits: size category
    /// Upper 62 bits: value number
    #[inline]
    fn encode(n: usize, size: ShadowSize) -> u64 {
        ((n as u64) << 2) | size.encode_bits()
    }

    /// Decode value number and size from a u64 shadow memory entry
    #[inline]
    fn decode(shadow_val: u64) -> (usize, ShadowSize) {
        let size_bits = shadow_val & 0x3;
        let n = (shadow_val >> 2) as usize;
        (n, ShadowSize::from_encoded_bits(size_bits))
    }

    /// Get reference to the segment containing this address
    fn get_segment(&self, addr: u32) -> Option<&Vec<u64>> {
        if addr >= self.text_start
            && (addr as usize)
                < self.text_start as usize + self.text_shadow.len()
        {
            Some(&self.text_shadow)
        } else if addr >= self.data_start
            && (addr as usize)
                < self.data_start as usize + self.data_shadow.len()
        {
            Some(&self.data_shadow)
        } else if addr >= self.stack_start
            && (addr as usize)
                < self.stack_start as usize + self.stack_shadow.len()
        {
            Some(&self.stack_shadow)
        } else {
            None
        }
    }

    /// Read shadow memory value at an address
    fn shadow_get(&self, addr: u32) -> Option<(usize, ShadowSize)> {
        let segment = self.get_segment(addr)?;
        let offset = if addr >= self.text_start
            && addr < self.text_start + self.text_shadow.len() as u32
        {
            (addr - self.text_start) as usize
        } else if addr >= self.data_start
            && addr < self.data_start + self.data_shadow.len() as u32
        {
            (addr - self.data_start) as usize
        } else {
            (addr - self.stack_start) as usize
        };
        Some(Self::decode(segment[offset]))
    }

    /// Write shadow memory value at an address
    fn shadow_insert(&mut self, addr: u32, n: usize, size: ShadowSize) {
        // Compute offset and determine which segment before borrowing
        let (offset, in_text) = if addr >= self.text_start
            && (addr as usize)
                < self.text_start as usize + self.text_shadow.len()
        {
            ((addr - self.text_start) as usize, true)
        } else if addr >= self.data_start
            && (addr as usize)
                < self.data_start as usize + self.data_shadow.len()
        {
            ((addr - self.data_start) as usize, false)
        } else if addr >= self.stack_start
            && (addr as usize)
                < self.stack_start as usize + self.stack_shadow.len()
        {
            ((addr - self.stack_start) as usize, false)
        } else {
            // This should never happen if memory bounds checking is working
            unreachable!(
                "shadow_insert called with out-of-bounds address 0x{:x}",
                addr
            );
        };

        // Now borrow the appropriate segment and write
        if in_text {
            if addr >= self.text_start
                && (addr as usize)
                    < self.text_start as usize + self.text_shadow.len()
            {
                self.text_shadow[offset] = Self::encode(n, size);
            }
        } else if addr >= self.data_start
            && (addr as usize)
                < self.data_start as usize + self.data_shadow.len()
        {
            self.data_shadow[offset] = Self::encode(n, size);
        } else {
            self.stack_shadow[offset] = Self::encode(n, size);
        }
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
                        if store_val.len() == 4 =>
                    {
                        // 32-bit word write to memory is okay
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
                let byte_count = write.value.len();
                let shadow_size = ShadowSize::from_byte_count(byte_count);

                // insist on aligned writes
                // partial register writes count as new values
                // since re-reading them does not restore a full register value
                // For full-register stores (sw on rv32), preserve the source register's value number
                let (alignment, n) = match instruction.op {
                    Op::Sb { .. } => (1, self.new_n()),
                    Op::Sh { .. } => (2, self.new_n()),
                    Op::Sw { rs2, .. } => {
                        // Full register store: use the source register's value number
                        let n = self.registers[rs2]
                            .expect("sw source register should be valid");
                        (4, n)
                    }
                    _ => unreachable!(),
                };

                if addr & (alignment - 1) != 0 {
                    return Err(format!(
                        "Unaligned {}-byte memory write at 0x{:x}",
                        alignment, addr
                    ));
                }

                // record the memory write
                for address in addr..addr + (byte_count as u32) {
                    self.shadow_insert(address, n, shadow_size);
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
                let byte_count = read.value.len();
                let read_size = ShadowSize::from_byte_count(byte_count);

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

                // We accept two kinds of reads:
                // 1. All bytes uninitialized (fresh read) - assign new value number
                // 2. All bytes initialized with same value number and size - reuse value number
                // Mixed initialized/uninitialized is an error (partial write)

                let Some((mem_n, mem_size)) = self.shadow_get(addr) else {
                    // Address not in valid segment - shouldn't happen
                    return Err(
                        "Cannot read: address not in valid memory segment"
                            .to_string(),
                    );
                };
                let n = {
                    if mem_size == ShadowSize::Uninitialized {
                        // First byte is uninitialized - all bytes must be uninitialized
                        let n = self.new_n();
                        for address in addr..addr + (byte_count as u32) {
                            if let Some((_, size)) = self.shadow_get(address)
                                && size != ShadowSize::Uninitialized
                            {
                                return Err("Cannot read: incomplete write before this read".to_string());
                            }
                            self.shadow_insert(address, n, read_size);
                        }
                        n
                    } else {
                        // First byte is initialized - all bytes must match same write
                        let n = mem_n;
                        for address in addr..addr + (byte_count as u32) {
                            match self.shadow_get(address) {
                                Some((shadow_mem_n, shadow_mem_size)) if shadow_mem_size != ShadowSize::Uninitialized => {
                                    if shadow_mem_n != n {
                                        return Err("Cannot read: data spans multiple separate writes".to_string());
                                    }
                                    if shadow_mem_size != read_size {
                                        return Err("Read size mismatches original write size".to_string());
                                    }
                                }
                                _ => return Err("Cannot read: incomplete write before this read".to_string()),
                            }
                        }
                        n
                    }
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
                        if let Some((_, shadow_size)) = self.shadow_get(address)
                            && shadow_size != ShadowSize::Byte
                            && shadow_size != ShadowSize::Uninitialized
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
                        if let Some((_, shadow_size)) = self.shadow_get(address)
                            && shadow_size != ShadowSize::Byte
                            && shadow_size != ShadowSize::Uninitialized
                        {
                            return Err(
                                "Syscall read would overwrite non-byte data"
                                    .to_string(),
                            );
                        }

                        // record data as individual bytes
                        let n = self.new_n();
                        self.shadow_insert(address, n, ShadowSize::Byte);
                    }
                }
            }

            _ => {}
        }

        Ok(())
    }
}
