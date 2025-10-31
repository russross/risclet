# Symbol Values Refactoring Plan

## Overview

Extract `symbol_values: HashMap<SymbolReference, EvaluatedValue>` from `EvaluationContext` into a standalone `SymbolValues` type. This creates a cleaner separation of concerns in the assembly pipeline:

```
Parse → Link Symbols → Compute Layout → Evaluate Symbol Values → Encode
```

Each assembler phase builds on previous read-only objects without mutation and
provides an input to later stages.

## Data Structures (in layout.rs)

Add a helper method to Layout (in `src/layout.rs`):

```rust
impl Layout {
    /// Compute absolute segment start addresses from text_start and layout sizes
    pub fn compute_segment_addresses(&self, text_start: u32) -> (u32, u32, u32) {
        // text_start_adjusted: account for ELF header
        let text_start_adjusted = text_start + self.header_size;

        // data_start: align to 4K boundary after text
        let data_start = (text_start_adjusted + self.text_size + 4095) & !(4096 - 1);

        // bss_start: immediately after data
        let bss_start = data_start + self.data_size;

        (text_start_adjusted, data_start, bss_start)
    }
}
```

**Implementation notes:**
- This logic is currently in expressions.rs:83-96 in `new_evaluation_context()`
- Extract lines 86-96 into this helper method
- The method encapsulates the segment address calculation that is currently scattered

## Data Structures (in expressions.rs)

New Type: `SymbolValues`

```rust
pub struct SymbolValues {
    values: HashMap<SymbolReference, EvaluatedValue>,
}

impl SymbolValues {
    /// Create an empty SymbolValues
    pub fn new() -> Self {
        SymbolValues { values: HashMap::new() }
    }

    /// Look up a symbol value
    pub fn get(&self, key: &SymbolReference) -> Option<EvaluatedValue> {
        self.values.get(key).copied()
    }

    /// Insert or update a symbol value (internal)
    fn insert(&mut self, key: SymbolReference, value: EvaluatedValue) {
        self.values.insert(key, value);
    }

    /// Check if a symbol is already evaluated (internal)
    fn contains_key(&self, key: &SymbolReference) -> bool {
        self.values.contains_key(key)
    }
}
```

**Implementation notes:**
- This wraps the `symbol_values` HashMap currently in EvaluationContext (line 41)
- Provides a clean interface for symbol value lookup
- Used by `eval_expr()` to look up symbol values without needing full EvaluationContext

New Function: `eval_symbol_values`

```rust
pub fn eval_symbol_values(
    source: &Source,
    symbol_links: &SymbolLinks,
    layout: &Layout,
    text_start: u32,
) -> Result<SymbolValues> {
    // Compute segment addresses
    let (text_start_adjusted, data_start, bss_start) =
        layout.compute_segment_addresses(text_start);

    // Start with empty symbol values
    let mut symbol_values = SymbolValues::new();

    // Iterate all files and lines, evaluating labels and .equ definitions
    for (file_index, file) in source.files.iter().enumerate() {
        for (line_index, line) in file.lines.iter().enumerate() {
            let pointer = LinePointer { file_index, line_index };

            // Only process lines that define symbols (labels and .equ)
            let symbol_name = match &line.content {
                LineContent::Label(name) => Some(name.clone()),
                LineContent::Directive(Directive::Equ(name, _)) => Some(name.clone()),
                _ => None,
            };

            if let Some(name) = symbol_name {
                // Create symbol reference for this definition
                let sym_ref = SymbolReference {
                    symbol: name,
                    pointer,
                };

                // Recursively evaluate this symbol and its dependencies
                let mut cycle_stack = Vec::new();
                eval_symbol_recursive(
                    &sym_ref,
                    source,
                    symbol_links,
                    layout,
                    text_start_adjusted,
                    data_start,
                    bss_start,
                    &mut symbol_values,
                    &mut cycle_stack,
                )?;
            }
        }
    }

    Ok(symbol_values)
}
```

**Implementation notes:**
- Replaces the two-phase approach in `evaluate_line_symbols()` + `resolve_symbol_dependencies()`
- Currently these functions are called per-line during encoding (expressions.rs:138-151)
- New approach evaluates ALL symbols upfront, once per convergence iteration
- Uses same recursive logic as current `resolve_symbol_dependencies()` (lines 157-254)
- **Special handling for `__global_pointer$`**: (No impact on this project) This symbol is defined in the builtin file at data_start + 2048
  - The builtin file (BUILTIN_FILE_NAME) has special layout handling (layout.rs:149-186)
  - When evaluating `__global_pointer$` label, it will compute to data_start + 2048 automatically from layout
  - No special case needed in eval_symbol_recursive since layout already has offset=2048 for this label

Helper: `eval_symbol_recursive`

```rust
fn eval_symbol_recursive(
    key: &SymbolReference,
    source: &Source,
    symbol_links: &SymbolLinks,
    layout: &Layout,
    text_start: u32,
    data_start: u32,
    bss_start: u32,
    symbol_values: &mut SymbolValues,
    cycle_stack: &mut Vec<SymbolReference>,
) -> Result<()> {
    // Base case: already evaluated
    if symbol_values.contains_key(key) {
        return Ok(());
    }

    // Base case: cycle detection
    if cycle_stack.contains(key) {
        let cycle_chain: Vec<String> =
            cycle_stack.iter().map(|k| k.symbol.clone()).collect();
        let line = source.get_line(&key.pointer)?;
        return Err(AssemblerError::from_context(
            format!(
                "Circular reference in symbol '{}': {} -> {}",
                key.symbol,
                cycle_chain.join(" -> "),
                key.symbol
            ),
            line.location.clone(),
        ));
    }

    // Get the line where this symbol is defined
    let line = source.get_line(&key.pointer)?;

    // Compute value based on line content
    let value = match &line.content {
        LineContent::Label(_) => {
            // Base case: label address is computed from layout
            let line_layout = layout.get(&key.pointer).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Internal error: no layout for label '{}'", key.symbol),
                    line.location.clone(),
                )
            })?;
            let segment_start = match line_layout.segment {
                Segment::Text => text_start,
                Segment::Data => data_start,
                Segment::Bss => bss_start,
            };
            let addr = segment_start.wrapping_add(line_layout.offset);
            EvaluatedValue::Address(addr)
        }
        LineContent::Directive(Directive::Equ(_, expr)) => {
            // Recursive case: evaluate dependencies first
            cycle_stack.push(key.clone());

            // Get all symbol references from this .equ line
            let sym_refs = symbol_links.get_line_refs(&key.pointer);
            for sym_ref in sym_refs {
                eval_symbol_recursive(
                    sym_ref,
                    source,
                    symbol_links,
                    layout,
                    text_start,
                    data_start,
                    bss_start,
                    symbol_values,
                    cycle_stack,
                )?;
            }

            cycle_stack.pop();

            // Now evaluate the expression (all dependencies resolved)
            let line_layout = layout.get(&key.pointer).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Internal error: no layout for .equ '{}'", key.symbol),
                    line.location.clone(),
                )
            })?;
            let segment_start = match line_layout.segment {
                Segment::Text => text_start,
                Segment::Data => data_start,
                Segment::Bss => bss_start,
            };
            let address = segment_start + line_layout.offset;

            eval_expr(expr, address, sym_refs, symbol_values, source, &key.pointer)?
        }
        _ => {
            return Err(AssemblerError::from_context(
                format!(
                    "Symbol '{}' definition points to invalid line",
                    key.symbol
                ),
                line.location.clone(),
            ));
        }
    };

    // Memoize the result
    symbol_values.insert(key.clone(), value);
    Ok(())
}

**Implementation notes:**
- This is a refactored version of `resolve_symbol_dependencies()` (expressions.rs:157-254)
- Key differences:
  - Takes explicit segment addresses instead of EvaluationContext
  - Uses new `eval_expr()` instead of `evaluate_expression()`
  - Works with SymbolValues instead of HashMap in EvaluationContext
  - Simpler: no need to manage `current_line_pointer` or clone contexts

### New Expression Evaluation

```rust
pub fn eval_expr(
    expr: &Expression,
    address: u32,
    refs: &[SymbolReference],
    symbol_values: &SymbolValues,
    source: &Source,
    pointer: &LinePointer,
) -> Result<EvaluatedValue> {
    // Get the line for error reporting
    let line = source.get_line(pointer)?;
    let location = &line.location;

    match expr {
        Expression::Literal(i) => Ok(EvaluatedValue::Integer(*i)),

        Expression::Identifier(name) => {
            // Find symbol in refs, then look up in symbol_values
            let sym_ref = refs
                .iter()
                .find(|r| r.symbol == *name)
                .ok_or_else(|| {
                    AssemblerError::from_context(
                        format!(
                            "Unresolved symbol '{}' (internal error - should have been caught earlier)",
                            name
                        ),
                        location.clone(),
                    )
                })?;

            symbol_values.get(sym_ref).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Symbol '{}' not resolved (internal error - should have been resolved in forward phase)",
                        name
                    ),
                    location.clone(),
                )
            })
        }

        Expression::CurrentAddress => Ok(EvaluatedValue::Address(address)),

        Expression::NumericLabelRef(nlr) => {
            let label_name = format!(
                "{}{}",
                nlr.num,
                if nlr.is_forward { "f" } else { "b" }
            );
            let sym_ref = refs
                .iter()
                .find(|r| r.symbol == label_name)
                .ok_or_else(|| {
                    AssemblerError::from_context(
                        format!("Unresolved numeric label '{}' (internal error)", nlr),
                        location.clone(),
                    )
                })?;

            symbol_values.get(sym_ref).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Numeric label '{}' not resolved (internal error)", label_name),
                    location.clone(),
                )
            })
        }

        Expression::Parenthesized(inner) => {
            eval_expr(inner, address, refs, symbol_values, source, pointer)
        }

        Expression::PlusOp { lhs, rhs } => {
            let lhs_val = eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val = eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            checked_add(lhs_val, rhs_val, location)
        }

        Expression::MinusOp { lhs, rhs } => {
            let lhs_val = eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val = eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            checked_sub(lhs_val, rhs_val, location)
        }

        Expression::MultiplyOp { lhs, rhs } => {
            let lhs_val = eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val = eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "multiplication", location)?;
            let rhs_int = require_integer(rhs_val, "multiplication", location)?;
            let result = lhs_int.checked_mul(rhs_int).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Arithmetic overflow in multiplication: {} * {}", lhs_int, rhs_int),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::DivideOp { lhs, rhs } => {
            let lhs_val = eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val = eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "division", location)?;
            let rhs_int = require_integer(rhs_val, "division", location)?;
            if rhs_int == 0 {
                return Err(AssemblerError::from_context(
                    "Division by zero".to_string(),
                    location.clone(),
                ));
            }
            let result = lhs_int.checked_div(rhs_int).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Arithmetic overflow in division: {} / {}", lhs_int, rhs_int),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }

        // ... similar for other operators (modulo, shifts, bitwise ops, negation)
        // All follow the same pattern: recursively evaluate operands, then apply operator
        // Use location for all error messages
    }
}
```

**Implementation notes:**
- This is a simplified version of `evaluate_expression()` (expressions.rs:261-568)
- Key differences from old implementation:
  - Takes explicit `address` parameter instead of computing from EvaluationContext
  - Takes `refs` slice directly instead of looking up via current_line_pointer
  - Takes SymbolValues instead of EvaluationContext
  - **Takes `source` and `pointer` for error reporting with proper context**
- Error reporting improvements:
  - Uses `source.get_line(pointer)` to get the line where the expression is being evaluated
  - All errors use `AssemblerError::from_context()` with the line's location
  - Maintains same error messages as current implementation for consistency
- The function is pure and doesn't need mutable state
- Symbol lookups are simple: find in refs, then look up in symbol_values
- All the type checking and arithmetic logic remains the same (reuse `checked_add`, `checked_sub`, `require_integer`)

## Deprecation Strategy

Deprecate old types, use compiler warnings to identify dependencies on old code
and then systematically convert them to the new API. Implement conversions in
dependency order.

### Deprecated Types (expressions.rs)

```rust
#[deprecated(since = "next", note = "EvaluationContext is being refactored out of the codebase")]
pub struct EvaluationContext { ... }
```

All methods on `EvaluationContext` are implicitly deprecated.

### Deprecated Functions (expressions.rs)

- `new_evaluation_context()` - Replaced by direct use of `SymbolValues`
- `eval_expr_old()` - Replaced by `eval_expr`
- `evaluate_line_symbols()` - No longer needed with new `eval_symbol_values` approach
- `resolve_symbol_dependencies()` - No longer needed with new `eval_symbol_values` approach

## Phase 1: Create New Structures and Deprecate Old

See above.

Do NOT delete any deprecated functions or add any new dependencies to deprecated code. Leave them as-is so deprecation warnings guide the refactoring.

### Additional Helper Method: Source::get_line (in ast.rs)

Add a helper method to Source for getting a line by pointer:

```rust
impl Source {
    /// Get a line from the source by pointer
    pub fn get_line(&self, pointer: &LinePointer) -> Result<&Line> {
        self.files
            .get(pointer.file_index)
            .and_then(|file| file.lines.get(pointer.line_index))
            .ok_or_else(|| {
                AssemblerError::no_context(format!(
                    "Internal error: invalid line pointer [{}:{}]",
                    pointer.file_index, pointer.line_index
                ))
            })
    }
}
```

**Implementation notes:**
- This is currently duplicated in EvaluationContext::get_line (expressions.rs:63-74)
- Move it to Source so it's available throughout the codebase
- Deprecate EvaluationContext::get_line and then fix references to use the new
  one
- Used by both eval_symbol_recursive and eval_expr for error reporting

## Phase 2: Update API usage

Use deprecation warnings to identify code that needs updating. Implement updates
in dependency order.

### Update 1: assembler.rs convergence loop (lines 98-132)

**Current code:**
```rust
// Step 2 & 3: Calculate symbol values and evaluate expressions
let mut eval_context = expressions::new_evaluation_context(
    source.clone(),
    symbol_links.clone(),
    layout.clone(),
    text_start,
);

// Evaluate all line symbols to populate the expression evaluation context
for (file_index, file) in source.files.iter().enumerate() {
    for (line_index, line) in file.lines.iter().enumerate() {
        let pointer = LinePointer { file_index, line_index };
        expressions::evaluate_line_symbols(
            line,
            &pointer,
            &mut eval_context,
        )?;
    }
}
```

**New code:**
```rust
// Step 2: Calculate all symbol values upfront
let (text_start_adjusted, data_start, bss_start) =
    layout.compute_segment_addresses(text_start);
let symbol_values = expressions::eval_symbol_values(
    source,
    symbol_links,
    layout,
    text_start,
)?;
```

**Benefits:**
- Removes nested loop over all files/lines during each convergence iteration
- Single call evaluates all symbols at once
- Clearer separation: symbol evaluation is completely separate from encoding
- No need to maintain EvaluationContext with mutable state

### Update 2: encoder.rs EncodingContext (lines 59-64)

**Current code:**
```rust
pub struct EncodingContext<'a> {
    pub eval_context: &'a mut EvaluationContext,
    pub layout: &'a crate::layout::Layout,
    pub file_index: usize,
    pub line_index: usize,
}
```

**New code:**
```rust
pub struct EncodingContext<'a> {
    pub source: &'a Source,
    pub symbol_values: &'a SymbolValues,
    pub symbol_links: &'a SymbolLinks,
    pub layout: &'a crate::layout::Layout,
    pub text_start: u32,
    pub data_start: u32,
    pub bss_start: u32,
    pub file_index: usize,
    pub line_index: usize,
}
```

**Changes needed:**
- Replace `eval_context` with direct references to what encoder needs
- Add `source` for error reporting in eval_expr()
- Add segment addresses for computing current address (`.` operator)
- Encoder no longer mutates evaluation state
- All symbol values are pre-computed and read-only during encoding

### Update 3: encoder.rs encode_source() (lines 71-144)

**Current signature:**
```rust
pub fn encode_source(
    source: &mut Source,
    eval_context: &mut EvaluationContext,
    layout: &mut crate::layout::Layout,
    relax: &Relax,
    any_changed: &mut bool,
) -> Result<(Vec<u8>, Vec<u8>, u32)>
```

**New signature:**
```rust
pub fn encode_source(
    source: &Source,
    symbol_values: &SymbolValues,
    symbol_links: &SymbolLinks,
    layout: &mut crate::layout::Layout,
    text_start: u32,
    data_start: u32,
    bss_start: u32,
    relax: &Relax,
    any_changed: &mut bool,
) -> Result<(Vec<u8>, Vec<u8>, u32)>
```

**Implementation notes:**
- Pass pre-computed symbol_values and segment addresses
- Create EncodingContext with new fields (see Update 2)
- Pass `source` reference to EncodingContext for error reporting
- Source no longer needs to be mutable (symbol values are separate)
- All expression evaluation uses new `eval_expr()` API

### Update 4: All eval_expr_old() call sites

Throughout encoder.rs, replace calls like:
```rust
let val = eval_expr_old(expr, line, &pointer, context.eval_context)?;
```

With:
```rust
let pointer = LinePointer {
    file_index: context.file_index,
    line_index: context.line_index,
};
let line_layout = context.layout.get(&pointer).unwrap();
let segment_start = match line_layout.segment {
    Segment::Text => context.text_start,
    Segment::Data => context.data_start,
    Segment::Bss => context.bss_start,
};
let address = segment_start + line_layout.offset;
let refs = context.symbol_links.get_line_refs(&pointer);
let val = eval_expr(expr, address, refs, context.symbol_values, context.source, &pointer)?;
```

**Implementation notes:**
- Need to compute current address from layout + segment addresses
- Get symbol references from symbol_links
- Call new eval_expr() with explicit parameters including source and pointer for error reporting
- Error handling remains the same but now has proper line context

## Phase 3: Update Tests

Tests that use the old API will need updating. Key test files to update:

### expressions_tests.rs

Currently uses `new_evaluation_context()` and `eval_expr_old()`. Update to:
1. Create SymbolValues directly
2. Call `eval_symbol_values()` or manually populate symbol_values for test cases
3. Use new `eval_expr()` function with explicit parameters

### Other test files

Search for uses of deprecated functions:
```bash
grep -r "eval_expr_old\|new_evaluation_context\|evaluate_line_symbols" src/
```

Update each usage to the new API.

## Phase 4: Delete Deprecated Code

Once all deprecation warnings are resolved and deprecated code is identified as
dead code, delete:

1. `EvaluationContext` struct (expressions.rs:30-75)
2. `new_evaluation_context()` function (expressions.rs:78-108)
3. `eval_expr_old()` function (expressions.rs:115-129)
4. `evaluate_line_symbols()` function (expressions.rs:138-151)
5. `resolve_symbol_dependencies()` function (expressions.rs:157-254)
6. `evaluate_expression()` function (expressions.rs:261-568)

Verify no compiler warnings or errors after deletion.

## Summary of Benefits

1. **Cleaner architecture**: Clear data flow through pipeline stages
2. **Better performance**: Symbol evaluation happens once per iteration, not per-line
3. **Simpler code**: No complex mutable context management
4. **More testable**: Pure functions with explicit parameters
5. **Type safety**: Explicit segment addresses instead of hidden state
6. **Read-only data**: Source and symbol_values are immutable during encoding

## Implementation Order

1. Phase 1: Add new code and deprecate old (keep everything working)
   - Add `Source::get_line()` helper method in ast.rs
   - Add `Layout::compute_segment_addresses()` helper in layout.rs
   - Add `SymbolValues` type in expressions.rs
   - Add `eval_symbol_values()` and `eval_symbol_recursive()` functions in expressions.rs
   - Add new `eval_expr()` function in expressions.rs
   - Mark old functions as `#[deprecated]` in expressions.rs

2. Phase 2: Update usage (fix deprecation warnings)
   - Update assembler.rs convergence loop
   - Update encoder.rs EncodingContext
   - Update encoder.rs encode_source()
   - Update all eval_expr_old() call sites throughout encoder.rs
   - Update main.rs if needed (dump callbacks, etc.)

3. Phase 3: Update tests
   - Fix expressions_tests.rs
   - Fix any other test files with deprecation warnings

4. Phase 4: Delete deprecated code
   - Remove deprecated functions and types
   - Verify clean build with no warnings
   - Run full test suite

## Migration Safety

- Keep deprecated code working alongside new code
- Use compiler warnings to guide the migration
- Each phase should result in a working, tested system
- Can pause migration at any phase if needed
