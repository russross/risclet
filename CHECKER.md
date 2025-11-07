# Analysis: The `--lint` Option

## Executive Summary

The `--lint` option (short form: `-l`) is **not a linter in the traditional sense**. It does not analyze source code to identify potential bugs, style issues, or best practices. Instead, it is a **runtime ABI (Application Binary Interface) checker** that validates whether a program adheres to the RISC-V ABI calling conventions as it executes instruction-by-instruction during simulation.

## What It Actually Does

The `--lint` option enables runtime ABI checking during program execution. When enabled, it:

1. **Traces value flow** through registers and memory during execution
2. **Enforces calling convention rules** at function boundaries (jal, jalr, ret)
3. **Validates register preservation guarantees**
4. **Detects data integrity violations** during memory operations
5. **Reports violations as fatal errors** that terminate execution

## Problems It Detects

### 1. Uninitialized Register Usage
- **Problem**: Reading from a register that was never written
- **Example**: Using a parameter before it was passed into a function
- **Message**: `"{register} is uninitialized"`

### 2. Saved Register Violations
- **Problem**: Using a value that was marked "save-only" in an operation other than memory storage
- **Message**: `"the value in {register} can only be saved to memory; it is not a valid input"`

### 3. Incorrect Function Call Protocol
- **Problem**: Using `jal`/`jalr` without writing the return address to `ra` (register 1)
- **Example**: An instruction sequence that calls a function but doesn't store the return address in `ra`
- **Message**: `"{jal|jalr} did not use ra for return address"`

### 4. Unlabeled Jumps
- **Problem**: Jumping to an address that has no symbol/label defined
- **Message**: `"{jal|jalr} to unlabeled address"`

### 5. Stack Pointer Misalignment
- **Problem**: `sp` (register 2) is not a multiple of 16 (ABI requirement for 16-byte stack alignment)
- **Message**: `"sp must always be a multiple of 16"`

### 6. Uninitialized Function Arguments
- **Problem**: Passing an uninitialized value as a function argument
- **Message**: `"argument in {register} is uninitialized"`

### 7. Register State Not Preserved
- **Problem**: Modified registers that should have been preserved across function calls (ra, gp, tp, s-registers)
- **Message**: `"{register} is not same value as when function called"`

### 8. Stack Frame Mismatch
- **Problem**: `sp` is not at the same address when returning from a function
- **Message**: `"sp is not same value as when function called"`

### 9. Unmatched Return
- **Problem**: Executing a `ret` instruction (jalr x0, ra, 0) without a corresponding function call
- **Message**: `"ret with no stack frame to return to"`

### 10. Memory Alignment Violations
- **Problem**: Load/store operations at unaligned addresses
- **Example**: Doing an `lw` (4-byte load) from an address not divisible by 4
- **Message**: `"{N}-byte memory {read|write} at/from unaligned address 0x{addr}"`

### 11. Partial Memory Access Violations
- **Problem**: Reading fewer bytes than were written, or reading data that spans multiple writes
- **Examples**:
  - Writing 4 bytes then reading only 2 bytes from that location
  - Writing to bytes 0x1000-0x1001, then reading bytes 0x1001-0x1003
- **Messages**:
  - `"reading data that was only partially written"`
  - `"reading data from multiple writes"`
  - `"reading data with different size than when written"`
  - `"reading data that is only partially from a previous write"`

### 12. System Call Data Corruption
- **Problem**: Syscalls (write/read) violating data integrity constraints
- **Examples**:
  - Writing non-byte data via syscall
  - Reading into a location that contains non-byte data
- **Messages**:
  - `"write syscall on non-byte data"`
  - `"read syscall overwriting non-byte data"`

## Restrictions It Imposes

### Value Tracking
- Tracks register values through a numbering system to ensure the same logical value is preserved when required
- Distinguishes between clones (via `mv`) and new values

### Register Categories
- **Always valid**: x0 (zero), sp (x2)
- **Argument registers** (a0-a7): Passed to functions, validity context-dependent
- **Temporary registers** (t0-t6): Invalidated at function calls
- **Saved registers** (s0-s11): Must preserve their values across function calls
- **Special registers**: ra (return address), gp (global pointer), tp (thread pointer)

### Memory State Machine
- Each memory location is tagged with a unique value number and size
- Reading from memory must match the size written (no partial reads/writes)
- Multiple writes to overlapping ranges cause value identity violations

### Function Call Context
- Maintains a stack of register contexts for nested function calls
- On `jal`/`jalr`: saves caller state, invalidates temporaries, validates argument count
- On `ret`: restores caller state, validates preservation requirements

## How It Works

The checker is implemented in `src/linter.rs:Linter` and is invoked during execution in `src/execution.rs` at line 683:

```rust
if !effects.terminate && config.lint
    && let Err(msg) = linter.check_instruction(m, instruction, &mut effects)
{
    effects.error(msg);
}
```

It operates as a **state machine** that:
1. Receives each instruction after it executes
2. Examines the instruction's effects (register reads/writes, memory operations)
3. Updates internal tracking state for registers and memory
4. Validates state transitions against ABI rules
5. Returns errors for violations

## Why the Name is Misleading

- **Traditional linters** (like eslint, pylint, clippy) analyze source code statically before execution
- **The `--lint` option** operates at **runtime** after decoding instructions, examining execution effects
- It is fundamentally a **runtime validator/checker**, not a static analyzer
- The term "linter" is borrowed metaphorically but creates false expectations

## Proposed Better Names

The following names better capture the actual functionality:

1. **`--check-abi`** (recommended)
   - Clear, descriptive, technical
   - Immediately communicates that it validates ABI compliance
   - Mirrors common naming (e.g., `--check-syntax`, `--check-types`)

2. **`--abi-strict`**
   - Implies strict checking of ABI rules
   - Short, memorable
   - Similar to `-std=c99 -Werror` style flags

3. **`--trace-values`**
   - Emphasizes the value-tracking mechanism
   - Highlights that it monitors data flow
   - Good if the intent is to teach data flow concepts

4. **`--validate-abi`**
   - Clear intent: validate that code follows ABI
   - More formal than "check"
   - Common pattern: `--validate-*`

5. **`--enforce-abi`**
   - Suggests this is a strict requirement
   - Makes clear that violations are fatal
   - Useful for understanding this is not a warning system

## Recommendation

**Use `--check-abi`** (short form: `-a`) as the replacement because:
- Accurate: it checks ABI compliance at runtime
- Unambiguous: "linter" implies static analysis, which this is not
- Consistent: follows the pattern of `--check-*` flags in other tools
- Clear: students immediately understand what it validates
- Formal: appropriate for an educational tool teaching ABI concepts
