use std::ops::Range;
use std::rc::Rc;

use crate::Instruction;
use crate::error::RiscletError;
use crate::riscv::R;

#[derive(Clone)]
pub struct MemoryValue {
    pub address: u32,
    pub value: Vec<u8>,
}

#[derive(Clone)]
pub struct MemoryWrite {
    pub address: u32,
    pub old_value: Vec<u8>,
    pub new_value: Vec<u8>,
}

#[derive(Clone)]
pub struct RegisterValue {
    pub register: usize,
}

#[derive(Clone, Copy)]
pub struct RegisterWrite {
    pub register: usize,
    pub old_value: i32,
    pub new_value: i32,
}

#[derive(Clone)]
pub enum SyscallInfo {
    Exit(i32),
    Write { fd: i32, buf_addr: u32, count: i32, data: Range<usize> },
    Read { fd: i32, buf_addr: u32, count: i32, data: Range<usize> },
}

// Stream bytes outlive replay state; events retain their execution order.
#[derive(Default)]
pub struct IoRecord {
    pub stdin: Vec<u8>,
    pub stdout: Vec<u8>,
}

#[derive(Clone)]
pub enum IoEvent {
    Input(Range<usize>),
    Output(Range<usize>),
}

// Syscall payloads and errors are allocated only for instructions that need them.
#[derive(Clone, Default)]
pub struct ExtraEffects {
    pub syscall: Option<SyscallInfo>,
    pub other_message: Option<RiscletError>,
}

#[derive(Clone, Copy)]
pub enum FrameChange {
    Enter(u32),
    Leave(u32),
}

#[derive(Clone)]
pub struct Effects {
    pub instruction: Rc<Instruction>,

    pub pc: (u32, u32),
    pub reg_write: Option<RegisterWrite>,
    pub mem_read: Option<MemoryValue>,
    pub mem_write: Option<MemoryWrite>,
    extra: Option<Box<ExtraEffects>>,
    pub frame_change: Option<FrameChange>,
}

impl Effects {
    pub fn new(instruction: &Rc<Instruction>) -> Self {
        Effects {
            instruction: instruction.clone(),
            pc: (0, 0),
            reg_write: None,
            mem_read: None,
            mem_write: None,
            extra: None,
            frame_change: None,
        }
    }

    pub fn error(&mut self, error: RiscletError) {
        self.extra_mut().other_message = Some(error);
    }

    // Ordinary instructions leave the optional extra record unallocated.
    pub fn extra_mut(&mut self) -> &mut ExtraEffects {
        self.extra.get_or_insert_with(Box::default)
    }

    pub fn other_message(&self) -> Option<&RiscletError> {
        self.extra.as_ref()?.other_message.as_ref()
    }

    pub fn is_terminal(&self) -> bool {
        self.other_message().is_some()
    }

    // Borrowed payloads support reporting and replay without allocating a record.
    pub fn stdin<'a>(&self, io: &'a IoRecord) -> Option<&'a [u8]> {
        match self.syscall()? {
            SyscallInfo::Read { data, .. } => Some(&io.stdin[data.clone()]),
            _ => None,
        }
    }

    pub fn stdout<'a>(&self, io: &'a IoRecord) -> Option<&'a [u8]> {
        match self.syscall()? {
            SyscallInfo::Write { data, .. } => Some(&io.stdout[data.clone()]),
            _ => None,
        }
    }

    pub fn syscall(&self) -> Option<&SyscallInfo> {
        self.extra.as_ref()?.syscall.as_ref()
    }

    pub fn report(&self, hex_mode: bool, io: &IoRecord) -> Vec<String> {
        let mut lines = Vec::new();

        // Handle syscalls specially - they replace normal output formatting
        if let Some(syscall) = self.syscall() {
            match syscall {
                SyscallInfo::Exit(status) => {
                    lines.push(format!("exit({})", status));
                }
                SyscallInfo::Write { buf_addr, count, data, .. } => {
                    let data = &io.stdout[data.clone()];
                    if hex_mode {
                        lines.push(format!(
                            "write(1, 0x{:x}, 0x{:x})",
                            buf_addr, count
                        ));
                    } else {
                        lines.push(format!(
                            "write(1, 0x{:x}, {})",
                            buf_addr, count
                        ));
                    }
                    let msg = String::from_utf8_lossy(data).into_owned();
                    lines.push(format!(
                        "a0 <- {}",
                        if hex_mode {
                            format!("0x{:x}", data.len())
                        } else {
                            data.len().to_string()
                        }
                    ));
                    lines.push(format!("0x{:x}: {:?}", buf_addr, msg));
                }
                SyscallInfo::Read { buf_addr, count, data, .. } => {
                    let data = &io.stdin[data.clone()];
                    if hex_mode {
                        lines.push(format!(
                            "read(0, 0x{:x}, 0x{:x})",
                            buf_addr, count
                        ));
                    } else {
                        lines.push(format!(
                            "read(0, 0x{:x}, {})",
                            buf_addr, count
                        ));
                    }
                    let msg = String::from_utf8_lossy(data).into_owned();
                    lines.push(format!(
                        "a0 <- {}",
                        if hex_mode {
                            format!("0x{:x}", data.len())
                        } else {
                            data.len().to_string()
                        }
                    ));
                    lines.push(format!("0x{:x}: {:?}", buf_addr, msg));
                }
            }
            // Don't add error message when syscall is already reported
        } else {
            // Normal instruction effect reporting
            let mut parts = Vec::new();
            if let Some(RegisterWrite {
                register: rd, new_value: val, ..
            }) = self.reg_write
            {
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
            lines.push(parts.join(", "));

            if let Some(error) = self.other_message() {
                lines.push(error.message());
            }
        }

        lines
    }
}
