# RV32IM Test Suite

This directory contains a comprehensive set of test programs for validating RV32IM (32-bit RISC-V with Integer and Multiply/Divide extensions) instruction execution in the risclet simulator.

## Building Tests

To build all tests:
```bash
make all
```

To clean up:
```bash
make clean
```

To disassemble all tests:
```bash
make disassemble
```

## Test Programs

### 1. test_basic_arithmetic
**File:** `test_basic_arithmetic.elf`
**Description:** Validates fundamental arithmetic operations

**Instructions Tested:**
- ADDI (Add Immediate)
- ADD (Add)
- SUB (Subtract)

**Expected Behavior:**
- Computes basic arithmetic: 100 + 42 = 142, 100 - 42 = 58
- Exits via ECALL

**How to Run:**
```bash
risclet -e tests/rv32im/test_basic_arithmetic.elf -l false -m run
```

**Validation:**
- x1 (ra) = 100
- x2 (sp) = 42
- x3 (gp) = 142
- x4 (tp) = 58

---

### 2. test_shifts
**File:** `test_shifts.elf`
**Description:** Validates logical and arithmetic shift operations

**Instructions Tested:**
- SLLI (Shift Left Logical Immediate)
- SRLI (Shift Right Logical Immediate)
- SRAI (Shift Right Arithmetic Immediate)

**Expected Behavior:**
- Left shift: 16 << 2 = 64
- Logical right shift: 64 >> 1 = 32
- Arithmetic right shift: -8 >> 2 = -2

**How to Run:**
```bash
risclet -e tests/rv32im/test_shifts.elf -l false -m run
```

**Validation:**
- x1 = 16 (0x10)
- x2 = 64 (0x40)
- x3 = 32 (0x20)
- x4 = -8 (0xFFFFFFF8)
- x5 = -2 (0xFFFFFFFE)

---

### 3. test_logical
**File:** `test_logical.elf`
**Description:** Validates bitwise logical operations

**Instructions Tested:**
- AND (Bitwise AND)
- OR (Bitwise OR)
- XOR (Bitwise XOR)
- ANDI (AND Immediate)
- ORI (OR Immediate)
- XORI (XOR Immediate)

**Expected Behavior:**
- AND: 0xAA & 0x55 = 0x00 (no common bits)
- OR: 0xAA | 0x55 = 0xFF (all bits set)
- XOR: 0xAA ^ 0x55 = 0xFF (all different bits)

**How to Run:**
```bash
risclet -e tests/rv32im/test_logical.elf -l false -m run
```

**Validation:**
- x1 = 0xAA
- x2 = 0x55
- x3 = 0x00 (AND result)
- x4 = 0xFF (OR result)
- x5 = 0xFF (XOR result)
- x6 = 0x0A (ANDI result)
- x7 = 0xAF (ORI result)
- x8 = 0x55 (XORI result)

---

### 4. test_compare
**File:** `test_compare.elf`
**Description:** Validates comparison operations (less-than)

**Instructions Tested:**
- SLT (Set Less Than - signed)
- SLTI (Set Less Than Immediate - signed)
- SLTU (Set Less Than Unsigned)
- SLTIU (Set Less Than Immediate Unsigned)

**Expected Behavior:**
- Returns 1 when first operand < second operand
- Returns 0 otherwise
- Handles both signed and unsigned comparisons

**How to Run:**
```bash
risclet -e tests/rv32im/test_compare.elf -l false -m run
```

**Validation:**
- x1 = 10
- x2 = 20
- x3 = 1 (10 < 20 is true)
- x4 = 0 (20 < 10 is false)
- x5 = 1 (10 < 15 is true)
- x6 = 0 (10 < 5 is false)

---

### 5. test_memory
**File:** `test_memory.elf`
**Description:** Validates load and store operations with different sizes

**Instructions Tested:**
- SW (Store Word - 32-bit)
- LW (Load Word - 32-bit)
- SH (Store Halfword - 16-bit)
- LH (Load Halfword signed - 16-bit)
- SB (Store Byte - 8-bit)
- LB (Load Byte signed - 8-bit)

**Expected Behavior:**
- Stores and loads values at various memory addresses
- Verifies sign extension on load operations

**How to Run:**
```bash
risclet -e tests/rv32im/test_memory.elf -l false -m run
```

**Memory Layout:**
- Address 256: 42 (32-bit word)
- Address 260: 100 (16-bit halfword)
- Address 264: 8 (8-bit byte)

**Validation:**
- x1 = 256 (address base)
- x3 = 42 (loaded from memory)
- x5 = 100 (16-bit signed load)
- x7 = 8 (8-bit signed load)

---

### 6. test_multiply
**File:** `test_multiply.elf`
**Description:** Validates multiplication operations (M extension)

**Instructions Tested:**
- MUL (Multiply - lower 32 bits)
- MULH (Multiply High - signed × signed)

**Expected Behavior:**
- 6 × 7 = 42
- Large multiplications properly handle overflow to upper bits

**How to Run:**
```bash
risclet -e tests/rv32im/test_multiply.elf -l false -m run
```

**Validation:**
- x1 = 6
- x2 = 7
- x3 = 42 (6 × 7)
- x4 = 4096
- x5 = 8192
- x6 = lower 32 bits of (4096 × 8192)
- x7 = upper 32 bits of (4096 × 8192)

---

### 7. test_divide
**File:** `test_divide.elf`
**Description:** Validates division and remainder operations (M extension)

**Instructions Tested:**
- DIV (Signed Division)
- REM (Signed Remainder)

**Expected Behavior:**
- 42 ÷ 6 = 7 with remainder 0
- 43 ÷ 6 = 7 with remainder 1
- Handles negative operands correctly

**How to Run:**
```bash
risclet -e tests/rv32im/test_divide.elf -l false -m run
```

**Validation:**
- 42 / 6 = 7, 42 % 6 = 0
- 43 / 6 = 7, 43 % 6 = 1
- -42 / 6 = -7, -42 % 6 = 0
- -43 / 6 = -7, -43 % 6 = -1

---

### 8. test_branches
**File:** `test_branches.elf`
**Description:** Validates conditional branch instructions

**Instructions Tested:**
- BEQ (Branch if Equal)
- BNE (Branch if Not Equal)
- BLT (Branch if Less Than - signed)
- BGE (Branch if Greater or Equal - signed)
- BLTU (Branch if Less Than Unsigned)
- BGEU (Branch if Greater or Equal Unsigned)

**Expected Behavior:**
- Branches are taken/not taken based on comparison results
- Program flow is affected by branch decisions

**How to Run:**
```bash
risclet -e tests/rv32im/test_branches.elf -l false -m run
```

**Validation:**
- x1 starts at 1, incremented based on branch outcomes
- Final value depends on which branches are taken

---

### 9. test_jumps
**File:** `test_jumps.elf`
**Description:** Validates jump instructions and subroutine calls

**Instructions Tested:**
- JAL (Jump and Link)
- JALR (Jump and Link Register)

**Expected Behavior:**
- JAL saves return address and jumps to target
- JALR jumps via register address
- Return addresses are correctly calculated

**How to Run:**
```bash
risclet -e tests/rv32im/test_jumps.elf -l false -m run
```

**Validation:**
- Subroutine at `target1` executes
- Return via JALR jumps back to code after JAL
- x1 = 43 (42 from subroutine + 1 after return)

---

## Running Tests with risclet

### Basic Execution (without validation)
```bash
risclet -e tests/rv32im/test_basic_arithmetic.elf -l false -m run
```

### Debug Mode (interactive UI)
```bash
risclet -e tests/rv32im/test_basic_arithmetic.elf -l false -m debug
```

### Disassembly Only
```bash
risclet -e tests/rv32im/test_basic_arithmetic.elf -m dasm
```

### Options
- `-e, --executable <path>`: Path to ELF file
- `-l, --lint <true|false>`: Enable/disable ABI validation (default: true)
- `-m, --mode <run|dasm|debug>`: Execution mode
- `-s, --steps <count>`: Maximum number of steps to execute

## Test Infrastructure

### Makefile Targets
- `make all` - Build all tests
- `make clean` - Remove all built files
- `make disassemble` - Generate disassembly listings
- `make show-<test>` - Show first 50 lines of disassembly

### Toolchain
- Assembler: `riscv64-unknown-elf-as` with `-march=rv32im -mabi=ilp32` flags
- Linker: `riscv64-unknown-elf-ld` with `-m elf32lriscv` flag
- Disassembler: `riscv64-unknown-elf-objdump`

All tests are compiled to 32-bit ELF format with text section at address 0x1000.

## Validation Checklist

These tests validate:
- ✅ RV32I base instruction set (arithmetic, logical, shift, compare)
- ✅ Load/Store operations (word, halfword, byte)
- ✅ Conditional branches (all comparison types)
- ✅ Jumps and subroutine calls
- ✅ RV32M extension (multiply, divide, remainder)
- ✅ Sign extension on loads
- ✅ PC-relative addressing
- ✅ Register-based jumps

## Future Test Enhancements

Potential additions:
- Unsigned load variants (LBU, LHU)
- Unsigned multiply variants (MULHU, MULHSU)
- Unsigned divide (DIVU, REMU)
- LUI and AUIPC testing
- Compressed instruction tests (C extension)
- Stack frame validation
- More complex program flow

## Notes

- All tests disable ABI linting (`-l false`) to avoid enforcing stack pointer requirements
- Register names in disassembly show ABI aliases (x1=ra, x2=sp, etc.)
- Tests are intentionally simple to isolate specific instruction behavior
- Memory addresses used are arbitrary but avoid program text segment (0x1000)
