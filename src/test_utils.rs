#[cfg(test)]
pub mod test_helpers {
    use crate::execution::{Machine, MachineBuilder};
    use crate::execution_context::TestExecutionContext;
    use crate::io_abstraction::TestIo;
    use crate::linter_context::TestLintContext;

    pub fn create_test_machine() -> Machine {
        Machine::for_testing()
    }

    pub fn create_test_machine_with_memory(size: u32) -> Machine {
        MachineBuilder::new().with_flat_memory(size).build()
    }

    pub fn create_test_execution_context() -> TestExecutionContext {
        TestExecutionContext::new()
    }

    pub fn create_test_lint_context() -> TestLintContext {
        TestLintContext::new()
    }

    pub fn create_test_io_with_stdin(data: Vec<u8>) -> TestIo {
        TestIo::new().with_stdin(data)
    }

    pub fn assert_register_eq(ctx: &TestExecutionContext, reg: usize, value: i32, reg_name: &str) {
        assert_eq!(ctx.registers[reg], value, "Register {} was {}, expected {}", reg_name, ctx.registers[reg], value);
    }

    pub fn assert_memory_eq(ctx: &TestExecutionContext, addr: u32, expected: &[u8]) {
        for (i, &expected_byte) in expected.iter().enumerate() {
            let actual_byte = ctx.memory.get(&(addr + i as u32)).copied().unwrap_or(0);
            assert_eq!(
                actual_byte,
                expected_byte,
                "Memory mismatch at address 0x{:x}: expected 0x{:x}, got 0x{:x}",
                addr + i as u32,
                expected_byte,
                actual_byte
            );
        }
    }

    pub fn assert_io_output(io: &TestIo, expected: &[u8]) {
        assert_eq!(
            io.stdout_buffer,
            expected,
            "I/O output mismatch: expected {:?}, got {:?}",
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(&io.stdout_buffer)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use crate::execution_context::ExecutionContext;
    use crate::io_abstraction::IoProvider;

    #[test]
    fn test_create_test_machine() {
        let m = create_test_machine();
        assert!(m.pc() > 0);
    }

    #[test]
    fn test_create_test_execution_context() {
        let ctx = create_test_execution_context();
        assert_eq!(ctx.pc, 0x1000);
        assert_eq!(ctx.registers[0], 0);
    }

    #[test]
    fn test_execution_context_register_operations() {
        let ctx = create_test_execution_context().with_register(1, 42).with_register(2, 100);

        assert_eq!(ctx.registers[1], 42);
        assert_eq!(ctx.registers[2], 100);
    }

    #[test]
    fn test_execution_context_memory_operations() {
        let mut ctx = create_test_execution_context().with_memory(0x1000, &[1, 2, 3, 4]);

        let mem = ctx.read_memory(0x1000, 4).unwrap();
        assert_eq!(mem, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_test_io() {
        let mut io = create_test_io_with_stdin(b"hello".to_vec());
        let mut buf = [0u8; 10];
        let n = io.read_stdin(&mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..5], b"hello");
    }

    #[test]
    fn test_test_io_output() {
        let mut io = create_test_io_with_stdin(vec![]);
        io.write_stdout(b"output").unwrap();
        assert_io_output(&io, b"output");
    }
}
