# Upstream Test Sources

## What's Been Removed

To keep the repository clean, the following upstream test source directories have been removed:

- `rv32ui/` - RV32I base instruction tests (43 .S files)
- `rv32um/` - RV32M multiply/divide tests (8 .S files)
- `rv32uc/` - RV32C compressed instruction tests (1 .S file, modified locally)
- `rv64ui/` - RV64I base instruction tests (included by rv32ui)
- `rv64um/` - RV64M tests (included by rv32um)
- `rv64uc/` - RV64C tests (included by rv32uc)

## What's Kept (Generated Files)

The repository keeps these generated/processed files:

```
test/
├── *.s              # 50 generated standalone assembly files
├── riscv_test.h     # Test framework macros
├── test_macros.h    # Test case macros (includes all necessary macros)
test/gen_test.sh          # Preprocessor script
test/build_all_tests.sh   # Build script
test/run_all_tests.sh     # Test runner
```

## Restoring Upstream Sources

If you need to add tests for new extensions or regenerate existing tests:

### 1. Clone riscv-tests

```bash
cd /tmp
git clone https://github.com/riscv/riscv-tests.git
cd riscv-tests
git submodule update --init --recursive
```

### 2. Copy Test Directories

From the riscv-tests checkout, copy to your risclet directory:

```bash
RISCLET=/path/to/risclet

# Required directories for RV32I/M/C
cp -r isa/rv32ui $RISCLET/
cp -r isa/rv32um $RISCLET/
cp -r isa/rv32uc $RISCLET/
cp -r isa/rv64ui $RISCLET/
cp -r isa/rv64um $RISCLET/
cp -r isa/rv64uc $RISCLET/
```

### 3. Apply Local Modifications

**Important:** The `rv32uc/rvc.S` file requires a local modification to work with read-execute-only text segments.

Replace the contents of `rv32uc/rvc.S` with the version from your repository backup, or manually apply this patch:

**Change 1:** Simplify test_2 (remove inline data):
```assembly
# Before:
TEST_CASE (2, a1, 667, \
      j 1f; \
      .align 3; \
      data: \
        .dword 0xfedcba9876543210; \
        .dword 0xfedcba9876543210; \
      .align 12; \
      .skip 4094; \
    1: addi a1, a1, 1)

# After:
TEST_CASE (2, a1, 667, addi a1, a1, 1)
```

**Change 2:** Add data to .data section (at end of file before RVTEST_DATA_END):
```assembly
  .data
RVTEST_DATA_BEGIN

  .align 3
data:
  .dword 0xfedcba9876543210
  .dword 0xfedcba9876543210

RVTEST_DATA_END
```

## Adding New Extensions

To add tests for additional RISC-V extensions (e.g., RV32A atomics, RV32F floating-point):

### 1. Copy Extension Tests

```bash
# For RV32A (atomics):
cp -r riscv-tests/isa/rv32ua $RISCLET/
cp -r riscv-tests/isa/rv64ua $RISCLET/

# For RV32F (single-precision floating-point):
cp -r riscv-tests/isa/rv32uf $RISCLET/
cp -r riscv-tests/isa/rv64uf $RISCLET/
```

### 2. Update test/build_all_tests.sh

Add the new suite to the loop:
```bash
for suite in rv32ui rv32um rv32uc rv32ua; do
```

Add architecture configuration:
```bash
ARCH="rv32im_zifencei"
if [[ "$suite" == "rv32uc" ]]; then
    ARCH="rv32imc_zifencei"
elif [[ "$suite" == "rv32ua" ]]; then
    ARCH="rv32ima_zifencei"
fi
```

### 3. Update test/run_all_tests.sh

Add new test binaries to the test list:
```bash
for test_bin in ... \
                "$TESTDIR"/amoadd "$TESTDIR"/amoand ... ; do
```

### 4. Update test_macros.h (if needed)

Some extensions may require additional test macros. Check if the new tests use any macros not in `test/test_macros.h`. If so, add them from `riscv-tests/isa/macros/scalar/test_macros.h`.

### 5. Rebuild and Test

```bash
./test/build_all_tests.sh
./test/run_all_tests.sh
```

## Common Extensions and Test Counts

| Extension | Description | Suite | Approximate Test Count |
|-----------|-------------|-------|----------------------|
| RV32I | Base integer | rv32ui | 43 tests |
| RV32M | Multiply/divide | rv32um | 8 tests |
| RV32A | Atomics | rv32ua | 8 tests |
| RV32F | Single-precision FP | rv32uf | ~20 tests |
| RV32D | Double-precision FP | rv32ud | ~20 tests |
| RV32C | Compressed | rv32uc | 1 test |
| Zicsr | CSR instructions | rv32ui (partial) | Included in base |
| Zifencei | Instruction fence | rv32ui (fence_i) | 1 test (skipped) |

## Version Information

The test macros and framework in this repository are compatible with:
- riscv-tests commit: Latest (as of Oct 2024)
- RISC-V ISA version: RV32IMC (2.2)

## Why These Sources Aren't Checked In

1. **Size**: The upstream directories contain ~100 .S files plus infrastructure
2. **Maintenance**: Tests rarely change; regeneration is infrequent
3. **Clarity**: Generated .s files show exactly what's being tested
4. **Local modifications**: rvc.S needs local patches; cleaner to maintain separately

## Recovery

If you lose your local test sources and need to regenerate:

1. Follow "Restoring Upstream Sources" above
2. Run `./test/build_all_tests.sh`
3. Verify with `./test/run_all_tests.sh`
4. Expected: 50/50 tests passing (fence_i excluded)
