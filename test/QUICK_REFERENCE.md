# Quick Reference

## Important Files

**Core test files (checked in):**
- `test/*.s` - 50 generated assembly test files
- `test/riscv_test.h` - Test framework macros  
- `test/test_macros.h` - Test case macros
- `test/gen_test.sh`, `test/build_all_tests.sh`, `test/run_all_tests.sh` - Scripts

**Upstream sources (NOT checked in):**
- `rv32ui/`, `rv32um/`, `rv32uc/` - Test sources (restore from riscv-tests if needed)
- See `UPSTREAM_SOURCES.md` for restoration instructions

## Generate a Single Test

```bash
test/gen_test.sh rv32ui/addi.S
```

This creates `test/addi.s` with all macros expanded.

## Build All Tests

```bash
test/build_all_tests.sh
```

Generates 50 standalone test executables in `test/` directory.

## Run All Tests

```bash
test/run_all_tests.sh
```

Expected output: 50/50 tests passing (100%)

## Manual Build Example

```bash
# Preprocess
cpp -nostdinc -I test -D__riscv_xlen=32 -P rv32ui/addi.S test/addi.s

# Assemble
riscv64-unknown-elf-as -march=rv32im_zifencei -mabi=ilp32 -o test/addi.o test/addi.s

# Link
riscv64-unknown-elf-ld -melf32lriscv --no-relax -o test/addi test/addi.o

# Run
qemu-riscv32 test/addi
echo $?  # Should be 0 for pass
```

## Test a Broken Case

```bash
# Modify test to force failure
sed 's/test_3: li x28, 3; .* bne x14, x7, fail;;/test_3: li x28, 3; li x7, 999; bne x14, x7, fail;;/' \
    test/addi.s > test/addi_broken.s

# Build and run
riscv64-unknown-elf-as -march=rv32im_zifencei -mabi=ilp32 -o test/addi_broken.o test/addi_broken.s
riscv64-unknown-elf-ld -melf32lriscv --no-relax -o test/addi_broken test/addi_broken.o
qemu-riscv32 test/addi_broken
# Output: "Trace/breakpoint trap", exit code 133
```

## Test Results Summary

- **50 passing**: All applicable RV32I base + RV32M multiply/divide + RV32C compressed + load/store bypass + misaligned access tests (100%)
- **1 skipped**: fence_i (cache coherency - not applicable for emulator)
