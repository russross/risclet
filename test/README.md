# RISC-V Test Suite

This directory contains a complete, self-contained test suite for validating RV32IMC instruction implementations.

## Status: ✅ 50/50 Tests Passing (100%)

## Quick Start

```bash
cd test
./run_all_tests.sh
```

## Documentation

- **README.md** (this file) - Quick overview
- **QUICK_REFERENCE.md** - Command reference and examples
- **TEST_GENERATION.md** - Detailed test generation process
- **UPSTREAM_SOURCES.md** - How to restore/extend from upstream sources
- **TEST_SUITE.md** - Complete test suite documentation
- **FINAL_SUMMARY.md** - Project completion summary

## Contents

### Generated Test Files (50 tests)
- `*.s` - Preprocessed assembly files (human-readable)
- `*` (no extension) - Compiled ELF executables

### Test Framework
- `riscv_test.h` - Test framework macros (_start, pass/fail handlers)
- `test_macros.h` - Comprehensive test case macros for all instruction types

### Scripts
- `gen_test.sh` - Generate single test from upstream source
- `build_all_tests.sh` - Build all tests
- `run_all_tests.sh` - Run all tests and report results

## Test Coverage

**RV32I Base (40 tests):**
- Arithmetic: add, addi, sub
- Logical: and, andi, or, ori, xor, xori
- Shifts: sll, slli, srl, srli, sra, srai
- Compare: slt, slti, sltu, sltiu
- Branches: beq, bne, blt, bge, bltu, bgeu
- Jumps: jal, jalr
- Upper imm: lui, auipc
- Loads: lb, lbu, lh, lhu, lw
- Stores: sb, sh, sw
- Special: ld_st, st_ld, ma_data, simple

**RV32M Extension (8 tests):**
- Multiply: mul, mulh, mulhsu, mulhu
- Divide: div, divu, rem, remu

**RV32C Extension (1 test):**
- Compressed instructions: rvc

**Skipped (1 test):**
- fence_i - Not applicable (tests cache coherency)

## Usage

```bash
# Run all tests
./run_all_tests.sh

# Build all tests
./build_all_tests.sh

# Generate single test (requires upstream sources)
./gen_test.sh ../rv32ui/addi.S
```

## Requirements

- RISC-V GNU toolchain: `riscv64-unknown-elf-{as,ld}`
- C preprocessor: `cpp`
- QEMU user-mode: `qemu-riscv32`

## Building from Upstream

If you need to regenerate tests or add new extensions (e.g., RV32A for atomics):

1. Restore upstream sources (see UPSTREAM_SOURCES.md)
2. Run `./build_all_tests.sh`
3. Run `./run_all_tests.sh` to verify

## Architecture Notes

- **rv32ui/rv32um tests**: Built with `-march=rv32im_zifencei`
- **rv32uc test**: Built with `-march=rv32imc_zifencei`
- **rvc.S modified**: Uses proper .data section instead of inline data for read-execute-only text segment compatibility

---

See individual documentation files for detailed information.
