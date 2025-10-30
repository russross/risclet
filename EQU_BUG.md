# EQU Label Reference Bug

## Problem Summary

The `.equ` directive cannot reference labels. When a `.equ` tries to use a label name (whether defined before or after), symbol linking fails to populate the reference in the Symbols struct.

### Example That Fails
```asm
.text
mylabel:
    nop

.equ val, mylabel      # Error: Unresolved symbol 'mylabel'

.global _start
_start:
    li a0, val
```

Error during convergence:
```
Error at line: Unresolved symbol 'mylabel' (internal error - should have been caught earlier)
```

## Root Cause Analysis

### When This Regressed

- **Working:** Commit 161b235 (`splitting symbol linkage from source data`)
- **Broken:** Commit cc047f5 and later (ELF refactoring onwards)

Testing shows the regression happened between these commits, likely in one of the intermediate refactorings that modified symbol linking.

### How Symbol References Should Be Linked

During symbol linking (`symbols.rs::link_file`), for each line:

1. **Extract references** from expressions using `extract_references_from_line(line)`
2. **For each symbol reference** found:
   - Check if it's in `definitions` map (already seen)
   - If found: Add `SymbolReference` to `line.outgoing_refs`
   - If not found: Add to `unresolved` list for later patching

3. **When a symbol is defined**:
   - Scan `unresolved` list for matching references
   - Schedule patches to resolve forward references

4. **At end of file**:
   - Apply all patches: add `SymbolReference` entries to appropriate lines

### The Bug: References Not Being Populated

For the test case:
```
Line 1: mylabel:              ← Label definition
Line 2:     nop
Line 3: .equ val, mylabel     ← Should extract "mylabel" reference
```

**What should happen in link_file:**

**Processing Line 1:**
- Content: `LineContent::Label("mylabel")`
- Extract refs: none
- Handle definition: `definitions.insert("mylabel", ptr_to_line_1)`
- Line 1's `outgoing_refs`: [] (empty)

**Processing Line 3:**
- Content: `LineContent::Directive(Directive::Equ("val", expr))`
- Extract refs: `extract_references_from_line()` should return `["mylabel"]`
- Check `definitions.get("mylabel")` → FOUND (from line 1)
- Add: `line.outgoing_refs.push(SymbolReference { symbol: "mylabel", pointer: ptr_to_line_1 })`
- Handle definition: `definitions.insert("val", ptr_to_line_3)`
- Line 3's `outgoing_refs`: `[SymbolReference { "mylabel", ... }]`

**After symbol linking:**
- `Symbols.line_refs[0][2]` (line 3) should contain the reference to `"mylabel"`

**During convergence (in expressions.rs):**
- When evaluating line 3's expression:
  - `evaluate_line_symbols(line_3, ptr_to_line_3, context)`
  - Gets refs from `context.symbols.get_line_refs(&ptr_to_line_3)`
  - Should find the `"mylabel"` reference and evaluate it
  - Then evaluate the `.equ` expression using the cached value

**What's actually happening:**
- Line 3's `outgoing_refs` is empty: `[]`
- When evaluating the expression, it can't find the reference
- Error: "Unresolved symbol 'mylabel'"

### Why References Aren't Being Extracted

The issue is likely in one of these areas:

1. **`extract_references_from_line()` not extracting from .equ expressions**
   - It should call `extract_from_expression()` for `.equ` directive
   - Current code at line 521-523 does handle this

2. **References extracted but not being added to line.outgoing_refs**
   - The loop at 226-264 should handle this
   - Checks `definitions.get()` and adds if found

3. **Symbol linking modified to skip label references in .equ**
   - Possible that intermediate refactoring added a filter

4. **Symbols struct not being populated correctly from line.outgoing_refs**
   - Lines 66-69 and 129-135 copy `line.outgoing_refs` to `Symbols.line_refs`
   - But if `line.outgoing_refs` is empty, Symbols will be empty too

### The Most Likely Cause (REVISED)

**PREVIOUS HYPOTHESIS (INCORRECT):** Symbol linking not populating refs

**CURRENT FINDING:** The bug is **NOT in symbol linking**. Unit test `test_equ_referencing_label` PASSES, confirming that:
- `extract_references_from_line()` correctly extracts label identifiers
- Reference matching logic correctly finds labels in `definitions`
- `line.outgoing_refs` is correctly populated with the label reference

**NEW HYPOTHESIS:** The bug is in **expression evaluation or Symbols struct usage** during convergence:
- Symbol linking correctly populates `line.outgoing_refs`
- Symbols struct is built from `line.outgoing_refs` (should be correct)
- But during convergence, when evaluating `.equ` expressions, the reference lookup fails
- This suggests either:
  - Symbols struct is not being populated correctly from `line.outgoing_refs`
  - Or the lookup in `context.symbols.get_line_refs()` is using wrong coordinates

## Evidence

1. Error occurs during **convergence**, not symbol linking
2. Error message comes from `evaluate_expression()` which can't find symbol in `Symbols.get_line_refs()`
3. Working commit (161b235) successfully evaluates `.equ` with numeric values
4. Broken commit (cc047f5+) fails on `.equ` with label values
5. The failure happens specifically when `.equ` references a label (identifier), not a literal
6. **Oct 28 UPDATE:** Unit test `test_equ_referencing_label` PASSES
   - Confirms symbol linking correctly populates `line.outgoing_refs`
   - End-to-end assembly still fails during convergence
   - **Bug is NOT in symbol linking, but in expression evaluation phase**

## Next Steps to Fix

1. Add debug logging to `link_file()` to see:
   - What references are extracted from `.equ` lines
   - Whether those references are being found in `definitions`
   - Whether they're being added to `line.outgoing_refs`

2. Compare symbol linking logic between working (161b235) and broken (cc047f5) commits

3. Check if there's a filter that excludes certain symbol types during reference extraction

4. Verify that label definitions are being added to `definitions` map before `.equ` processing

## Impact

- `.equ` directives cannot reference labels
- Any assembly code using `.equ symbol_name, label_name` will fail
- Workaround: Only use `.equ` with numeric literal expressions or previously-defined `.equ` symbols

