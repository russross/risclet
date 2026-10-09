#[cfg(test)]
mod tests {
    use crate::config::{Config, Mode};
    use crate::error::RiscletError;
    use crate::execution::{Instruction, Machine, MachineBuilder, trace};
    use crate::riscv::{Op, ZERO};
    use crate::trace::{Effects, FrameChange, SyscallInfo};
    use std::collections::HashMap;
    use std::rc::Rc;

    fn instruction(machine: &Machine, op: Op) -> Rc<Instruction> {
        instruction_at(machine.pc(), op)
    }

    fn instruction_at(address: u32, op: Op) -> Rc<Instruction> {
        Rc::new(Instruction {
            address,
            verbose_fields: op.to_fields(),
            pseudo_fields: op.to_pseudo_fields(),
            op,
            length: 4,
            encoding: 0,
            pseudo_index: 0,
        })
    }

    #[test]
    fn test_atomic_memory_write_replays_both_values() {
        let mut machine = Machine::for_testing();
        let address = machine.stack_start();
        machine.store(address, &17_i32.to_le_bytes()).unwrap();
        machine.set(5, address as i32);
        machine.set(6, 9);
        let inst = instruction(
            &machine,
            Op::AmoaddW { rd: 7, rs1: 5, rs2: 6, aq: false, rl: false },
        );

        // Atomic instructions retain their read as well as the before/after write.
        let effects = machine.execute_and_collect_effects(&inst);
        let read = effects.mem_read.as_ref().expect("memory read");
        let write = effects.mem_write.as_ref().expect("memory write");
        assert_eq!(read.address, address);
        assert_eq!(read.value, 17_i32.to_le_bytes());
        assert_eq!(write.address, address);
        assert_eq!(write.old_value, 17_i32.to_le_bytes());
        assert_eq!(write.new_value, 26_i32.to_le_bytes());

        // Reverse replay restores memory and the destination register together.
        machine.apply(&effects, false);
        assert_eq!(machine.load_i32(address).unwrap(), 17);
        assert_eq!(machine.get_reg(7), 0);
        machine.apply(&effects, true);
        assert_eq!(machine.load_i32(address).unwrap(), 26);
        assert_eq!(machine.get_reg(7), 17);
    }

    #[test]
    fn test_boxed_io_and_error_survive_clone_and_replay() {
        let mut machine = Machine::for_testing();
        let inst = instruction(&machine, Op::Ecall);
        let mut effects = Effects::new(&inst);
        assert!(effects.stdin(&machine.io).is_none());
        assert!(effects.stdout(&machine.io).is_none());
        assert!(effects.syscall().is_none());
        assert!(!effects.is_terminal());

        // I/O payloads and a later failure coexist without losing replay data.
        let extra = effects.extra_mut();
        machine.io.stdout.extend_from_slice(b"out");
        extra.syscall = Some(SyscallInfo::Write {
            fd: 1,
            buf_addr: 0x1000,
            count: 3,
            data: 0..3,
        });
        effects.error(RiscletError::io("output failed".to_string()));
        let cloned = effects.clone();
        machine.io.stdout.extend_from_slice(b"more");
        assert_eq!(cloned.stdout(&machine.io), Some(b"out".as_slice()));
        assert!(cloned.is_terminal());
        assert_eq!(cloned.other_message().unwrap().message(), "output failed");
        assert_eq!(cloned.report(false, &machine.io)[0], "write(1, 0x1000, 3)");

        // Rewinding hides the event while retaining its backing bytes.
        machine.apply(&cloned, true);
        assert_eq!(machine.output_events.len(), 1);
        machine.apply(&cloned, false);
        assert!(machine.output_events.is_empty());
        assert_eq!(cloned.stdout(&machine.io), Some(b"out".as_slice()));
    }

    // The same syscall can be executed repeatedly without retaining non-debug I/O.
    #[test]
    fn io_retention_follows_execution_mode() {
        for mode in [Mode::Run, Mode::Trace, Mode::Debug] {
            for syscall in [63, 64] {
                let mut machine =
                    Machine::for_testing().with_stdin(b"ab".to_vec());
                let address = machine.stack_start();
                machine.store(address, b"xy").unwrap();
                machine.set(10, if syscall == 63 { 0 } else { 1 });
                machine.set(11, address as i32);
                machine.set(12, 1);
                machine.set(17, syscall);
                let inst = instruction(&machine, Op::Ecall);
                let instructions = vec![
                    inst.clone(),
                    instruction_at(
                        inst.address + 4,
                        Op::Addi {
                            rd: 10,
                            rs1: ZERO,
                            imm: if syscall == 63 { 0 } else { 1 },
                        },
                    ),
                    instruction_at(inst.address + 8, Op::Ecall),
                ];
                let addresses = HashMap::from([
                    (inst.address, 0),
                    (inst.address + 4, 1),
                    (inst.address + 8, 2),
                ]);
                let mut config = Config::for_mode(mode.clone());
                config.max_steps = 3;
                config.strict = false;
                let effects =
                    trace(&mut machine, &instructions, &addresses, &config);
                if mode == Mode::Debug {
                    let data = if syscall == 63 {
                        &machine.io.stdin
                    } else {
                        &machine.io.stdout
                    };
                    assert_eq!(data, if syscall == 63 { b"ab" } else { b"xx" });
                    machine.reset();
                    for effect in &effects {
                        machine.apply(effect, true);
                    }
                    assert_eq!(machine.output_events.len(), 2);
                    for effect in effects.iter().rev() {
                        machine.apply(effect, false);
                    }
                    assert!(machine.output_events.is_empty());
                    assert_eq!(
                        machine.io.stdin.len() + machine.io.stdout.len(),
                        2
                    );
                } else {
                    assert!(machine.io.stdin.is_empty());
                    assert!(machine.io.stdout.is_empty());
                    assert!(machine.output_events.is_empty());
                }
            }
        }
    }

    // A short read records only consumed bytes; EOF remains a successful empty event.
    #[test]
    fn input_ranges_preserve_short_reads_and_eof() {
        let mut machine = Machine::for_testing().with_stdin(b"ab".to_vec());
        let address = machine.stack_start();
        machine.set(11, address as i32);
        machine.set(12, 4);
        machine.set(17, 63);
        let mut effects = Vec::new();
        for expected in [b"ab".as_slice(), b"".as_slice()] {
            machine.set(10, 0);
            let inst = instruction(&machine, Op::Ecall);
            let effect = machine.execute_and_collect_effects(&inst);
            assert!(!effect.is_terminal());
            assert_eq!(effect.stdin(&machine.io), Some(expected));
            effects.push(effect);
        }
        assert_eq!(machine.io.stdin, b"ab");
        machine.reset();
        for effect in &effects {
            machine.apply(effect, true);
        }
        assert_eq!(machine.output_events.len(), 2);
        for effect in effects.iter().rev() {
            machine.apply(effect, false);
        }
        assert!(machine.output_events.is_empty());
        assert_eq!(effects[0].stdin(&machine.io), Some(b"ab".as_slice()));
    }

    #[test]
    fn test_frame_changes_replay_in_both_directions() {
        let mut machine = Machine::for_testing();
        let inst =
            instruction(&machine, Op::Addi { rd: ZERO, rs1: ZERO, imm: 0 });
        let mut enter = Effects::new(&inst);
        enter.frame_change = Some(FrameChange::Enter(0x1000));
        let mut leave = Effects::new(&inst);
        leave.frame_change = Some(FrameChange::Leave(0x1000));

        // Calls push frames and returns pop them during forward replay.
        machine.apply(&enter, true);
        assert_eq!(machine.stack_frames(), &[0x1000]);
        machine.apply(&leave, true);
        assert!(machine.stack_frames().is_empty());

        // Backward replay reverses each operation with the same frame address.
        machine.apply(&leave, false);
        assert_eq!(machine.stack_frames(), &[0x1000]);
        machine.apply(&enter, false);
        assert!(machine.stack_frames().is_empty());
    }

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

    #[test]
    fn test_register_write_replays_old_and_new_values() {
        let mut machine = Machine::for_testing();
        machine.set(5, -17);
        let instruction = Rc::new(Instruction {
            address: machine.pc(),
            op: Op::Addi { rd: 5, rs1: 5, imm: 6 },
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        });

        // An in-place update records the original value before changing the register.
        let effects = machine.execute_and_collect_effects(&instruction);
        let write = effects.reg_write.as_ref().expect("register write");
        assert_eq!(write.register, 5);
        assert_eq!(write.old_value, -17);
        assert_eq!(write.new_value, -11);
        assert_eq!(effects.report(false, &machine.io), vec!["t0 <- -11"]);
        assert_eq!(effects.report(true, &machine.io), vec!["t0 <- 0xfffffff5"]);

        // Replaying in either direction restores the value and instruction position.
        machine.apply(&effects, false);
        assert_eq!(machine.get_reg(5), -17);
        assert_eq!(machine.pc(), instruction.address);
        machine.apply(&effects, true);
        assert_eq!(machine.get_reg(5), -11);
        assert_eq!(machine.pc(), instruction.address + instruction.length);
    }

    #[test]
    fn test_register_reads_are_scoped_to_instruction_execution() {
        let mut machine = Machine::for_testing();
        machine.set(5, 7);
        let instruction = Rc::new(Instruction {
            address: machine.pc(),
            op: Op::Add { rd: 6, rs1: 5, rs2: 5 },
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        });

        // Repeated reads retain one input register, even when it is read twice.
        let effects = machine.execute_and_collect_effects(&instruction);
        let reads = machine.register_reads();
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].register, 5);
        assert_eq!(machine.get_reg(6), 14);

        // Reads outside execution and replay do not append inputs or alter the record.
        machine.get(7);
        machine.apply(&effects, false);
        assert_eq!(machine.get_reg(6), 0);
        machine.apply(&effects, true);
        assert_eq!(machine.get_reg(6), 14);
        assert_eq!(machine.register_reads().len(), 1);

        // The next instruction replaces the inputs and excludes the zero register.
        let next = Rc::new(Instruction {
            address: machine.pc(),
            op: Op::Add { rd: 7, rs1: ZERO, rs2: 6 },
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        });
        machine.execute_and_collect_effects(&next);
        let reads = machine.register_reads();
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].register, 6);

        // Reset removes the last instruction's inputs along with the CPU state.
        machine.reset();
        assert!(machine.register_reads().is_empty());
    }

    #[test]
    fn test_zero_register_write_not_recorded_as_effect() {
        // Verify that writes to x0 (zero register) are not recorded in effects.
        // This prevents spurious "zero <- ..." messages in the debugger.
        let mut machine = Machine::for_testing();

        // Create an instruction that writes to x0: addi x0, x0, 5
        let op = Op::Addi { rd: ZERO, rs1: ZERO, imm: 5 };
        let instruction = Rc::new(Instruction {
            address: machine.pc(),
            op,
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        });

        // Execute the instruction
        let effects = machine.execute_and_collect_effects(&instruction);

        // Verify that no register write effect was recorded
        assert!(
            effects.reg_write.is_none(),
            "Writing to x0 should not record a register write effect"
        );
    }

    #[test]
    fn test_ret_instruction_no_zero_register_effect() {
        // Verify that the ret pseudo-instruction (jalr x0, ra, 0)
        // does not record a spurious zero register write effect.
        let mut machine = Machine::for_testing();

        // Initialize ra register with a return address
        machine.set(1, 0x1000);

        // Create a ret instruction: jalr x0, ra, 0
        let op = Op::Jalr { rd: ZERO, rs1: 1, offset: 0 };
        let instruction = Rc::new(Instruction {
            address: machine.pc(),
            op,
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        });

        let initial_pc = machine.pc();

        // Execute the instruction
        let effects = machine.execute_and_collect_effects(&instruction);

        // Verify that no register write effect was recorded
        assert!(
            effects.reg_write.is_none(),
            "ret instruction should not record a register write effect for x0"
        );

        // Verify that PC was updated correctly (should jump to return address)
        let (old_pc, new_pc) = effects.pc;
        assert_eq!(old_pc, initial_pc, "Old PC should match initial PC");
        assert_eq!(new_pc, 0x1000, "New PC should be the return address");
    }

    #[test]
    fn test_exit_syscall_effect_not_duplicated() {
        // Verify that exit syscalls don't show duplicate "exit(...)" messages
        // in the status line. The syscall message should be shown once, not
        // duplicated with the error message.
        let mut machine = Machine::for_testing();

        // Set up for exit syscall: a0 = 1 (exit status), a7 = 93 (exit syscall)
        machine.set(10, 1); // a0 = 1
        machine.set(17, 93); // a7 = 93 (exit syscall number)

        // Create an ecall instruction
        let op = Op::Ecall;
        let instruction = Rc::new(Instruction {
            address: machine.pc(),
            op,
            length: 4,
            encoding: 0,
            pseudo_index: 0,
            verbose_fields: Vec::new(),
            pseudo_fields: Vec::new(),
        });

        // Execute the instruction (this will trigger the exit syscall)
        let effects = machine.execute_and_collect_effects(&instruction);

        // Verify that a syscall was recorded
        assert!(
            effects.syscall().is_some(),
            "Exit syscall should be recorded in effects"
        );

        // Get the report (as displayed in the debugger status line)
        let report = effects.report(false, &machine.io);

        // Verify that the report contains exactly one "exit(1)" line
        let exit_messages: Vec<_> =
            report.iter().filter(|msg| msg.starts_with("exit(")).collect();

        assert_eq!(
            exit_messages.len(),
            1,
            "Should have exactly one exit message, got: {:?}",
            report
        );

        assert_eq!(
            exit_messages[0], "exit(1)",
            "Exit message should be 'exit(1)'"
        );
    }
}
