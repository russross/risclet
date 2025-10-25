# RISC-V Test Suite Integration Plan

## Overview

This document describes the process for converting the RISC-V ISA test suite from riscv-tests into embedded unit tests for the risclet emulator. Each test is converted into a standalone binary that exits with code 0 for success and code 1 for failure, then the binary is embedded as a byte array in a Rust unit test.

## Background

The riscv-tests repository contains comprehensive tests for RISC-V ISA instructions. We are converting tests from three specific suites:

- `riscv-tests/isa/rv32ui` - RV32I base integer instruction set (43 tests)
- `riscv-tests/isa/rv32um` - RV32M multiply/divide extension (8 tests)  
- `riscv-tests/isa/rv32uc` - RV32C compressed instruction extension (2 tests)

Total: **53 tests** to convert

## Test Structure Analysis

### Original Test Format

Tests use macros from `riscv-tests/isa/macros/scalar/test_macros.h`:
- `TEST_IMM_OP` - Tests immediate operand instructions
- `TEST_RR_OP` - Tests register-register operations
- `TEST_LD_OP` / `TEST_ST_OP` - Tests load/store operations
- `TEST_BR2_OP_TAKEN` / `TEST_BR2_OP_NOTTAKEN` - Tests branch instructions
- `TEST_PASSFAIL` - Defines pass/fail logic

Tests track the current test number in register x30 (TESTNUM) and branch to `fail:` on mismatch.

## Complete Conversion Process

### Step 1: Macro Expansion and Standalone Assembly Creation (Automated)



This step is automated by the `generate_standalone.py` script.



For each test file (e.g., `addi.S`):



1.  **Run `generate_standalone.py`**:

    ```bash

    python3 generate_standalone.py <test_name> <suite> [march]

    ```

    - `<test_name>`: The name of the test file (e.g., `addi`).

    - `<suite>`: The RISC-V ISA suite (e.g., `rv32ui`, `rv32um`, `rv32uc`).

    - `[march]`: Optional. The RISC-V architecture string (e.g., `rv32im`, `rv32imc`). Defaults to `rv32im`.



    This script will:

    - Locate the appropriate test file within `riscv-tests/isa/`.

    - Expand all `TEST_IMM_OP`, `TEST_RR_OP`, and related macros into raw RISC-V assembly.

    - Add the `_start` entry point and `pass`/`fail` exit handlers.

    - Save the resulting standalone assembly to `/home/russ/risclet/<test_name>_standalone.S`.



    **Example**:

    ```bash

    python3 generate_standalone.py addi rv32ui

    ```

    This will create `/home/russ/risclet/addi_standalone.S`.

### Step 2: Assembly and Linking

Use the RISC-V GNU toolchain to assemble and link the generated standalone assembly file.

**Example for `addi` (rv32ui suite)**:

```bash
riscv64-unknown-elf-as -march=rv32im -mabi=ilp32 -mno-relax addi_standalone.S -o addi_standalone.o
riscv64-unknown-elf-ld -m elf32lriscv -Ttext=0x80000000 --no-relax addi_standalone.o -o addi_standalone.elf
```

**General Commands**:

```bash
# For rv32ui and rv32um tests (use rv32im, NOT rv32imc):
riscv64-unknown-elf-as -march=rv32im -mabi=ilp32 -mno-relax <testname>_standalone.S -o <testname>_standalone.o

# For rv32uc tests (compressed instructions):
riscv64-unknown-elf-as -march=rv32imc -mabi=ilp32 -mno-relax <testname>_standalone.S -o <testname>_standalone.o

# Link (same for all):
riscv64-unknown-elf-ld -m elf32lriscv -Ttext=0x80000000 --no-relax <testname>_standalone.o -o <testname>_standalone.elf
```

**Critical flags**:
- `-march=rv32im` - Prevents use of compressed instructions in rv32ui/rv32um suites
- `-march=rv32imc` - Allows compressed instructions for rv32uc suite
- `-mno-relax` - Prevents assembler from optimizing/relaxing instructions
- `--no-relax` - Prevents linker relaxation (e.g., gp-relative addressing)
- `-Ttext=0x80000000` - Sets code to start at address 0x80000000 (BASE_ADDR)

### Step 3: Extract Binary

Extract the raw binary from the ELF file and convert it to a hex format suitable for embedding in Rust.

**Example for `addi`**:

```bash
riscv64-unknown-elf-objcopy -O binary addi_standalone.elf addi_standalone.bin
hexdump -v -e '4/1 "0x%02x, "' -e '"\n"' addi_standalone.bin > addi_hex.txt
```

**General Commands**:

```bash
# Extract binary
riscv64-unknown-elf-objcopy -O binary <testname>_standalone.elf <testname>_standalone.bin

# Convert to hex format (4 bytes per line for easy formatting)
hexdump -v -e '4/1 "0x%02x, "' -e '"\n"' <testname>_standalone.bin > <testname>_hex.txt
```

Optional: Disassemble to verify correctness:
```bash
riscv64-unknown-elf-objdump -d -M numeric <testname>_standalone.elf > <testname>_disasm.txt
```

### Step 4: Add to Rust Test File



Add the generated binary to `/home/russ/risclet/src/riscv_tests.rs`.



**For `addi` test**: This test is already integrated as a reference example.



**General Steps**:



1.  **Read the hex bytes** from `<testname>_hex.txt`.



2.  **Add a constant for the binary** (below existing constants, before the test declarations):

    ```rust

    #[rustfmt::skip]

    const <TESTNAME_UPPER>_BINARY: &[u8] = &[

        // Paste hex bytes from <testname>_hex.txt, ensuring proper Rust array formatting

        0x13, 0x0f, 0x00, 0x00, 0x13, 0x0f, 0x20, 0x00, // ...

        // ... all bytes ...

    ];

    ```



3.  **Add the test using the macro**:

    ```rust

    riscv_test!(test_rv32ui_<testname>, <TESTNAME_UPPER>_BINARY);

    ```

### Step 5: Run and Verify

Run the Rust unit test to verify the converted test.

**Example for `addi`**:

```bash
cargo test test_rv32ui_addi
```

The test should compile and pass. If it fails:
- Check the exit code in the error message
- Review the disassembly to identify which test case failed (look at x30 value)
- Verify macro expansion was correct
- Check that the appropriate -march was used

## Helper Infrastructure

The test file `src/riscv_tests.rs` provides:

### Constants
```rust
const BASE_ADDR: u32 = 0x80000000;  // Entry point address
const MAX_STEPS: usize = 100000;     // Maximum execution steps
```

### Helper Function
```rust
fn run_test_binary(binary: &[u8]) -> Result<i32, String>
```
- Creates a Machine with the binary loaded at BASE_ADDR
- Executes instructions until ecall is reached
- Returns the exit code (register a0)

### Macro
```rust
riscv_test!($test_name:ident, $binary:expr)
```
- Generates a `#[test]` function
- Calls `run_test_binary`
- Asserts exit code == 0

## Test Suite Inventory

### rv32ui Tests (43 tests)
Located in `riscv-tests/isa/rv32ui/`, use `-march=rv32im`:

**Arithmetic/Logical Immediate**:
- addi, andi, ori, xori, slti, sltiu

**Arithmetic/Logical Register**:
- add, sub, and, or, xor, slt, sltu, sll, srl, sra

**Load Instructions**:
- lb, lbu, lh, lhu, lw

**Store Instructions**:
- sb, sh, sw

**Branch Instructions**:
- beq, bne, blt, bltu, bge, bgeu

**Jump Instructions**:
- jal, jalr

**Upper Immediate**:
- lui, auipc

**Special**:
- simple (basic test)
- fence_i (instruction fence)
- ma_data (misaligned data access)

### rv32um Tests (8 tests)
Located in `riscv-tests/isa/rv32um/`, use `-march=rv32im`:

- mul, mulh, mulhsu, mulhu
- div, divu, rem, remu

### rv32uc Tests (2 tests)
Located in `riscv-tests/isa/rv32uc/`, use `-march=rv32imc`:

- rvc (comprehensive compressed instruction test)
- (possible others - check directory)

## Example: Complete Conversion of ADDI

The ADDI test has been fully converted as a reference example. See:
- Source assembly: `/home/russ/risclet/addi_standalone.S`
- Binary: `/home/russ/risclet/addi_standalone.bin`
- Test: `test_rv32ui_addi` in `/home/russ/risclet/src/riscv_tests.rs`

This test validates 25 different addi operations including:
- Basic arithmetic (0+0, 1+1, 3+7)
- Sign extension of immediates
- Overflow behavior
- Zero source/destination registers
- Pipeline bypass conditions

## Important Notes

### Architecture Selection
- **rv32ui and rv32um**: Use `-march=rv32im` (NO compressed instructions)
- **rv32uc**: Use `-march=rv32imc` (WITH compressed instructions)

Using the wrong architecture will either:
- Allow compressed instructions where they shouldn't be (making tests invalid)
- Fail to assemble compressed instruction tests

### Relaxation Must Be Disabled
Both `-mno-relax` (assembler) and `--no-relax` (linker) are critical:
- Without these, the toolchain may substitute different instructions
- Example: `la` (load address) may become gp-relative addressing
- This would make the tests not match the original test intent

### Test Numbers
Tests use register x30 to track which test case is currently executing. If a test fails:
1. The fail handler sets exit code 1
2. Register x30 contains the test number that failed
3. You can disassemble the binary and search for `li x30, <num>` to find the failing test

### Binary Size
Tests vary in size:
- Simple tests: ~100-300 bytes
- Complex tests with loops: ~500-1000 bytes
- Comprehensive tests (like `rvc`): may be >1KB

### Compressed Instructions (rv32uc)
The RV32C extension tests require special handling:
- Must use `-march=rv32imc` to allow C extension
- Tests validate that compressed encodings work correctly
- The test itself may contain both compressed and uncompressed instructions

## Validation Checklist

For each converted test:

- [ ] Standalone assembly file created with proper entry point
- [ ] Assembled with correct `-march` flag
- [ ] Linked with `--no-relax` flag  
- [ ] Binary extracted and converted to hex
- [ ] Constant added to `src/riscv_tests.rs`
- [ ] Test macro invocation added
- [ ] `cargo test test_rv32ui_<name>` passes
- [ ] Test name follows naming convention: `test_rv32ui_<inst>`, `test_rv32um_<inst>`, or `test_rv32uc_<name>`

## Troubleshooting

### Test Fails with Exit Code 1
- Check which test number failed (would need enhanced error reporting)
- Disassemble the binary and find the failing test case
- Verify macro expansion matches expected behavior
- Check immediate value encoding (sign extension)

### Assembler Errors
- Verify `-march` flag is correct for the suite
- Check that macro expansions use valid RISC-V syntax
- Ensure immediate values are in valid ranges (-2048 to 2047 for I-type)

### Linker Errors
- Verify you're using `elf32lriscv` target
- Check that entry point is defined with `.globl _start`

### Infinite Loop / Timeout
- Test exceeded MAX_STEPS (100,000)
- Likely caused by incorrect branch target or missing exit handler
- Verify pass: and fail: labels are present and reachable

## Future Enhancements

Possible improvements to the test infrastructure:

1. **Enhanced Error Reporting**: Capture and report the test number (x30) on failure
2. **Automated Conversion**: Script to automate steps 1-4 for all tests
3. **Macro Preprocessor**: Tool to mechanically expand test macros
4. **Test Discovery**: Auto-generate test list from riscv-tests directory
5. **Parallel Execution**: Run tests in parallel for faster CI

## Conclusion

This plan provides a complete, step-by-step process for converting all 53 RISC-V ISA tests into embedded unit tests. The process is mechanical and can be performed by following the steps exactly. The converted tests will validate the correctness of the risclet emulator's instruction implementations against the official RISC-V test suite.
