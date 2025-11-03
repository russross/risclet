use std::io::{self, Read, Write};

pub trait IoProvider {
    fn read_stdin(&mut self, buffer: &mut [u8]) -> Result<usize, String>;
    fn write_stdout(&mut self, data: &[u8]) -> Result<(), String>;
}

pub struct SystemIo;

impl IoProvider for SystemIo {
    fn read_stdin(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        let mut handle = io::stdin().lock();
        handle.read(buffer).map_err(|e| format!("read syscall error: {}", e))
    }

    fn write_stdout(&mut self, data: &[u8]) -> Result<(), String> {
        let mut handle = io::stdout().lock();
        handle.write_all(data).map_err(|e| format!("write syscall error: {}", e))
    }
}

#[cfg(test)]
pub struct TestIo {
    pub stdin_data: Vec<u8>,
    pub stdin_pos: usize,
    pub stdout_buffer: Vec<u8>,
}

#[cfg(test)]
impl TestIo {
    pub fn new() -> Self {
        Self { stdin_data: Vec::new(), stdin_pos: 0, stdout_buffer: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn with_stdin(mut self, data: Vec<u8>) -> Self {
        self.stdin_data = data;
        self.stdin_pos = 0;
        self
    }
}

#[cfg(test)]
impl Default for TestIo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl IoProvider for TestIo {
    fn read_stdin(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        let available = self.stdin_data.len() - self.stdin_pos;
        let to_read = std::cmp::min(buffer.len(), available);
        buffer[..to_read].copy_from_slice(&self.stdin_data[self.stdin_pos..self.stdin_pos + to_read]);
        self.stdin_pos += to_read;
        Ok(to_read)
    }

    fn write_stdout(&mut self, data: &[u8]) -> Result<(), String> {
        self.stdout_buffer.extend_from_slice(data);
        Ok(())
    }
}
