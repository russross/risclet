# Getting Started with the Test Suite

## Prerequisites

Install required tools:

```bash
# On Ubuntu/Debian
sudo apt-get install gcc-riscv64-unknown-elf qemu-user

# On macOS with Homebrew
brew install riscv-gnu-toolchain qemu
```

Verify installation:

```bash
riscv64-unknown-elf-as --version
qemu-riscv32 --version
```

## Quick Start

1. **Run all tests** (recommended first step):
   ```bash
   cd test
   ./run_all_tests.sh
   ```
   
   Expected output:
   ```
   Running all tests...
   ====================
   ✓ addi
   ✓ add
   ... (48 more)
   ====================
   Total: 50
   Passed: 50
   Failed: 0
   All tests passed!
   ```

2. **Rebuild tests** (if needed):
   ```bash
   ./build_all_tests.sh
   ```

3. **Run a single test**:
   ```bash
   qemu-riscv32 ./addi
   echo $?  # Should print 0 for success
   ```

## Understanding Test Output

### Success
- Exit code 0
- No output (silent success)

### Failure
- Exit code 133 (ebreak)
- Register a0 contains the test number that failed

### Example: Debugging a Failed Test

If test `mul` fails with exit code 133:

1. Check which test number failed by examining the test source:
   ```bash
   grep "li x28" mul.s | head -20
   ```

2. Disassemble the binary to find the failing test:
   ```bash
   riscv64-unknown-elf-objdump -d mul | grep -A5 "test_3:"
   ```

3. Look for the comparison that failed and check expected vs actual values

## File Organization

```
test/
├── README.md              # Main documentation
├── GETTING_STARTED.md     # This file
├── QUICK_REFERENCE.md     # Command cheat sheet
├── TEST_GENERATION.md     # How tests are generated
├── UPSTREAM_SOURCES.md    # Extending the test suite
├── TEST_SUITE.md          # Complete test documentation
├── FINAL_SUMMARY.md       # Project summary
│
├── gen_test.sh            # Generate single test
├── build_all_tests.sh     # Build all tests
├── run_all_tests.sh       # Run all tests
│
├── riscv_test.h           # Test framework
├── test_macros.h          # Test macros
│
└── *.s, *                 # Test files (50 tests)
```

## Common Tasks

### Add a New Test

If you have upstream sources in parent directory:

```bash
./gen_test.sh ../rv32ui/newtest.S
riscv64-unknown-elf-as -march=rv32im_zifencei -mabi=ilp32 -o newtest.o newtest.s
riscv64-unknown-elf-ld -melf32lriscv --no-relax -o newtest newtest.o
qemu-riscv32 ./newtest
```

### Verify Test Binary

```bash
# Check ELF sections
riscv64-unknown-elf-readelf -S addi

# Disassemble
riscv64-unknown-elf-objdump -d addi | less

# Check for compressed instructions
riscv64-unknown-elf-objdump -d rvc | grep -E "^\s+[0-9a-f]+:\s+[0-9a-f]{4}\s"
```

### Clean Up

```bash
# Remove all binaries (keep .s files)
rm -f add addi and andi auipc beq ... (all test names)

# Or rebuild everything
./build_all_tests.sh
```

## What's Next?

- Read `README.md` for test suite overview
- Check `QUICK_REFERENCE.md` for command reference
- See `TEST_GENERATION.md` to understand test generation
- Review `UPSTREAM_SOURCES.md` to add new extensions
- Read `FINAL_SUMMARY.md` for project accomplishments

## Troubleshooting

### "Command not found: riscv64-unknown-elf-as"
Install the RISC-V GNU toolchain (see Prerequisites)

### "Command not found: qemu-riscv32"
Install QEMU user-mode emulation (see Prerequisites)

### Tests fail with "Illegal instruction"
Check that you're using the correct architecture flags:
- Most tests: `-march=rv32im_zifencei`
- rvc test: `-march=rv32imc_zifencei`

### "Segmentation fault" when running test
Binary may be corrupted. Rebuild:
```bash
./build_all_tests.sh
```

### All tests fail
Verify tools are correctly installed:
```bash
riscv64-unknown-elf-as --version
riscv64-unknown-elf-ld --version
qemu-riscv32 --version
```

---

**Ready to go!** Start with `./run_all_tests.sh` to verify everything works.
