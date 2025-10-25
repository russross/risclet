use crate::memory::Segment;
use std::collections::HashMap;

pub trait MemoryInterface {
    fn load(&self, addr: u32, size: u32) -> Result<Vec<u8>, String>;
    fn load_raw(&self, addr: u32, size: u32) -> Result<&[u8], String>;
    fn store(&mut self, addr: u32, data: &[u8]) -> Result<(), String>;
    fn load_instruction(&self, addr: u32) -> Result<(i32, u32), String>;
    fn reset(&mut self);
}

pub struct FlatMemory {
    data: Vec<u8>,
    base: u32,
    size: u32,
}

impl FlatMemory {
    pub fn new(base: u32, size: u32) -> Self {
        Self {
            data: vec![0; size as usize],
            base,
            size,
        }
    }

    pub fn with_init(base: u32, init_data: &[u8]) -> Self {
        let size = (init_data.len() as u32).max(1024);
        let mut data = vec![0; size as usize];
        data[..init_data.len()].copy_from_slice(init_data);
        Self { data, base, size }
    }
}

impl MemoryInterface for FlatMemory {
    fn load(&self, addr: u32, size: u32) -> Result<Vec<u8>, String> {
        if addr < self.base || addr + size > self.base + self.size {
            return Err(format!("segfault: load addr=0x{:x} size={}", addr, size));
        }
        let offset = (addr - self.base) as usize;
        Ok(self.data[offset..offset + size as usize].to_vec())
    }

    fn load_raw(&self, addr: u32, size: u32) -> Result<&[u8], String> {
        if addr < self.base || addr + size > self.base + self.size {
            return Err(format!("segfault: load addr=0x{:x} size={}", addr, size));
        }
        let offset = (addr - self.base) as usize;
        Ok(&self.data[offset..offset + size as usize])
    }

    fn store(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        let size = data.len() as u32;
        if addr < self.base || addr + size > self.base + self.size {
            return Err(format!("segfault: store addr=0x{:x} size={}", addr, size));
        }
        let offset = (addr - self.base) as usize;
        self.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    fn load_instruction(&self, addr: u32) -> Result<(i32, u32), String> {
        if addr < self.base || addr + 2 > self.base + self.size {
            return Err(format!("segfault: instruction fetch addr=0x{:x}", addr));
        }

        let offset = (addr - self.base) as usize;
        let raw = &self.data[offset..offset + 2];
        let half = i16::from_le_bytes(raw.try_into().unwrap());

        if (half & 0b11) != 0b11 {
            return Ok((half as i32, 2));
        } else if addr + 4 <= self.base + self.size {
            let raw = &self.data[offset..offset + 4];
            return Ok((i32::from_le_bytes(raw.try_into().unwrap()), 4));
        } else {
            return Err(format!("partial instruction at end of segment addr=0x{:x}", addr));
        }
    }

    fn reset(&mut self) {
        self.data.fill(0);
    }
}
