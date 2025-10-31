# Layout Refactoring Plan

## Overview

This refactoring separates address layout information from the AST and symbol linking phases into a dedicated `Layout` structure. The goal is to make processing phases more self-contained and establish clear data flow:

1. **Parsing** → `Source` (AST only, immutable after parsing)
2. **Symbol Linking** → `SymbolLinks` (immutable after linking)
3. **Layout Computation** → `Layout` (created after linking, updated during convergence)

## Current State

Currently, layout-related data is scattered across multiple structures:

### In `Source` struct (ast.rs:55-62)
```rust
pub struct Source {
    pub files: Vec<SourceFile>,
    pub header_size: u32,  // ← Layout data
    pub text_size: u32,    // ← Layout data
    pub data_size: u32,    // ← Layout data
    pub bss_size: u32,     // ← Layout data
}
```

### In `SourceFile` struct (ast.rs:44-52)
```rust
pub struct SourceFile {
    pub file: String,
    pub lines: Vec<Line>,
    pub text_size: u32,    // ← Temporary/cache data
    pub data_size: u32,    // ← Temporary/cache data
    pub bss_size: u32,     // ← Temporary/cache data
}
```

### In `Line` struct (ast.rs:238-249)
```rust
pub struct Line {
    pub location: Location,
    pub content: LineContent,
    pub segment: Segment,  // ← Layout data
    pub offset: u32,       // ← Layout data
    pub size: u32,         // ← Layout data
}
```

## New Design

### New `Layout` struct

Create a new struct in a new file `src/layout.rs`:

```rust
use crate::ast::{LinePointer, Segment};
use std::collections::HashMap;

/// Information about a line's position and size in the binary
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineLayout {
    pub segment: Segment,
    pub offset: u32,  // Offset within the segment
    pub size: u32,    // Size in bytes
}

/// Complete layout information for the assembled program
/// Created after symbol linking and updated during convergence
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Per-line layout information
    pub lines: HashMap<LinePointer, LineLayout>,
    
    /// ELF header size (depends on number of segments)
    pub header_size: u32,
    
    /// Total size of each segment
    pub text_size: u32,
    pub data_size: u32,
    pub bss_size: u32,
}

impl Layout {
    /// Create a new empty layout
    pub fn new() -> Self {
        Layout {
            lines: HashMap::new(),
            header_size: 0,
            text_size: 0,
            data_size: 0,
            bss_size: 0,
        }
    }
    
    /// Get layout info for a specific line (returns None if not found)
    pub fn get(&self, pointer: &LinePointer) -> Option<&LineLayout> {
        self.lines.get(pointer)
    }
    
    /// Set layout info for a specific line
    pub fn set(&mut self, pointer: LinePointer, layout: LineLayout) {
        self.lines.insert(pointer, layout);
    }
}
```

## Refactoring Steps

### Step 1: Create the Layout infrastructure

**Files to create:**
- `src/layout.rs` - New module with `Layout` and `LineLayout` structs

**Files to modify:**
- `src/main.rs` - Add `pub mod layout;` in module declarations

**Note:** AST fields remain in place during this step. We'll delete them at the very end after all consumers are updated.

### Step 2: Move layout functions to layout.rs

**Move from `src/assembler.rs` to `src/layout.rs`:**

Functions to move:
- `guess_line_size()` - needed by `create_initial_layout()`
- `compute_offsets()` - main layout computation function

For `compute_offsets()`, create a NEW version with signature:
```rust
pub fn compute_offsets(source: &Source, layout: &mut Layout)
```

This new version should:
- Read from `layout.lines` instead of `line.segment`, `line.offset`, `line.size`
- Write to `layout.lines` and `layout.{text,data,bss,header}_size`
- NOT modify `source` (immutable reference)
- Segment calculation: During layout calculation, iterate through all lines and recompute the segment on-the-fly based on directives encountered (similar to how it's done in `create_initial_layout`). This happens during both initial layout and re-calculations during convergence.

**Keep the old `compute_offsets(source: &mut Source)` in `assembler.rs` for now** - it will continue to be used by existing code until we update all consumers. We'll delete it later.

### Step 3: Create initial layout after symbol linking

**In `src/layout.rs`:**

Add a new function `create_initial_layout()`:
```rust
pub fn create_initial_layout(
    source: &Source,
) -> Layout {
    let mut layout = Layout::new();

    // Track current segment as we iterate through lines
    let mut current_segment = Segment::Text;

    // Set initial size guesses and segment for all lines
    for (file_index, file) in source.files.iter().enumerate() {
        for (line_index, line) in file.lines.iter().enumerate() {
            let pointer = LinePointer { file_index, line_index };

            // Update current segment based on directives
            if let LineContent::Directive(directive) = &line.content {
                match directive.name.as_str() {
                    ".text" => current_segment = Segment::Text,
                    ".data" => current_segment = Segment::Data,
                    ".bss" => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            let size = guess_line_size(&line.content);

            layout.set(pointer, LineLayout {
                segment: current_segment,
                offset: 0,  // Will be computed by compute_offsets
                size,
            });
        }
    }

    // Compute initial offsets and segment sizes
    compute_offsets(source, &mut layout);

    layout
}
```

This function should be called **after** symbol linking is complete.

**Note:** This step does not modify existing code, just adds new functionality. Old code still works with AST fields.

### Step 4: Update expression evaluation to use Layout

**In `src/expressions.rs`:**

Add `Layout` to `EvaluationContext`:
```rust
pub struct EvaluationContext {
    source: Source,
    symbol_links: SymbolLinks,
    layout: Layout,  // ← Add this
    // ... other fields
}
```

Update `new_evaluation_context()` to accept `layout: Layout` parameter.

Update functions that compute addresses to read from Layout:
- `segment_start()` - use `layout.{text,data,bss}_size` instead of `source.*_size`
- Expression evaluation that references `.` (CurrentAddress) needs to lookup from `layout.lines`
- Symbol value computation for labels needs to lookup offset/segment from `layout.lines`

**Note:** AST fields still exist at this point, but expression evaluation now reads from Layout instead.

### Step 5: Update encoder to use Layout

**In `src/encoder.rs`:**

Update `EncodingContext`:
```rust
pub struct EncodingContext<'a> {
    pub source: &'a Source,
    pub eval_context: &'a mut EvaluationContext,
    pub layout: &'a mut Layout,  // ← Add this
    pub file_index: usize,
    pub line_index: usize,
}
```

Update `encode_source()` to:
- Accept `layout: &mut Layout` parameter
- Read `segment` and `old_size` from `layout` instead of line fields
- Write updated `size` to `layout` instead of line fields

Update other encoding functions to get segment/offset/size from `layout`:
- `encode_bss_line()` - compute addresses using `layout`
- `get_line_address()` - use `layout` instead of `line.segment`/`line.offset`

**Note:** AST fields still exist, but encoder now reads/writes Layout instead.

### Step 6: Update convergence loop

**In `src/assembler.rs`:**

Update `converge_and_encode()` signature:
```rust
pub fn converge_and_encode<C: ConvergenceCallback>(
    source: &Source,           // ← Now immutable!
    symbol_links: &SymbolLinks,
    layout: &mut Layout,       // ← Add this
    text_start: u32,
    relax: &Relax,
    callback: &C,
    show_progress: bool,
) -> Result<(Vec<u8>, Vec<u8>, u32)>
```

Update the convergence loop to:
- Call `layout::compute_offsets(source, layout)` (new version) instead of `assembler::compute_offsets(source)` (old version)
- Create `eval_context` with `layout.clone()` (or pass reference appropriately)
- Pass `layout` to encoder
- Display sizes from `layout` instead of `source`

**Note:** After this step, you can delete the old `compute_offsets(source: &mut Source)` from `assembler.rs` since it's no longer used.

### Step 7: Update ELF generation

**In `src/elf.rs`:**

Update `generate_elf()` and related functions to:
- Accept `layout: &Layout` parameter
- Read segment sizes from `layout` instead of `source`
- Look up line addresses by querying `layout.lines`

**Note:** AST fields still exist, but ELF generation now reads from Layout.

### Step 8: Update dump functions

**In `src/dump.rs`:**

Update all dump functions to:
- Accept `layout: &Layout` parameter
- Look up segment/offset/size from `layout` instead of `line` fields
- Access segment sizes from `layout` instead of `source`

**Note:** AST fields still exist, but dump functions now read from Layout.

### Step 9: Update main.rs pipeline

**In `src/main.rs`:**

Update the assembly pipeline:
```rust
// After parsing (keep old code that sets line.size for now)
let mut source = Source { files, /* size fields still present */ };

// After symbol linking
let symbol_links = link_symbols(&source)?;

// Create initial layout (NEW!)
let mut layout = layout::create_initial_layout(&source);

// Convergence (UPDATED signature)
let (text, data, bss_size) = converge_and_encode(
    &source,          // now immutable
    &symbol_links,
    &mut layout,      // NEW parameter
    text_start,
    &relax,
    &callback,
    show_progress,
)?;

// Generate ELF (UPDATED signature)
let elf = generate_elf(&source, &symbol_links, &layout, ...)?;
```

**Note:** At this point, AST fields are still being set during parsing but are no longer read by any consumer code (they all read from Layout instead). This is intentional - we'll delete them next.

### Step 10: Update all tests

**Files to update:**
- `src/encoder_tests.rs`
- `src/symbols_tests.rs`
- `src/expressions_tests.rs`
- Any other test files

For each test:
1. Create `Layout` structures as needed
2. Pass `layout` to functions that now require it
3. Update assertions to check `layout` instead of AST fields

**Note:** For now, keep initializing AST fields in tests (even though they're not used). We'll clean this up in the next step.

### Step 11: Remove layout fields from AST (BREAKING - but now safe!)

**At this point, all code reads from Layout instead of AST fields. Now we can safely delete the unused fields.**

**In `src/ast.rs`:**

Remove from `Source`:
```rust
// DELETE these fields:
pub header_size: u32,
pub text_size: u32,
pub data_size: u32,
pub bss_size: u32,
```

Remove from `SourceFile`:
```rust
// DELETE these fields:
pub text_size: u32,
pub data_size: u32,
pub bss_size: u32,
```

Remove from `Line`:
```rust
// DELETE these fields:
pub segment: Segment,
pub offset: u32,
pub size: u32,
```

**In `src/main.rs` (parse_file function):**

Remove the line that sets `new_line.size = assembler::guess_line_size(&new_line.content);` since size is no longer part of the AST.

Also remove initialization of `text_size`, `data_size`, `bss_size` fields in `Source` and `SourceFile` construction.

**In test files:**

Remove initialization of these deleted fields in test code.

### Step 12: Final verification

**Run all checks:**

```bash
cargo fmt              # Format code
cargo clippy          # Lint (should have no warnings)
cargo build           # Should compile successfully
cargo test            # All tests should pass
```

**Verify the refactoring is complete:**
- AST fields removed and code still compiles
- All tests pass
- No warnings from clippy
- Source is immutable after parsing (passed as `&Source`)
- Layout is mutable during convergence (passed as `&mut Layout`)
- Expression evaluation, encoder, ELF generation, and dump functions all use Layout
- Old `compute_offsets(source: &mut Source)` removed from assembler.rs

## Verification Checklist

After completing the refactoring:

- [ ] All fields removed from `Source`: `header_size`, `text_size`, `data_size`, `bss_size`
- [ ] All fields removed from `SourceFile`: `text_size`, `data_size`, `bss_size`
- [ ] All fields removed from `Line`: `segment`, `offset`, `size`
- [ ] `Layout` struct created with `HashMap<LinePointer, LineLayout>`
- [ ] `Layout` contains segment sizes and header size
- [ ] `layout.rs` module declared in `main.rs` (not `lib.rs`)
- [ ] `guess_line_size()` moved from `assembler.rs` to `layout.rs`
- [ ] `compute_offsets()` moved from `assembler.rs` to `layout.rs`
- [ ] `compute_offsets()` takes `&Source` (immutable) and `&mut Layout`
- [ ] `create_initial_layout()` implemented in `layout.rs`
- [ ] `converge_and_encode()` takes `&Source` (immutable) and `&mut Layout`
- [ ] Expression evaluation uses `Layout` for address computation
- [ ] Encoder uses `Layout` for segment/offset/size
- [ ] ELF generation uses `Layout` for addresses
- [ ] Dump functions use `Layout` for display
- [ ] All imports updated to use `layout::` instead of `assembler::` for moved functions
- [ ] Parsing does not set `line.size` (field no longer exists)
- [ ] No segment tracking during parsing (computed during layout instead)
- [ ] All tests pass
- [ ] No cloning of `Source` after parsing (except for eval context setup)
- [ ] `Source` is passed by shared reference with name `source`
- [ ] `SymbolLinks` is passed by shared reference with name `symbol_links`
- [ ] `Layout` is passed by shared reference with name `layout` (mut when needed)

## Benefits

1. **Clear separation of concerns**: Parsing produces AST, linking produces symbol info, layout computation produces layout info
2. **Immutable AST**: `Source` is never modified after parsing, making reasoning easier
3. **Immutable symbol links**: `SymbolLinks` is never modified after linking
4. **Efficient convergence**: Only `Layout` needs to be updated during convergence iterations
5. **Better testing**: Tests can create minimal `Layout` structures without full AST
6. **Clearer data flow**: Each phase has well-defined inputs and outputs

## Migration Order

Execute the steps in order (designed for minimal breakage):

1. **Step 1**: Create `Layout` infrastructure (non-breaking) - adds new types without removing anything
2. **Step 2**: Move/copy layout functions to `layout.rs` (non-breaking) - creates new implementations alongside old ones
3. **Step 3**: Add `create_initial_layout()` (non-breaking) - new functionality, doesn't modify existing code
4. **Steps 4-8**: Update consumers one by one (incremental) - each module switches from AST fields to Layout, but AST fields still exist
5. **Step 9**: Update main.rs pipeline (integration) - calls `create_initial_layout`, passes Layout to converge_and_encode
6. **Step 10**: Update tests (compatibility) - tests work with Layout while AST fields still exist
7. **Step 11**: Delete AST fields (breaking, but safe) - removes now-unused fields; everything already uses Layout
8. **Step 12**: Final verification (validation) - run all tests and checks

**Key insight:** AST fields remain until Step 11. Everything switches to reading from Layout first (Steps 4-9), and only after all code uses Layout do we delete the obsolete AST fields. This keeps the code compiling throughout most of the refactoring.

**Note:** Steps 2-3 should be done together since `create_initial_layout()` depends on both `guess_line_size()` and `compute_offsets()` being in `layout.rs`.

## Notes

- The `segment` for each line is calculated during layout computation by tracking the current segment as we iterate through lines and encounter `.text`, `.data`, or `.bss` directives. This is an easy calculation that should be done on-the-fly during both initial layout calculation and later re-calculations during convergence - no need to pull it out as a special case or compute it separately during parsing.
- The `SourceFile.{text,data,bss}_size` fields are truly temporary - they're just aggregations computed during `compute_offsets` and only used to display progress. With `Layout`, we don't need them at all since we can compute per-file sizes if needed from the `Layout` data.
- Tests currently initialize size fields to 0. After refactoring, they won't need to initialize these fields at all, simplifying test code.
- All initial layout code (including `create_initial_layout()`, `compute_offsets()`, and `guess_line_size()`) should be in `layout.rs`.
- Since `guess_line_size()` is moving to `layout.rs`, all imports in test files and main.rs need to be updated from `crate::assembler::guess_line_size` to `crate::layout::guess_line_size`.
