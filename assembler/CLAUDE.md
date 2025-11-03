# CLAUDE.md - RISC-V Assembler Project

Single-pass RISC-V RV32IMAC assembler in Rust. Directly produces executable ELF binaries (no object files or separate linker).

## Quick Reference

```bash
cargo build                          # Compile
cargo test                           # Run tests
cargo run -- [options] <file.s>...  # Assemble to ELF

# Key options
-o <file>                Output file (default: a.out)
-t <address>             Text start address (default: 0x10000)
-v, --verbose            Show relaxation progress
--no-relax               Disable all relaxations
--no-relax-gp            Disable GP-relative 'la' optimization
--no-relax-pseudo        Disable 'call'/'tail' optimization
--no-relax-compressed    Disable RV32C auto-encoding
--dump-ast               Dump AST (s-expression format)
--dump-symbols           Dump symbol table with references
--dump-code              Dump machine code bytes
--dump-elf               Dump ELF structure
```

## Architecture

### Key Design Choices
1. **Direct ELF generation** - No `.o` files, no separate linker
2. **Relaxation-based layout** - Iteratively refines instruction sizes until stable
3. **Integrated symbol linking** - Back-patching handles forward references before encoding
4. **Typed expressions** - Integer vs. Address types prevent errors (e.g., `addr1 + addr2` is invalid)
5. **Combined assembly and linking** - Source files are assembled and linked together (but local and global scopes are still supported) outputing an ELF binary

### Three Relaxation Types (all enabled by default)
1. **GP-relative LA** (`--relax-gp`): `la rd, symbol` → `addi rd, gp, offset` when within ±2 KiB of gp
2. **Pseudo-instruction** (`--relax-pseudo`): `call`/`tail` → `jal` (4 bytes) when within ±1 MiB, else `auipc+jalr` (8 bytes), `la` and `li`, etc.
3. **Compressed** (`--relax-compressed`): Auto-encode eligible 32-bit instructions as 16-bit RV32C equivalents

## Module Structure

### `src/main.rs` - Entry Point
Minimal entry point that parses CLI args and invokes the main assembly driver.

**Data flow:**
```
CLI args → process_cli_args (config.rs) → Config
                          ↓
        drive_assembler (assembler.rs)
                          ↓
Input files → tokenize → parse → Source
                          ↓
            link_symbols (symbols.rs) → SymbolLinks
                          ↓
         create_initial_layout (layout.rs) → Layout
                          ↓
         relaxation_loop (assembler.rs)
                          ↓
              ELF generation (elf.rs) → Output file
```

### `src/config.rs` - Configuration & CLI Parsing
Handles command-line argument parsing and assembler configuration.

**Key types:**
- `Config`: Complete assembler configuration (input/output files, relaxation flags, dump options, etc.)
- `Relax`: Individual relaxation settings (gp, pseudo, compressed)
- `DumpConfig`: Debug dump configuration (passed to dump.rs)

**Key functions:**
- `process_cli_args()`: Parse command-line arguments → Config
- `Relax::all()`: Enable all optimizations
- `Relax::none()`: Disable all optimizations

### `src/ast.rs` - Data Structures
**Key types:**
- `Source` / `SourceFile` / `Line`: AST hierarchy
- `LineContent`: `Label`, `Instruction`, or `Directive`
- `Instruction`: R/I/B/U/J-type, LoadStore, Pseudo, Special variants
- `Expression`: Recursive type for assembly-time arithmetic with type safety
- `LinePointer`: References a specific line (file_index, line_index)
- `SymbolReference`: Links symbol name to definition point

### `src/tokenizer.rs` - Lexical Analysis
Converts source text → `Vec<Token>` (Register, Integer, Identifier, Directive, Operator, etc.)
- Handles RISC-V registers (ABI names `a0` and numeric `x10`)
- Integer literals: binary `0b`, octal `0o`, decimal, hex `0x`, character `'x'`
- String literals with escape sequences
- Strips comments (after `#`)

### `src/parser.rs` - Syntax Analysis
Recursive descent parser with backtracking. Builds AST from tokens.

**Expression grammar** (lowest to highest precedence):
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

Splits lines with labels into separate entries. Desugars some pseudo-instructions immediately (e.g., `ret` → `jalr x0, ra, 0`).

### `src/layout.rs` - Layout Management
Manages address layout separately from AST.

**Key types:**
```rust
struct Layout {
    lines: HashMap<LinePointer, LineLayout>,
    header_size: u32,
    text_size: u32, data_size: u32, bss_size: u32,
}

struct LineLayout {
    segment: Segment,    // Text | Data | Bss
    offset: u32,         // Byte offset within segment
    size: u32,           // Byte size of generated code/data
}
```

**Key functions:**
- `guess_line_size(content)`: Initial conservative size estimates
- `compute_offsets(source, layout)`: Assigns offsets based on current sizes (called each iteration)
- `create_initial_layout(source)`: Creates layout before sizes are fully known

### `src/assembler.rs` - Assembly Driver & Relaxation Loop
Orchestrates the complete assembly pipeline with phase checkpoints and relaxation.

**Key functions:**
- `drive_assembler(config)`: Main entry point - coordinates all 4 phases (Parse → SymbolLinking → Relaxation → ELF)
- `relaxation_loop(config, source, symbol_links, layout)`: Iteratively refines instruction sizes until stable (max 10 iterations)
- `process_files(files)`: Read and parse input files into AST
- `print_input_statistics()`: Display source statistics before relaxation

**Phase checkpoints:**
Each of the 4 phases has a dump checkpoint that can optionally exit early:
1. **Parse**: After tokenization/parsing → `--dump-ast`
2. **SymbolLinking**: After linking symbols → `--dump-symbols`
3. **Relaxation**: During encoding iterations → `--dump-values`, `--dump-code`
4. **ELF**: After binary generation → `--dump-elf`

**Relaxation loop:**
```rust
for iteration in 0..MAX_ITERATIONS {
    compute_offsets(source, layout);     // 1. Assign addresses based on current size guesses
    eval_symbol_values(source, ...);     // 2. Compute all symbol values upfront
    encode(config, source, ...);         // 3. Generate code, update line sizes
    if !any_changed { return (text, data, bss); }  // 4. Check if sizes stabilized
}
```

Handles size changes from relaxations (pseudo-instructions shrinking from 8→4 bytes, compressed instructions 4→2 bytes).

### `src/symbols.rs` - Symbol Linking
Links symbol references to definitions using back-patching.

**Strategy:**
1. **Local pass**: Link symbols within each file (backward refs immediately, forward refs queued)
2. **Global pass**: Link cross-file references via `.global` symbols
3. **Validation**: Ensure all refs linked

**Special handling:**
- Numeric labels (`1:`, `2:`) flushed at non-numeric labels or segment boundaries
- `.equ` can redefine symbols (later definitions shadow earlier)
- `__global_pointer$` filtered (handled during evaluation)

**Output:** Populates each `Line`'s `outgoing_refs` with `SymbolReference` entries.

### `src/expressions.rs` - Expression Evaluation
Evaluates expressions with type checking.

**Type system:**
```rust
enum ValueType { Integer, Address }
```

**Type rules:**
- `Integer ± Integer` → `Integer`
- `Address ± Integer` → `Address`
- `Address - Address` → `Integer` (distance)
- `Address + Address` → **ERROR**
- `Integer - Address` → **ERROR**
- Multiply/divide/bitwise require `Integer` operands

**Strategy:** Lazy evaluation with memoization and cycle detection.

**Special symbols:**
- `.` (current address) → `segment_start + current_line.offset`
- `__global_pointer$` → `data_start + 2048`

**Address computation:**
```
text_start = (user-provided, default 0x10000)
data_start = align_to_4k(text_start + text_size)
bss_start  = data_start + data_size
label_addr = segment_start + line.offset
```

### `src/encoder.rs` - Code Generation
Translates AST → machine code bytes with integrated relaxation support.

**Architecture:**
- Unified encoder with relaxation variants inline (no separate compilation phases)
- All instruction families handled in one place with compression/optimization checks
- Tracks size changes to drive relaxation loop convergence

**Key functions:**
- `encode(config, source, symbol_links, symbol_values, layout)`: Main entry point
  - Returns `(any_changed, text_bytes, data_bytes, bss_size)`
  - Updates line sizes in layout if relaxations change size
- `encode_line()`: Encodes a single line (instruction, directive, or label)
- Per-instruction-type encoders (called internally)

**Instruction formats:**
- `encode_r_type`: R-type (add, sub, etc.)
- `encode_i_type`: I-type (addi, lw, etc.)
- `encode_s_type`: S-type (stores)
- `encode_b_type`: B-type (branches)
- `encode_u_type`: U-type (lui, auipc)
- `encode_j_type`: J-type (jal)

**Immediate validation:**
- 12-bit signed: -2048 to 2047 (I/S-type)
- 13-bit signed even: ±4 KiB (branches)
- 21-bit signed even: ±1 MiB (jal)
- 20-bit unsigned: 0 to 0xFFFFF (U-type)
- 5-bit unsigned: 0-31 (shifts)

**Key pseudo-instructions:**
- `li rd, imm`: Load immediate (expands to addi or lui+addiw)
- `la rd, symbol`: Load address (GP-relative if within ±2 KiB, else auipc+addi)
- `call target`: Call function (jal if within ±1 MiB, else auipc+jalr)
- `tail target`: Tail call (j if within ±1 MiB, else auipc+jalr)

**Data directives:**
- `.byte`/`.2byte`/`.4byte`/`.8byte`: Emit little-endian integers
- `.string`/`.asciz`: Emit UTF-8 bytes (asciz adds null terminator)
- `.space n`: Emit n zero bytes
- `.balign n`: Align to n-byte boundary

### `src/encoder_compressed.rs` - Compressed Instructions
Encodes 16-bit RV32C instructions (CR, CI, CL, CS, CA, CB, CJ, CIW formats).

**Register constraints:** Most formats require compressed register set (x8-x15).

**Supported:**
- CR: `c.add`, `c.mv`, `c.jr`, `c.jalr` (full register set)
- CI: `c.li`, `c.addi`, `c.slli`, `c.lwsp`, `c.addi16sp`, `c.addi4spn`
- CL/CS: `c.lw`, `c.sw`, `c.swsp` (compressed registers)
- CA: `c.and`, `c.or`, `c.xor`, `c.sub` (compressed registers)
- CB: `c.beqz`, `c.bnez`, `c.srli`, `c.srai`, `c.andi` (compressed registers)
- CJ: `c.j`, `c.jal` (RV32C only)
- Special: `c.nop`, `c.ebreak`

### `src/dump.rs` - Debug Output
Provides introspection for debugging with flexible filtering by passes and files.

**Key types:**
- `DumpConfig`: Complete dump configuration (created by config.rs)
- `PassRange`: Filtering for relaxation passes (Final, Specific, Range, From, UpTo, All)
- `FileSelection`: Filtering for source files (All or Specific)
- `DumpSpec`: Pass + file filtering specification
- `ElfDumpParts`: Select which ELF parts to dump (headers, sections, symbols)

**Dump functions:**
- `dump_ast()`: AST in s-expression format (after parsing)
- `dump_symbols()`: Symbol table with cross-references (after linking)
- `dump_values()`: Symbol values at specific passes (during relaxation)
- `dump_code()`: Generated machine code bytes (during relaxation)
- `dump_elf()`: ELF structure details (before output)

**Filtering syntax:**
- Passes: empty (final), `N` (specific), `N-M` (range), `N-` (from N), `-M` (up to M), `*` (all)
- Files: empty (all), `file1.s,file2.s` (comma-separated list)
- Example: `--dump-code=1-3:file1.s,file2.s` dumps code from passes 1-3 for specific files

### `src/error.rs` - Error Reporting
Displays errors with 7-line context (3 before, error line marked `>>>`, 3 after).

## Assembly Pipeline

1. **CLI Parsing** (`config.rs:process_cli_args`): Command-line arguments → `Config`
2. **Tokenization** (`assembler.rs:process_file`): Source lines → `Vec<Token>`
3. **Parsing** (`assembler.rs:process_file`): Tokens → AST (`Source`)
4. **Symbol Linking** (`symbols.rs:link_symbols`): Connect symbol uses to definitions
5. **Initial Layout** (`layout.rs:create_initial_layout`): Guess sizes, compute initial offsets
6. **Relaxation Loop** (`assembler.rs:relaxation_loop`): Iteratively refine sizes until stable
   - Compute addresses based on current size guesses
   - Evaluate all symbol values
   - Encode all lines (generating code and tracking size changes)
   - Repeat if any sizes changed, exit if stable or max iterations reached
7. **ELF Generation** (`elf.rs:ElfBuilder`): Build ELF binary with program headers and symbol table
8. **File Output** (`assembler.rs:drive_assembler`): Write executable to disk and set permissions

## Key Data Structures

```rust
// Configuration (from config.rs)
struct Config {
    input_files: Vec<String>,
    output_file: String,
    text_start: u32,
    verbose: bool,
    dump: DumpConfig,
    relax: Relax,
}

struct Relax {
    gp: bool,       // Enable GP-relative la optimization
    pseudo: bool,   // Enable call/tail pseudo-instruction optimization
    compressed: bool, // Enable automatic RV32C compressed encoding
}

// AST and Layout
struct Source {
    files: Vec<SourceFile>,
}

struct SourceFile {
    file: String,
    lines: Vec<Line>,
}

struct Line {
    location: Location,
    content: LineContent,
    outgoing_refs: Vec<SymbolReference>,
}

struct Layout {
    lines: HashMap<LinePointer, LineLayout>,
    text_size: u32, data_size: u32, bss_size: u32,
}

// Evaluation context (used during relaxation)
type SymbolValues = HashMap<SymbolReference, EvaluatedValue>;
```

## A Extension (Atomic Instructions)

**Load-Reserved/Store-Conditional:**
- `lr.w rd, (rs1)` - Load reserved
- `sc.w rd, rs2, (rs1)` - Store conditional (rd = 0 on success)

**Atomic Memory Operations:**
- `amoswap.w`, `amoadd.w`, `amoxor.w`, `amoand.w`, `amoor.w`
- `amomin.w`, `amomax.w`, `amominu.w`, `amomaxu.w`

**Memory ordering suffixes:** `.aq`, `.rel`, `.aqrl`

**Encoding:** R-type format with AMO opcode (0b0101111), funct5 specifies operation, aq/rl bits set ordering.

## C Extension (Compressed Instructions)

16-bit instruction encodings, ~25% code size reduction.

**Auto-relaxation (enabled by default):** Eligible 32-bit instructions automatically encoded as 16-bit equivalents during relaxation.

**Compressed register set:** x8-x15 (s0, s1, a0-a5) for most formats. CR format (c.add, c.mv, c.jr, c.jalr) uses full register set (x1-x31).

**Common pitfalls:**
- Immediate ranges much smaller (6-bit vs 12-bit)
- Load/store offsets must be 4-byte aligned
- Branch range only ±256 bytes (vs ±4 KiB for base)

## Testing

```bash
cargo test                           # Run all unit tests
find tests -name "*.s" -exec cargo run {} \;  # Integration tests
```

Key test modules: `symbols.rs::tests`, `expressions.rs::tests`, `encoder.rs::tests`, `parser.rs::tests`, `tokenizer.rs::tests`

## Extending the Assembler

**Add instruction:** Update `ast.rs` (add opcode variant), `parser.rs` (parse), `encoder.rs` (encode)

**Add directive:** Update `ast.rs` (DirectiveOp), `tokenizer.rs` (recognize), `parser.rs` (parse operands), `encoder.rs` (generate bytes)

**Add pseudo-instruction:** Update `ast.rs` (PseudoOp variant), `parser.rs` (parse), `encoder.rs` (expand to base instructions)

**Add CLI option:** Update `config.rs` (add field to Config, parse argument), use config throughout pipeline

**Add debug dump:** Update `dump.rs` (add DumpSpec variant to DumpConfig, implement dump function), update `config.rs` (parse --dump-* option)
