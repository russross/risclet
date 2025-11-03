use crate::trace::Effects;

pub trait ExecutionContext {
    fn read_register(&mut self, reg: usize) -> i32;
    fn write_register(&mut self, reg: usize, value: i32);
    fn read_memory(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, String>;
    fn write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), String>;
    fn read_pc(&self) -> u32;
    fn write_pc(&mut self, pc: u32) -> Result<(), String>;
    fn current_effects(&mut self) -> Option<&mut Effects>;
}
