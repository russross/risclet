# RV64IMC to RV32IMC Conversion Plan

## Overview
This plan outlines the conversion of risclet from RV64IMC (64-bit RISC-V with integer, multiplication/division, and compressed instruction support) to RV32IMC (32-bit variant). The primary changes involve:
- Reducing register width from 64 bits to 32 bits
- Changing address space from 64-bit (i64) to 32-bit (u32)
- Removing 64-bit specific instructions (e.g., ADDW, LD, SD, etc.)
- Updating ELF file parsing to handle 32-bit ELF format
- Adjusting all memory and register operations accordingly

## Instructions to Remove

### RV64-Specific R-Type Instructions (Opcode 0x3b)
- **ADDW** - Add Word (32-bit add with sign extension)
- **SUBW** - Subtract Word (32-bit subtract with sign extension)
- **SLLW** - Shift Left Logical Word
- **SRLW** - Shift Right Logical Word
- **SRAW** - Shift Right Arithmetic Word

### RV64-Specific I-Type Instructions (Opcode 0x1b)
- **ADDIW** - Add Immediate Word
- **SLLIW** - Shift Left Logical Immediate Word
- **SRLIW** - Shift Right Logical Immediate Word
- **SRAIW** - Shift Right Arithmetic Immediate Word

### RV64-Specific Load Instructions
- **LD** (Load Doubleword) - Load 64-bit value
- **LWU** (Load Word Unsigned) - Load 32-bit unsigned (zero-extended to 64 bits)

### RV64-Specific Store Instructions
- **SD** (Store Doubleword) - Store 64-bit value

### RV64-Specific M Extension Instructions
- **MULW** - Multiply Word
- **DIVW** - Divide Word
- **DIVUW** - Divide Unsigned Word
- **REMW** - Remainder Word
- **REMUW** - Remainder Unsigned Word

### RV64-Specific Compressed Instructions
- **C.LD** / **C.LDSP** - Compressed load doubleword
- **C.SD** / **C.SDSP** - Compressed store doubleword
- **C.ADDIW** - Compressed add immediate word
- **C.ADDW** / **C.SUBW** - Compressed arithmetic word operations

## Instructions That Remain (No Changes to Opcodes)

All RV32I base instructions remain:
- R-type: ADD, SUB, SLL, SLT, SLTU, XOR, SRL, SRA, OR, AND
- I-type: ADDI, SLTI, SLTIU, XORI, ORI, ANDI, SLLI, SRLI, SRAI
- Branch: BEQ, BNE, BLT, BGE, BLTU, BGEU
- Jump: JAL, JALR
- Load: LB, LH, LW, LBU, LHU
- Store: SB, SH, SW
- U-type: LUI, AUIPC
- Misc: FENCE, ECALL, EBREAK

All RV32M extension instructions remain:
- MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU

Most compressed instructions remain, except those specifically for 64-bit operations.

## Data Type Changes

### Core Type Changes

#### Memory Addresses
- **Current**: `i64` (signed 64-bit)
- **New**: `u32` (unsigned 32-bit)
- **Rationale**: RV32 uses 32-bit addressing. Using `u32` is more idiomatic for addresses and avoids sign-extension issues.

#### Register Values
- **Current**: `i64` (signed 64-bit)
- **New**: `i32` (signed 32-bit)
- **Rationale**: RV32 registers are 32 bits wide. Operations need sign/zero extension based on instruction semantics.

#### Program Counter
- **Current**: `i64`
- **New**: `u32`
- **Rationale**: Consistent with address space

#### Immediates and Offsets
- **Current**: `i64`
- **New**: `i32`
- **Rationale**: Sign-extended immediates fit in 32 bits

### Affected Structures and Functions

#### `src/memory.rs`
- `Segment`: `start`, `end` fields: `i64` → `u32`
- `MemoryLayout`: All fields (`stack_start`, `stack_end`, `data_start`, etc.): `i64` → `u32`
- `MemoryManager`: Address parameters in `load`, `store`, `load_instruction`: `i64` → `u32`
- `RegisterFile`: `x` array: `[i64; 32]` → `[i32; 32]`
  - Remove `get32()` and `set32()` methods (no longer needed)
- `CpuState`: `pc`: `i64` → `u32`

#### `src/riscv.rs`
- All immediate values in `Op` enum variants: `i64` → `i32`
- Helper functions returning immediates: `i64` → `i32`
- `get_imm_i`, `get_imm_s`, `get_imm_b`, `get_imm_u`, `get_imm_j`: `i64` → `i32`
- All compressed instruction immediate decoders: `i64` → `i32`
- `sign_extend`: Return type `i64` → `i32`

#### `src/execution.rs`
- `Machine`: `pc_start`, `global_pointer`: `i64` → `u32`
- `Instruction`: `address`, `length`: `i64` → `u32` (length can stay as `u32` or even `u8`)
- All address parameters: `i64` → `u32`
- `load_*` and `store` methods: address parameters `i64` → `u32`
- `get`/`set` register methods: `i64` → `i32`, remove `get32`/`set32`

#### `src/trace.rs`
- `MemoryValue`: `address`: `i64` → `u32`
- `RegisterValue`: `value`: `i64` → `i32`
- `Effects`: `pc`: `(i64, i64)` → `(u32, u32)`
- `function_start`, `function_end`: `Option<i64>` → `Option<u32>`

#### `src/linter.rs`
- `at_entry_sp`: `i64` → `u32`
- Stack frame addresses: `i64` → `u32`

#### `src/elf.rs`
- Complete rewrite for 32-bit ELF format
- `e_entry`: `i64` → `u32`
- All ELF header parsing changes
- `p_vaddr`, `p_offset`, etc.: Use 32-bit values
- Symbol values: `i64` → `u32`

### Type Conversion Considerations

#### Widening Operations
In RV32, some operations naturally produce wider results:
- **MULH/MULHSU/MULHU**: Produce upper 32 bits of 64-bit product
  - Implementation: Cast to `i64`/`u64`, multiply, shift right 32 bits, cast back to `i32`
- **Shifts**: In RV32, shift amounts are 5 bits (0-31), not 6 bits

#### Unsigned Operations
- **Unsigned comparisons** (SLTU, BLTU, BGEU): Cast to `u32` for comparison
- **Logical shifts** (SRL, SRLI): Cast to `u32`, shift, cast back to `i32`
- **Unsigned division/remainder** (DIVU, REMU): Cast to `u32` for operation

#### Load/Store Operations
- All load operations that sign-extend (LB, LH, LW in RV32) work naturally with `i8`, `i16`, `i32`
- All unsigned load operations (LBU, LHU) need zero-extension via `u8`, `u16` casts

## ELF File Format Changes

### Header Differences (64-bit vs 32-bit ELF)

#### ELF Header (Ehdr)
```
Field           RV64 Size    RV64 Offset    RV32 Size    RV32 Offset
e_ident[16]     16 bytes     0x00           16 bytes     0x00
e_type          2 bytes      0x10           2 bytes      0x10
e_machine       2 bytes      0x12           2 bytes      0x12
e_version       4 bytes      0x14           4 bytes      0x14
e_entry         8 bytes      0x18           4 bytes      0x18
e_phoff         8 bytes      0x20           4 bytes      0x1c
e_shoff         8 bytes      0x28           4 bytes      0x20
e_flags         4 bytes      0x30           4 bytes      0x24
e_ehsize        2 bytes      0x34           2 bytes      0x28
e_phentsize     2 bytes      0x36           2 bytes      0x2a
e_phnum         2 bytes      0x38           2 bytes      0x2c
e_shentsize     2 bytes      0x3a           2 bytes      0x2e
e_shnum         2 bytes      0x3c           2 bytes      0x30
e_shstrndx      2 bytes      0x3e           2 bytes      0x32
Total size      64 bytes                    52 bytes
```

**Key Changes**:
- `e_ident[4]` must be `1` (32-bit) instead of `2` (64-bit)
- All 64-bit addresses/offsets become 32-bit
- Header size changes from 0x40 (64) to 0x34 (52) bytes

#### Program Header (Phdr)
```
Field           RV64 Size    RV64 Offset    RV32 Size    RV32 Offset
p_type          4 bytes      0x00           4 bytes      0x00
p_flags         4 bytes      0x04           4 bytes      0x04
p_offset        8 bytes      0x08           4 bytes      0x08
p_vaddr         8 bytes      0x10           4 bytes      0x0c
p_paddr         8 bytes      0x18           4 bytes      0x10
p_filesz        8 bytes      0x20           4 bytes      0x14
p_memsz         8 bytes      0x28           4 bytes      0x18
p_align         8 bytes      0x30           4 bytes      0x1c
Total size      56 (0x38)                   32 (0x20) bytes
```

**Key Changes**:
- All 64-bit values become 32-bit
- Size reduces from 56 to 32 bytes
- Field order differs: In RV32, `p_flags` comes after `p_type` at the beginning

#### Section Header (Shdr)
```
Field           RV64 Size    RV64 Offset    RV32 Size    RV32 Offset
sh_name         4 bytes      0x00           4 bytes      0x00
sh_type         4 bytes      0x04           4 bytes      0x04
sh_flags        8 bytes      0x08           4 bytes      0x08
sh_addr         8 bytes      0x10           4 bytes      0x0c
sh_offset       8 bytes      0x18           4 bytes      0x10
sh_size         8 bytes      0x20           4 bytes      0x14
sh_link         4 bytes      0x28           4 bytes      0x18
sh_info         4 bytes      0x2c           4 bytes      0x1c
sh_addralign    8 bytes      0x30           4 bytes      0x20
sh_entsize      8 bytes      0x38           4 bytes      0x24
Total size      64 (0x40)                   40 (0x28) bytes
```

**Key Changes**:
- All 64-bit values become 32-bit
- Size reduces from 64 to 40 bytes

#### Symbol Table Entry
```
Field           RV64 Size    RV64 Offset    RV32 Size    RV32 Offset
st_name         4 bytes      0x00           4 bytes      0x00
st_info         1 byte       0x04           1 byte       0x04
st_other        1 byte       0x05           1 byte       0x05
st_shndx        2 bytes      0x06           2 bytes      0x06
st_value        8 bytes      0x08           4 bytes      0x08
st_size         8 bytes      0x10           4 bytes      0x0c
Total size      24 (0x18)                   16 (0x10) bytes
```

**Key Changes**:
- All 64-bit values become 32-bit
- Size reduces from 24 to 16 bytes

### ELF Parsing Updates Required

1. **Magic number checks** (lines 13-15 in elf.rs):
   - Change `raw[4] != 2` to `raw[4] != 1` (32-bit class)

2. **Header size validations** (line 36):
   - `e_phoff` should be `0x34` (52) instead of `0x40` (64)
   - `e_ehsize` should be `0x34` instead of `0x40`
   - `e_phentsize` should be `0x20` (32) instead of `0x38` (56)

3. **Field extraction** (lines 24-34):
   - Use appropriate offsets and sizes for 32-bit format
   - All address/offset fields are 4 bytes instead of 8

4. **Program header parsing** (lines 42-69):
   - Adjust offsets and use 32-bit reads
   - Program header size is 0x20 (32) bytes

5. **Section header parsing** (lines 72-167):
   - Adjust offsets and use 32-bit reads
   - Section header size is 0x28 (40) bytes

6. **Symbol table parsing** (lines 177-223):
   - Adjust offsets and use 32-bit reads
   - Symbol entry size is 0x10 (16) bytes

## Phase 1: Type System Updates

### Phase 1.1: Update Core Type Definitions
**File**: `src/memory.rs`

- [ ] Change `Segment` fields:
  - [ ] `start: i64` → `start: u32`
  - [ ] `end: i64` → `end: u32`
  - [ ] Update `in_range` parameter types: `addr: i64, size: i64` → `addr: u32, size: u32`
  - [ ] Update `load` parameter: `addr: i64, size: i64` → `addr: u32, size: u32`
  - [ ] Update `store` parameter: `addr: i64` → `addr: u32`

- [ ] Change `MemoryLayout` fields:
  - [ ] `stack_start: i64` → `stack_start: u32`
  - [ ] `stack_end: i64` → `stack_end: u32`
  - [ ] `data_start: i64` → `data_start: u32`
  - [ ] `data_end: i64` → `data_end: u32`
  - [ ] `text_start: i64` → `text_start: u32`
  - [ ] `text_end: i64` → `text_end: u32`
  - [ ] Update `STACK_SIZE` constant if needed (should be compatible with u32)

- [ ] Change `MemoryManager` method signatures:
  - [ ] `load(addr: i64, size: i64)` → `load(addr: u32, size: u32)`
  - [ ] `load_raw(addr: i64, size: i64)` → `load_raw(addr: u32, size: u32)`
  - [ ] `store(addr: i64, raw: &[u8])` → `store(addr: u32, raw: &[u8])`
  - [ ] `store_with_tracking(addr: i64, ...)` → `store_with_tracking(addr: u32, ...)`
  - [ ] `load_instruction(addr: i64)` → `load_instruction(addr: u32)` return `(i32, u32)`

- [ ] Change `RegisterFile`:
  - [ ] `x: [i64; 32]` → `x: [i32; 32]`
  - [ ] `get(reg: usize) -> i64` → `get(reg: usize) -> i32`
  - [ ] `set(reg: usize, value: i64)` → `set(reg: usize, value: i32)`
  - [ ] **Remove** `get32()` and `set32()` methods

- [ ] Change `CpuState`:
  - [ ] `pc: i64` → `pc: u32`
  - [ ] `get_reg(reg: usize) -> i64` → `get_reg(reg: usize) -> i32`
  - [ ] `set_reg(reg: usize, value: i64)` → `set_reg(reg: usize, value: i32)`
  - [ ] `pc() -> i64` → `pc() -> u32`
  - [ ] `set_pc(value: i64)` → `set_pc(value: u32)`
  - [ ] **Remove** `get_reg32()` and `set_reg32()` methods
  - [ ] Update `reset` signature: `reset(pc_start: i64, stack_end: i64)` → `reset(pc_start: u32, stack_end: u32)`
  - [ ] Update `push_stack_frame(frame: i64)` → `push_stack_frame(frame: u32)`
  - [ ] `stack_frames: Vec<i64>` → `stack_frames: Vec<u32>`

### Phase 1.2: Update Instruction Representation
**File**: `src/riscv.rs`

- [ ] Update immediate extraction functions to return `i32`:
  - [ ] `get_imm_i(inst: i32) -> i64` → `get_imm_i(inst: i32) -> i32`
  - [ ] `get_imm_s(inst: i32) -> i64` → `get_imm_s(inst: i32) -> i32`
  - [ ] `get_imm_b(inst: i32) -> i64` → `get_imm_b(inst: i32) -> i32`
  - [ ] `get_imm_u(inst: i32) -> i64` → `get_imm_u(inst: i32) -> i32`
  - [ ] `get_imm_j(inst: i32) -> i64` → `get_imm_j(inst: i32) -> i32`
  - [ ] `sign_extend(value: i32, width: u32) -> i64` → `sign_extend(value: i32, width: u32) -> i32`

- [ ] Update all compressed instruction immediate decoders in `define_immediate_decoders!` macro:
  - [ ] Change return type from `i64` to `i32` in macro body

- [ ] Update `Op` enum:
  - [ ] Change all `imm: i64` fields to `imm: i32`
  - [ ] Change all `offset: i64` fields to `offset: i32`
  - [ ] Change all `shamt: i64` fields to `shamt: i32`
  - [ ] **Remove** RV64-specific variants:
    - [ ] `Addw`, `Subw`, `Sllw`, `Srlw`, `Sraw`
    - [ ] `Addiw`, `Slliw`, `Srliw`, `Sraiw`
    - [ ] `Ld`, `Lwu`, `Sd`
    - [ ] `Mulw`, `Divw`, `Divuw`, `Remw`, `Remuw`

- [ ] Update `Op::new()` method:
  - [ ] **Remove** opcode 0x3b case (rv64 r-type)
  - [ ] **Remove** opcode 0x1b case (rv64 i-type)
  - [ ] **Remove** `decode_rv64_r_type()` method
  - [ ] **Remove** `decode_rv64_i_type()` method

- [ ] Update `Op::decode_load()`:
  - [ ] **Remove** funct3=3 case (LD)
  - [ ] **Remove** funct3=6 case (LWU)

- [ ] Update `Op::decode_store()`:
  - [ ] **Remove** funct3=3 case (SD)

- [ ] Update `Op::decode_compressed()`:
  - [ ] **Remove** (0, 3) case (C.LD)
  - [ ] **Remove** (0, 7) case (C.SD)
  - [ ] **Remove** (1, 1) case (C.ADDIW)
  - [ ] **Remove** (1, 4) cases for (1, 0) and (1, 1) (C.SUBW, C.ADDW)
  - [ ] **Remove** (2, 3) case (C.LDSP)
  - [ ] **Remove** (2, 7) case (C.SDSP)

- [ ] Update shift amount masking in `decode_i_type()`:
  - [ ] Change `shamt = imm & 0x3f` to `shamt = imm & 0x1f` (5-bit shift amount for RV32)
  - [ ] Change `imm_high = imm >> 6` to `imm_high = imm >> 5`
  - [ ] Update `imm_high` comparison from `0x00` and `0x10` to `0x00` and `0x20`

### Phase 1.3: Update Instruction Execution
**File**: `src/riscv.rs` (continued)

- [ ] Update `Op::execute()` signature:
  - [ ] `execute(&self, m: &mut Machine, length: i64)` → `execute(&self, m: &mut Machine, length: u32)`

- [ ] Update register operations to use `i32`:
  - [ ] All `m.get()` calls now return `i32`
  - [ ] All `m.set()` calls now take `i32`
  - [ ] **Remove** all `m.get32()` and `m.set32()` calls

- [ ] Update shift operations for RV32:
  - [ ] Sll: Change mask from `0x3f` to `0x1f`
  - [ ] Srl: Change mask from `0x3f` to `0x1f`, cast to `u32` for shift
  - [ ] Sra: Change mask from `0x3f` to `0x1f`
  - [ ] Slli, Srli, Srai: Shift amount already correct from immediate decoding

- [ ] Update multiply operations for RV32:
  - [ ] Mulh: Cast to `i64`, multiply, shift right 32 bits, cast to `i32`
  - [ ] Mulhsu: Similar 64-bit math
  - [ ] Mulhu: Cast to `u64`, multiply, shift right 32 bits, cast to `i32`

- [ ] Update load operations:
  - [ ] Load functions now work with `u32` addresses
  - [ ] Lw: Returns `i32`, set directly (no need for sign extension from i64)
  - [ ] **Remove** Ld and Lwu execution cases

- [ ] Update store operations:
  - [ ] **Remove** Sd execution case
  - [ ] Store addresses are now `u32`

- [ ] Update branch/jump operations:
  - [ ] PC arithmetic uses `u32`
  - [ ] Ensure no overflow issues with signed immediates added to unsigned PC

- [ ] **Remove** execution cases for all RV64-specific instructions:
  - [ ] Addw, Subw, Sllw, Srlw, Sraw
  - [ ] Addiw, Slliw, Srliw, Sraiw
  - [ ] Ld, Lwu, Sd
  - [ ] Mulw, Divw, Divuw, Remw, Remuw

- [ ] Update `Op::to_fields()` and `Op::to_pseudo_fields()`:
  - [ ] **Remove** cases for RV64-specific instructions
  - [ ] Update `Imm(i64)` to `Imm(i32)` in Field enum

- [ ] Update `Op::to_string()` signature:
  - [ ] `to_string(&self, pc: i64, gp: i64, ...)` → `to_string(&self, pc: u32, gp: u32, ...)`

- [ ] Update `Op::branch_target()`:
  - [ ] Return type: `Option<i64>` → `Option<u32>`
  - [ ] PC arithmetic: `i64` → `u32`

- [ ] Update `Field` enum:
  - [ ] `Imm(i64)` → `Imm(i32)`
  - [ ] `Indirect(i64, usize)` → `Indirect(i32, usize)`
  - [ ] `PCRelAddr(i64)` → `PCRelAddr(i32)`
  - [ ] `GPRelAddr(i64)` → `GPRelAddr(i32)`

- [ ] Update `Field::to_string()`:
  - [ ] `to_string(&self, pc: i64, gp: i64, ...)` → `to_string(&self, pc: u32, gp: u32, ...)`

- [ ] Update helper functions:
  - [ ] `fields_to_string(..., pc: i64, gp: i64, ...)` → `fields_to_string(..., pc: u32, gp: u32, ...)`
  - [ ] `get_pseudo_sequence(...)` address handling

## Phase 2: Execution Engine Updates

### Phase 2.1: Update Machine Structure
**File**: `src/execution.rs`

- [ ] Update `Machine` fields:
  - [ ] `pc_start: i64` → `pc_start: u32`
  - [ ] `global_pointer: i64` → `global_pointer: u32`
  - [ ] `address_symbols: HashMap<i64, String>` → `HashMap<u32, String>`

- [ ] Update `Machine` method signatures:
  - [ ] `new(..., pc_start: i64, global_pointer: i64, address_symbols: HashMap<i64, String>, ...)` → use `u32`
  - [ ] `load(addr: i64, size: i64)` → `load(addr: u32, size: u32)`
  - [ ] `load_i8(addr: i64)` → `load_i8(addr: u32)` return `i32`
  - [ ] `load_u8(addr: i64)` → `load_u8(addr: u32)` return `i32`
  - [ ] `load_i16(addr: i64)` → `load_i16(addr: u32)` return `i32`
  - [ ] `load_u16(addr: i64)` → `load_u16(addr: u32)` return `i32`
  - [ ] `load_i32(addr: i64)` → `load_i32(addr: u32)` return `i32`
  - [ ] `load_u32(addr: i64)` → `load_u32(addr: u32)` return `i32` (this becomes redundant, essentially same as load_i32)
  - [ ] **Remove** `load_i64()` and `load_u64()`
  - [ ] `load_instruction(addr: i64)` → `load_instruction(addr: u32)` return `(i32, u32)`
  - [ ] `store(addr: i64, raw: &[u8])` → `store(addr: u32, raw: &[u8])`
  - [ ] `get(reg: usize) -> i64` → `get(reg: usize) -> i32`
  - [ ] `set(reg: usize, value: i64)` → `set(reg: usize, value: i32)`
  - [ ] **Remove** `get32()` and `set32()`
  - [ ] `set_pc(value: i64)` → `set_pc(value: u32)`
  - [ ] `text_start() -> i64` → `text_start() -> u32`
  - [ ] `text_end() -> i64` → `text_end() -> u32`
  - [ ] `data_start() -> i64` → `data_start() -> u32`
  - [ ] `data_end() -> i64` → `data_end() -> u32`
  - [ ] `stack_start() -> i64` → `stack_start() -> u32`
  - [ ] `stack_end() -> i64` → `stack_end() -> u32`
  - [ ] `pc() -> i64` → `pc() -> u32`
  - [ ] `get_reg(reg: usize) -> i64` → `get_reg(reg: usize) -> i32`
  - [ ] `most_recent_memory() -> i64` → `most_recent_memory() -> u32`
  - [ ] `most_recent_data() -> (i64, usize)` → `most_recent_data() -> (u32, usize)`
  - [ ] `most_recent_stack() -> (i64, usize)` → `most_recent_stack() -> (u32, usize)`
  - [ ] `push_stack_frame(frame: i64)` → `push_stack_frame(frame: u32)`
  - [ ] `stack_frames() -> &[i64]` → `stack_frames() -> &[u32]`

- [ ] Update `Instruction` structure:
  - [ ] `address: i64` → `address: u32`
  - [ ] `length: i64` → `length: u32`

- [ ] Update `add_local_labels()`:
  - [ ] `HashMap` key type: `i64` → `u32`
  - [ ] Instruction addresses: `i64` → `u32`

- [ ] Update `trace()` function:
  - [ ] `addresses: &HashMap<i64, usize>` → `&HashMap<u32, usize>`

### Phase 2.2: Update Trace System
**File**: `src/trace.rs`

- [ ] Update `MemoryValue`:
  - [ ] `address: i64` → `address: u32`

- [ ] Update `RegisterValue`:
  - [ ] `value: i64` → `value: i32`

- [ ] Update `Effects`:
  - [ ] `pc: (i64, i64)` → `pc: (u32, u32)`
  - [ ] `function_start: Option<i64>` → `function_start: Option<u32>`
  - [ ] `function_end: Option<i64>` → `function_end: Option<u32>`

- [ ] Update `ExecutionTrace`:
  - [ ] `set_most_recent_memory() -> (i64, (i64, usize), (i64, usize))` → return types use `u32`

### Phase 2.3: Update Linter
**File**: `src/linter.rs`

- [ ] Update `FunctionRegisters`:
  - [ ] `at_entry_sp: i64` → `at_entry_sp: u32`

- [ ] Update `Linter`:
  - [ ] `at_entry_sp: i64` → `at_entry_sp: u32`
  - [ ] `new(at_entry_sp: i64)` → `new(at_entry_sp: u32)`
  - [ ] Update alignment checks to work with `u32` addresses
  - [ ] Update stack frame tracking to use `u32`

- [ ] Update store/load checks:
  - [ ] Remove 64-bit specific checks for SD/LD (8-byte aligned checks)
  - [ ] Note: In RV32, largest load/store is 32-bit (4 bytes), so alignment checks change:
    - [ ] SB: no alignment needed (0)
    - [ ] SH: 2-byte aligned (check bit 0 is clear)
    - [ ] SW: 4-byte aligned (check bits 1-0 are clear)
    - [ ] **Remove** SD checks

## Phase 3: ELF Loader Updates

### Phase 3.1: Update ELF Format Constants
**File**: `src/elf.rs`

- [ ] Update magic number check:
  - [ ] Change `raw[4] != 2` to `raw[4] != 1` (line 14)
  - [ ] Update error message: "64-bit" → "32-bit"

- [ ] Define constants for 32-bit ELF sizes:
  ```rust
  const ELF32_EHDR_SIZE: usize = 52;     // 0x34
  const ELF32_PHDR_SIZE: usize = 32;     // 0x20
  const ELF32_SHDR_SIZE: usize = 40;     // 0x28
  const ELF32_SYM_SIZE: usize = 16;      // 0x10
  ```

### Phase 3.2: Rewrite ELF Header Parsing
**File**: `src/elf.rs`

- [ ] Update header validation (lines 36-38):
  - [ ] Change expected `e_phoff`: `0x40` → `0x34`
  - [ ] Change expected `e_ehsize`: `0x40` → `0x34`
  - [ ] Change expected `e_phentsize`: `0x38` → `0x20`

- [ ] Update header field extraction (lines 24-34):
  - [ ] `e_entry`: Read u32 at offset 0x18 (was i64 at 0x18)
  - [ ] `e_phoff`: Read u32 at offset 0x1c (was u64 at 0x20)
  - [ ] `e_shoff`: Read u32 at offset 0x20 (was u64 at 0x28)
  - [ ] `e_ehsize`: Read u16 at offset 0x28 (was at 0x34)
  - [ ] `e_phentsize`: Read u16 at offset 0x2a (was at 0x36)
  - [ ] `e_phnum`: Read u16 at offset 0x2c (was at 0x38)
  - [ ] `e_shentsize`: Read u16 at offset 0x2e (was at 0x3a)
  - [ ] `e_shnum`: Read u16 at offset 0x30 (was at 0x3c)
  - [ ] `e_shstrndx`: Read u16 at offset 0x32 (was at 0x3e)

### Phase 3.3: Rewrite Program Header Parsing
**File**: `src/elf.rs`

- [ ] Update program header loop (lines 42-69):
  - [ ] `p_type`: Read u32 at offset 0x00 (same)
  - [ ] `p_flags`: Read u32 at offset 0x04 (was at 0x04, stays same)
  - [ ] `p_offset`: Read u32 at offset 0x08 (was u64 at 0x08)
  - [ ] `p_vaddr`: Read u32 at offset 0x0c (was u64 at 0x10)
  - [ ] `p_paddr`: Read u32 at offset 0x10 (was u64 at 0x18) - if needed
  - [ ] `p_filesz`: Read u32 at offset 0x14 (was u64 at 0x20)
  - [ ] `p_memsz`: Read u32 at offset 0x18 (was u64 at 0x28) - if needed
  - [ ] `p_align`: Read u32 at offset 0x1c (was u64 at 0x30) - if needed
  - [ ] Change all i64 casts to u32

- [ ] Update `chunks` vector type:
  - [ ] `Vec<(i64, Vec<u8>)>` → `Vec<(u32, Vec<u8>)>`

### Phase 3.4: Rewrite Section Header Parsing
**File**: `src/elf.rs`

- [ ] Update section header string table location (lines 72-90):
  - [ ] Section header entry size: 0x40 → 0x28
  - [ ] Field offsets for 32-bit format:
    - [ ] `sh_name`: offset 0x00 (same)
    - [ ] `sh_type`: offset 0x04 (same)
    - [ ] `sh_flags`: Read u32 at offset 0x08 (was u64 at 0x08)
    - [ ] `sh_addr`: Read u32 at offset 0x0c (was u64 at 0x10)
    - [ ] `sh_offset`: Read u32 at offset 0x10 (was u64 at 0x18)
    - [ ] `sh_size`: Read u32 at offset 0x14 (was u64 at 0x20)
    - [ ] `sh_link`: offset 0x18 (was 0x28)
    - [ ] `sh_info`: offset 0x1c (was 0x2c)
    - [ ] `sh_addralign`: Read u32 at offset 0x20 (was u64 at 0x30) - if needed
    - [ ] `sh_entsize`: Read u32 at offset 0x24 (was u64 at 0x38) - if needed

- [ ] Update section header loop (lines 107-168):
  - [ ] Apply same offset changes as above
  - [ ] Change all address/size types from i64/u64 to u32

### Phase 3.5: Rewrite Symbol Table Parsing
**File**: `src/elf.rs`

- [ ] Update symbol table parsing (lines 177-223):
  - [ ] Change `SYMBOL_SIZE` constant: 24 → 16
  - [ ] Update field extraction:
    - [ ] `st_name`: offset 0x00 (same)
    - [ ] `st_info`: offset 0x04 (same)
    - [ ] `st_other`: offset 0x05 (same)
    - [ ] `st_shndx`: offset 0x06 (same)
    - [ ] `st_value`: Read u32 at offset 0x08 (was i64 at 0x08)
    - [ ] `st_size`: Read u32 at offset 0x0c (was u64 at 0x10) - if needed
  - [ ] Change types: `st_value: i64` → `st_value: u32`

- [ ] Update return types:
  - [ ] `address_symbols: HashMap<i64, String>` → `HashMap<u32, String>`
  - [ ] `global_pointer: i64` → `global_pointer: u32`

### Phase 3.6: Update Function Signature
**File**: `src/elf.rs`

- [ ] Update `load_elf()` return type:
  - [ ] All address types in Machine constructor call use `u32`

## Phase 4: Main Application Updates

### Phase 4.1: Update Main Loop
**File**: `src/main.rs`

- [ ] Update variable types:
  - [ ] `pc: i64` → `pc: u32` (line 108)

- [ ] Update instruction disassembly loop (lines 108-121):
  - [ ] Address arithmetic uses `u32`
  - [ ] `length: i64` → `length: u32`

- [ ] Update `addresses` HashMap:
  - [ ] `HashMap<i64, usize>` → `HashMap<u32, usize>`

### Phase 4.2: Update UI Components
**File**: `src/ui.rs` (if present, or wherever UI code lives)

- [ ] Update address display formatting:
  - [ ] Change format strings for u32 instead of i64
  - [ ] Ensure hex displays show appropriate width (8 hex digits for 32-bit)

- [ ] Update any address-related state:
  - [ ] Cursor positions, memory views, etc. use `u32`

- [ ] Update symbol lookups:
  - [ ] `HashMap<i64, String>` → `HashMap<u32, String>`

## Phase 5: Testing and Validation

### Phase 5.1: Create RV32 Test Environment
**Directory**: `test32/` (copy and modify from `test64/`)

- [ ] Copy test64 to test32:
  ```bash
  cp -r test64 test32
  ```

- [ ] Update `test32/Makefile`:
  - [ ] Change `ASFLAGS`: `-march=rv64im` → `-march=rv32im`
  - [ ] Change `ASFLAGS`: `-mabi=lp64` → `-mabi=ilp32`
  - [ ] Change toolchain prefix detection to look for `riscv32-*` tools
  - [ ] Update `RUN` to `qemu-riscv32` if testing with QEMU

- [ ] Update test files to remove RV64-specific instructions:
  - [ ] Remove `test64/addw.S`, `subw.S`, etc. (W-suffix instructions)
  - [ ] Remove `test64/addiw.S`, `slliw.S`, etc. 
  - [ ] Remove `test64/ld.S`, `sd.S`, `lwu.S`
  - [ ] Remove `test64/mulw.S`, `divw.S`, `divuw.S`, `remw.S`, `remuw.S`

- [ ] Update `start.s`:
  - [ ] If it uses any 64-bit instructions, replace with 32-bit equivalents
  - [ ] Ensure stack pointer initialization uses 32-bit values

- [ ] Create new test cases if needed:
  - [ ] Tests for proper handling of removed instructions (should error)
  - [ ] Tests for 32-bit overflow behavior
  - [ ] Tests for proper sign/zero extension

### Phase 5.2: Compile and Test Basic Operations

- [ ] Build risclet:
  ```bash
  cargo build --release
  ```

- [ ] Test with simple RV32 binary:
  - [ ] Create minimal test: `add.S`, `addi.S`
  - [ ] Verify disassembly looks correct
  - [ ] Verify execution produces correct results

- [ ] Test register operations:
  - [ ] All base RV32I instructions
  - [ ] Verify register values stay in 32-bit range
  - [ ] Check sign extension works correctly

- [ ] Test memory operations:
  - [ ] LB, LH, LW - verify sign extension
  - [ ] LBU, LHU - verify zero extension
  - [ ] SB, SH, SW - verify correct storage

- [ ] Test multiplication/division:
  - [ ] MUL, MULH, MULHSU, MULHU
  - [ ] DIV, DIVU, REM, REMU
  - [ ] Verify 64-bit intermediate results for MULH variants

### Phase 5.3: Test Compressed Instructions

- [ ] Test C.ADDI, C.LI, C.LUI, C.ADDI16SP, C.ADDI4SPN
- [ ] Test C.LW, C.SW (C.LWSP, C.SWSP)
- [ ] Test C.J, C.JAL, C.JR, C.JALR
- [ ] Test C.BEQZ, C.BNEZ
- [ ] Test C.SRLI, C.SRAI, C.ANDI, C.SLLI
- [ ] Test C.MV, C.ADD, C.AND, C.OR, C.XOR, C.SUB
- [ ] Verify removed compressed instructions (C.LD, C.SD, C.ADDIW, etc.) are not decoded

### Phase 5.4: Test ELF Loading

- [ ] Create simple RV32 binaries with:
  - [ ] Text section only
  - [ ] Text + data sections
  - [ ] Text + data + BSS
  - [ ] Multiple segments

- [ ] Verify ELF parser correctly:
  - [ ] Reads 32-bit ELF header
  - [ ] Parses program headers correctly
  - [ ] Parses section headers correctly
  - [ ] Extracts symbols with correct addresses
  - [ ] Handles global pointer correctly

- [ ] Test error handling:
  - [ ] Loading 64-bit ELF should fail with clear error
  - [ ] Malformed ELF should be rejected

### Phase 5.5: Test Linter

- [ ] Verify linter works with 32-bit addresses:
  - [ ] Stack alignment checks (16-byte boundary)
  - [ ] Register convention checks
  - [ ] Memory access alignment (1, 2, 4 bytes)

- [ ] Test function call/return tracking:
  - [ ] Saved registers preserved
  - [ ] Stack pointer management
  - [ ] Return address handling

### Phase 5.6: Integration Testing

- [ ] Run comprehensive test suite:
  ```bash
  cd test32
  make clean
  make
  ../target/release/risclet -e a.out -m run
  ```

- [ ] Test all execution modes:
  - [ ] `run` mode
  - [ ] `dasm` mode
  - [ ] `debug` mode (TUI)

- [ ] Test edge cases:
  - [ ] Maximum positive/negative 32-bit values
  - [ ] Overflow behavior
  - [ ] Sign extension in branches/loads
  - [ ] Address space boundaries

- [ ] Verify output:
  - [ ] Disassembly matches expected
  - [ ] Register values correct
  - [ ] Memory contents correct
  - [ ] Linter errors appropriate

## Phase 6: Documentation and Cleanup

### Phase 6.1: Update Documentation

- [ ] Update `README.md`:
  - [ ] Change "rv64imc" → "rv32imc"
  - [ ] Update feature list
  - [ ] Update any 64-bit specific references

- [ ] Update `Cargo.toml`:
  - [ ] Update version number (indicate major change)
  - [ ] Update description if needed

- [ ] Create `CHANGELOG.md`:
  - [ ] Document conversion from RV64 to RV32
  - [ ] List removed instructions
  - [ ] Note breaking changes

### Phase 6.2: Code Cleanup

- [ ] Remove commented-out code for RV64 instructions
- [ ] Remove unused helper functions
- [ ] Ensure all clippy warnings are addressed
- [ ] Run rustfmt on all files
- [ ] Review all `unwrap()` calls and ensure they're safe

### Phase 6.3: Performance Testing

- [ ] Benchmark execution speed
- [ ] Check memory usage
- [ ] Profile hot paths
- [ ] Optimize if needed (but keep code simple)

## Potential Issues and Mitigations

### Issue 1: PC Arithmetic with Signed Offsets
**Problem**: PC is `u32` but branch offsets are `i32`. Adding could overflow.

**Solution**: 
```rust
// Safe addition with wrapping
let new_pc = pc.wrapping_add(offset as u32);
// Or use checked arithmetic and error on overflow
let new_pc = pc.checked_add_signed(offset).ok_or("PC overflow")?;
```

### Issue 2: Sign Extension in Loads
**Problem**: Loading signed bytes/halfwords into 32-bit registers.

**Solution**: Rust's type system handles this naturally:
```rust
// LB: i8 → i32 automatic sign extension
let val = i8::from_le_bytes(...) as i32;
// LBU: u8 → i32 automatic zero extension
let val = u8::from_le_bytes(...) as i32;
```

### Issue 3: Address Space Limits
**Problem**: 32-bit address space is limited to 4GB.

**Solution**: This is expected and correct for RV32. Ensure:
- Stack and heap fit in 32-bit address space
- ELF segments don't specify out-of-range addresses
- All address arithmetic uses wrapping/checked operations

### Issue 4: Multiply Operations
**Problem**: MULH* instructions need 64-bit intermediate results.

**Solution**:
```rust
// MULH: signed × signed → upper 32 bits
let result = ((rs1 as i64) * (rs2 as i64)) >> 32;
rd = result as i32;

// MULHU: unsigned × unsigned → upper 32 bits
let result = ((rs1 as u32 as u64) * (rs2 as u32 as u64)) >> 32;
rd = result as i32;
```

### Issue 5: Linter 64-bit Assumptions
**Problem**: Linter may assume 8-byte loads/stores exist.

**Solution**: 
- Remove all SD/LD specific checks
- Largest valid load/store in RV32 is 4 bytes
- Update alignment checks accordingly

### Issue 6: Compressed Instruction Decode
**Problem**: Some compressed instructions expand to removed 64-bit instructions.

**Solution**:
- C.LD → should decode to Unimplemented
- C.SD → should decode to Unimplemented  
- C.ADDIW → should decode to Unimplemented
- C.LDSP/C.SDSP → should decode to Unimplemented
- Ensure all these cases are removed or return Unimplemented variant

## Testing Checklist Summary

- [ ] All basic RV32I instructions execute correctly
- [ ] All M extension instructions execute correctly
- [ ] All valid compressed instructions execute correctly
- [ ] Removed instructions properly rejected/unimplemented
- [ ] ELF loading works for 32-bit ELF files
- [ ] ELF loading rejects 64-bit ELF files
- [ ] Memory operations handle sign/zero extension correctly
- [ ] PC arithmetic handles signed offsets correctly
- [ ] Linter catches ABI violations
- [ ] Linter handles 32-bit addresses correctly
- [ ] TUI displays addresses in 32-bit format
- [ ] All test cases pass
- [ ] Documentation updated
- [ ] No compiler warnings
- [ ] Cargo clippy passes
- [ ] Code is formatted with rustfmt

## Estimated Effort

- **Phase 1 (Type System)**: 4-6 hours
- **Phase 2 (Execution Engine)**: 3-4 hours
- **Phase 3 (ELF Loader)**: 4-6 hours
- **Phase 4 (Main Application)**: 1-2 hours
- **Phase 5 (Testing)**: 6-8 hours
- **Phase 6 (Documentation)**: 2-3 hours

**Total**: 20-29 hours

## Success Criteria

The conversion is complete when:

1. ✅ All RV32IMC instructions are correctly decoded and executed
2. ✅ All RV64-specific instructions are removed or return Unimplemented
3. ✅ 32-bit ELF files load and execute correctly
4. ✅ 64-bit ELF files are rejected with clear error message
5. ✅ All existing test cases pass (after conversion to RV32)
6. ✅ Linter correctly validates RV32 ABI compliance
7. ✅ TUI displays all information correctly with 32-bit addresses
8. ✅ No compiler warnings or clippy lints
9. ✅ Documentation accurately reflects RV32IMC support
10. ✅ Code is clean, formatted, and ready for production use
