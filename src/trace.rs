use crate::memory::MemoryLayout;
use std::rc::Rc;

#[derive(Clone)]
pub struct MemoryValue {
    pub address: u32,
    pub value: Vec<u8>,
}

#[derive(Clone)]
pub struct RegisterValue {
    pub register: usize,
    pub value: i32,
}

#[derive(Clone)]
pub struct Effects {
    pub instruction: Rc<crate::Instruction>,

    pub pc: (u32, u32),
    pub reg_reads: Vec<RegisterValue>,
    pub reg_write: Option<(RegisterValue, RegisterValue)>,
    pub mem_read: Option<MemoryValue>,
    pub mem_write: Option<(MemoryValue, MemoryValue)>,
    pub stdin: Option<Vec<u8>>,
    pub stdout: Option<Vec<u8>>,
    pub other_message: Option<String>,
    pub terminate: bool,
    pub function_start: Option<u32>,
    pub function_end: Option<u32>,
}

impl Effects {
    pub fn new(instruction: &Rc<crate::Instruction>) -> Self {
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

    pub fn error(&mut self, msg: String) {
        self.other_message = Some(msg);
        self.terminate = true;
    }

    pub fn report(&self, hex_mode: bool) -> Vec<String> {
        use crate::riscv::R;

        let mut parts = Vec::new();
        if let Some((_, RegisterValue { register: rd, value: val })) =
            self.reg_write
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

pub struct ExecutionTrace {
    effects: Vec<Effects>,
    layout: MemoryLayout,
}

impl ExecutionTrace {
    pub fn new(layout: MemoryLayout) -> Self {
        Self { effects: Vec::new(), layout }
    }

    pub fn add(&mut self, effect: Effects) {
        self.effects.push(effect);
    }

    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn set_most_recent_memory(&self) -> (u32, (u32, usize), (u32, usize)) {
        let mut most_recent_memory = if self.layout.data_start > 0 {
            self.layout.data_start
        } else {
            self.layout.stack_end.saturating_sub(8)
        };
        let mut most_recent_data = (self.layout.data_start, 0);
        let mut most_recent_stack =
            (self.layout.stack_end.saturating_sub(8), 0);

        let mut stack = false;
        let mut data = false;

        for effect in self.effects.iter().rev() {
            let (address, value_len) = if let Some(read) = &effect.mem_read {
                (read.address, read.value.len())
            } else if let Some((_, write)) = &effect.mem_write {
                (write.address, write.value.len())
            } else {
                continue;
            };

            if !stack && address >= self.layout.stack_start {
                most_recent_stack = (address, value_len);
                if !data {
                    most_recent_memory = address;
                }
                stack = true;
            }

            if !data && address < self.layout.data_end {
                most_recent_data = (address, value_len);
                if !stack {
                    most_recent_memory = address;
                }
                data = true;
            }

            if stack && data {
                break;
            }
        }

        (most_recent_memory, most_recent_data, most_recent_stack)
    }
}
