# RISC-V ISA Test Suite

Complete test suite for validating RV32IMAC instruction implementations against the official RISC-V test specification.

## Quick Start

```bash
cd test
make all      # Build all test binaries
make test     # Run all tests
make clean    # Delete binaries (keep .s files)
```

## Test Status

- **Total Tests**: 60 (50 existing + 10 atomic)
- **Current Status**: ✅ 60/60 Passing (100%)

### Test Coverage

| Extension | Tests | Status |
|-----------|-------|--------|
| RV32I (Base Integer) | 40 | ✅ All pass |
| RV32M (Multiply/Divide) | 8 | ✅ All pass |
| RV32C (Compressed) | 1 | ✅ Passes |
| RV32A (Atomic) | 10 | ✅ All pass |
| **Total** | **60** | **✅ All pass** |

## File Organization

```
test/
├── README.md              # This file
├── Makefile              # Build automation
├── riscv_test.h          # Test framework macros
├── test_macros.h         # Test case macros
├── *.s                   # Generated assembly files (60 tests)
└── * (no extension)      # Compiled ELF binaries (generated)
```

## Using the Makefile

### `make all`
Regenerates all test binaries from `.s` files. Requires:
- `riscv64-unknown-elf-as` - RISC-V assembler
- `riscv64-unknown-elf-ld` - RISC-V linker
- `cpp` - C preprocessor

### `make test`
Runs all tests with QEMU. Requires:
- `qemu-riscv32` - RISC-V 32-bit emulator

Output format:
```
Running all tests...
====================
✓ addi
✓ add
... (60 total)
====================
Total: 60
Passed: 60
Failed: 0
All tests passed!
```

### `make clean`
Deletes all binary test files but preserves `.s` source files. Useful for:
- Cleaning up object files and binaries
- Preparing for rebuild
- Saving disk space

### `make help`
Shows available targets and their descriptions.

## Installing Prerequisites

### Ubuntu/Debian
```bash
sudo apt-get install gcc-riscv64-unknown-elf qemu-user build-essential
```

### macOS (Homebrew)
```bash
brew install riscv-gnu-toolchain qemu
```

### Verify Installation
```bash
riscv64-unknown-elf-as --version
riscv64-unknown-elf-ld --version
qemu-riscv32 --version
cpp --version
```

## Adding New Tests

To add tests for a new extension (e.g., RV32F for floating-point):

### 1. Get Upstream Sources
```bash
cd /tmp
git clone https://github.com/riscv/riscv-tests.git
cd riscv-tests
git submodule update --init --recursive
```

### 2. Copy Test Sources
```bash
# For RV32F (floating-point) example:
cp -r riscv-tests/isa/rv32uf /path/to/risclet/
cp -r riscv-tests/isa/rv64uf /path/to/risclet/

# Also needed (if not already present):
cp -r riscv-tests/isa/rv32ui /path/to/risclet/
cp -r riscv-tests/isa/rv64ui /path/to/risclet/
# ... etc for other extensions
```

### 3. Generate Test Files

Use the `gen_test.sh` script to preprocess a single test source file:
```bash
./gen_test.sh path/to/source.S

# Example:
./gen_test.sh ../rv32uf/fadd_s.S
# Outputs: fadd_s.s (preprocessed assembly)
```

### 4. Update Makefile

Edit the `Makefile` to handle the new architecture/extension:
- Add new architecture detection for the extension's test suite
- Update the architecture flags in `ARCH_*` variables
- List new binary targets in the test list

### 5. Build and Test
```bash
make all
make test
```

## Understanding Test Results

### Success
- Test exits with status 0 (via `ecall` with a0=0)
- QEMU exit code: 0
- Display: ✓ testname

### Failure
- Test hits `ebreak` instruction with test number in a0
- QEMU exit code: 133 (SIGTRAP)
- Display: ✗ testname (exit code: 133)

### Debugging Failed Tests

If a test fails, identify which sub-test failed:

```bash
# Look at the generated .s file to find test_N labels
grep "^test_" test/mul.s | head -10

# Disassemble to see test structure
riscv64-unknown-elf-objdump -d test/mul | grep "test_3:" -A 10
```

The test number in register a0 at failure time tells you which test case failed.

## Technical Details

### Test Framework

- **Entry Point**: `_start` symbol (execution begins here)
- **Success Path**: `li a0, 0; li a7, 93; ecall` (exit with status 0)
- **Failure Path**: `mv a0, TESTNUM; ebreak` (test number in a0, trap)
- **Test Counter**: Register x28 holds current test number (TESTNUM)

### Architecture Flags

| Suite | Flag | Purpose |
|-------|------|---------|
| rv32ui, rv32um | `rv32im_zifencei` | Base + Multiply + Fence |
| rv32ua | `rv32ima_zifencei` | Base + Multiply + Atomic + Fence |
| rv32uc | `rv32imc_zifencei` | Base + Multiply + Compressed + Fence |

### Special Considerations

**RV32C (rvc) Test**: Modified from upstream version
- Original embedded writable data in .text section (incompatible with user-mode)
- Modified version uses proper .data section
- All compressed instruction tests remain intact
- Page-boundary test simplified (not relevant for emulator)

## Regenerating from Upstream

The `.s` and binary files are generated from upstream RISC-V test sources. To regenerate:

1. **Restore upstream sources** (see "Adding New Tests" above)
2. **Run**: `make all`
3. **Verify**: `make test`

Expected: 60/60 tests passing (with all extensions present)

## Requirements

- **RISC-V Toolchain**: `riscv64-unknown-elf-{as,ld}`
- **C Preprocessor**: `cpp`
- **QEMU**: `qemu-riscv32`
- **Build Tools**: `make`, `bash`

## Test Suites Reference

When restoring from upstream, these are the test directories you'll encounter:

| Directory | Origin | Purpose | Included |
|-----------|--------|---------|----------|
| rv32ui/ | riscv-tests/isa | RV32I base instructions | ✅ Yes (40 tests) |
| rv32um/ | riscv-tests/isa | RV32M multiply/divide | ✅ Yes (8 tests) |
| rv32ua/ | riscv-tests/isa | RV32A atomic operations | ✅ Yes (10 tests) |
| rv32uc/ | riscv-tests/isa | RV32C compressed (modified) | ✅ Yes (1 test) |
| rv64ui/ | riscv-tests/isa | RV64I (included by rv32ui) | ⚠️ Needed for build |
| rv64um/ | riscv-tests/isa | RV64M (included by rv32um) | ⚠️ Needed for build |
| rv64ua/ | riscv-tests/isa | RV64A (included by rv32ua) | ⚠️ Needed for build |
| rv64uc/ | riscv-tests/isa | RV64C (included by rv32uc) | ⚠️ Needed for build |

Note: RV32 tests include RV64 versions via preprocessor, so both are needed.

## Troubleshooting

### "Command not found: riscv64-unknown-elf-as"
Install the RISC-V GNU toolchain (see "Installing Prerequisites")

### "Command not found: qemu-riscv32"
Install QEMU user-mode emulation (see "Installing Prerequisites")

### Tests fail with "Illegal instruction"
Verify architecture flags match the test suite:
- Check `Makefile` for correct `-march` flags
- Rebuild with `make clean && make all`

### "Segmentation fault" when running test
Binary may be corrupted:
```bash
make clean
make all
```

### All tests fail
Verify tools are correctly installed:
```bash
riscv64-unknown-elf-as --version
riscv64-unknown-elf-ld --version
qemu-riscv32 --version
```

## Example Workflow

```bash
# Clean previous build
make clean

# Rebuild all tests
make all

# Run tests
make test

# If you added new .S sources:
# 1. Generate: ./gen_test.sh path/to/new_test.S
# 2. Update Makefile with new binary target
# 3. Run: make all && make test
```

## References

- [RISC-V ISA Specification](https://riscv.org/technical/specifications/)
- [riscv-tests Repository](https://github.com/riscv/riscv-tests)
- [RISC-V GNU Toolchain](https://github.com/riscv-collab/riscv-gnu-toolchain)

---

**Last Updated**: November 2024
**Test Suite Version**: RV32IMAC (2.2)
**Status**: 60/60 tests passing
