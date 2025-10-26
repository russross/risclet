# RISC-V Assembler Codebase - Exploration Summary

**Date**: 2025-10-17  
**Repository**: /home/russ/assembler  
**Branch**: master  
**Total Lines of Code**: 13,248 (across 13 Rust source files)

---

## 1. Source Files Overview

### Core Modules (src/*.rs)

| File | LOC | Purpose |
|------|-----|---------|
| `ast.rs` | 966 | Core AST type definitions (Instruction, Directive, Expression, Register enums, Source/Line structures) |
| `main.rs` | 603 | CLI argument parsing, file processing orchestration, assembly pipeline coordination |
| `symbols.rs` | 2,697 | Symbol resolution with back-patching, local and global symbol management |
| `encoder.rs` | 1,286 | Machine code generation, instruction encoding, pseudo-instruction expansion |
| `encoder_tests.rs` | 1,620 | Comprehensive unit tests for instruction encoding and edge cases |
| `expressions.rs` | 1,586 | Expression evaluation with type checking (Integer vs Address), lazy evaluation, cycle detection |
| `parser.rs` | 1,287 | Recursive descent parser, operator precedence handling, ambiguity resolution |
| `assembler.rs` | 301 | Core assembly pipeline (offset computation, convergence loop, size guessing) |
| `elf.rs` | 1,060 | ELF binary format generation, header structures, symbol table construction |
| `tokenizer.rs` | 405 | Lexical analysis, register/directive/operator recognition, literal parsing |
| `dump.rs` | 1,344 | Debug output utilities (AST, symbol, value, code, ELF dumps) |
| `error.rs` | 81 | Error reporting with source context (7-line window display) |
| `lib.rs` | 12 | Library exports |

**Total: 13,248 lines**

---

## 2. Key Type Definitions

### EvaluatedValue (expressions.rs)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluatedValue {
    pub value: i64,
    pub value_type: ValueType,  // Integer or Address
}
```

**Type System Rules**:
- Integer + Integer = Integer
- Address + Integer = Address (and commutative)
- Address - Address = Integer
- Address - Integer = Address
- Integer - Address = ERROR
- Address + Address = ERROR
- Multiply/divide/bitwise: require Integer operands

### Line (ast.rs)
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub location: Location,              // file.s:42
    pub content: LineContent,            // Label | Instruction | Directive
    pub segment: Segment,                // Text | Data | Bss
    pub offset: i64,                     // Byte offset within segment
    pub size: i64,                       // Byte size of generated code/data
    pub outgoing_refs: Vec<SymbolReference>,  // Resolved symbol references
}
```

### Source (ast.rs)
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub files: Vec<SourceFile>,
    pub header_size: i64,                // ELF header + program headers size
    pub text_size: i64,
    pub data_size: i64,
    pub bss_size: i64,
    pub global_symbols: Vec<GlobalDefinition>,
}
```

### SourceFile (ast.rs)
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub file: String,
    pub lines: Vec<Line>,
    pub text_size: i64,
    pub data_size: i64,
    pub bss_size: i64,
    pub local_symbols: Vec<SymbolDefinition>,
}
```

### EvaluationContext (expressions.rs)
```rust
pub struct EvaluationContext {
    pub source: Source,                                    // Complete program AST
    symbol_values: HashMap<SymbolKey, EvaluatedValue>,   // Memoization cache
    pub text_start: i64,                                  // Segment start addresses
    pub data_start: i64,
    pub bss_start: i64,
}
```

---

## 3. RV64-Specific Instruction Opcodes

### Instruction Formats and Encodings (encoder.rs, lines 649-838)

#### R-Type Instructions (opcode 0b0110011 and 0b0111011)
**Base I Extension** (0b0110011):
- Add (funct7=0b0000000), Sub (0b0100000)
- Sll, Slt, Sltu, Xor, Srl, Sra, Or, And

**64-Bit W Extension** (0b0111011):
- Addw, Subw, Sllw, Srlw, Sraw
- All use funct7=0b0000000 or 0b0100000

**M Extension Multiply** (funct7=0b0000001):
- Mul, Mulh, Mulhsu, Mulhu (opcode 0b0110011)
- Div, Divu, Rem, Remu (opcode 0b0110011)
- Mulw, Divw, Divuw, Remw, Remuw (opcode 0b0111011)

#### I-Type Instructions (opcode 0b0010011, 0b0011011, 0b1100111)
**Arithmetic/Logical** (0b0010011):
- Addi, Slti, Sltiu, Xori, Ori, Andi
- Slli (funct3=0b001, funct7=0b0000000 in imm[11:5])
- Srli (funct3=0b101, funct7=0b0000000)
- Srai (funct3=0b101, funct7=0b0100000)

**W Variants** (0b0011011):
- Addiw, Slliw, Srliw, Sraiw

**Jump Register** (0b1100111):
- Jalr (funct3=0b000)

#### B-Type Branch Instructions (opcode 0b1100011)
- Beq (funct3=0b000)
- Bne (funct3=0b001)
- Blt, Bge (funct3=0b100, 0b101)
- Bltu, Bgeu (funct3=0b110, 0b111)

#### U-Type Instructions
- Lui (opcode 0b0110111)
- Auipc (opcode 0b0010111)

#### J-Type Instructions
- Jal (opcode 0b1101111)

#### Load/Store Instructions
**Load** (opcode 0b0000011):
- Lb, Lh, Lw, Ld (funct3=0b000-0b011)
- Lbu, Lhu, Lwu (funct3=0b100-0b110)

**Store** (opcode 0b0100011):
- Sb, Sh, Sw, Sd (funct3=0b000-0b011)

#### Special Instructions
- Ecall, Ebreak (both funct7=0b0000000, funct3=0b000, rd=rs1=0)

---

## 4. ELF Header Structures (elf.rs)

### Elf64Header
```rust
pub struct Elf64Header {
    pub e_ident: [u8; 16],          // ELF identification
    pub e_type: u16,                // Object file type (ET_EXEC=2)
    pub e_machine: u16,             // Machine type (EM_RISCV=0xF3)
    pub e_version: u32,             // Object file version (EV_CURRENT=1)
    pub e_entry: u64,               // Entry point address
    pub e_phoff: u64,               // Program header offset
    pub e_shoff: u64,               // Section header offset
    pub e_flags: u32,               // Processor flags (EF_RISCV_FLOAT_ABI_DOUBLE=0x4)
    pub e_ehsize: u16,              // ELF header size (64 bytes)
    pub e_phentsize: u16,           // Program header entry size (56 bytes)
    pub e_phnum: u16,               // Number of program headers
    pub e_shentsize: u16,           // Section header entry size (64 bytes)
    pub e_shnum: u16,               // Number of section headers
    pub e_shstrndx: u16,            // Section name string table index
}
```

**Fixed Values**:
- e_ident[0:4]: 0x7f 'E' 'L' 'F' (magic)
- e_ident[4]: 0x2 (ELFCLASS64)
- e_ident[5]: 0x1 (ELFDATA2LSB - little endian)
- e_ident[6]: 0x1 (EV_CURRENT)
- e_ident[7]: 0x0 (ELFOSABI_SYSV)
- e_ident[8]: 0x0 (ABI version)
- e_ehsize: 64 (fixed)
- e_phentsize: 56 (fixed)
- e_shentsize: 64 (fixed)

### Elf64ProgramHeader
```rust
pub struct Elf64ProgramHeader {
    pub p_type: u32,       // Segment type (PT_LOAD=1, PT_RISCV_ATTRIBUTES=0x7000_0003)
    pub p_flags: u32,      // Segment flags (PF_R=0x4, PF_W=0x2, PF_X=0x1)
    pub p_offset: u64,     // Segment file offset
    pub p_vaddr: u64,      // Segment virtual address
    pub p_paddr: u64,      // Segment physical address
    pub p_filesz: u64,     // Segment size in file
    pub p_memsz: u64,      // Segment size in memory
    pub p_align: u64,      // Segment alignment (0x1000 for LOAD, 1 for RISCV_ATTRIBUTES)
}
```

**Program Headers Generated** (in order):
1. PT_RISCV_ATTRIBUTES (non-allocating, non-loading)
2. PT_LOAD .text (flags=PF_R|PF_X)
3. PT_LOAD .data+.bss (flags=PF_R|PF_W, if present)

### Header Size Computation
```rust
pub fn compute_header_size(num_segments: i64) -> i64 {
    const ELF_HEADER_SIZE: i64 = 64;
    const PROGRAM_HEADER_SIZE: i64 = 56;
    ELF_HEADER_SIZE + (num_segments * PROGRAM_HEADER_SIZE)
}
```

**Example**: 3 segments → 64 + (3 × 56) = 232 bytes

---

## 5. Argument Parsing and text_start Handling (main.rs)

### CLI Options Processing
```rust
struct Config {
    input_files: Vec<String>,
    output_file: String,            // -o <file> (default: a.out)
    text_start: i64,                // -t <address> (default: 0x10000)
    verbose: bool,                  // -v, --verbose
    dump: dump::DumpConfig,         // --dump-* options
}
```

### Text Start Address Handling
- **Default**: `0x10000` (65536 bytes)
- **Parsing**: `parse_address()` function (lines 188-195)
  - Recognizes hex (0x prefix) and decimal formats
  - Example: `-t 0x10000` or `-t 65536`

### Main Assembly Flow (lines 261-473)
```
1. process_cli_args() → Config
2. drive_assembler(Config)
   ├─ Phase 1: process_files() → Source AST
   ├─ Phase 2: symbols::resolve_symbols(&mut Source)
   ├─ Phase 3: assembler::converge_and_encode(&mut Source, text_start, ...)
   │          Returns: (text_bytes, data_bytes, bss_size)
   ├─ Phase 4: expressions::new_evaluation_context(Source, text_start)
   ├─ Phase 5: elf::build_symbol_table(..., eval_context)
   └─ Phase 6: Write ELF binary to output file
```

### Text Start Segment Address Calculation
In `expressions.rs` (lines 135-161):
```rust
pub fn new_evaluation_context(source: Source, text_start: i64) -> EvaluationContext {
    let text_first_instruction = text_start + source.header_size;
    let text_size = source.text_size;
    let data_size = source.data_size;
    
    // data_start = next 4K page boundary after (text_start + header_size + text_size)
    let data_start = ((text_first_instruction + text_size + 4095) / 4096) * 4096;
    
    // bss_start = immediately after data
    let bss_start = data_start + data_size;
    
    // Return with text_start adjusted for header offset
    EvaluationContext {
        text_start: text_first_instruction,  // NOT the raw text_start parameter!
        data_start,
        bss_start,
        ...
    }
}
```

**Key Point**: The `text_start` parameter is the file offset where the first instruction appears in the executable. The actual virtual address of labels depends on segment alignment and is calculated as:
- text_vaddr = text_start + offset_within_segment
- data_vaddr = ((text_vaddr + text_size + 4095) / 4096) * 4096 (4K-aligned)

---

## 6. Assembly Pipeline Summary

### Phase Flow
1. **Parse**: Tokenize → Parse lines into AST
2. **Resolve**: Link symbol uses to definitions (back-patching)
3. **Converge** (iterative):
   - Compute offsets based on size guesses
   - Evaluate symbols to populate context
   - Encode instructions, track actual sizes
   - Check if sizes changed; loop if yes
4. **Generate ELF**: Build headers, sections, symbol table
5. **Output**: Write to binary file, set executable permissions

### Convergence Loop Details (assembler.rs, lines 147-250)
```rust
pub fn converge_and_encode<C: ConvergenceCallback>(
    source: &mut Source,
    text_start: i64,
    callback: &C,
    show_progress: bool,
) -> Result<(Vec<u8>, Vec<u8>, i64)>
```

**Loop (up to 10 iterations)**:
1. `compute_offsets(source)` - Assign addresses
2. `new_evaluation_context(source, text_start)` - Set up symbol evaluator
3. `encode_source_with_size_tracking(source, eval_context, &mut any_changed)` - Generate code
4. If `any_changed`, continue; else return result

### Critical Data Structures
- **Source**: Container for all files with cumulative sizes
- **SourceFile**: One input file with its lines and segment sizes
- **Line**: Individual instruction/directive/label with offset/size/segment
- **EvaluationContext**: Symbol evaluation state (memoization + segment addresses)

---

## Current State Before Conversion

The assembler is **complete and functional**:
- ✅ RV64IM ISA support (base + M extension)
- ✅ Type-safe expression evaluation (Integer vs Address)
- ✅ Symbol resolution with proper scoping (numeric labels, .equ, .global)
- ✅ Convergence-based relaxation (pseudo-instructions size optimization)
- ✅ ELF binary generation with proper headers/sections/symbols
- ✅ Comprehensive error reporting with source context
- ✅ Debug dumps for AST, symbols, values, code, ELF
- ✅ 1,620 lines of unit tests for encoding correctness

### Key Design Patterns
1. **Lazy Evaluation**: Symbols evaluated only when needed, memoized for efficiency
2. **Type Safety**: ValueType enforced at expression evaluation time
3. **Convergence**: Handles variable-size instructions through iterative refinement
4. **Back-Patching**: Forward references resolved in second pass during symbol resolution
5. **Separation of Concerns**: Each module has single responsibility (tokenize, parse, resolve, encode, generate ELF)

---

## File Sizes Reference
- Smallest module: error.rs (81 LOC)
- Largest module: symbols.rs (2,697 LOC)
- Test suite: encoder_tests.rs (1,620 LOC)
- Average module: ~1,019 LOC

