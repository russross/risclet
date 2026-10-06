use std::iter::once;
use std::rc::Rc;

use crate::execution::{Instruction, Machine};
use crate::riscv::{A_REGS, GP, Op, R, RA, S_REGS, SP, T_REGS, TP, ZERO};
use crate::trace::{
    Effects, FrameChange, MemoryValue, MemoryWrite, RegisterValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValueId(u64);

// A preserved value may be copied or saved without becoming usable input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisterState {
    Unavailable,
    Value(ValueId),
    SaveOnly(ValueId),
}

impl RegisterState {
    fn identity(self) -> Option<ValueId> {
        match self {
            Self::Unavailable => None,
            Self::Value(id) | Self::SaveOnly(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShadowSize {
    Uninitialized,
    Byte,
    HalfWord,
    Word,
}

impl ShadowSize {
    fn from_byte_count(bytes: usize) -> Self {
        match bytes {
            1 => Self::Byte,
            2 => Self::HalfWord,
            4 => Self::Word,
            _ => unreachable!("unsupported memory width: {bytes}"),
        }
    }
}

struct ShadowSegment {
    start: u32,
    entries: Vec<u64>,
}

impl ShadowSegment {
    fn new(start: u32, end: u32) -> Self {
        Self { start, entries: vec![0; end.saturating_sub(start) as usize] }
    }

    // All segment operations use the same checked address-to-offset mapping.
    fn offset(&self, address: u32) -> Option<usize> {
        let offset = address.checked_sub(self.start)? as usize;
        (offset < self.entries.len()).then_some(offset)
    }

    fn get(&self, address: u32) -> Option<(RegisterState, ShadowSize)> {
        let entry = self.entries[self.offset(address)?];
        let size = match entry & 3 {
            0 => {
                return Some((
                    RegisterState::Unavailable,
                    ShadowSize::Uninitialized,
                ));
            }
            1 => ShadowSize::Byte,
            2 => ShadowSize::HalfWord,
            _ => ShadowSize::Word,
        };
        let id = ValueId(entry >> 3);
        let state = if entry & 4 == 0 {
            RegisterState::Value(id)
        } else {
            RegisterState::SaveOnly(id)
        };
        Some((state, size))
    }

    // Two size bits and a save-only bit accompany each stored value identity.
    fn insert(&mut self, address: u32, state: RegisterState, size: ShadowSize) {
        let offset = self.offset(address).expect("validated shadow address");
        let id = state.identity().expect("initialized shadow value");
        let size_bits = match size {
            ShadowSize::Uninitialized => {
                unreachable!("cannot store uninitialized value")
            }
            ShadowSize::Byte => 1,
            ShadowSize::HalfWord => 2,
            ShadowSize::Word => 3,
        };
        let save_bit =
            u64::from(matches!(state, RegisterState::SaveOnly(_))) << 2;
        self.entries[offset] = (id.0 << 3) | save_bit | size_bits;
    }

    fn clear(&mut self, start: u32, end: u32) {
        let first = start.saturating_sub(self.start) as usize;
        let last =
            (end.saturating_sub(self.start) as usize).min(self.entries.len());
        if first < last {
            self.entries[first..last].fill(0);
        }
    }
}

#[derive(Clone)]
struct FunctionContext {
    registers: [RegisterState; 32],
    at_entry: [Option<ValueId>; 32],
    at_entry_sp: u32,
}

pub struct CheckABI {
    program_shadow: [ShadowSegment; 2],
    stack_shadow: ShadowSegment,
    context: FunctionContext,
    callers: Vec<FunctionContext>,
    next_id: u64,
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
        // Startup exposes only zero and the loader's stack pointer as inputs.
        let mut registers = [RegisterState::Unavailable; 32];
        registers[ZERO] = RegisterState::Value(ValueId(0));
        registers[SP] = RegisterState::Value(ValueId(1));
        Self {
            program_shadow: [
                ShadowSegment::new(text_start, text_end),
                ShadowSegment::new(data_start, data_end),
            ],
            stack_shadow: ShadowSegment::new(stack_start, stack_end),
            context: FunctionContext {
                registers,
                at_entry: registers.map(RegisterState::identity),
                at_entry_sp,
            },
            callers: Vec::new(),
            next_id: 2,
        }
    }

    fn new_id(&mut self) -> ValueId {
        let id = ValueId(self.next_id);
        self.next_id += 1;
        id
    }

    // Program data retains lazy shadow initialization independently of stack lifetime.
    fn shadow_get(&self, address: u32) -> Option<(RegisterState, ShadowSize)> {
        self.program_shadow
            .iter()
            .chain(once(&self.stack_shadow))
            .find_map(|segment| segment.get(address))
    }

    fn shadow_insert(
        &mut self,
        address: u32,
        state: RegisterState,
        size: ShadowSize,
    ) {
        let segment = self
            .program_shadow
            .iter_mut()
            .chain(once(&mut self.stack_shadow))
            .find(|segment| segment.offset(address).is_some())
            .expect("machine-validated memory address");
        segment.insert(address, state, size);
    }

    fn check_stack_access(
        &self,
        address: u32,
        size: usize,
        sp: u32,
        read: bool,
    ) -> Result<(), String> {
        for address in u64::from(address)..u64::from(address) + size as u64 {
            let address = u32::try_from(address)
                .map_err(|_| "Memory access wraps address space")?;
            if self.stack_shadow.offset(address).is_none() {
                continue;
            }
            if address < sp {
                return Err(format!(
                    "Cannot access stack below sp at 0x{address:x}"
                ));
            }
            if read
                && self
                    .stack_shadow
                    .get(address)
                    .is_some_and(|(_, size)| size == ShadowSize::Uninitialized)
            {
                return Err(format!(
                    "Cannot read uninitialized stack at 0x{address:x}"
                ));
            }
        }
        Ok(())
    }

    fn enter_call(
        &mut self,
        m: &Machine,
        effects: &mut Effects,
    ) -> Result<(), String> {
        if !effects.reg_write.is_some_and(|write| write.register == RA) {
            return Err("Return address must be stored in ra".to_string());
        }
        let (_, target) = effects.pc;
        let name = m
            .address_symbols
            .get(&target)
            .ok_or("Cannot jump to unlabeled address")?;

        // Explicit argument metadata validates inputs and hides unused arguments.
        let args_symbol = format!("{name}_args");
        let count = m.other_symbols.get(&args_symbol).copied();
        if let Some(count) = count {
            if !(0..=8).contains(&count) {
                return Err(format!(
                    "Invalid argument count for {name}: {count}; expected 0 through 8"
                ));
            }
            for &register in A_REGS.iter().take(count as usize) {
                match self.context.registers[register] {
                    RegisterState::Value(_) => {}
                    RegisterState::Unavailable => {
                        return Err(format!(
                            "Function argument {} is uninitialized",
                            R[register]
                        ));
                    }
                    RegisterState::SaveOnly(_) => {
                        return Err(format!(
                            "Function argument {} is save-only",
                            R[register]
                        ));
                    }
                }
            }
        } else {
            // Unknown signatures permit available arguments, never save-only values.
            for &register in &A_REGS {
                if matches!(
                    self.context.registers[register],
                    RegisterState::SaveOnly(_)
                ) {
                    return Err(format!(
                        "Function argument {} is save-only",
                        R[register]
                    ));
                }
            }
        }

        self.callers.push(self.context.clone());
        for &register in &T_REGS {
            self.context.registers[register] = RegisterState::Unavailable;
        }
        if let Some(count) = count {
            for &register in A_REGS.iter().skip(count as usize) {
                self.context.registers[register] = RegisterState::Unavailable;
            }
        }

        // Incoming saved registers have identities even when their caller cannot use them.
        for &register in &S_REGS {
            let id = match self.context.registers[register].identity() {
                Some(id) => id,
                None => self.new_id(),
            };
            self.context.registers[register] = RegisterState::SaveOnly(id);
        }
        self.context.at_entry =
            self.context.registers.map(RegisterState::identity);
        self.context.at_entry_sp = m.get_reg(SP) as u32;
        effects.frame_change =
            Some(FrameChange::Enter(self.context.at_entry_sp));
        Ok(())
    }

    fn leave_call(
        &mut self,
        m: &Machine,
        effects: &mut Effects,
    ) -> Result<(), String> {
        if self.callers.is_empty() {
            return Err(
                "Unexpected return: no matching function call".to_string()
            );
        }
        for register in [RA, GP, TP].iter().chain(S_REGS.iter()).copied() {
            if self.context.registers[register].identity()
                != self.context.at_entry[register]
            {
                return Err(format!(
                    "{} must be preserved across function call",
                    R[register]
                ));
            }
        }
        let sp = m.get_reg(SP) as u32;
        if sp != self.context.at_entry_sp {
            return Err(
                "Stack pointer must be restored before return".to_string()
            );
        }

        // Restore caller availability while retaining only the callee's supported result.
        let result = self.context.registers[A_REGS[0]];
        self.context = self.callers.pop().expect("checked matching call");
        self.context.registers[A_REGS[0]] = result;
        for &register in T_REGS.iter().chain(A_REGS.iter().skip(1)) {
            self.context.registers[register] = RegisterState::Unavailable;
        }
        effects.frame_change = Some(FrameChange::Leave(sp));
        Ok(())
    }

    fn store_value(
        &mut self,
        write: &MemoryWrite,
        state: RegisterState,
    ) -> Result<(), String> {
        let width = write.new_value.len();
        if write.address & (width as u32 - 1) != 0 {
            return Err(format!(
                "Unaligned {width}-byte memory write at 0x{:x}",
                write.address
            ));
        }
        let size = ShadowSize::from_byte_count(width);
        for address in write.address..write.address + width as u32 {
            self.shadow_insert(address, state, size);
        }
        Ok(())
    }

    fn load_value(
        &mut self,
        read: &MemoryValue,
    ) -> Result<RegisterState, String> {
        let width = read.value.len();
        if read.address & (width as u32 - 1) != 0 {
            return Err(format!(
                "Unaligned {width}-byte memory read at 0x{:x}",
                read.address
            ));
        }
        let size = ShadowSize::from_byte_count(width);
        let (stored, stored_size) = self
            .shadow_get(read.address)
            .ok_or("Cannot read: address not in valid memory segment")?;

        // Validate the whole read before lazily assigning an identity to program data.
        for address in read.address..read.address + width as u32 {
            let (state, byte_size) = self
                .shadow_get(address)
                .ok_or("Cannot read: incomplete write before this read")?;
            if stored_size == ShadowSize::Uninitialized {
                if byte_size != ShadowSize::Uninitialized {
                    return Err(
                        "Cannot read: incomplete write before this read"
                            .to_string(),
                    );
                }
                continue;
            }
            if byte_size == ShadowSize::Uninitialized {
                return Err("Cannot read: incomplete write before this read"
                    .to_string());
            }
            if state != stored {
                return Err("Cannot read: data spans multiple separate writes"
                    .to_string());
            }
            if byte_size != size {
                return Err(
                    "Read size mismatches original write size".to_string()
                );
            }
        }

        if stored_size != ShadowSize::Uninitialized {
            return Ok(stored);
        }
        let state = RegisterState::Value(self.new_id());
        for address in read.address..read.address + width as u32 {
            self.shadow_insert(address, state, size);
        }
        Ok(state)
    }

    fn check_syscall_memory(
        &mut self,
        effects: &Effects,
    ) -> Result<(), String> {
        // Syscall buffers remain byte-oriented, with each input byte a new value.
        if let Some(read) = &effects.mem_read {
            for address in read.address..read.address + read.value.len() as u32
            {
                if let Some((_, size)) = self.shadow_get(address)
                    && !matches!(
                        size,
                        ShadowSize::Byte | ShadowSize::Uninitialized
                    )
                {
                    return Err(
                        "Syscall write requires byte-level data".to_string()
                    );
                }
            }
        }
        if let Some(write) = &effects.mem_write {
            for address in
                write.address..write.address + write.new_value.len() as u32
            {
                if let Some((_, size)) = self.shadow_get(address)
                    && !matches!(
                        size,
                        ShadowSize::Byte | ShadowSize::Uninitialized
                    )
                {
                    return Err("Syscall read would overwrite non-byte data"
                        .to_string());
                }
            }
            for address in
                write.address..write.address + write.new_value.len() as u32
            {
                let state = RegisterState::Value(self.new_id());
                self.shadow_insert(address, state, ShadowSize::Byte);
            }
        }
        Ok(())
    }

    pub fn check_instruction(
        &mut self,
        m: &Machine,
        instruction: &Rc<Instruction>,
        effects: &mut Effects,
        reg_reads: &[RegisterValue],
    ) -> Result<(), String> {
        // Operand roles distinguish saving a value from using it as an address.
        for read in reg_reads {
            let register = read.register;
            match self.context.registers[register] {
                RegisterState::Unavailable => {
                    return Err(format!(
                        "Cannot use uninitialized {}",
                        R[register]
                    ));
                }
                RegisterState::Value(_) => {}
                RegisterState::SaveOnly(_) => {
                    let allowed = match instruction.op {
                        Op::Sw { rs1, rs2, .. } => {
                            register == rs2 && register != rs1
                        }
                        Op::Addi { rs1, imm: 0, .. } => register == rs1,
                        _ => false,
                    };
                    if !allowed {
                        return Err(format!(
                            "{} can only be stored or moved, not used as input",
                            R[register]
                        ));
                    }
                }
            }
        }

        // Memory effects carry actual addresses, including aliases and syscall buffers.
        let sp = m.get_reg(SP) as u32;
        let access_sp = effects
            .reg_write
            .filter(|write| write.register == SP)
            .map_or(sp, |write| write.old_value as u32);
        if let Some(read) = &effects.mem_read {
            self.check_stack_access(
                read.address,
                read.value.len(),
                access_sp,
                true,
            )?;
        }
        if let Some(write) = &effects.mem_write {
            self.check_stack_access(
                write.address,
                write.new_value.len(),
                access_sp,
                false,
            )?;
        }

        if let Some(write) = &effects.reg_write {
            let register = write.register;
            if !self.callers.is_empty() && matches!(register, GP | TP) {
                return Err(format!(
                    "{} must be preserved across function call; cannot modify inside a function",
                    R[register]
                ));
            }
            if register == SP && sp & 15 != 0 {
                return Err("Stack pointer must be 16-byte aligned".to_string());
            }
            self.context.registers[register] = match instruction.op {
                Op::Addi { rs1, imm: 0, .. } => self.context.registers[rs1],
                _ => RegisterState::Value(self.new_id()),
            };
        }

        match instruction.op {
            Op::Jal { rd: 1..32, .. } | Op::Jalr { rd: 1..32, .. } => {
                self.enter_call(m, effects)?
            }
            Op::Jalr { rd: ZERO, rs1: RA, offset: 0 } => {
                self.leave_call(m, effects)?
            }
            Op::Sb { .. } | Op::Sh { .. } | Op::Sw { .. } => {
                let write = effects
                    .mem_write
                    .as_ref()
                    .ok_or("store instruction with no memory write")?;
                // Only full-word stores preserve the source's identity and save-only status.
                let state = match instruction.op {
                    Op::Sw { rs2, .. } => self.context.registers[rs2],
                    _ => RegisterState::Value(self.new_id()),
                };
                self.store_value(write, state)?;
            }
            Op::Lb { rd, .. }
            | Op::Lbu { rd, .. }
            | Op::Lh { rd, .. }
            | Op::Lhu { rd, .. }
            | Op::Lw { rd, .. } => {
                let read = effects
                    .mem_read
                    .as_ref()
                    .ok_or("load instruction with no memory read")?;
                let state = self.load_value(read)?;
                if rd != ZERO {
                    self.context.registers[rd] = state;
                }
            }
            Op::Ecall => self.check_syscall_memory(effects)?,
            _ => {}
        }
        // Complete loads using the old frame before clearing bytes released by SP.
        if let Some(write) =
            effects.reg_write.filter(|write| write.register == SP)
        {
            let old_sp = write.old_value as u32;
            if sp > old_sp {
                self.stack_shadow.clear(old_sp, sp);
            }
        }
        Ok(())
    }
}
