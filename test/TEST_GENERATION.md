# RISC-V Test Generation

This document describes the test case generation system for RV32IMC instruction validation.

## Repository Structure

**Checked In:**
- `test/*.s` - Generated standalone assembly test files (50 tests)
- `test/*.h` - Test framework header files (riscv_test.h, test_macros.h)
- `test/gen_test.sh` - Script to generate individual tests
- `test/build_all_tests.sh` - Script to build all tests
- `test/run_all_tests.sh` - Script to run all tests

**NOT Checked In (Upstream Sources):**
If you need to regenerate tests or add new extensions, restore these directories from upstream:
- `rv32ui/` - RV32I base instruction test sources (from riscv/riscv-tests)
- `rv32um/` - RV32M multiply/divide test sources (from riscv/riscv-tests)
- `rv32uc/` - RV32C compressed instruction test sources (from riscv/riscv-tests)
- `rv64ui/`, `rv64um/`, `rv64uc/` - RV64 test sources (RV32 tests include these)

**Note on rv32uc/rvc.S:** This file has been **modified locally** to use a proper `.data` section instead of embedding writable data in `.text`. If restoring from upstream, you'll need to reapply this modification.

## Overview

The test suite consists of standalone assembly programs that validate RISC-V instructions. Each test is self-contained and can be assembled, linked, and run independently.

## Files

### Header Files

- **test/riscv_test.h** - Test framework macros adapted for 32-bit standalone execution
  - Defines `_start` as the global entry point
  - `RVTEST_PASS` - Exit with status 0 via ecall (syscall 93)
  - `RVTEST_FAIL` - Load test number into a0 and execute ebreak

- **test/test_macros.h** - Test case macros for various instruction types
  - Arithmetic, logical, branch, load/store test macros
  - Uses x28 (t3) for TESTNUM register
  - MASK_XLEN set to 0xffffffff for 32-bit

## Regenerating Tests from Upstream

If you need to add tests for new extensions (e.g., RV32A for atomics, RV32F for floating-point):

### 1. Restore Upstream Test Sources

Clone the riscv-tests repository:
```bash
git clone https://github.com/riscv/riscv-tests.git
cd riscv-tests
git submodule update --init --recursive
```

Copy the relevant test directories to your project:
```bash
# For RV32A (atomics) example:
cp -r riscv-tests/isa/rv32ua /path/to/risclet/rv32ua
cp -r riscv-tests/isa/rv64ua /path/to/risclet/rv64ua

# Ensure you also have the shared directories:
cp -r riscv-tests/isa/rv32ui /path/to/risclet/rv32ui
cp -r riscv-tests/isa/rv32um /path/to/risclet/rv32um
cp -r riscv-tests/isa/rv32uc /path/to/risclet/rv32uc
cp -r riscv-tests/isa/rv64ui /path/to/risclet/rv64ui
cp -r riscv-tests/isa/rv64um /path/to/risclet/rv64um
cp -r riscv-tests/isa/rv64uc /path/to/risclet/rv64uc
```

**Important:** Remember to restore the local modification to `rv32uc/rvc.S` (see below).

### 2. Update Build Scripts

Add the new extension to `build_all_tests.sh`:
```bash
for suite in rv32ui rv32um rv32uc rv32ua; do  # Add rv32ua
```

Update the architecture flags as needed:
```bash
# For atomics extension
if [[ "$suite" == "rv32ua" ]]; then
    ARCH="rv32ima_zifencei"
fi
```

### 3. Rebuild Tests

```bash
./build_all_tests.sh
./run_all_tests.sh
```

### Scripts

All scripts are located in the `test/` directory for self-containment.

- **test/gen_test.sh** - Generate a single test from source
  ```bash
  test/gen_test.sh rv32ui/addi.S
  ```
  This runs the C preprocessor with `-D__riscv_xlen=32` to expand macros and produce standalone assembly.

- **test/build_all_tests.sh** - Generate, assemble, and link all tests
  ```bash
  test/build_all_tests.sh
  ```
  Processes all tests in rv32ui/, rv32um/, and rv32uc/ directories.
  - Uses architecture rv32im_zifencei (rv32imc_zifencei for compressed tests)
  - Assembles with `riscv64-unknown-elf-as -march=rv32im_zifencei -mabi=ilp32`
  - Links with `riscv64-unknown-elf-ld -melf32lriscv --no-relax`
  - Skips ld_st, st_ld, ma_data (require additional macros not in test_macros.h)

- **test/run_all_tests.sh** - Run all tests with qemu-riscv32
  ```bash
  test/run_all_tests.sh
  ```
  Reports pass/fail status for each test.

## Test Results

Current status: **50 out of 50 applicable tests passing (100%)**

### Passing Tests (50)

**RV32I Base Integer Instructions:**
- Arithmetic: add, addi, sub
- Logical: and, andi, or, ori, xor, xori
- Shifts: sll, slli, srl, srli, sra, srai
- Comparisons: slt, slti, sltu, sltiu
- Branches: beq, bne, blt, bge, bltu, bgeu
- Jumps: jal, jalr
- Upper immediates: lui, auipc
- Loads: lb, lbu, lh, lhu, lw
- Stores: sb, sh, sw
- Load/Store bypass: ld_st, st_ld
- Misaligned access: ma_data
- Basic: simple

**RV32M Multiply/Divide Extension:**
- Multiply: mul, mulh, mulhsu, mulhu
- Divide: div, divu, rem, remu

**RV32C Compressed Instructions:**
- rvc - Comprehensive compressed instruction test

### Skipped Tests (1)

- **fence_i** - Not applicable for this project
  - Tests instruction cache coherency with self-modifying code
  - Requires machine-mode execution and cache management
  - Not relevant for an emulator that doesn't simulate caches

### RVC Test - Modified for User-Mode

The **rvc** test has been modified from the upstream version:
- Original test embedded writable data in .text section with `.align 12` and `.skip 4094` to test page boundary instruction fetching
- Modified version moves data to proper `.data` section for user-mode compatibility
- Page boundary test (test_2) simplified to basic addi test (not relevant for emulator)
- All compressed instruction tests remain intact and functional
- Proper memory segments: .text is R-E only, .data is R-W only

**To restore from upstream:** If you restore `rv32uc/` from riscv-tests, you must reapply the modification to `rv32uc/rvc.S`:

1. Replace the inline data in test_2:
   ```diff
   - TEST_CASE (2, a1, 667, \
   -       j 1f; \
   -       .align 3; \
   -       data: \
   -         .dword 0xfedcba9876543210; \
   -         .dword 0xfedcba9876543210; \
   -       .align 12; \
   -       .skip 4094; \
   -     1: addi a1, a1, 1)
   + TEST_CASE (2, a1, 667, addi a1, a1, 1)
   ```

2. Add data to the .data section at the end:
   ```diff
     .data
   RVTEST_DATA_BEGIN
   
   +  .align 3
   +data:
   +  .dword 0xfedcba9876543210
   +  .dword 0xfedcba9876543210
   +
   RVTEST_DATA_END
   ```

Alternatively, use the version already in your repository as the authoritative local copy.



## Test Structure

Each test follows this pattern:

1. **Entry point**: `_start:` symbol
2. **Test cases**: Individual test_N labels that:
   - Set TESTNUM to test number
   - Execute instruction being tested
   - Compare result with expected value
   - Branch to `fail` on mismatch
3. **Success path**: Falls through to RVTEST_PASS
   - `li a0, 0; li a7, 93; ecall` (exit syscall with status 0)
4. **Failure path**: `fail:` label
   - `mv a0, TESTNUM; ebreak` (breakpoint with test number in a0)

## Validation

To verify the test framework works correctly:

1. **Normal test**: `qemu-riscv32 test/addi` exits with status 0
2. **Broken test**: Create a test that deliberately fails
   ```bash
   sed 's/test_3: li x28, 3; .* bne x14, x7, fail;;/test_3: li x28, 3; li x7, 999; bne x14, x7, fail;;/' test/addi.s > test/addi_broken.s
   ```
   Then assemble, link and run - should get "Trace/breakpoint trap" and exit code 133

## Future Work

- Add missing macros for ld_st, st_ld, and ma_data tests
- Investigate fence_i and rvc failures
- Consider adding more detailed reporting (which specific test number failed)
