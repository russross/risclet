use std::collections::HashMap;

pub trait LintContext {
    fn get_register(&self, reg: usize) -> i32;
    fn get_symbol_for_address(&self, addr: u32) -> Option<&String>;
    fn get_symbol_value(&self, name: &str) -> Option<u32>;
}

pub struct TestLintContext {
    pub registers: [i32; 32],
    pub symbols: HashMap<u32, String>,
    pub other_symbols: HashMap<String, u32>,
}

impl TestLintContext {
    pub fn new() -> Self {
        Self { registers: [0; 32], symbols: HashMap::new(), other_symbols: HashMap::new() }
    }

    pub fn with_register(mut self, reg: usize, value: i32) -> Self {
        self.registers[reg] = value;
        self
    }

    pub fn with_symbol(mut self, addr: u32, name: String) -> Self {
        self.symbols.insert(addr, name);
        self
    }

    pub fn with_other_symbol(mut self, name: String, value: u32) -> Self {
        self.other_symbols.insert(name, value);
        self
    }
}

impl Default for TestLintContext {
    fn default() -> Self {
        Self::new()
    }
}

impl LintContext for TestLintContext {
    fn get_register(&self, reg: usize) -> i32 {
        self.registers[reg]
    }

    fn get_symbol_for_address(&self, addr: u32) -> Option<&String> {
        self.symbols.get(&addr)
    }

    fn get_symbol_value(&self, name: &str) -> Option<u32> {
        self.other_symbols.get(name).copied()
    }
}
