use crate::io_abstraction::IoProvider;
use crate::trace::Effects;
use std::rc::Rc;

pub trait ExecutionContext {
    fn read_register(&mut self, reg: usize) -> i32;
    fn write_register(&mut self, reg: usize, value: i32);
    fn read_memory(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, String>;
    fn write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), String>;
    fn read_pc(&self) -> u32;
    fn write_pc(&mut self, pc: u32) -> Result<(), String>;
    fn io_provider(&mut self) -> &mut dyn IoProvider;
    fn current_effects(&mut self) -> Option<&mut Effects>;
}

pub struct TestExecutionContext {
    pub registers: [i32; 32],
    pub memory: std::collections::HashMap<u32, u8>,
    pub pc: u32,
    pub io: crate::io_abstraction::TestIo,
}

impl TestExecutionContext {
    pub fn new() -> Self {
        Self {
            registers: [0; 32],
            memory: std::collections::HashMap::new(),
            pc: 0x1000,
            io: crate::io_abstraction::TestIo::new(),
        }
    }

    pub fn with_register(mut self, reg: usize, value: i32) -> Self {
        self.registers[reg] = value;
        self
    }

    pub fn with_memory(mut self, addr: u32, data: &[u8]) -> Self {
        for (i, &byte) in data.iter().enumerate() {
            self.memory.insert(addr + i as u32, byte);
        }
        self
    }

    pub fn with_stdin(mut self, data: Vec<u8>) -> Self {
        self.io = self.io.with_stdin(data);
        self
    }
}

impl Default for TestExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionContext for TestExecutionContext {
    fn read_register(&mut self, reg: usize) -> i32 {
        self.registers[reg]
    }

    fn write_register(&mut self, reg: usize, value: i32) {
        if reg != 0 {
            self.registers[reg] = value;
        }
    }

    fn read_memory(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, String> {
        let mut result = Vec::new();
        for i in 0..size {
            match self.memory.get(&(addr + i)) {
                Some(&byte) => result.push(byte),
                None => return Err(format!("segfault: load addr=0x{:x} size={}", addr, size)),
            }
        }
        Ok(result)
    }

    fn write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        for (i, &byte) in data.iter().enumerate() {
            self.memory.insert(addr + i as u32, byte);
        }
        Ok(())
    }

    fn read_pc(&self) -> u32 {
        self.pc
    }

    fn write_pc(&mut self, pc: u32) -> Result<(), String> {
        if pc & 1 != 0 {
            return Err(format!("bus error: pc addr={:x}", pc));
        }
        self.pc = pc;
        Ok(())
    }

    fn io_provider(&mut self) -> &mut dyn IoProvider {
        &mut self.io
    }

    fn current_effects(&mut self) -> Option<&mut Effects> {
        None
    }
}
