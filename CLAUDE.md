# CLAUDE.md - RISC-V Assembler Project

This is a single-pass RISC-V RV32IMAC assembler written in Rust. Unlike traditional assemblers that separate assembly and linking, this assembler directly produces executable ELF binaries from assembly source.

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
./target/debug/assembler program.s                        # Outputs to a.out
./target/debug/assembler -o prog program.s                # Outputs to prog
./target/debug/assembler -t 0x10000 program.s             # Set text start address
./target/debug/assembler -v program.s                     # Show input stats and convergence progress
./target/debug/assembler --no-relax program.s             # Disable all relaxations
./target/debug/assembler --no-relax-gp program.s          # Disable GP-relative addressing
./target/debug/assembler --no-relax-pseudo program.s      # Disable call/tail optimization
./target/debug/assembler --no-relax-compressed program.s  # Disable RV32C auto-encoding
./target/debug/assembler --dump-ast program.s             # Dump AST in s-expression format
./target/debug/assembler --dump-symbols program.s         # Dump symbol table
./target/debug/assembler --dump-code program.s            # Dump generated machine code

# Options
-o <file>                Write output to <file> (default: a.out)
-t <address>             Set text start address (default: 0x10000)
-v, --verbose            Show input statistics and convergence progress
--no-relax               Disable all relaxations
--relax-gp               Enable GP-relative 'la' optimization (default: on)
--no-relax-gp            Disable GP-relative 'la' optimization
--relax-pseudo           Enable 'call'/'tail' pseudo-instruction optimization (default: on)
--no-relax-pseudo        Disable 'call'/'tail' pseudo-instruction optimization
--relax-compressed       Enable automatic RV32C compressed encoding (default: on)
--no-relax-compressed    Disable automatic RV32C compressed encoding
--dump-ast[=PASSES[:FILES]] Dump AST after parsing (s-expression format)
--dump-symbols[=PASSES[:FILES]] Dump after symbol resolution with references
--dump-values[=PASSES[:FILES]]  Dump symbol values for specific passes/files
--dump-code[=PASSES[:FILES]]    Dump generated code for specific passes/files
--dump-elf[=PARTS]       Dump detailed ELF info
-h, --help               Show help message

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
   - Three types of relaxation (GP-relative, pseudo-instruction, and compressed)
   - Complex forward references
   - Address-dependent code size

3. **Integrated Symbol Linking**: Symbol linking happens before code generation and uses back-patching to handle forward references. All symbols must be linked before encoding begins.

4. **Typed Expression System**: Expressions have types (Integer vs. Address) enforced at evaluation time, preventing common assembly errors like adding two addresses.

5. **Single-File Design**: All source files are processed together into a unified `Source` structure. There's no concept of separate compilation units or relocations.

### Three Types of Relaxation

The assembler supports three independent relaxation optimizations, all enabled by default:

1. **GP-Relative LA Relaxation** (`--relax-gp`)
   - The `la rd, symbol` pseudo-instruction can use `addi rd, gp, offset` when the symbol is within ±2 KiB of the global pointer
   - This saves 4 bytes per instruction (2 bytes instead of auipc+addi)
   - Requires the global pointer (x3) to be initialized correctly at runtime
   - Can be disabled with `--no-relax-gp` if gp initialization is unavailable

2. **Pseudo-Instruction Optimization** (`--relax-pseudo`)
   - The `call` and `tail` pseudo-instructions check if the target is within ±1 MiB
   - If so, they use `jal`/`j` (4 bytes) instead of `auipc+jalr` (8 bytes)
   - Works regardless of global pointer initialization
   - Can be disabled with `--no-relax-pseudo` if you need predictable instruction sizes

3. **Compressed Instruction Auto-Encoding** (`--relax-compressed`)
    - Eligible 32-bit base instructions are automatically encoded as 16-bit RV32C compressed equivalents
    - Reduces code size by ~25% through automatic instruction compression
    - Only applies to instructions with valid compressed equivalents
    - Can be disabled with `--no-relax-compressed` for debugging or predictable code size

Use `--no-relax` to disable all three optimizations at once.

### Why This Matters

If you're used to GNU assembler, note these differences:

- **No `.o` files**: This assembler doesn't create relocatable object files
- **No separate linking phase**: All files are assembled together; `.global` symbols are linked immediately
- **No relocations**: All addresses are concrete after assembly
- **Strict expression typing**: `label1 + label2` is a type error (can't add two addresses)
- **Three configurable relaxations**: GP-relative addressing, pseudo-instruction optimization, and compressed instruction encoding

---

## Module Structure

The assembler is organized into focused modules with clear responsibilities:

### `src/main.rs` - Entry Point and Orchestration
**What it does**: Coordinates the overall assembly pipeline
- Parses command-line arguments (options for output, text start, relaxations, dumps, etc.)
- Reads source files and tokenizes each line
- Orchestrates parsing, symbol resolution, convergence, and ELF generation
- Handles dump options for debugging (--dump-ast, --dump-symbols, --dump-code, --dump-elf)

**Key data flow**:
```
Command-line args → process_cli_args → Config
                                         ↓
Input files → process_files → tokenize → parse → Source
                                                   ↓
                              link_symbols (symbols.rs)
                                                   ↓
                         converge_and_encode (assembler.rs)
                                                   ↓
                          ELF generation (elf.rs)
                                                   ↓
                    Output binary or debug dumps
```

### `src/dump.rs` - Debug Output and Introspection
**What it does**: Provides visibility into intermediate assembly states for debugging
- Dumps AST in s-expression format (--dump-ast)
- Dumps symbol table with cross-references (--dump-symbols)
- Dumps expression values at specific convergence passes (--dump-values)
- Dumps generated machine code bytes (--dump-code)
- Dumps ELF binary structure (--dump-elf)

**Key features**:
- Pass filtering: Can dump specific passes or ranges (e.g., `--dump-ast=1-3:file.s`)
- File filtering: Can filter output to specific source files
- Doesn't generate output file when dump options are used (for clean debugging)
- Supports detailed inspection of symbol resolution, expression evaluation, and code generation

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

### `src/symbols.rs` - Symbol Linking
**What it does**: Links symbol references to their definitions using back-patching

**Linking strategy**:
1. **Local pass**: Link symbols within each file
   - Build a `definitions` map as symbols are encountered
   - For backward references, immediately link to definition
   - For forward references, add to `unresolved` list
   - When a definition appears, link all pending forward references
2. **Global pass**: Link cross-file references using `.global` symbols
3. **Validation**: Ensure all references are linked (or error)

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
- 5-bit unsigned: 0-31 (shift amounts for slli, srli, srai)

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

### `src/encoder_compressed.rs` - Compressed Instruction Encoding
**What it does**: Encodes 16-bit (2-byte) RV32C compressed instructions

**Key responsibilities**:
- Encodes all RV32C instruction formats (CR, CI, CL, CS, CA, CB, CJ, CIW)
- Handles compressed register constraints (x8-x15 for most formats)
- Validates immediates within smaller compressed ranges (6-bit, 9-bit, etc.)
- Separates instruction formatting logic from relaxation mechanisms

**Supported instruction families**:
- CR format: add, mv, jr, jalr (full register set)
- CI format: li, addi, addi16sp, slli, lwsp (various immediate ranges)
- CL/CS formats: lw, sw operations with compressed registers
- CA format: and, or, xor, sub on compressed registers
- CB format: beqz, bnez, srli, srai, andi with conditional logic
- CJ format: j, jal with limited range

**Note**: This module is separate from encoder.rs to isolate compressed instruction logic and ease maintenance of the expanding C extension support.

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

### 4. **Symbol Linking** (`main.rs:main`)
```rust
symbols::link_symbols(&mut source)?;
```

**Per-file pass**:
- Build `definitions` map while scanning lines
- Link backward references immediately
- Accumulate forward references in `unresolved` list
- When definition appears, patch all pending references

**Cross-file pass**:
- Collect `.global` symbols into `source.global_symbols`
- Link cross-file references

**Output**: Every `Line` has `outgoing_refs` populated with pointers to definitions.

### 5. **Convergence Loop** (`assembler.rs:converge_and_encode`)
```rust
let (text_bytes, data_bytes, bss_size) =
    assembler::converge_and_encode(&mut source, &symbols, text_start, &relax, &callback, show_progress)?;
```

The convergence loop iteratively refines instruction sizes until stable:

```rust
for iteration in 0..MAX_ITERATIONS {
    compute_offsets(source);                     // Step 1: Assign addresses
    let mut eval_context = new_evaluation_context(...);  // Step 2: Set up evaluation
    evaluate_line_symbols(source, &eval_context)?;       // Step 3: Compute symbol values
    callback.on_values_computed(...);                    // Step 3b: Debug callback
    encode_source(&source, &eval_context, &relax)?;     // Step 4: Generate code
    callback.on_code_generated(...);                     // Step 4b: Debug callback
    if !any_sizes_changed { return (text_bytes, data_bytes, bss_size); }  // Step 5
}
```

**Step 1: compute_offsets**
Assigns addresses to every line within its segment, based on current size guesses.

**Step 2: Create evaluation context**
Sets up symbol lookup and expression evaluation. Segments are computed as:
```
text_start = (user-provided, default 0x10000)
data_start = align_to_4k(text_start + text_size)
bss_start = data_start + data_size
```

**Step 3: Evaluate symbol values**
Computes concrete addresses for all labels in all files based on current offsets.

**Step 4: Encode and track size changes**
For each line:
1. Encode instruction/directive using final expression values
2. Compare actual byte count to `line.size` estimate
3. If different: update `line.size`, set `any_sizes_changed = true`

**Step 5: Check convergence**
If any sizes changed, loop again with updated sizes. Otherwise, return the final code.

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
    header_size: u32,              // ELF header size estimate
    text_size: u32,                // Total .text size
    data_size: u32,                // Total .data size
    bss_size: u32,                 // Total .bss size
    global_symbols: Vec<GlobalDefinition>,  // Cross-file symbols
}
```

### `SourceFile` - One Input File
```rust
struct SourceFile {
    file: String,                  // Filename
    lines: Vec<Line>,              // All lines from this file
    text_size: u32,                // This file's contribution to .text
    data_size: u32,                // This file's contribution to .data
    bss_size: u32,                 // This file's contribution to .bss
    local_symbols: Vec<SymbolDefinition>,  // (currently unused)
}
```

### `Line` - One Parsed Line
```rust
struct Line {
    location: Location,            // file.s:42
    content: LineContent,          // Label | Instruction | Directive
    segment: Segment,              // Text | Data | Bss
    offset: u32,                   // Byte offset within segment
    size: u32,                     // Byte size of generated code/data
    outgoing_refs: Vec<SymbolReference>,  // Symbols this line references
}
```

### `EvaluationContext` - Expression Evaluator State
```rust
struct EvaluationContext {
    source: Source,                             // Complete program (for symbol lookup)
    symbols: Symbols,                           // Symbol information from resolution
    symbol_values: HashMap<SymbolReference, EvaluatedValue>,  // Memoization cache
    text_start: u32,                            // Segment start addresses
    data_start: u32,
    bss_start: u32,
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

With `--relax-pseudo` enabled (default), if `func` is far, `call` is 8 bytes. If it's close, it relaxes to 4 bytes during convergence. Disable with `--no-relax-pseudo` to always use 8 bytes.

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

## C Extension (Compressed Instructions)

The assembler supports the RISC-V Compressed extension (RV32C/RV64C) with 16-bit instruction encodings that reduce code size by ~25%.

### Supported Instructions

**CR Format (two register, full register set x1-x31):**
- `c.add rd, rs2` - Compressed add (rd += rs2)
- `c.mv rd, rs2` - Compressed move (rd = rs2, pseudo-instruction)
- `c.jr rs1` - Compressed jump register (PC = rs1)
- `c.jalr rs1` - Compressed jump and link register (ra = PC+2; PC = rs1)

**CI Format (register + immediate, various ranges):**
- `c.li rd, imm` - Load immediate (rd = imm, 6-bit signed: -32 to 31)
- `c.addi rd, imm` - Compressed add immediate (rd += imm, 6-bit signed, rd != x0)
- `c.addi16sp sp, imm` - Adjust stack pointer (sp += imm, 10-bit signed << 4, range [-512, 496])
- `c.addi4spn rd', imm` - Adjust stack pointer + load (rd' = sp + imm, 10-bit zero-extended << 2)
- `c.slli rd, shamt` - Compressed shift left logical immediate (rd <<= shamt, 6-bit shift, rd != x0)
- `c.lwsp rd, offset(sp)` - Load word from stack (rd = mem[sp + offset], offset 10-bit << 2)

**CIW Format (compressed register + wide immediate):**
- `c.addi4spn rd', imm` - Load word from stack-relative address

**CL Format (compressed register load, x8-x15 only):**
- `c.lw rd', offset(rs1')` - Load word (4-byte aligned offset, rd' and rs1' in {x8-x15})

**CS Format (compressed register store, x8-x15 only):**
- `c.sw rs2', offset(rs1')` - Store word (4-byte aligned offset, rs2' and rs1' in {x8-x15})
- `c.swsp rs2, offset(sp)` - Store word to stack

**CA Format (compressed arithmetic, x8-x15 only):**
- `c.and rd', rs2'` - Compressed logical AND (rd' &= rs2')
- `c.or rd', rs2'` - Compressed logical OR (rd' |= rs2')
- `c.xor rd', rs2'` - Compressed logical XOR (rd' ^= rs2')
- `c.sub rd', rs2'` - Compressed subtract (rd' -= rs2')

**CB Format (compressed branch/immediate, x8-x15 only):**
- `c.beqz rs1', offset` - Branch if equal to zero (9-bit signed even offset, ±256 bytes)
- `c.bnez rs1', offset` - Branch if not equal to zero (9-bit signed even offset, ±256 bytes)
- `c.srli rd', shamt` - Shift right logical immediate (rd' >>= shamt, 6-bit shift amount)
- `c.srai rd', shamt` - Shift right arithmetic immediate (rd' >>= shamt (sign-extend), 6-bit shift amount)
- `c.andi rd', imm` - Compressed logical AND with immediate (rd' &= imm, 6-bit signed)

**CJ Format (unconditional jump):**
- `c.j offset` - Unconditional jump (11-bit signed even offset, ±2 KiB)
- `c.jal offset` - Jump and link (rd=ra, 11-bit signed even offset, ±2 KiB) [RV32C only, not in RV64C]

**Special Instructions (no operands):**
- `c.nop` - Compressed no-operation (0x0001, typically used for alignment)
- `c.ebreak` - Compressed environment break (debugger breakpoint)

### Instruction Encoding Details

**Compressed Register Set:**
- x8-x15 (s0, s1, a0-a5) - used as "rd'" and "rs1'" and "rs2'" in most formats
- Encoded as 3-bit values 0-7 instead of 5-bit register numbers

**Offset/Immediate Encoding:**

| Format | Field | Range | Encoding | Notes |
|--------|-------|-------|----------|-------|
| CI (li/addi) | imm | -32 to 31 | 6-bit signed | bits [5:0] |
| CI (addi16sp) | imm | -512 to 496 | 10-bit << 4 | bits [9:4] |
| CI (slli/c.lwsp) | offset/imm | varies | varies | 10-bit values << 2 for loads |
| CL/CS | offset | 0-124 | 7-bit << 2 (words) | bits [5:3] + [2] + [6] scattered |
| CB (branch) | offset | ±256 | 9-bit signed even | bits [8] + [4:3] + [7:6] + [2:1] |
| CB (shifts) | shamt | 0-63 | 6-bit unsigned | bits [5:0] |
| CJ | offset | ±2048 | 11-bit signed even | bits [10:1] |

**Bit Layout:**

```
CR format:  funct4[15:12] | rd[11:7] | rs2[6:2] | op[1:0]
CI format:  funct3[15:13] | imm[12]  | rd[11:7] | imm[6:2] | op[1:0]
CL/CS:      funct3[15:13] | imm[5:3] | rs1[9:7] | imm[2] imm[6] | rd/rs2[4:2] | op[1:0]
CA format:  funct6[15:10] | rd[9:7] | funct2[6:5] | rs2[4:2] | op[1:0]
CB format:  funct3[15:13] | offset[8] | offset[4:3] | rs1[9:7] | offset[7:6] | offset[2:1] | op[1:0]
CJ format:  funct3[15:13] | offset[10:1] | op[1:0]
```

### Compressed Register Constraints

Most compressed instructions can only use the "compressed register set": x8, x9, x10, x11, x12, x13, x14, x15 (s0, s1, a0-a5). This is denoted with a prime (') in assembly, though the assembler accepts either notation:

```asm
c.lw s0, 0(a0)         # Using ABI names (compressed registers only)
c.lw x8, 0(x10)        # Using numeric names (must be x8-x15)
c.lw a0, 0(a0)         # Error: a0 (x10) is OK, but a0 is x10, not in compressed set... wait

# Actually a0 IS x10 which IS in the compressed set (x10 = a0)
# So let me fix this example:
c.lw x16, 0(a0)        # Error: x16 is not in compressed register set
c.lw s2, 0(a0)         # Error: s2 (x18) is not in compressed register set
```

Non-compressed-register instructions (CR format: c.add, c.mv, c.jr, c.jalr) can use the full register set (x1-x31).

### Examples

```asm
# Arithmetic with compressed registers
c.add a0, a1         # a0 += a1 (both must be x10-x15)
c.addi a0, 5         # a0 += 5
c.li t0, 10          # t0 = 10

# Loads and stores (compressed registers only)
c.lw a0, 0(sp)       # Load word at sp + 0
c.sw a0, 4(sp)       # Store word at sp + 4

# Branches (9-bit range ±256)
c.beqz a0, skip      # if (a0 == 0) jump to skip
skip: c.nop

# Unconditional jump (11-bit range ±2 KiB)
c.j begin

# Move between any registers
c.add x1, x2         # x1 = x2 (CR format allows full register set)
c.mv x1, x2          # Shorthand for c.add x1, x2
```

### Auto-Relaxation

**Enabled by default:** The assembler automatically compresses eligible base instructions to their 2-byte equivalents during the convergence loop:

- `addi x8, x8, 5` → `c.addi x8, 5` (saves 2 bytes)
- `addi x10, x0, 10` → `c.li x10, 10` (saves 2 bytes)
- `add x8, x8, x9` → `c.add x8, x9` (saves 2 bytes)

This automatic compression reduces code size while maintaining correct semantics. The convergence loop automatically re-layouts the program if instructions shrink from 4 to 2 bytes.

**Disabling compression:** Use the `--no-relax-compressed` flag to disable automatic compression:
```bash
./target/debug/assembler --no-relax-compressed program.s    # Produces standard 4-byte instructions
```

Or use `--no-relax` to disable all relaxations:
```bash
./target/debug/assembler --no-relax program.s    # Disables GP-relative, pseudo, and compressed relaxations
```

This is useful for debugging, targeting platforms without the C extension, or when you explicitly want full-sized base instructions instead of compressed equivalents.

### Common Pitfalls

1. **Register Constraints:** Most compressed instructions require the compressed register set (x8-x15). Attempting to use other registers will result in a parse error.

2. **Immediate Ranges:** Compressed immediates are much smaller (6-bit signed for most) than base instruction immediates (12-bit). If your immediate doesn't fit, use the base instruction instead:
   ```asm
   c.addi a0, 50        # Error: 50 > 31 (6-bit signed max)
   addi a0, a0, 50      # OK: base instruction
   ```

3. **Offset Alignment:** Load/store offsets must be 4-byte aligned (for c.lw/c.sw). The offset is automatically divided by 4 during encoding:
   ```asm
   c.lw a0, 4(sp)       # OK: offset 4 = 1 * 4
   c.lw a0, 5(sp)       # Error: 5 not divisible by 4
   ```

4. **Branch Ranges:** Compressed branches have only 9 bits of offset, limiting range to ±256 bytes. For longer jumps, use base instructions:
   ```asm
   c.beqz a0, near      # OK for targets within ±256 bytes
   beq a0, x0, far      # OK for any target (32-bit offset)
   ```

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
