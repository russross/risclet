use std::iter::once;
use std::rc::Rc;

use crate::error::{Diagnostic, DiagnosticSite, address as address_string};
use crate::execution::{Instruction, Machine};
use crate::memory::MemoryLayout;
use crate::riscv::{A_REGS, GP, Op, R, RA, S_REGS, SP, T_REGS, TP, ZERO};
use crate::trace::{
    Effects, FrameChange, MemoryAccess, MemoryWrite, RegisterValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValueId(u64);

// Reasons belong to a dynamic invocation, so loops and recursion retain the
// call that actually invalidated a value rather than a nearby static call.
#[derive(Debug, Clone, Copy)]
enum UnavailableReason {
    ProgramEntry,
    FunctionEntry(u32),
    ExcludedArgument(u32),
    ReturnedCall(u32),
    MissingReturn(u32),
}

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
    fn byte_count(self) -> usize {
        match self {
            Self::Uninitialized => 0,
            Self::Byte => 1,
            Self::HalfWord => 2,
            Self::Word => 4,
        }
    }
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
    origins: Vec<u32>,
}

impl ShadowSegment {
    fn new(start: u32, end: u32) -> Self {
        let length = end.saturating_sub(start) as usize;
        Self { start, entries: vec![0; length], origins: vec![0; length] }
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

    fn clear(&mut self, start: u32, end: u32, instruction: u32) {
        let first = start.saturating_sub(self.start) as usize;
        let last =
            (end.saturating_sub(self.start) as usize).min(self.entries.len());
        if first < last {
            self.entries[first..last].fill(0);
            self.origins[first..last].fill(instruction);
        }
    }
}

#[derive(Clone)]
struct FunctionContext {
    registers: [RegisterState; 32],
    at_entry: [Option<ValueId>; 32],
    at_entry_sp: u32,
    unavailable: [UnavailableReason; 32],
    last_write: [Option<u32>; 32],
    call_site: Option<u32>,
}

pub struct CheckABI {
    program_shadow: [ShadowSegment; 2],
    stack_shadow: ShadowSegment,
    heap_shadow: ShadowSegment,
    context: FunctionContext,
    callers: Vec<FunctionContext>,
    next_id: u64,
    current_instruction: u32,
    layout: MemoryLayout,
    hex: bool,
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
            heap_shadow: ShadowSegment::new(0, 0),
            context: FunctionContext {
                registers,
                at_entry: registers.map(RegisterState::identity),
                at_entry_sp,
                unavailable: [UnavailableReason::ProgramEntry; 32],
                last_write: [None; 32],
                call_site: None,
            },
            callers: Vec::new(),
            next_id: 2,
            current_instruction: 0,
            layout: MemoryLayout {
                text_start,
                text_end,
                data_start,
                data_end,
                stack_start,
                stack_end,
            },
            hex: false,
        }
    }

    pub fn with_numeric_style(mut self, hex: bool) -> Self {
        self.hex = hex;
        self
    }

    fn memory_address(&self, value: u32) -> String {
        if self.heap_shadow.offset(value).is_some() {
            return format!("heap address {}", address_string(value, self.hex));
        }
        self.layout.describe_address(value, self.hex)
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
            .chain(once(&self.heap_shadow))
            .find_map(|segment| segment.get(address))
    }

    fn shadow_insert(
        &mut self,
        address: u32,
        state: RegisterState,
        size: ShadowSize,
        origin: u32,
    ) {
        let segment = self
            .program_shadow
            .iter_mut()
            .chain(once(&mut self.stack_shadow))
            .chain(once(&mut self.heap_shadow))
            .find(|segment| segment.offset(address).is_some())
            .expect("machine-validated memory address");
        segment.insert(address, state, size);
        let offset = segment.offset(address).expect("validated shadow address");
        segment.origins[offset] = origin;
    }

    fn check_dynamic_access(
        &self,
        address: u32,
        size: usize,
        sp: u32,
        read: bool,
    ) -> Result<(), Diagnostic> {
        for address in u64::from(address)..u64::from(address) + size as u64 {
            let address = u32::try_from(address)
                .map_err(|_| "Memory access wraps address space")?;
            if read
                && self
                    .heap_shadow
                    .get(address)
                    .is_some_and(|(_, size)| size == ShadowSize::Uninitialized)
            {
                let mut diagnostic = Diagnostic::from(format!(
                    "cannot read from uninitialized {}",
                    self.memory_address(address)
                ));
                diagnostic.help = Some("Initialize these heap bytes before reading them. Growing the heap does not initialize its contents in strict mode.".into());
                return Err(diagnostic);
            }
            if self.stack_shadow.offset(address).is_none() {
                continue;
            }
            if address < sp {
                let mut diagnostic = Diagnostic::from(format!(
                    "cannot access {} below sp ({})",
                    self.memory_address(address),
                    address_string(sp, self.hex)
                ));
                diagnostic.help = Some("Allocate stack space by decreasing sp before accessing it. Do not use storage that has already been released.".into());
                return Err(diagnostic);
            }
            if read
                && self
                    .stack_shadow
                    .get(address)
                    .is_some_and(|(_, size)| size == ShadowSize::Uninitialized)
            {
                let mut diagnostic = Diagnostic::from(format!(
                    "cannot read from uninitialized {}",
                    self.memory_address(address)
                ));
                diagnostic.help = Some("Initialize these stack bytes before reading them. Allocating stack space does not initialize its contents.".into());
                return Err(diagnostic);
            }
        }
        Ok(())
    }

    fn enter_call(
        &mut self,
        m: &Machine,
        effects: &mut Effects,
    ) -> Result<(), Diagnostic> {
        if effects.reg_write.is_none_or(|write| write.register.index() != RA) {
            return Err("Return address must be stored in ra; use call for ordinary function calls".to_string().into());
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
                ).into());
            }
            for &register in A_REGS.iter().take(count as usize) {
                match self.context.registers[register] {
                    RegisterState::Value(_) => {}
                    RegisterState::Unavailable => {
                        return Err(self.unavailable_diagnostic(
                            register,
                            format!(
                                "Function argument {} is uninitialized",
                                R[register]
                            ),
                        ));
                    }
                    RegisterState::SaveOnly(_) => {
                        return Err(format!(
                            "Function argument {} is save-only",
                            R[register]
                        )
                        .into());
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
                    )
                    .into());
                }
            }
        }

        self.callers.push(self.context.clone());
        self.context.call_site = Some(effects.instruction.address);
        for &register in &T_REGS {
            self.context.registers[register] = RegisterState::Unavailable;
            self.context.unavailable[register] =
                UnavailableReason::FunctionEntry(effects.instruction.address);
        }
        if let Some(count) = count {
            for &register in A_REGS.iter().skip(count as usize) {
                self.context.registers[register] = RegisterState::Unavailable;
                self.context.unavailable[register] =
                    UnavailableReason::ExcludedArgument(
                        effects.instruction.address,
                    );
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
        self.context.last_write = [None; 32];
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
        let result_reason = self.context.unavailable[A_REGS[0]];
        let call_site =
            self.context.call_site.expect("matching call has a site");
        self.context = self.callers.pop().expect("checked matching call");
        self.context.registers[A_REGS[0]] = result;
        self.context.unavailable[A_REGS[0]] =
            if matches!(result, RegisterState::Unavailable)
                && matches!(
                    result_reason,
                    UnavailableReason::ProgramEntry
                        | UnavailableReason::ExcludedArgument(_)
                )
            {
                UnavailableReason::MissingReturn(call_site)
            } else {
                result_reason
            };
        self.context.last_write[A_REGS[0]] = Some(call_site);
        for &register in T_REGS.iter().chain(A_REGS.iter().skip(1)) {
            self.context.registers[register] = RegisterState::Unavailable;
            self.context.unavailable[register] =
                UnavailableReason::ReturnedCall(call_site);
        }
        effects.frame_change = Some(FrameChange::Leave(sp));
        Ok(())
    }

    fn store_value(
        &mut self,
        write: &MemoryWrite,
        state: RegisterState,
    ) -> Result<(), Diagnostic> {
        let width = write.byte_count();
        if write.address & (width as u32 - 1) != 0 {
            let mut diagnostic = Diagnostic::from(format!(
                "unaligned {width}-byte memory write to {}",
                self.memory_address(write.address)
            ));
            diagnostic.help = Some(format!(
                "a {width}-byte write to memory must be to an address that is divisible by {width}"
            ));
            return Err(diagnostic);
        }
        let size = ShadowSize::from_byte_count(width);
        for address in write.address..write.address + width as u32 {
            self.shadow_insert(address, state, size, self.current_instruction);
        }
        Ok(())
    }

    fn load_value(
        &mut self,
        read: &MemoryAccess,
    ) -> Result<RegisterState, Diagnostic> {
        let width = read.length as usize;
        if read.address & (width as u32 - 1) != 0 {
            let mut diagnostic = Diagnostic::from(format!(
                "unaligned {width}-byte memory read from {}",
                self.memory_address(read.address)
            ));
            diagnostic.help = Some(format!(
                "a {width}-byte read from memory must be from an address that is divisible by {width}"
            ));
            return Err(diagnostic);
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
                        "Cannot read: incomplete write before this read".into(),
                    );
                }
                continue;
            }
            if byte_size == ShadowSize::Uninitialized {
                return Err(
                    "Cannot read: incomplete write before this read".into()
                );
            }
            if state != stored {
                return Err(
                    "Cannot read: data spans multiple separate writes".into()
                );
            }
            if byte_size != size {
                let mut diagnostic = Diagnostic::from(format!(
                    "read size mismatches original write size: reading {width} bytes at {}, but the stored value has {} bytes",
                    self.memory_address(read.address),
                    stored_size.byte_count()
                ));
                diagnostic.help = Some("Use a matching load/store width. A load must read one complete value rather than part of a wider value.".into());
                return Err(diagnostic);
            }
        }

        if stored_size != ShadowSize::Uninitialized {
            return Ok(stored);
        }
        let state = RegisterState::Value(self.new_id());
        for address in read.address..read.address + width as u32 {
            self.shadow_insert(address, state, size, 0);
        }
        Ok(state)
    }

    fn check_syscall_memory(
        &mut self,
        effects: &Effects,
    ) -> Result<(), String> {
        // Syscall buffers remain byte-oriented, with each input byte a new value.
        if let Some(read) = &effects.mem_read {
            for address in read.address..read.address + read.length {
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
                write.address..write.address + write.byte_count() as u32
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
                write.address..write.address + write.byte_count() as u32
            {
                let state = RegisterState::Value(self.new_id());
                self.shadow_insert(
                    address,
                    state,
                    ShadowSize::Byte,
                    self.current_instruction,
                );
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
    ) -> Result<(), Diagnostic> {
        self.current_instruction = instruction.address;
        self.check_register_reads(instruction, reg_reads)?;
        self.check_effects(m, instruction, effects).map_err(|mut diagnostic| {
            self.add_failure_context(m, instruction, effects, &mut diagnostic);
            diagnostic
        })
    }

    // This validation also runs after a failed machine operation, before an
    // invalid pointer can hide the uninitialized register that produced it.
    pub fn check_register_reads(
        &self,
        instruction: &Instruction,
        reg_reads: &[RegisterValue],
    ) -> Result<(), Diagnostic> {
        // Operand roles distinguish saving a value from using it as an address.
        for read in reg_reads {
            let register = read.register;
            match self.context.registers[register] {
                RegisterState::Unavailable => {
                    return Err(self.unavailable_diagnostic(
                        register,
                        format!(
                            "cannot read from uninitialized {} register",
                            R[register]
                        ),
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
                        let mut diagnostic = Diagnostic::from(format!(
                            "{} can only be stored or moved, not used as input",
                            R[register]
                        ));
                        diagnostic.help = Some("This is an incoming callee-saved value. Save it before initializing the register for local work, then restore it before returning.".into());
                        if let Some(call) = self.context.call_site {
                            diagnostic.sites.push(DiagnosticSite::new(call, "this call supplied the incoming value that must be preserved"));
                        }
                        return Err(diagnostic);
                    }
                }
            }
        }

        Ok(())
    }

    fn add_failure_context(
        &self,
        m: &Machine,
        instruction: &Instruction,
        effects: &Effects,
        diagnostic: &mut Diagnostic,
    ) {
        // Shadow origins identify the write or release behind a bad read.
        if let Some(read) = &effects.mem_read {
            for address in u64::from(read.address)
                ..u64::from(read.address) + u64::from(read.length)
            {
                let Ok(address) = u32::try_from(address) else { break };
                if let Some((segment, offset)) = self
                    .program_shadow
                    .iter()
                    .chain(once(&self.stack_shadow))
                    .chain(once(&self.heap_shadow))
                    .find_map(|segment| {
                        segment.offset(address).map(|offset| (segment, offset))
                    })
                {
                    let origin = segment.origins[offset];
                    if origin != 0 {
                        let label = if std::ptr::eq(segment, &self.heap_shadow)
                            && segment.entries[offset] == 0
                        {
                            "this instruction acquired uninitialized heap storage"
                        } else if segment.entries[offset] == 0 {
                            "this instruction released the stack storage; allocating it again does not initialize it"
                        } else {
                            "this earlier write established the stored value and its width"
                        };
                        diagnostic
                            .sites
                            .push(DiagnosticSite::new(origin, label));
                        break;
                    }
                }
            }
        }
        if matches!(instruction.op, Op::Jalr { rd: ZERO, rs1: RA, offset: 0 })
            && !self.callers.is_empty()
        {
            for register in [RA, GP, TP].iter().chain(S_REGS.iter()).copied() {
                if self.context.registers[register].identity()
                    != self.context.at_entry[register]
                {
                    if let Some(address) = self.context.last_write[register] {
                        diagnostic.sites.push(DiagnosticSite::new(
                            address,
                            format!(
                                "this write replaced the incoming {} value",
                                R[register]
                            ),
                        ));
                    }
                    diagnostic.help = Some(format!(
                        "Save the incoming {} value before changing it and restore that saved value before ret; recomputing equal bits does not restore its identity.",
                        R[register]
                    ));
                    return;
                }
            }
            if m.get_reg(SP) as u32 != self.context.at_entry_sp {
                diagnostic.help = Some(format!(
                    "Restore sp to its entry value {}; its current value is {}.",
                    address_string(self.context.at_entry_sp, self.hex),
                    address_string(m.get_reg(SP) as u32, self.hex)
                ));
                if let Some(address) = self.context.last_write[SP] {
                    diagnostic.sites.push(DiagnosticSite::new(
                        address,
                        "this instruction last changed sp",
                    ));
                }
            }
        }
    }

    fn unavailable_diagnostic(
        &self,
        register: usize,
        message: String,
    ) -> Diagnostic {
        let mut diagnostic = Diagnostic::from(message);
        let (site, help) = match self.context.unavailable[register] {
            UnavailableReason::ProgramEntry => (None, format!("Initialize {} before reading it; only zero and sp are initialized at program entry.", R[register])),
            UnavailableReason::FunctionEntry(call) => (Some((call, "this call entered a function where temporary registers are unavailable")), "Pass function inputs in a0–a7, or initialize the temporary register inside the function.".into()),
            UnavailableReason::ExcludedArgument(call) => (Some((call, "this call excluded the register through the function's _args declaration")), "Check the function's _args count and pass inputs in the declared argument registers.".into()),
            UnavailableReason::ReturnedCall(call) => (Some((call, "caller-saved invalidation")), "The callee (the function being called) may overwrite a caller-saved register; it cannot be used to save a value across a function call even if the callee does not actually use it. Keep values needed across calls in saved registers, or write them to the stack before the call and read them back afterward.".into()),
            UnavailableReason::MissingReturn(call) => (Some((call, "this call returned without an initialized a0 result")), "Set a0 in the called function before returning if it produces a result; otherwise initialize a0 in the caller before using it.".into()),
        };
        diagnostic.help = Some(help);
        if let Some((address, label)) = site {
            let label = if matches!(
                self.context.unavailable[register],
                UnavailableReason::ReturnedCall(_)
            ) {
                format!(
                    "this earlier call made {} (a caller-saved register) unavailable for reading",
                    R[register]
                )
            } else {
                label.to_string()
            };
            diagnostic.sites.push(DiagnosticSite::new(address, label));
        }
        diagnostic
    }

    fn check_effects(
        &mut self,
        m: &Machine,
        instruction: &Rc<Instruction>,
        effects: &mut Effects,
    ) -> Result<(), Diagnostic> {
        // Resizing precedes subsequent accesses and drops released identities.
        if let Some(change) = effects.heap_change() {
            self.heap_shadow.start = m.heap_start();
            let length = (change.new_break - m.heap_start()) as usize;
            self.heap_shadow.entries.resize(length, 0);
            self.heap_shadow.origins.resize(length, instruction.address);
        }
        // Memory effects carry actual addresses, including aliases and syscall buffers.
        let sp = m.get_reg(SP) as u32;
        let access_sp = effects
            .reg_write
            .filter(|write| write.register.index() == SP)
            .map_or(sp, |write| write.old_value as u32);
        if let Some(read) = &effects.mem_read {
            self.check_dynamic_access(
                read.address,
                read.length as usize,
                access_sp,
                true,
            )?;
        }
        if let Some(write) = &effects.mem_write {
            self.check_dynamic_access(
                write.address,
                write.byte_count(),
                access_sp,
                false,
            )?;
        }

        if let Some(write) = &effects.reg_write {
            let register = write.register.index();
            if !self.callers.is_empty() && matches!(register, GP | TP) {
                return Err(format!(
                    "{} must be preserved across function call; cannot modify inside a function",
                    R[register]
                ).into());
            }
            if register == SP && sp & 15 != 0 {
                let mut diagnostic = Diagnostic::from(format!(
                    "stack pointer must be 16-byte aligned; sp is {}",
                    address_string(sp, self.hex)
                ));
                diagnostic.help = Some("Allocate and release stack space in multiples of 16 bytes so that sp remains divisible by 16.".into());
                return Err(diagnostic);
            }
            self.context.registers[register] = match instruction.op {
                Op::Addi { rs1, imm: 0, .. } => self.context.registers[rs1],
                _ => RegisterState::Value(self.new_id()),
            };
            self.context.last_write[register] = Some(instruction.address);
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
            effects.reg_write.filter(|write| write.register.index() == SP)
        {
            let old_sp = write.old_value as u32;
            if sp > old_sp {
                self.stack_shadow.clear(old_sp, sp, instruction.address);
            }
        }
        Ok(())
    }
}
