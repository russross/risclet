#[cfg(test)]
mod checkabi_tests {
    use crate::config::{Config, Mode, Relax};
    use crate::elf_loader::{ElfInput, load_elf};
    use crate::execution::trace;
    use crate::riscv::Op;
    use std::collections::HashMap;

    // ============================================================================
    // HELPER FUNCTIONS
    // ============================================================================

    /// Result of running a program with ABI checking
    #[derive(Debug)]
    enum AbiTestResult {
        Success,
        Violation(String),
        RuntimeError(String),
    }

    /// Create a minimal config for testing
    fn make_test_config(check_abi: bool) -> Config {
        Config {
            mode: Mode::Run,
            verbose: false,
            max_steps: 1_000_000,
            executable: "a.out".to_string(),
            check_abi,
            hex_mode: false,
            show_addresses: false,
            verbose_instructions: false,
            input_files: vec!["test.s".to_string()],
            output_file: "a.out".to_string(),
            text_start: 0x10000,
            dump: crate::dump::DumpConfig::new(),
            relax: Relax {
                gp: true,
                pseudo: true,
                compressed: false,
            },
        }
    }

    /// Assemble source code in-memory to ELF bytes
    fn assemble_source(source: &str) -> Result<Vec<u8>, String> {
        let config = make_test_config(false);
        let sources = vec![("test.s".to_string(), source.to_string())];
        crate::assembler::assemble(&config, sources)
            .map_err(|e| e.to_string())
    }

    /// Run assembled code with ABI checking and capture result
    fn run_with_abi_check(source: &str) -> AbiTestResult {
        // Assemble
        let elf_bytes = match assemble_source(source) {
            Ok(bytes) => bytes,
            Err(e) => return AbiTestResult::RuntimeError(format!("Assembly error: {}", e)),
        };

        // Load and prepare to run
        let mut config = make_test_config(true);
        config.mode = Mode::Run;

        // Load ELF from bytes
        let mut m = match load_elf(ElfInput::Bytes(&elf_bytes)) {
            Ok(machine) => machine,
            Err(e) => return AbiTestResult::RuntimeError(format!("Load error: {}", e)),
        };

        // Load all instructions
        let mut instructions = Vec::new();
        let mut pc = m.text_start();
        while pc < m.text_end() {
            match m.load_instruction(pc) {
                Ok((inst, length)) => {
                    let op = Op::new(inst);
                    let instruction = crate::execution::Instruction {
                        address: pc,
                        op,
                        length,
                        pseudo_index: 0,
                        verbose_fields: Vec::new(),
                        pseudo_fields: Vec::new(),
                    };
                    instructions.push(instruction);
                    pc += length;
                }
                Err(e) => {
                    return AbiTestResult::RuntimeError(format!("Load instruction error: {}", e))
                }
            }
        }

        // Build address map
        let mut addresses = HashMap::new();
        for (index, instruction) in instructions.iter().enumerate() {
            addresses.insert(instruction.address, index);
        }

        // Add local labels
        crate::execution::add_local_labels(&mut m, &instructions);

        // Set up pseudo-instructions
        let mut pseudo_addresses = HashMap::new();
        {
            let mut i = 0;
            let mut j = 0;
            while i < instructions.len() {
                let n = if let Some((n, fields)) =
                    crate::riscv::get_pseudo_sequence(&instructions[i..], &m.address_symbols)
                {
                    instructions[i].pseudo_fields = fields;
                    n
                } else {
                    instructions[i].pseudo_fields = instructions[i].op.to_pseudo_fields();
                    1
                };
                for inst in &mut instructions[i..i + n] {
                    inst.verbose_fields = inst.op.to_fields();
                    inst.pseudo_index = j;
                }
                pseudo_addresses.insert(j, i);
                i += n;
                j += 1;
            }
        }

        let instructions: Vec<std::rc::Rc<crate::execution::Instruction>> =
            instructions.into_iter().map(std::rc::Rc::new).collect();

        // Run with ABI checking
        let effects = trace(&mut m, &instructions, &addresses, &config);

        // Check for ABI violations in the effects
        for effect in effects {
            if let Some(err) = effect.other_message.as_ref() {
                // Only treat AbiViolation errors as ABI violations, not other errors
                if matches!(err, crate::error::RiscletError::AbiViolation(_)) {
                    return AbiTestResult::Violation(err.to_string());
                }
            }
        }

        AbiTestResult::Success
    }

    /// Assert program triggers ABI violation containing pattern
    fn check_abi_violation(source: &str, error_pattern: &str) {
        match run_with_abi_check(source) {
            AbiTestResult::Violation(msg) if msg.contains(error_pattern) => {}
            AbiTestResult::Violation(msg) => {
                panic!("Expected '{}', got: {}", error_pattern, msg)
            }
            AbiTestResult::Success => {
                panic!("Expected violation '{}', but program succeeded", error_pattern)
            }
            AbiTestResult::RuntimeError(msg) => {
                panic!("Expected violation, got error: {}", msg)
            }
        }
    }

    /// Assert program completes successfully
    fn check_abi_success(source: &str) {
        match run_with_abi_check(source) {
            AbiTestResult::Success => {}
            AbiTestResult::Violation(msg) => {
                panic!("Expected success, got ABI violation: {}", msg)
            }
            AbiTestResult::RuntimeError(msg) => {
                panic!("Expected success, got error: {}", msg)
            }
        }
    }

    // ============================================================================
    // ASSEMBLY TEMPLATE HELPERS
    // ============================================================================

    fn exit_code(code: i32) -> String {
        format!("    li a0, {}\n    li a7, 93\n    ecall", code)
    }

    fn bss_space(label: &str, size: usize) -> String {
        format!("\n.bss\n{}:\n    .space {}\n", label, size)
    }

    // ============================================================================
    // 1. REGISTER INITIALIZATION CHECKS
    // ============================================================================

    #[test]
    fn test_uninitialized_register_read() {
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    add a0, t0, zero
    li a7, 93
    ecall
"#,
            "Cannot use uninitialized",
        );
    }

    #[test]
    fn test_valid_register_use() {
        check_abi_success(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    add a0, t0, zero
    li a7, 93
    ecall
"#,
        );
    }

    #[test]
    fn test_zero_register_always_valid() {
        check_abi_success(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    add a0, x0, x0
    li a7, 93
    ecall
"#,
        );
    }

    #[test]
    fn test_sp_valid_at_start() {
        check_abi_success(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    addi t0, sp, 0
    li a7, 93
    ecall
"#,
        );
    }

    // ============================================================================
    // 2. SAVE-ONLY REGISTER CHECKS
    // ============================================================================

    #[test]
    fn test_save_only_store_allowed() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal foo
    {}

.global foo
foo:
    addi sp, sp, -16
    sw ra, 12(sp)
    sw s0, 0(sp)
    li s0, 42
    la t0, buffer
    sw s0, 0(t0)
    lw ra, 12(sp)
    lw s0, 0(sp)
    addi sp, sp, 16
    ret

.global foo_args
.equ foo_args, 0
{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_save_only_move_allowed() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    addi sp, sp, -16
    sw s0, 0(sp)
    li s0, 42
    mv t0, s0
    lw s0, 0(sp)
    addi sp, sp, 16
    ret

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
            ),
        );
    }

    #[test]
    fn test_save_only_arithmetic_forbidden() {
        // Student forgot to initialize s0 before using it in arithmetic
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    addi sp, sp, -16
    sw s0, 0(sp)
    # BUG: forgot to initialize s0 (e.g., mv s0, a0)
    add a0, s0, x0
    lw s0, 0(sp)
    addi sp, sp, 16
    ret

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
            ),
            "can only be stored",
        );
    }

    // ============================================================================
    // 3. STACK POINTER ALIGNMENT
    // ============================================================================

    #[test]
    fn test_sp_aligned_16_bytes() {
        check_abi_success(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    addi sp, sp, -16
    addi sp, sp, 16
    li a7, 93
    ecall
"#,
        );
    }

    #[test]
    fn test_sp_misaligned_4_bytes() {
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    addi sp, sp, -4
    li a7, 93
    ecall
"#,
            "Stack pointer must be 16-byte aligned",
        );
    }

    #[test]
    fn test_sp_misaligned_8_bytes() {
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    addi sp, sp, -8
    li a7, 93
    ecall
"#,
            "Stack pointer must be 16-byte aligned",
        );
    }

    #[test]
    fn test_sp_misaligned_1_byte() {
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    addi sp, sp, -1
    li a7, 93
    ecall
"#,
            "Stack pointer must be 16-byte aligned",
        );
    }

    // ============================================================================
    // 4. FUNCTION CALL CHECKS
    // ============================================================================

    #[test]
    fn test_valid_function_call_with_label() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    li a0, 42
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
        );
    }

    #[test]
    fn test_function_call_with_arg_count() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li a0, 10
    li a1, 20
    jal ra, add_fn
    {}

.global add_fn
add_fn:
    add a0, a0, a1
    {}

.global add_fn_args
.equ add_fn_args, 2
"#,
                exit_code(0),
                "    ret",
            ),
        );
    }

    #[test]
    fn test_function_call_zero_args() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    li a0, 99
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
        );
    }

    #[test]
    fn test_function_call_wrong_return_register() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal t0, foo
    {}

.global foo
foo:
    li a0, 42
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
            "Return address must be stored in ra",
        );
    }

    #[test]
    fn test_function_call_unlabeled_address() {
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 0x10000
    jalr ra, 0(t0)
    li a7, 93
    ecall
"#,
            "Cannot jump to unlabeled address",
        );
    }

    #[test]
    fn test_function_call_arg_count_callee_side() {
        // Test that callee-side arg count declaration works:
        // Function declares 2 args, so a0 and a1 should be valid, a2+ invalid
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li a0, 10
    li a1, 20
    jal ra, add_two
    {}

.global add_two
add_two:
    # a0 and a1 are valid because add_two_args = 2
    add a0, a0, a1
    {}

.global add_two_args
.equ add_two_args, 2
"#,
                exit_code(0),
                "    ret",
            ),
        );
    }

    #[test]
    fn test_function_call_arg_count_invalidates_unused() {
        // Test that arg count declaration invalidates registers beyond the count:
        // Function declares 1 arg, so a0 is valid but a1+ should be invalid
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li a0, 10
    li a1, 20
    li a2, 30
    jal ra, use_a2
    {}

.global use_a2
.global use_a2_args
use_a2:
    # This should fail: function declares 1 arg so only a0 is valid
    # Trying to use a2 should be an error
    add a0, a0, a2
    {}

.equ use_a2_args, 1
"#,
                exit_code(0),
                "    ret",
            ),
            "Cannot use uninitialized",
        );
    }

    // ============================================================================
    // 5. FUNCTION RETURN CHECKS
    // ============================================================================

    #[test]
    fn test_valid_function_return() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li a0, 5
    jal ra, foo
    {}

.global foo
foo:
    addi sp, sp, -16
    sw s0, 0(sp)
    mv s0, a0         # Properly initialize s0 from argument
    addi s0, s0, 10   # Use s0
    mv a0, s0         # Return result
    lw s0, 0(sp)
    addi sp, sp, 16
    ret

.global foo_args
.equ foo_args, 1
"#,
                exit_code(0),
            ),
        );
    }

    #[test]
    fn test_return_with_sp_restored() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    addi sp, sp, -16
    sw s0, 0(sp)
    li s0, 42         # Initialize s0 before using it
    mv a0, s0         # Use s0
    lw s0, 0(sp)
    addi sp, sp, 16
    ret

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
            ),
        );
    }

    #[test]
    fn test_return_ra_modified() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    li ra, 999
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
            "must be preserved",
        );
    }

    #[test]
    fn test_return_gp_modified() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    li gp, 999
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
            "must be preserved",
        );
    }

    #[test]
    fn test_return_s_register_modified() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    li s0, 42
    li s1, 99
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
            "must be preserved",
        );
    }

    #[test]
    fn test_return_sp_value_wrong() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, foo
    {}

.global foo
foo:
    addi sp, sp, -16
    {}

.global foo_args
.equ foo_args, 0
"#,
                exit_code(0),
                "    ret",
            ),
            "Stack pointer must be restored",
        );
    }

    #[test]
    fn test_return_without_call() {
        // Attempting to return without a function call uses uninitialized ra
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jr ra
"#,
            "Cannot use uninitialized ra",
        );
    }

    // ============================================================================
    // 6. STORE ALIGNMENT CHECKS
    // ============================================================================

    #[test]
    fn test_sb_any_alignment() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sb t0, 0(t1)
    sb t0, 1(t1)
    sb t0, 2(t1)
    sb t0, 3(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_sh_aligned_2_bytes() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sh t0, 0(t1)
    sh t0, 2(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_sw_aligned_4_bytes() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sw t0, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_sh_misaligned() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sh t0, 1(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Unaligned 2-byte memory write",
        );
    }

    #[test]
    fn test_sw_misaligned_1() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    addi t1, t1, 1
    sw t0, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 8)
            ),
            "Unaligned 4-byte memory write",
        );
    }

    #[test]
    fn test_sw_misaligned_2() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    addi t1, t1, 2
    sw t0, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 8)
            ),
            "Unaligned 4-byte memory write",
        );
    }

    #[test]
    fn test_sw_misaligned_3() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    addi t1, t1, 3
    sw t0, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 8)
            ),
            "Unaligned 4-byte memory write",
        );
    }

    // ============================================================================
    // 7. LOAD ALIGNMENT CHECKS
    // ============================================================================

    #[test]
    fn test_lb_lbu_any_alignment() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 1
    sb t5, 0(t0)
    li t5, 2
    sb t5, 1(t0)
    li t5, 3
    sb t5, 2(t0)
    li t5, 4
    sb t5, 3(t0)
    lb t1, 0(t0)
    lbu t2, 1(t0)
    lb t3, 2(t0)
    lbu t4, 3(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_lh_lhu_aligned_2_bytes() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 0x1234
    sh t5, 0(t0)
    li t5, 0x5678
    sh t5, 2(t0)
    lh t1, 0(t0)
    lhu t2, 2(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_lw_aligned_4_bytes() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 0x04030201
    sw t5, 0(t0)
    lw t1, 0(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_lh_lhu_misaligned() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 0x04030201
    sw t5, 0(t0)
    lh t1, 1(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Unaligned 2-byte memory read",
        );
    }

    #[test]
    fn test_lw_misaligned_1() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 0x04030201
    sw t5, 0(t0)
    li t5, 0x08070605
    sw t5, 4(t0)
    addi t0, t0, 1
    lw t1, 0(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 8)
            ),
            "Unaligned 4-byte memory read",
        );
    }

    #[test]
    fn test_lw_misaligned_2() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 0x04030201
    sw t5, 0(t0)
    li t5, 0x08070605
    sw t5, 4(t0)
    addi t0, t0, 2
    lw t1, 0(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 8)
            ),
            "Unaligned 4-byte memory read",
        );
    }

    #[test]
    fn test_lw_misaligned_3() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    li t5, 0x04030201
    sw t5, 0(t0)
    li t5, 0x08070605
    sw t5, 4(t0)
    addi t0, t0, 3
    lw t1, 0(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 8)
            ),
            "Unaligned 4-byte memory read",
        );
    }

    // ============================================================================
    // 8. MEMORY READ/WRITE TRACKING
    // ============================================================================

    #[test]
    fn test_store_then_load_same_size() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 0x12345678
    la t1, buffer
    sw t0, 0(t1)
    lw t2, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_load_unwritten_memory() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la t0, buffer
    lw t1, 0(t0)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_separate_byte_stores_and_loads() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 10
    li t1, 20
    la t2, buffer
    sb t0, 0(t2)
    sb t1, 1(t2)
    lb t3, 0(t2)
    lb t4, 1(t2)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
        );
    }

    #[test]
    fn test_load_after_partial_write() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sh t0, 0(t1)
    lw t2, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Read size mismatches original write size",
        );
    }

    #[test]
    fn test_load_size_mismatch() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sw t0, 0(t1)
    lh t2, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Read size mismatches original write size",
        );
    }

    #[test]
    fn test_load_spanning_writes() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 10
    li t1, 20
    la t2, buffer
    sh t0, 0(t2)
    sh t1, 2(t2)
    lh t3, 0(t2)
    lh t4, 2(t2)
    lw t5, 0(t2)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Read size mismatches original write size",
        );
    }

    #[test]
    fn test_incomplete_write_before_read() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    la t1, buffer
    sb t0, 0(t1)
    lw t2, 0(t1)
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Read size mismatches original write size",
        );
    }

    // ============================================================================
    // 9. SYSCALL CHECKS
    // ============================================================================

    #[test]
    fn test_syscall_write_byte_data() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    la a0, msg
    li t0, 104
    sb t0, 0(a0)
    li t0, 101
    sb t0, 1(a0)
    li t0, 108
    sb t0, 2(a0)
    li t0, 108
    sb t0, 3(a0)
    li t0, 111
    sb t0, 4(a0)
    li a1, 5
    li a7, 64
    ecall
    {}

{}
"#,
                exit_code(0),
                bss_space("msg", 5)
            ),
        );
    }

    #[test]
    fn test_syscall_write_word_data() {
        check_abi_violation(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 0x12345678
    la t1, buffer
    sw t0, 0(t1)
    li a0, 1
    mv a1, t1
    li a2, 4
    li a7, 64
    ecall
    {}

{}
"#,
                exit_code(0),
                bss_space("buffer", 4)
            ),
            "Syscall write requires byte-level data",
        );
    }

    // ============================================================================
    // 10. VALUE NUMBER TRACKING
    // ============================================================================

    #[test]
    fn test_mv_preserves_value_number() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 42
    mv t1, t0
    add a0, t1, x0
    {}
"#,
                exit_code(0)
            ),
        );
    }

    #[test]
    fn test_arithmetic_creates_new_value() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li t0, 10
    li t1, 20
    add t2, t0, t1
    add a0, t2, x0
    {}
"#,
                exit_code(0)
            ),
        );
    }

    // ============================================================================
    // 11. NESTED FUNCTION CALLS
    // ============================================================================

    #[test]
    fn test_nested_function_calls() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jal ra, outer
    {}

.global outer
outer:
    addi sp, sp, -16
    sw ra, 0(sp)
    jal ra, inner
    lw ra, 0(sp)
    addi sp, sp, 16
    {}

.global outer_args
.equ outer_args, 0

.global inner
inner:
    li a0, 42
    {}

.global inner_args
.equ inner_args, 0
"#,
                exit_code(0),
                "    ret",
                "    ret",
            ),
        );
    }

    #[test]
    fn test_recursive_function() {
        check_abi_success(
            &format!(
                r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    li a0, 3
    jal ra, countdown
    {}

.global countdown
countdown:
    addi sp, sp, -16
    sw ra, 0(sp)
    li t0, 0
    beq a0, t0, 1f
    addi a0, a0, -1
    jal ra, countdown
1:
    lw ra, 0(sp)
    addi sp, sp, 16
    ret

.global countdown_args
.equ countdown_args, 1
"#,
                exit_code(0),
            ),
        );
    }

    #[test]
    fn test_stack_underflow_multiple_returns() {
        // Attempting to return without a function call uses uninitialized ra
        check_abi_violation(
            r#"
.global _start
.text
_start:
    la gp, __global_pointer$
    jr ra
"#,
            "Cannot use uninitialized ra",
        );
    }
}
