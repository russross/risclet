use crate::assembler::AssemblyOutput;
#[cfg(test)]
use crate::config::{Config, Mode, Relax};
use crate::elf_loader::{ElfInput, load_elf};
use crate::error::RiscletError;
use crate::execution::trace;
use crate::riscv::Op;
use std::collections::HashMap;
use std::rc::Rc;

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
fn make_test_config(strict: bool) -> Config {
    Config {
        mode: Mode::Run,
        verbose: false,
        max_steps: 1_000_000,
        executable: "a.out".to_string(),
        strict,
        hex_mode: false,
        show_addresses: false,
        show_encoding: false,
        verbose_instructions: false,
        input_files: vec!["test.s".to_string()],
        output_file: "a.out".to_string(),
        text_start: 0x10000,
        dump: crate::dump::DumpConfig::new(),
        relax: Relax { gp: Some(true), pseudo: true, compressed: false },
    }
}

/// Assemble source code in-memory to ELF bytes
fn assemble_source(source: &str, compressed: bool) -> Result<Vec<u8>, String> {
    let mut config = make_test_config(false);
    config.relax.compressed = compressed;
    let sources = vec![("test.s".to_string(), source.to_string())];
    match crate::assembler::assemble(&mut config, sources) {
        Ok(AssemblyOutput::Elf(bytes)) => Ok(bytes),
        Ok(AssemblyOutput::Dumped) => {
            panic!("unexpected dump in execution test")
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Run assembled code with ABI checking and capture result
fn run_with_abi_check(source: &str) -> AbiTestResult {
    run_with_abi_options(source, &[], false)
}

// Input and instruction compression exercise the same checker through real execution.
fn run_with_abi_options(
    source: &str,
    input: &[u8],
    compressed: bool,
) -> AbiTestResult {
    // Assemble
    let elf_bytes = match assemble_source(source, compressed) {
        Ok(bytes) => bytes,
        Err(e) => {
            return AbiTestResult::RuntimeError(format!(
                "Assembly error: {}",
                e
            ));
        }
    };

    // Load and prepare to run
    let mut config = make_test_config(true);
    config.mode = Mode::Run;

    // Load ELF from bytes
    let mut m = match load_elf(ElfInput::Bytes(&elf_bytes)) {
        Ok(machine) => machine,
        Err(e) => {
            return AbiTestResult::RuntimeError(format!("Load error: {}", e));
        }
    };
    m = m.with_stdin(input.to_vec());

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
                    encoding: inst as u32,
                    pseudo_index: 0,
                    verbose_fields: Vec::new(),
                    pseudo_fields: Vec::new(),
                };
                instructions.push(instruction);
                pc += length;
            }
            Err(e) => {
                return AbiTestResult::RuntimeError(format!(
                    "Load instruction error: {}",
                    e
                ));
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
            let n = if let Some((n, fields)) = crate::riscv::get_pseudo_sequence(
                &instructions[i..],
                &m.address_symbols,
            ) {
                instructions[i].pseudo_fields = fields;
                n
            } else {
                instructions[i].pseudo_fields =
                    instructions[i].op.to_pseudo_fields();
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

    let instructions: Vec<Rc<crate::execution::Instruction>> =
        instructions.into_iter().map(Rc::new).collect();

    // Run with ABI checking
    let effects = trace(&mut m, &instructions, &addresses, &config);

    // A successful test must reach an ordinary exit rather than another execution error.
    for effect in effects {
        if let Some(err) = effect.other_message() {
            match err {
                RiscletError::AbiViolation(_) => {
                    return AbiTestResult::Violation(err.to_string());
                }
                RiscletError::Exit(_) => return AbiTestResult::Success,
                _ => return AbiTestResult::RuntimeError(err.to_string()),
            }
        }
    }

    AbiTestResult::RuntimeError("Program did not exit".to_string())
}

/// Assert program triggers ABI violation containing pattern
fn check_abi_violation(source: &str, error_pattern: &str) {
    match run_with_abi_check(source) {
        AbiTestResult::Violation(msg) if msg.contains(error_pattern) => {}
        AbiTestResult::Violation(msg) => {
            panic!("Expected '{}', got: {}", error_pattern, msg)
        }
        AbiTestResult::Success => {
            panic!(
                "Expected violation '{}', but program succeeded",
                error_pattern
            )
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_save_only_move_allowed() {
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_function_call_with_arg_count() {
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_function_call_zero_args() {
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_return_with_sp_restored() {
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_sh_aligned_2_bytes() {
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_sw_aligned_4_bytes() {
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_lh_lhu_aligned_2_bytes() {
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_lw_aligned_4_bytes() {
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_load_unwritten_memory() {
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_separate_byte_stores_and_loads() {
    check_abi_success(&format!(
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
    ));
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
    check_abi_success(&format!(
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
    mv a1, a0
    li a0, 1
    li a2, 5
    li a7, 64
    ecall
    {}

{}
"#,
        exit_code(0),
        bss_space("msg", 5)
    ));
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
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_arithmetic_creates_new_value() {
    check_abi_success(&format!(
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
    ));
}

// ============================================================================
// 11. NESTED FUNCTION CALLS
// ============================================================================

#[test]
fn test_nested_function_calls() {
    check_abi_success(&format!(
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
    ));
}

#[test]
fn test_recursive_function() {
    check_abi_success(&format!(
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
    ));
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

// Small programs keep the instruction under test visible while exercising real calls.
fn function_program(start: &str, body: &str, args: Option<u32>) -> String {
    let metadata = args.map_or(String::new(), |count| {
        format!(".global foo_args\n.equ foo_args, {count}\n")
    });
    format!(
        ".global _start\n.text\n_start:\nla gp, __global_pointer$\n{start}\njal ra, foo\n{}\nfoo:\n{body}\nret\n{metadata}\n{}",
        exit_code(0),
        bss_space("buffer", 16),
    )
}

#[test]
fn test_save_only_move_and_restore() {
    check_abi_success(&function_program(
        "",
        "mv t0, s0\nli s0, 42\nmv s0, t0",
        Some(0),
    ));
}

#[test]
fn test_save_only_move_does_not_initialize_value() {
    check_abi_violation(
        &function_program("", "mv t0, s0\naddi t1, t0, 1", Some(0)),
        "t0 can only be stored",
    );
}

#[test]
fn test_save_only_self_move_remains_save_only() {
    check_abi_violation(
        &function_program("", "mv s0, s0\naddi t0, s0, 1", Some(0)),
        "s0 can only be stored",
    );
}

#[test]
fn test_save_only_word_reload_remains_save_only() {
    check_abi_violation(
        &function_program(
            "",
            "addi sp, sp, -16\nsw s0, 0(sp)\nlw t0, 0(sp)\naddi t1, t0, 1",
            Some(0),
        ),
        "t0 can only be stored",
    );
}

#[test]
fn test_save_only_move_store_reload_restore() {
    check_abi_success(&function_program(
        "",
        "addi sp, sp, -16\nmv t0, s0\nsw t0, 0(sp)\nli s0, 42\nlw s0, 0(sp)\naddi sp, sp, 16",
        Some(0),
    ));
}

#[test]
fn test_save_only_cannot_be_store_address() {
    for store in ["sw t0, 0(s0)", "sw s0, 0(s0)"] {
        check_abi_violation(
            &function_program(
                "la s0, buffer",
                &format!("li t0, 1\n{store}"),
                Some(0),
            ),
            "s0 can only be stored",
        );
    }
}

#[test]
fn test_save_only_cannot_be_partially_saved() {
    for store in ["sb s0, 0(sp)", "sh s0, 0(sp)"] {
        check_abi_violation(
            &function_program(
                "",
                &format!("addi sp, sp, -16\n{store}"),
                Some(0),
            ),
            "s0 can only be stored",
        );
    }
}

#[test]
fn test_save_only_cannot_be_passed_as_argument() {
    for metadata in ["", ".global inner_args\n.equ inner_args, 1"] {
        check_abi_violation(
            &function_program(
                "",
                &format!("mv a0, s0\njal ra, inner\ninner:\nret\n{metadata}"),
                Some(0),
            ),
            "Function argument a0 is save-only",
        );
    }
}

#[test]
fn test_return_result_valid_when_caller_a0_uninitialized() {
    check_abi_success(&format!(
        ".global _start\n.text\n_start:\njal foo\naddi t0, a0, 1\n{}\nfoo:\nli a0, 42\nret\n.global foo_args\n.equ foo_args, 0",
        exit_code(0),
    ));
}

#[test]
fn test_uninitialized_result_does_not_inherit_caller_validity() {
    check_abi_violation(
        ".global _start\n.text\n_start:\nli a0, 42\njal foo\naddi t0, a0, 1\nfoo:\nret\n.global foo_args\n.equ foo_args, 0",
        "Cannot use uninitialized a0",
    );
}

#[test]
fn test_save_only_result_remains_save_only() {
    check_abi_violation(
        ".global _start\n.text\n_start:\njal foo\naddi t0, a0, 1\nfoo:\nmv a0, s0\nret\n.global foo_args\n.equ foo_args, 0",
        "a0 can only be stored",
    );
}

#[test]
fn test_saved_register_caller_availability_is_restored() {
    check_abi_success(&format!(
        ".global _start\n.text\n_start:\nli s0, 42\njal foo\naddi t0, s0, 1\n{}\nfoo:\nret",
        exit_code(0),
    ));
    check_abi_violation(
        ".global _start\n.text\n_start:\njal foo\naddi t0, s0, 1\nfoo:\nret",
        "Cannot use uninitialized s0",
    );
}

#[test]
fn test_nested_call_preserves_outer_save_only_state() {
    check_abi_violation(
        &function_program(
            "",
            "addi sp, sp, -16\nsw ra, 0(sp)\njal inner\nlw ra, 0(sp)\naddi t0, s0, 1\ninner:\nret",
            Some(0),
        ),
        "s0 can only be stored",
    );
}

#[test]
fn test_nested_call_preserves_outer_initialized_saved_register() {
    check_abi_success(&function_program(
        "",
        "addi sp, sp, -16\nsw ra, 0(sp)\nsw s0, 4(sp)\nli s0, 42\njal inner\naddi t0, s0, 1\nlw s0, 4(sp)\nlw ra, 0(sp)\naddi sp, sp, 16\nret\ninner:\nret",
        Some(0),
    ));
}

#[test]
fn test_caller_saved_registers_unavailable_after_return() {
    for register in ["t0", "t6", "a1", "a7"] {
        check_abi_violation(
            &format!(
                ".global _start\n.text\n_start:\nli {register}, 1\njal foo\naddi s0, {register}, 1\nfoo:\nli {register}, 2\nret",
            ),
            &format!("Cannot use uninitialized {register}"),
        );
    }
}

#[test]
fn test_unknown_signature_does_not_revive_uninitialized_arguments() {
    check_abi_violation(
        &function_program("", "addi t0, a0, 1", None),
        "Cannot use uninitialized a0",
    );
}

#[test]
fn test_eight_arguments_supported() {
    check_abi_success(&function_program(
        "li a0, 0\nli a1, 1\nli a2, 2\nli a3, 3\nli a4, 4\nli a5, 5\nli a6, 6\nli a7, 7",
        "add t0, a0, a7",
        Some(8),
    ));
}

#[test]
fn test_eighth_argument_must_be_initialized() {
    check_abi_violation(
        &function_program(
            "li a0, 0\nli a1, 1\nli a2, 2\nli a3, 3\nli a4, 4\nli a5, 5\nli a6, 6",
            "",
            Some(8),
        ),
        "Function argument a7 is uninitialized",
    );
}

#[test]
fn test_invalid_argument_counts_are_diagnostics() {
    for count in [9, u32::MAX] {
        check_abi_violation(
            &function_program("", "", Some(count)),
            "Invalid argument count",
        );
    }
}

#[test]
fn test_loading_zero_does_not_change_its_identity() {
    check_abi_success(&function_program(
        "mv s0, zero",
        "addi sp, sp, -16\nli t0, 99\nsw t0, 0(sp)\nlw zero, 0(sp)\nmv s0, zero\naddi sp, sp, 16",
        Some(0),
    ));
}

#[test]
fn test_loads_to_zero_still_validate_memory() {
    check_abi_violation(
        &function_program("", "addi sp, sp, -16\nlw zero, 0(sp)", Some(0)),
        "uninitialized stack",
    );
}

#[test]
fn test_stack_access_below_sp_rejected_with_aliases() {
    for access in [
        "sw zero, -4(sp)",
        "lw t0, -4(sp)",
        "mv t0, sp\nsw zero, -4(t0)",
        "mv t0, sp\nlw t1, -4(t0)",
    ] {
        check_abi_violation(
            &function_program("", access, Some(0)),
            "stack below sp",
        );
    }
}

#[test]
fn test_uninitialized_stack_reads_rejected_for_each_width() {
    for load in ["lb", "lbu", "lh", "lhu", "lw"] {
        check_abi_violation(
            &function_program(
                "",
                &format!("addi sp, sp, -16\n{load} t0, 0(sp)"),
                Some(0),
            ),
            "uninitialized stack",
        );
    }
}

#[test]
fn test_initialized_stack_reads_allowed_for_each_width() {
    for (store, load) in
        [("sb", "lb"), ("sb", "lbu"), ("sh", "lh"), ("sh", "lhu"), ("sw", "lw")]
    {
        check_abi_success(&function_program(
            "",
            &format!(
                "addi sp, sp, -16\nli t0, 42\n{store} t0, 0(sp)\n{load} t1, 0(sp)\naddi sp, sp, 16"
            ),
            Some(0),
        ));
    }
}

#[test]
fn test_partial_stack_initialization_does_not_validate_word() {
    check_abi_violation(
        &function_program(
            "",
            "addi sp, sp, -16\nsb zero, 0(sp)\nlw t0, 0(sp)",
            Some(0),
        ),
        "uninitialized stack",
    );
}

#[test]
fn test_released_stack_access_rejected() {
    check_abi_violation(
        &function_program(
            "",
            "addi sp, sp, -16\nsw zero, 0(sp)\nmv t0, sp\naddi sp, sp, 16\nlw t1, 0(t0)",
            Some(0),
        ),
        "stack below sp",
    );
}

#[test]
fn test_reallocated_stack_requires_new_store() {
    check_abi_violation(
        &function_program(
            "",
            "addi sp, sp, -16\nsw zero, 0(sp)\naddi sp, sp, 16\naddi sp, sp, -16\nlw t0, 0(sp)",
            Some(0),
        ),
        "uninitialized stack",
    );
}

#[test]
fn test_partial_stack_release_retains_live_slots() {
    check_abi_success(&function_program(
        "",
        "addi sp, sp, -32\nsw zero, 16(sp)\naddi sp, sp, 16\nlw t0, 0(sp)\naddi sp, sp, 16",
        Some(0),
    ));
    check_abi_violation(
        &function_program(
            "",
            "addi sp, sp, -32\nsw zero, 0(sp)\nsw zero, 16(sp)\naddi sp, sp, 16\naddi sp, sp, -16\nlw t0, 0(sp)",
            Some(0),
        ),
        "uninitialized stack",
    );
}

#[test]
fn test_stack_slot_not_reused_across_calls() {
    check_abi_violation(
        &format!(
            ".global _start\n.text\n_start:\njal writer\njal reader\n{}\nwriter:\naddi sp, sp, -16\nsw zero, 0(sp)\naddi sp, sp, 16\nret\nreader:\naddi sp, sp, -16\nlw t0, 0(sp)\nret",
            exit_code(0)
        ),
        "uninitialized stack",
    );
}

#[test]
fn test_callee_can_read_initialized_caller_stack_arguments() {
    check_abi_success(&function_program(
        "addi sp, sp, -16\nsw zero, 0(sp)",
        "lw t0, 0(sp)",
        Some(0),
    ));
}

#[test]
fn test_load_sp_uses_old_frame_then_releases_it() {
    let setup = "mv t0, sp\naddi sp, sp, -16\nsw t0, 0(sp)\nlw sp, 0(sp)";
    check_abi_success(&function_program("", setup, Some(0)));
    check_abi_violation(
        &function_program(
            "",
            &format!("{setup}\naddi sp, sp, -16\nlw t1, 0(sp)"),
            Some(0),
        ),
        "uninitialized stack",
    );
}

#[test]
fn test_fixed_register_modifications_rejected_immediately() {
    for register in ["gp", "tp"] {
        check_abi_violation(
            &function_program(
                "",
                &format!("li {register}, 99\nli t0, 1"),
                Some(0),
            ),
            &format!(
                "{register} must be preserved across function call; cannot modify"
            ),
        );
    }
}

#[test]
fn test_fixed_register_startup_initialization_allowed() {
    check_abi_success(&function_program("li tp, 0", "", Some(0)));
}

#[test]
fn test_compressed_save_only_and_stack_paths() {
    let source = function_program(
        "",
        "addi sp, sp, -16\nmv t0, s0\nsw t0, 0(sp)\nli s0, 1\nlw s0, 0(sp)\naddi sp, sp, 16",
        Some(0),
    );
    assert!(matches!(
        run_with_abi_options(&source, &[], true),
        AbiTestResult::Success
    ));
    let source =
        function_program("", "addi sp, sp, -16\nlw t0, 0(sp)", Some(0));
    assert!(
        matches!(run_with_abi_options(&source, &[], true), AbiTestResult::Violation(message) if message.contains("uninitialized stack"))
    );
}

#[test]
fn test_syscall_stack_write_requires_initialized_bytes() {
    for setup in ["", "sb zero, 0(sp)"] {
        check_abi_violation(
            &function_program(
                "",
                &format!(
                    "addi sp, sp, -16\n{setup}\nli a0, 1\nmv a1, sp\nli a2, 2\nli a7, 64\necall"
                ),
                Some(0),
            ),
            "uninitialized stack",
        );
    }
}

#[test]
fn test_syscall_stack_buffer_below_sp_rejected() {
    check_abi_violation(
        &function_program(
            "",
            "li a0, 1\naddi a1, sp, -4\nli a2, 1\nli a7, 64\necall",
            Some(0),
        ),
        "stack below sp",
    );
}

#[test]
fn test_syscall_input_initializes_only_received_bytes() {
    let setup =
        "addi sp, sp, -16\nli a0, 0\nmv a1, sp\nli a2, 4\nli a7, 63\necall";
    let source = function_program(
        "",
        &format!("{setup}\nlbu t0, 0(sp)\naddi sp, sp, 16"),
        Some(0),
    );
    assert!(matches!(
        run_with_abi_options(&source, b"x", false),
        AbiTestResult::Success
    ));
    let source =
        function_program("", &format!("{setup}\nlbu t0, 1(sp)"), Some(0));
    assert!(
        matches!(run_with_abi_options(&source, b"x", false), AbiTestResult::Violation(message) if message.contains("uninitialized stack"))
    );
}

#[test]
fn test_syscall_input_below_sp_rejected() {
    let source = function_program(
        "",
        "li a0, 0\naddi a1, sp, -4\nli a2, 1\nli a7, 63\necall",
        Some(0),
    );
    assert!(
        matches!(run_with_abi_options(&source, b"x", false), AbiTestResult::Violation(message) if message.contains("stack below sp"))
    );
}

#[test]
fn test_syscall_input_cannot_overwrite_saved_word() {
    let source = function_program(
        "",
        "addi sp, sp, -16\nsw s0, 0(sp)\nli a0, 0\nmv a1, sp\nli a2, 1\nli a7, 63\necall",
        Some(0),
    );
    assert!(
        matches!(run_with_abi_options(&source, b"x", false), AbiTestResult::Violation(message) if message.contains("overwrite non-byte data"))
    );
}

#[test]
fn test_test_helper_reports_execution_errors() {
    let source = ".global _start\n.text\n_start:\nli a7, 999\necall";
    assert!(
        matches!(run_with_abi_check(source), AbiTestResult::RuntimeError(message) if message.contains("unsupported syscall"))
    );
    let source = ".global _start\n.text\n_start:\nli t0, 0\nlw t1, 0(t0)";
    assert!(matches!(
        run_with_abi_check(source),
        AbiTestResult::RuntimeError(_)
    ));
    let source = ".global _start\n.text\n_start:\nnop";
    assert!(matches!(
        run_with_abi_check(source),
        AbiTestResult::RuntimeError(_)
    ));
}
