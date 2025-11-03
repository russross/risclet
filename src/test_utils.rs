#[cfg(test)]
mod tests {
    use crate::execution::{Machine, MachineBuilder};

    #[test]
    fn test_create_test_machine() {
        let m = Machine::for_testing();
        assert!(m.pc() > 0);
    }

    #[test]
    fn test_create_test_machine_with_memory() {
        let _m = MachineBuilder::new().with_flat_memory(1024).build();
        // Machine created successfully
    }
}
