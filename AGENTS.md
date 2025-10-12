# AGENTS.md - RISC-V Assembler Project

## Build/Lint/Test Commands

### Building
- `cargo build` - Compile the assembler
- `cargo check` - Check for compilation errors without building
- `cargo run <file.s>` - Run assembler on a specific assembly file (now includes symbol table building, expression evaluation reports, and ELF output generation)
- Use `riscv64-unknown-elf-objdump -d <output.elf>` to disassemble and verify generated ELF files
- Use `riscv64-unknown-elf-readelf -a <output.elf>` to inspect ELF sections, symbols, and headers

### Formatting & Linting
- `cargo fmt` - Format all Rust code according to standard conventions
- `cargo clippy` - Run the Rust linter to check for code quality issues

### Testing
- `cargo test` - Run unit tests (includes tests for Phase 2: symbol table and expression evaluation)
- Integration tests: Run `cargo run` on each `.s` file in `tests/` directory
- All test files should parse successfully without errors
- Example: `find tests -name "*.s" -exec cargo run {} \;`

## Code Style Guidelines
- Standard Rust conventions

### Error Handling
- All errors are fatal (fail fast) and tied to input lines with context (7 lines with error in middle)

### Code Structure
- Use comprehensive documentation comments explaining grammar rules
- Prefer early returns for error conditions

### Formatting
- Use `cargo fmt` for consistent formatting
- Files end with newline
