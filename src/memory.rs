const STACK_SIZE: i64 = 8192;

pub struct Segment {
    start: i64,
    end: i64,
    mem: Vec<u8>,
    init: Vec<u8>,
    writeable: bool,
    executable: bool,
}

impl Segment {
    pub fn new(start: i64, end: i64, writeable: bool, executable: bool, init: Vec<u8>) -> Self {
        assert!(start > 0 && end > start);
        assert!(init.len() <= (end - start) as usize);
        Self { start, end, mem: Vec::new(), init, writeable, executable }
    }

    pub fn in_range(&self, addr: i64, size: i64) -> bool {
        addr >= self.start && addr + size <= self.end
    }

    pub fn is_executable(&self) -> bool {
        self.executable
    }

    pub fn is_writeable(&self) -> bool {
        self.writeable
    }

    pub fn reset(&mut self) {
        self.mem = self.init.clone();
        self.mem.resize((self.end - self.start) as usize, 0);
    }

    pub fn load(&self, addr: i64, size: i64) -> &[u8] {
        assert!(self.in_range(addr, size));
        &self.mem[(addr - self.start) as usize..(addr + size - self.start) as usize]
    }

    pub fn store(&mut self, addr: i64, raw: &[u8]) {
        assert!(self.in_range(addr, raw.len() as i64));
        let offset = (addr - self.start) as usize;
        self.mem[offset..offset + raw.len()].copy_from_slice(raw);
    }
}

#[derive(Clone, Copy)]
pub struct MemoryLayout {
    pub stack_start: i64,
    pub stack_end: i64,
    pub data_start: i64,
    pub data_end: i64,
    pub text_start: i64,
    pub text_end: i64,
}

impl MemoryLayout {
    pub fn new(segments: &[Segment]) -> Self {
        let mut stack_start = 0x100000 - STACK_SIZE;
        for segment in segments {
            if segment.end + STACK_SIZE >= stack_start {
                stack_start = (segment.end + STACK_SIZE * 2 - 1) & (STACK_SIZE - 1);
            }
        }

        let stack_end = stack_start + STACK_SIZE;
        let mut data_start = stack_end - 8;
        let mut data_end = 0;
        let mut text_start = stack_end;
        let mut text_end = 0;

        for segment in segments {
            if segment.executable {
                text_start = text_start.min(segment.start);
                text_end = text_end.max(segment.end);
            } else {
                data_start = data_start.min(segment.start);
                data_end = data_end.max(segment.end);
            }
        }

        if data_start == stack_end - 8 && data_end == 0 {
            data_start = 0;
        }

        Self { stack_start, stack_end, data_start, data_end, text_start, text_end }
    }
}

pub struct MemoryManager {
    pub segments: Vec<Segment>,
    pub layout: MemoryLayout,
}

impl MemoryManager {
    pub fn new(mut segments: Vec<Segment>) -> Self {
        let layout = MemoryLayout::new(&segments);
        segments.push(Segment::new(layout.stack_start, layout.stack_end, true, false, Vec::new()));
        Self { segments, layout }
    }

    pub fn reset(&mut self) {
        for segment in &mut self.segments {
            segment.reset();
        }
    }

    pub fn load(&self, addr: i64, size: i64) -> Result<Vec<u8>, String> {
        for segment in &self.segments {
            if segment.in_range(addr, size) {
                return Ok(segment.load(addr, size).to_vec());
            }
        }
        Err(format!("segfault: load addr=0x{:x} size={}", addr, size))
    }

    pub fn load_raw(&self, addr: i64, size: i64) -> Result<&[u8], String> {
        for segment in &self.segments {
            if segment.in_range(addr, size) {
                return Ok(segment.load(addr, size));
            }
        }
        Err(format!("segfault: load addr=0x{:x} size={}", addr, size))
    }

    pub fn store(&mut self, addr: i64, raw: &[u8]) -> Result<(), String> {
        let size = raw.len() as i64;
        for segment in &mut self.segments {
            if segment.in_range(addr, size) && segment.writeable {
                segment.store(addr, raw);
                return Ok(());
            }
        }
        Err(format!("segfault: store addr=0x{:x} size={}", addr, size))
    }

    pub fn store_with_tracking(&mut self, addr: i64, raw: &[u8], mem_write: &mut Option<(Vec<u8>, Vec<u8>)>) -> Result<(), String> {
        let size = raw.len() as i64;
        for segment in &mut self.segments {
            if segment.in_range(addr, size) && segment.writeable {
                if let Some(tracking) = mem_write {
                    let offset = (addr - segment.start) as usize;
                    let old_val = segment.mem[offset..offset + raw.len()].to_vec();
                    *tracking = (old_val, raw.to_vec());
                }
                segment.store(addr, raw);
                return Ok(());
            }
        }
        Err(format!("segfault: store addr=0x{:x} size={}", addr, size))
    }

    pub fn load_instruction(&self, addr: i64) -> Result<(i32, i64), String> {
        for segment in &self.segments {
            if !segment.in_range(addr, 2) || !segment.executable {
                continue;
            }

            let raw = segment.load(addr, 2);
            let half = i16::from_le_bytes(raw.try_into().unwrap());

            if (half & 0b11) != 0b11 {
                return Ok((half as i32, 2));
            } else if segment.in_range(addr, 4) {
                let raw = segment.load(addr, 4);
                return Ok((i32::from_le_bytes(raw.try_into().unwrap()), 4));
            } else {
                return Err(format!("partial instruction at end of segment addr=0x{:x}", addr));
            }
        }
        Err(format!("segfault: instruction fetch addr=0x{:x}", addr))
    }
}

pub struct RegisterFile {
    x: [i64; 32],
}

impl RegisterFile {
    pub fn new() -> Self {
        Self { x: [0; 32] }
    }

    pub fn reset(&mut self) {
        self.x = [0; 32];
    }

    pub fn get(&self, reg: usize) -> i64 {
        self.x[reg]
    }

    pub fn set(&mut self, reg: usize, value: i64) {
        if reg != 0 {
            self.x[reg] = value;
        }
    }

    pub fn get32(&self, reg: usize) -> i32 {
        self.get(reg) as i32
    }

    pub fn set32(&mut self, reg: usize, value: i32) {
        self.set(reg, value as i64);
    }
}

pub struct CpuState {
    registers: RegisterFile,
    pc: i64,
    stdout: Vec<u8>,
    stdin: Vec<u8>,
    stack_frames: Vec<i64>,
}

impl CpuState {
    pub fn new(pc_start: i64) -> Self {
        Self {
            registers: RegisterFile::new(),
            pc: pc_start,
            stdout: Vec::new(),
            stdin: Vec::new(),
            stack_frames: Vec::new(),
        }
    }

    pub fn reset(&mut self, pc_start: i64, stack_end: i64) {
        self.registers.reset();
        self.registers.set(2, stack_end);
        self.pc = pc_start;
        self.stdout.clear();
        self.stdin.clear();
        self.stack_frames.clear();
    }

    pub fn get_reg(&self, reg: usize) -> i64 {
        self.registers.get(reg)
    }

    pub fn set_reg(&mut self, reg: usize, value: i64) {
        self.registers.set(reg, value);
    }

    pub fn get_reg32(&self, reg: usize) -> i32 {
        self.registers.get32(reg)
    }

    pub fn set_reg32(&mut self, reg: usize, value: i32) {
        self.registers.set32(reg, value);
    }

    pub fn pc(&self) -> i64 {
        self.pc
    }

    pub fn set_pc(&mut self, value: i64) {
        self.pc = value;
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stdout_mut(&mut self) -> &mut Vec<u8> {
        &mut self.stdout
    }

    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }

    pub fn stdin_mut(&mut self) -> &mut Vec<u8> {
        &mut self.stdin
    }

    pub fn stack_frames(&self) -> &[i64] {
        &self.stack_frames
    }

    pub fn push_stack_frame(&mut self, frame: i64) {
        self.stack_frames.push(frame);
    }

    pub fn pop_stack_frame(&mut self) {
        self.stack_frames.pop();
    }
}
