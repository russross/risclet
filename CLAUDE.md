# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**risclet** is a lightweight RISC-V disassembler, simulator, and assembler for students learning assembly language. It focuses on simplicity and approachability with minimal controls and strict ABI checking.

The tool supports rv32imac (RV32I base + M multiplication/divide + A atomic + C compressed instructions). It operates as a unified binary with two main subsystems:
- **Simulator**: Loads ELF binaries, simulates execution with ABI linting, records instruction traces, and provides an interactive TUI debugger
- **Assembler**: Parses RISC-V assembly source files, performs relaxation for branch encoding optimization, and generates ELF binaries

## Key Architectural Insights

### Unified Binary Architecture
The tool merged separate assembler and simulator repositories into a single binary (`src/main.rs`). The `main` function dispatches to either assembler or simulator based on the subcommand (assemble/run/disassemble/debug).

### Assembler Pipeline (src/assembler.rs: core driver)
1. **Tokenize** (`tokenizer.rs`): Lexical analysis producing tokens with source locations
2. **Parse** (`parser.rs`): Recursive descent parser building AST from tokens
3. **Symbol Linking** (`symbols.rs`): Resolve symbol references to definitions, compute widths
4. **Relaxation Loop** (`layout.rs` + `encoder.rs`): Iteratively encode instructions and compute offsets until stable:
   - Fixed-point computation: instruction widths determine offsets, offsets determine branch encodings
   - Pseudo-instruction expansion affects widths (e.g., `li` expands to 2-8 instructions depending on immediates)
   - Loop continues until two consecutive iterations produce identical layouts
5. **ELF Generation** (`elf_builder.rs`): Build executable with encoded instructions

### Simulator Pipeline (src/simulator.rs: core driver)
1. **ELF Loading** (`elf_loader.rs`): Parse executable, set up memory regions (text, data, stack, heap)
2. **Instruction Decoding** (`decoder.rs`): Translate binary to pseudo-instruction representation
3. **Pseudo-sequence Detection** (`riscv.rs`): Identify when multiple instructions are pseudoinstructions
4. **Execution Tracing** (`execution.rs` + `trace.rs`): Simulate execution recording effects of each instruction:
   - Linting enforces ABI calling conventions if enabled
   - Records register changes, memory operations, control flow
5. **TUI Debugger** (`ui.rs`): Interactive terminal interface for stepping/jumping through execution

### Key Data Structures

**Assembler:**
- `AST (ast.rs)`: Source-level representation with location tracking for error reporting
- `Symbol (symbols.rs)`: Symbol table with forward reference resolution
- `Layout`: Computed offsets and instruction widths used during relaxation
- `Encoding`: Machine code with relocation information

**Simulator:**
- `Instruction (execution.rs)`: Decoded instruction with verbose and pseudo field representations
- `Machine (elf_loader.rs)`: Memory state with registers, segments, symbols
- `Effects (trace.rs)`: What an instruction does (register writes, memory access, next PC)
- `Tui`: Terminal display state for debugging interface

### Important Implementation Details

**Relaxation Stability (encoder.rs:527+):**
- Critical: Pseudo-instructions have variable encoding lengths based on immediate values
- Example: `li x1, imm` needs 1-4 instructions depending on imm magnitude
- Relaxation loop must converge; offsets changing can change instruction widths
- Tests use fixed instruction widths to validate encoder without relaxation

**Pseudo-instruction Handling:**
- Assembler: `parser.rs` generates pseudo-instructions which `encoder.rs` expands
- Simulator: `riscv.rs:get_pseudo_sequence()` reverse-detects when consecutive instructions form a pseudo-instruction
- Display shows pseudo view; verbose view shows actual instructions

**Symbol Resolution:**
- Two-phase: `symbols.rs` computes symbol widths and values after parsing
- Forward references (branches to undefined labels) are allowed; resolver computes final values
- ABI built-in symbols (gp, sp) created by `create_builtin_symbols_file()`

**Linting (linter.rs):**
- Enforces RISC-V ABI calling conventions: register preservation, stack frame format, argument passing
- Strict and educational; violations are fatal errors, not warnings
- Can be disabled with `--lint false` for educational exploration

## Common Development Tasks

### Build and Run
```bash
cargo build --release
./target/release/risclet --help
```

### Run All Tests
```bash
cargo test
```

### Run Specific Test
```bash
cargo test assembler::tests::test_name -- --nocapture
cargo test encoder_tests::test_name
```

### Test Binary Execution
```bash
# Build test binaries (requires RISC-V tools and preprocessor)
cd src/test_binaries/src
make all

# Run with risclet
risclet run -e test_binaries/add
risclet debug -e test_binaries/add
```

### Assemble and Test Assembly
```bash
./target/debug/risclet assemble -o output.out input.s
./target/debug/risclet run -e output.out
```

### Check Compiler Warnings
```bash
cargo clippy
cargo check
```

### Debugging Strategy

For assembler issues:
- Use `--dump parse` to see parsed AST
- Use `--dump symbols` to see symbol resolution
- Use `--dump code` to see encoded instructions
- Test with simple files first; relaxation complexity reveals itself progressively

For simulator issues:
- Use disassemble mode to verify binary before simulation
- Add debug output to `trace()` function to see instruction effects
- Linting errors often indicate incorrect instruction sequences

## Testing Infrastructure

**Unit Tests:**
- Embedded in source files with `#[cfg(test)]` modules
- `encoder_tests.rs`: 2183 lines testing instruction encoding without relaxation
- `symbols_tests.rs`: 2454 lines testing symbol resolution and expression evaluation
- `expressions_tests.rs`: Tests immediate value expressions with operator precedence
- `parser_tests.rs`: Tests AST generation from source

**Test Binaries (src/test_binaries/):**
- 60 binary test executables covering rv32imac instruction set
- Generated from preprocessed assembly source
- Used for simulator verification (not automated; manual testing)
- Build requires `riscv64-unknown-elf-as`, `riscv64-unknown-elf-ld`, `qemu-riscv32`

## Code Style and Conventions

- Follow Rust idioms; use `cargo fmt` for formatting
- `rustfmt.toml` defines project style rules
- Error handling: custom `Result<T>` type in `error.rs` with context display
- Location tracking: `Location` struct in `ast.rs` preserves file/line for error messages
- Module organization: public APIs in module roots, private implementation details modularized

## Per-AGENTS.md Requirements

See `AGENTS.md` for critical rules on git operations, testing expectations, and handling failures. Key points:
- Never use destructive git commands without permission
- Always run tests after changes but report expectations first
- When tests fail, understand what they measure (not just mechanics) before debugging
- Never claim code is "production ready" without explicit context

## Important Files Reference

| File | Purpose | Lines |
|------|---------|-------|
| src/riscv.rs | Instruction definitions, decoding, pseudo-instruction logic | 2217 |
| src/encoder.rs | Machine code generation, relaxation encoding | 2508 |
| src/parser.rs | Recursive descent assembly parser | 1465 |
| src/ui.rs | Interactive TUI debugger | 1291 |
| src/symbols.rs | Symbol table management, expression evaluation | 932 |
| src/ast.rs | Abstract syntax tree for assembly | 1190 |
| src/execution.rs | Instruction simulation and effects | 583 |
