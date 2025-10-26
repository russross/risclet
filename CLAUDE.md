# CLAUDE.md - RISC-V Assembler Project

This is a single-pass RISC-V RV64IM assembler written in Rust. Unlike traditional assemblers that separate assembly and linking, this assembler directly produces executable ELF binaries from assembly source.

## Quick Reference

### Build/Lint/Test
```bash
cargo build                          # Compile the assembler
cargo check                          # Fast syntax/type checking
cargo run -- [options] <file.s>...  # Assemble files and generate ELF binary
cargo test                           # Run all unit tests
cargo fmt                            # Format code
cargo clippy                         # Lint code

# Usage examples
./target/debug/assembler program.s                  # Outputs to a.out
./target/debug/assembler -o prog program.s          # Outputs to prog
./target/debug/assembler -t 0x10000 program.s       # Set text start address
./target/debug/assembler -v program.s               # Verbose output with listing

# Options
-o <file>        Write output to <file> (default: a.out)
-t <address>     Set text start address (default: 0x10000)
-v, --verbose    Show detailed assembly listing
-h, --help       Show help message

# Verify output with GNU binutils
riscv64-unknown-elf-objdump -d a.out
riscv64-unknown-elf-readelf -a a.out
```

### Code Style
- Standard Rust conventions, enforced by `rustfmt` and `clippy`
- Fail-fast error handling with rich context (shows 7 lines around error)
- Comprehensive documentation comments, especially for grammar rules
- Files end with newline

---

## Architecture Overview

### What Makes This Assembler Different

This assembler has several unique design choices that distinguish it from typical two-pass assemblers (like GNU `as` + `ld`):

1. **Direct ELF Generation**: No intermediate object files or separate linker. The assembler outputs a complete, executable ELF binary.

2. **Convergence-Based Layout**: Instead of making multiple discrete passes, the assembler iteratively refines instruction sizes until they stabilize. This naturally handles:
   - Relaxation (pseudo-instructions can shrink from 8 to 4 bytes)
   - Complex forward references
   - Address-dependent code size

3. **Integrated Symbol Resolution**: Symbol resolution happens before code generation and uses back-patching to handle forward references. All symbols must be resolved before encoding begins.

4. **Typed Expression System**: Expressions have types (Integer vs. Address) enforced at evaluation time, preventing common assembly errors like adding two addresses.

5. **Single-File Design**: All source files are processed together into a unified `Source` structure. There's no concept of separate compilation units or relocations.

### Why This Matters

If you're used to GNU assembler, note these differences:

- **No `.o` files**: This assembler doesn't create relocatable object files
- **No separate linking**: All files are assembled together; `.global` symbols are resolved immediately
- **No relocations**: All addresses are concrete after assembly
- **Strict expression typing**: `label1 + label2` is a type error (can't add two addresses)
- **Built-in relaxation**: Pseudo-instructions automatically use the smallest encoding

---

## Module Structure

The assembler is organized into focused modules with clear responsibilities:

### `src/main.rs` - Entry Point and Orchestration
**What it does**: Coordinates the overall assembly pipeline
- Parses command-line arguments (-o, -t, -v flags)
- Reads source files and tokenizes each line
- Orchestrates parsing, symbol resolution, convergence, and ELF generation
- Formats and displays the final assembled output (concise or verbose)

**Key data flow**:
```
Command-line args → parse_args → Config
                                    ↓
Input files → process_files → tokenize → parse → Source
                                                    ↓
                              resolve_symbols (symbols.rs)
                                                    ↓
                          converge_and_encode (assembler.rs)
                                                    ↓
                           ELF generation (elf.rs)
                                                    ↓
                         Output binary + summary/listing
```

### `src/ast.rs` - Data Structures
**What it does**: Defines the core type system for the entire assembler

**Key types**:
- `Source` / `SourceFile` / `Line`: The complete AST hierarchy
- `LineContent`: Can be a `Label`, `Instruction`, or `Directive`
- `Instruction`: Enum with variants for R/I/B/U/J-type, LoadStore, Pseudo, Special
- `Expression`: Recursive type for assembly-time arithmetic with type safety
- `Segment`: Text, Data, or BSS
- `LinePointer`: References a specific line (file_index, line_index)
- `SymbolReference`: Links a symbol name to its definition point

**Important distinctions**:
- Instructions are fully type-checked at parse time (no "generic instruction" representation)
- Expressions are AST nodes, not immediately evaluated
- Each `Line` tracks its segment, offset, size, and outgoing symbol references

### `src/tokenizer.rs` - Lexical Analysis
**What it does**: Converts source text into tokens
- Handles RISC-V register names (both ABI names like `a0` and numeric like `x10`)
- Recognizes directives (`.text`, `.global`, etc.)
- Parses integer literals (binary `0b`, octal `0o`, decimal, hex `0x`, character constants `'x'`)
- Parses string literals with escape sequences (`\n`, `\t`, `\\`, etc.)
- Recognizes operators (`+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `|`, `&`, `^`, `~`)
- Strips comments (everything after `#`)

**Output**: `Vec<Token>` where each token is strongly typed (Register, Integer, Identifier, Directive, Operator, etc.)

**Note**: Identifiers can contain alphanumeric characters, underscores, dots, and dollar signs (`_`, `.`, `$`)

### `src/parser.rs` - Syntax Analysis
**What it does**: Builds an Abstract Syntax Tree from tokens

**Parser design**:
- Recursive descent parser with backtracking for ambiguous grammar
- Handles operator precedence for expressions (C-style precedence)
- Splits lines with labels into separate `Line` entries (one for label, one for content)
- Desugars some pseudo-instructions immediately (like `ret` → `jalr x0, ra, 0`)

**Expression grammar** (from lowest to highest precedence):
```
expression     := bitwise_or
bitwise_or     := bitwise_xor ('|' bitwise_xor)*
bitwise_xor    := bitwise_and ('^' bitwise_and)*
bitwise_and    := shift ('&' shift)*
shift          := additive ('<<' | '>>' additive)*
additive       := multiplicative ('+' | '-' multiplicative)*
multiplicative := unary ('*' | '/' | '%' unary)*
unary          := '-' | '~' | operand
operand        := literal | identifier | '(' expression ')' | '.'
```

**Notable features**:
- Load/store syntax `ld a0, offset(base)` requires backtracking to distinguish from parenthesized expressions
- Numeric labels (`1f`, `2b`) are part of the expression grammar
- Current address (`.`) is a first-class expression operand

### `src/assembler.rs` - Layout and Convergence
**What it does**: Manages address assignment and size convergence

**Key functions**:

#### `compute_offsets(source: &mut Source)`
Assigns offsets to every line based on current size guesses. Maintains separate offset counters for text/data/bss segments. This is called repeatedly during convergence.

#### `guess_line_size(content: &LineContent) -> i64`
Provides initial size estimates:
- Pseudo-instructions: 8 bytes (conservative worst-case)
- Regular instructions: 4 bytes
- Directives: Based on content (`.byte` → 1 per value, `.space` → expression value, etc.)

#### `converge_and_encode(source: &mut Source, text_start: i64) -> (Vec<u8>, Vec<u8>, i64)`
**The heart of the assembler**. Iteratively refines sizes until stable:
```rust
loop {
    compute_offsets(source);              // Assign addresses
    create_evaluation_context();          // Set up for expression eval
    encode_source_with_size_tracking();   // Generate code, update sizes
    if !any_sizes_changed { break; }      // Converged!
}
```

**Why convergence?** Pseudo-instructions can use different encodings depending on address:
- `call` uses `jal` (4 bytes) if target is within ±1 MiB
- `call` uses `auipc + jalr` (8 bytes) otherwise
- Changing from 8→4 bytes shifts subsequent addresses, which might enable more relaxations

### `src/symbols.rs` - Symbol Resolution
**What it does**: Links symbol references to their definitions using back-patching

**Resolution strategy**:
1. **Local pass**: Resolve symbols within each file
   - Build a `definitions` map as symbols are encountered
   - For backward references, immediately link to definition
   - For forward references, add to `unresolved` list
   - When a definition appears, resolve all pending forward references
2. **Global pass**: Resolve cross-file references using `.global` symbols
3. **Validation**: Ensure all references are resolved (or error)

**Key features**:
- Numeric labels (`1:`, `2:`) have special scoping rules:
  - Flushed when crossing a non-numeric label or segment boundary
  - `1f` always refers to the next `1:` label
  - `1b` always refers to the most recent `1:` label
- `.equ` can redefine symbols (later definitions shadow earlier ones)
- `.global` exports symbols for cross-file references
- Special symbol `__global_pointer$` is filtered out (handled during expression evaluation)

**Output**: Each `Line`'s `outgoing_refs` field is populated with `SymbolReference` entries pointing to definition sites.

### `src/expressions.rs` - Expression Evaluation
**What it does**: Evaluates expressions to concrete values with type checking

**Type system**:
```rust
enum ValueType { Integer, Address }

struct EvaluatedValue {
    value: i64,
    value_type: ValueType,
}
```

**Type rules** (strictly enforced):
- Integer + Integer → Integer
- Address + Integer → Address  ✓
- Integer + Address → Address  ✓
- Address - Address → Integer  ✓  (computes distance)
- Address - Integer → Address  ✓
- Integer - Address → **ERROR** ✗
- Address + Address → **ERROR** ✗
- Multiply/divide/bitwise ops → require Integer operands

**Evaluation strategy**:
- **Lazy**: Symbols are evaluated only when needed
- **Memoized**: Results are cached in `EvaluationContext.symbol_values`
- **Cycle detection**: Tracks evaluation stack to detect circular `.equ` definitions

**Evaluation context**:
```rust
pub struct EvaluationContext {
    source: Source,                              // Complete AST
    symbol_values: HashMap<SymbolKey, EvaluatedValue>,  // Memoization
    text_start: i64,
    data_start: i64,
    bss_start: i64,
}
```

**Label addresses**:
```
label_address = segment_start + line.offset
```
Where `segment_start` is computed as:
```
text_start = (user-provided, default 0x10000)
data_start = next 4K page after (text_start + text_size)
bss_start  = data_start + data_size
```

**Special symbols**:
- `.` (current address) → `segment_start + current_line.offset`
- `__global_pointer$` → `data_start + 2048`

**Precision checks**:
- Left/right shifts error if they would lose non-zero bits
- Arithmetic operations check for overflow/underflow
- Division/modulo check for zero divisor

### `src/encoder.rs` - Code Generation
**What it does**: Translates AST instructions into machine code bytes

**Encoding pipeline**:
```
Instruction → evaluate operand expressions → validate immediates → emit bytes
```

**Key responsibilities**:

#### Instruction Encoding
Each instruction format has a dedicated encoder:
- `encode_r_type`: R-type (opcode, funct3, funct7, rd, rs1, rs2)
- `encode_i_type`: I-type (opcode, funct3, rd, rs1, imm[11:0])
- `encode_s_type`: S-type (stores: opcode, funct3, rs1, rs2, imm[11:0] split)
- `encode_b_type`: B-type (branches: opcode, funct3, rs1, rs2, imm[12:1] scrambled)
- `encode_u_type`: U-type (opcode, rd, imm[31:12])
- `encode_j_type`: J-type (jal: opcode, rd, imm[20:1] scrambled)

#### Immediate Validation
Strict bounds checking before encoding:
- 12-bit signed: -2048 to 2047 (I-type, S-type)
- 13-bit signed even: ±4 KiB (branches)
- 21-bit signed even: ±1 MiB (jal)
- 20-bit unsigned: 0 to 0xFFFFF (U-type)
- 6-bit unsigned: 0-63 (RV64 shifts)
- 5-bit unsigned: 0-31 (RV64W shifts)

#### Pseudo-Instruction Expansion

**`li rd, imm`** (Load Immediate):
- If fits in 12 bits: `addi rd, x0, imm`
- If fits in 32 bits: `lui rd, upper; addiw rd, rd, lower`
- 64-bit values: Not yet implemented (error)

**`la rd, symbol`** (Load Address):
- If `symbol` within ±2KiB of `gp`: `addi rd, gp, offset`
- Otherwise: `auipc rd, hi; addi rd, rd, lo`
- Special case: `la gp, __global_pointer$` always uses PC-relative

**`call target`** (Call Function) - **WITH RELAXATION**:
- If target within ±1 MiB: `jal ra, offset` (4 bytes)
- Otherwise: `auipc ra, hi; jalr ra, ra, lo` (8 bytes)

**`tail target`** (Tail Call) - **WITH RELAXATION**:
- If target within ±1 MiB: `jal x0, offset` (4 bytes)
- Otherwise: `auipc t1, hi; jalr x0, t1, lo` (8 bytes)

**Global load/store**:
- `lb rd, symbol` → `auipc rd, hi; lb rd, lo(rd)`
- `sb rs, symbol, temp` → `auipc temp, hi; sb rs, lo(temp)`

#### Data Directives
- `.byte`/`.2byte`/`.4byte`/`.8byte`: Evaluate expressions, emit little-endian
- `.string`: Raw UTF-8 bytes
- `.asciz`: UTF-8 bytes + null terminator
- `.space n`: Emit `n` zero bytes
- `.balign n`: Emit zero bytes until next `n`-byte boundary

#### BSS Segment Handling
BSS can only contain:
- Labels (size 0)
- `.space` (tracks size, no bytes)
- `.balign` (tracks padding, no bytes)
- Non-data directives (`.global`, `.equ`)

Data directives (`.byte`, `.string`, etc.) in `.bss` → error

### `src/error.rs` - Error Reporting
**What it does**: Provides rich, contextual error messages with source context

**Data structure**:
```rust
pub struct AssemblerError {
    pub location: Location,  // file:line
    pub message: String,
}
```

**Error format**:
```
Error at file.s:42: <error message>
    40: previous line
    41: previous line
>>> 42: instruction causing error
    43: next line
    44: next line
```

Shows 3 lines before and after the error (7 total), with the error line marked with `>>>`.

**Features**:
- Reads source file to display context
- Handles file read failures gracefully
- Implements `Display` trait for pretty-printing

---

## Assembly Pipeline Walkthrough

Here's what happens when you run `./assembler program.s`:

### 1. **File Reading and Tokenization** (`main.rs:process_file`)
```rust
for line in file {
    let tokens = tokenizer::tokenize(line)?;  // String → Vec<Token>
}
```

Each source line becomes a token stream. Comments are stripped, literals are parsed, registers are identified.

### 2. **Parsing** (`main.rs:process_file`)
```rust
let parsed_lines = parser::parse(&tokens, file_path, line_num)?;
```

Tokens become AST nodes. A line like `loop: beq a0, a1, loop` produces TWO `Line` entries:
1. `LineContent::Label("loop")`
2. `LineContent::Instruction(BType(Beq, X10, X11, Identifier("loop")))`

### 3. **Size Guessing** (`main.rs:process_file`)
```rust
new_line.size = assembler::guess_line_size(&new_line.content)?;
```

Initial size estimates:
- `beq` → 4 bytes
- `call` → 8 bytes (conservative)
- `.byte 1, 2, 3` → 3 bytes

### 4. **Symbol Resolution** (`main.rs:main`)
```rust
symbols::resolve_symbols(&mut source)?;
```

**Per-file pass**:
- Build `definitions` map while scanning lines
- Resolve backward references immediately
- Accumulate forward references in `unresolved` list
- When definition appears, patch all pending references

**Cross-file pass**:
- Collect `.global` symbols into `source.global_symbols`
- Resolve cross-file references

**Output**: Every `Line` has `outgoing_refs` populated with pointers to definitions.

### 5. **Convergence Loop** (`main.rs:main`)
```rust
let (text_bytes, data_bytes, bss_size) =
    assembler::converge_and_encode(&mut source, text_start)?;
```

```rust
for iteration in 0..MAX_ITERATIONS {
    compute_offsets(source);                    // Step A
    let mut eval_context = new_evaluation_context(source, text_start);  // Step B
    let (encoded, any_changed) = encode_source_with_size_tracking(source, eval_context);  // Step C
    if !any_changed { return encoded; }         // Step D
}
```

**Step A: compute_offsets**
```rust
// Assign addresses based on current size guesses
text_offset = 0
for line in lines {
    line.offset = text_offset
    text_offset += line.size
}
```

**Step B: create evaluation context**
```rust
// Set up segment addresses
text_start = 0x10000  (user-provided)
data_start = align_to_4k(text_start + text_size)
bss_start = data_start + data_size
```

**Step C: encode and track changes**
For each line:
1. Encode instruction/directive
2. Compare actual byte count to `line.size`
3. If different: update `line.size`, set `any_changed = true`

**Step D: check convergence**
If any sizes changed, discard the encoded bytes and loop again with updated sizes.

**Example convergence**:
```
Iteration 0:
  0x10000: call far_func (guess 8 bytes)
  0x10008: nop
  ...
  0x20000: far_func:

Iteration 1:
  call's offset = 0x20000 - 0x10000 = 0x10000 (64 KiB, within ±1MiB)
  → relaxes to 'jal' (4 bytes)
  Sizes changed! Loop again.

Iteration 2:
  0x10000: call far_func (now 4 bytes after relaxation)
  0x10004: nop (offset shifted by -4)
  ...
  0x1FFFC: far_func: (offset shifted by -4)

  call's offset = 0x1FFFC - 0x10000 = 0xFFFC (still within ±1MiB)
  → stays as 'jal' (4 bytes)
  No size changes. Converged!
```

### 6. **ELF Binary Generation** (`main.rs`, `elf.rs`)
```rust
let mut elf_builder = elf::ElfBuilder::new(text_start);
elf_builder.set_segments(text_bytes, data_bytes, bss_size, ...);
elf::build_symbol_table(&source, &mut elf_builder, ...);
let elf_bytes = elf_builder.build(entry_point);
// Write to output file
```

**Symbol table generation**:
1. Null symbol (entry 0)
2. Section symbols (.text, .data, .bss if present)
3. For each source file:
   - FILE symbol
   - Special $xrv64i2p1_m2p0 marker symbol
   - Local labels from that file
4. Linker-provided globals (__global_pointer$, __SDATA_BEGIN__, etc.)
5. User-defined global symbols

**Program headers**:
- RISCV_ATTRIBUTES segment (non-allocating)
- LOAD segment for .text (read + execute)
- LOAD segment for .data + .bss (read + write)

### 7. **Output Display** (`main.rs:dump_source_with_values`)
**Concise mode** (default):
```
a.out: text=24 data=0 bss=0 total=4352
```

**Verbose mode** (`-v` flag):
```
Source (text: 24, data: 0, bss: 0)
SourceFile: program.s (text: 24, data: 0, bss: 0)
  00010000: 6f 00 c0 ff          jal      ra, far_func  # 0x1fffc
  00010004: 13 00 00 00          nop
  ...
  0001fffc: 67 80 00 00          ret
  Exported symbols:
    main -> [0:0]
```

---

## Key Data Structures

### `Source` - The Complete Program
```rust
struct Source {
    files: Vec<SourceFile>,        // All input files
    text_size: i64,                // Total .text size
    data_size: i64,                // Total .data size
    bss_size: i64,                 // Total .bss size
    global_symbols: Vec<GlobalDefinition>,  // Cross-file symbols
}
```

### `SourceFile` - One Input File
```rust
struct SourceFile {
    file: String,                  // Filename
    lines: Vec<Line>,              // All lines from this file
    text_size: i64,                // This file's contribution to .text
    data_size: i64,                // This file's contribution to .data
    bss_size: i64,                 // This file's contribution to .bss
    local_symbols: Vec<SymbolDefinition>,  // (currently unused)
}
```

### `Line` - One Parsed Line
```rust
struct Line {
    location: Location,            // file.s:42
    content: LineContent,          // Label | Instruction | Directive
    segment: Segment,              // Text | Data | Bss
    offset: i64,                   // Byte offset within segment
    size: i64,                     // Byte size of generated code/data
    outgoing_refs: Vec<SymbolReference>,  // Symbols this line references
}
```

### `EvaluationContext` - Expression Evaluator State
```rust
struct EvaluationContext {
    source: Source,                             // Complete program (for symbol lookup)
    symbol_values: HashMap<SymbolKey, EvaluatedValue>,  // Memoization cache
    text_start: i64,                            // Segment addresses
    data_start: i64,
    bss_start: i64,
}
```

---

## Common Pitfalls and Gotchas

### 1. **No Object Files**
This assembler doesn't create `.o` files. If you're used to:
```bash
as -o program.o program.s
ld -o program program.o
```

With this assembler:
```bash
cargo run program.s    # Directly creates executable
```

There's no equivalent to `ld` – linking happens during assembly.

### 2. **All Files Are Linked Together**
```bash
cargo run file1.s file2.s file3.s
```

All three files are assembled as one unit. Symbols don't need to be `.global` unless they're referenced across files.

### 3. **Expression Type Errors**
```asm
start:
    nop
end:
    .equ offset, end - start   # ✓ OK: Address - Address = Integer
    .equ bad, start + end      # ✗ ERROR: Cannot add Address + Address
```

### 4. **Numeric Labels Have Limited Scope**
```asm
1:
    nop
regular_label:
    beq a0, a1, 1b   # ✗ ERROR: numeric labels flushed by non-numeric label
```

Numeric labels are flushed when:
- A non-numeric label is encountered
- Segment changes (`.text` → `.data`)
- File ends

### 5. **Pseudo-Instructions May Shrink**
Don't rely on pseudo-instruction sizes being stable:
```asm
start:
    call func    # Could be 4 or 8 bytes depending on distance
    nop
func:
    ret
```

If `func` is far, `call` is 8 bytes. If it's close, it relaxes to 4 bytes during convergence.

### 6. **BSS Is Zero-Only**
```asm
.bss
buffer: .space 1024   # ✓ OK
value: .byte 42       # ✗ ERROR: Can't initialize data in .bss
```

Use `.data` for initialized data, `.bss` for zero-initialized.

---

## A Extension (Atomic Instructions)

The assembler supports the RISC-V Atomic extension (RV32A) with Load-Reserved/Store-Conditional and Atomic Memory Operations.

### Supported Instructions

**Load-Reserved / Store-Conditional (word):**
- `lr.w rd, (rs1)` - Load reserved word
- `sc.w rd, rs2, (rs1)` - Store conditional word (rd receives status: 0=success, 1=failure)

**Atomic Memory Operations (word):**
- `amoswap.w rd, rs2, (rs1)` - Atomic swap (load, store rs2, return old value)
- `amoadd.w rd, rs2, (rs1)` - Atomic add
- `amoxor.w rd, rs2, (rs1)` - Atomic XOR
- `amoand.w rd, rs2, (rs1)` - Atomic AND
- `amoor.w rd, rs2, (rs1)` - Atomic OR
- `amomin.w rd, rs2, (rs1)` - Atomic min (signed)
- `amomax.w rd, rs2, (rs1)` - Atomic max (signed)
- `amominu.w rd, rs2, (rs1)` - Atomic min (unsigned)
- `amomaxu.w rd, rs2, (rs1)` - Atomic max (unsigned)

### Memory Ordering Annotations

All atomic operations support optional suffixes for memory ordering:
- `.aq` - Acquire semantics (load-acquire)
- `.rel` - Release semantics (store-release)
- `.aqrl` - Both acquire and release (full barrier)

**Examples:**
```asm
# Load-reserved with acquire semantics
lr.w.aq a0, (a1)

# Store-conditional with full barrier
sc.w.aqrl a0, a2, (a1)

# Atomic swap with release semantics
# Note: .rel is not supported by some assemblers; use .aqrl for full barrier
amoswap.w.aqrl a0, a2, (a1)
```

### Implementation Details

**Encoding:**
- Uses R-type format with special AMO opcode (0b0101111)
- funct3 = 010 for word (32-bit) operations
- funct5 (bits 31-27): Specifies the atomic operation
- aq bit (bit 26): Acquire flag
- rl bit (bit 25): Release flag

**Semantics:**
- `lr.w` loads a value from memory and reserves the address
- `sc.w` stores a value to reserved address; writes 0 to rd if successful, 1 if failed
- AMO instructions perform the operation, returning the original value to rd
- Memory ordering affects visibility of the atomic operation across processors

---

## Testing

### Unit Tests
Each module has inline tests (`#[cfg(test)]`):
```bash
cargo test
```

Key test modules:
- `src/symbols.rs::tests`: Symbol resolution (200+ test cases)
- `src/expressions.rs::tests`: Expression evaluation and type checking
- `src/encoder.rs::tests`: Instruction encoding
- `src/parser.rs::tests`: Parsing edge cases
- `src/tokenizer.rs::tests`: Tokenization

### Integration Tests
```bash
# Test all `.s` files in tests/ directory
find tests -name "*.s" -exec cargo run {} \;
```

Each test file should assemble without errors. Output is compared manually or with `objdump`.

---

## Extending the Assembler

### Adding a New Instruction

1. **Add opcode variant** (`src/ast.rs`):
```rust
pub enum RTypeOp {
    // ... existing ...
    NewInst,
}
```

2. **Add parsing** (`src/parser.rs`):
```rust
"newinst" => self.parse_rtype(RTypeOp::NewInst),
```

3. **Add encoding** (`src/encoder.rs`):
```rust
fn encode_r_type_inst(...) {
    let (opcode, funct3, funct7) = match op {
        // ... existing ...
        NewInst => (0b0110011, 0b010, 0b0000001),
    };
}
```

### Adding a New Directive

1. **Add to `DirectiveOp`** (`src/ast.rs`):
```rust
pub enum DirectiveOp {
    // ... existing ...
    NewDirective,
}
```

2. **Tokenize** (`src/tokenizer.rs`):
```rust
".newdirective" => Some(DirectiveOp::NewDirective),
```

3. **Parse** (`src/parser.rs`):
```rust
DirectiveOp::NewDirective => { /* parse operands */ }
```

4. **Encode** (`src/encoder.rs`):
```rust
Directive::NewDirective => { /* generate bytes */ }
```

### Adding a New Pseudo-Instruction

1. **Add variant** (`src/ast.rs`):
```rust
pub enum PseudoOp {
    // ... existing ...
    NewPseudo(Register, Box<Expression>),
}
```

2. **Parse** (`src/parser.rs`):
```rust
"newpseudo" => {
    let rd = self.parse_register()?;
    let expr = self.parse_expression()?;
    Ok(Instruction::Pseudo(PseudoOp::NewPseudo(rd, Box::new(expr))))
}
```

3. **Expand** (`src/encoder.rs`):
```rust
PseudoOp::NewPseudo(rd, expr) => {
    let val = eval_expr(expr, line, context.eval_context)?;
    // Emit base instruction(s)
}
```
