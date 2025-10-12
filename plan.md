# Expression Evaluation Implementation Plan

## Overview

This document provides a detailed plan for implementing expression evaluation for the RISC-V assembler. The implementation will go in `src/expressions.rs` and follows the specification in GUIDE.md section "Expression evaluation".

## Background and Context

Expression evaluation occurs after:
1. **Tokenizing**: Source code → tokens
2. **Parsing**: Tokens → AST (with Expression nodes)
3. **Symbol resolution**: All symbol references are linked to their definition sites via LinePointers
4. **Offset computation**: All lines have concrete offsets within their segments, and segment sizes are known

Expression evaluation happens during or just before code generation, when segment start addresses are known and we need concrete values for:
- Immediate operands in instructions
- Branch/jump targets
- Data directive values (`.byte`, `.4byte`, etc.)
- `.space` and `.balign` sizes
- `.equ` symbol values

## Type System Requirements

The assembler uses a two-type system for expressions:

### Types
- **Integer**: A pure numeric value (from integer literals, or Address - Address)
- **Address**: A memory address (from labels, `.`, or Address ± Integer)

### Type Rules
1. `Address + Address` → **error**
2. `Address - Address` → `Integer`
3. `Address + Integer` → `Address`
4. `Integer + Address` → `Address`
5. `Integer - Integer` → `Integer`
6. `Multiply, Divide, Modulo` → only defined for `Integer`, error on `Address`
7. `LeftShift, RightShift` → only defined for `Integer`, error on `Address`
8. `BitwiseOr, BitwiseAnd, BitwiseXor, BitwiseNot` → only defined for `Integer`, error on `Address`
9. `Negate (unary -)` → only defined for `Integer`, error on `Address`

### Type Sources
- Integer literals → `Integer`
- Labels → `Address`
- `.` (current address) → `Address`
- `.equ` symbols → type depends on the evaluated expression
- `__global_pointer$` → `Address` (special symbol = .data start + 2048)

### Precision Requirements

Any lost precision is a **compile-time error**:

1. **Overflow**: Result exceeds i64::MAX
2. **Underflow**: Result is less than i64::MIN
3. **Right shift precision loss**: Any non-zero bits shifted out
4. **Left shift precision loss**: Any non-sign-extension bits shifted out (RISC-V treats addresses as signed)
5. **Division by zero**: Error
6. **Modulo by zero**: Error

## Data Structures

### Core Types

```rust
use std::collections::HashMap;
use crate::ast::{Expression, Line, LinePointer, Location, Segment, Source};
use crate::error::Result;

/// The type of an evaluated expression value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Integer,
    Address,
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::Integer => write!(f, "Integer"),
            ValueType::Address => write!(f, "Address"),
        }
    }
}

/// A fully evaluated expression with its type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluatedValue {
    pub value: i64,
    pub value_type: ValueType,
}

impl EvaluatedValue {
    pub fn new_integer(value: i64) -> Self {
        Self { value, value_type: ValueType::Integer }
    }

    pub fn new_address(value: i64) -> Self {
        Self { value, value_type: ValueType::Address }
    }
}

impl std::fmt::Display for EvaluatedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x} ({})", self.value, self.value_type)
    }
}

/// A key for uniquely identifying a symbol definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolKey {
    pub name: String,
    pub pointer: LinePointer,
}

/// Special constant for __global_pointer$ (no definition pointer)
pub const SPECIAL_GLOBAL_POINTER: &str = "__global_pointer$";
```

### Evaluation Context

```rust
pub struct EvaluationContext {
    /// The complete source with all files, lines, and resolved symbols
    /// Note: This is NOT mut because we only read from Source during evaluation
    source: Source,

    /// Memoization table: (symbol, definition location) -> evaluated value
    symbol_values: HashMap<SymbolKey, EvaluatedValue>,

    /// Segment start addresses (computed from segment sizes)
    text_start: u64,
    data_start: u64,
    bss_start: u64,
}

impl EvaluationContext {
    /// Get the start address of a segment
    fn segment_start(&self, segment: &Segment) -> u64 {
        match segment {
            Segment::Text => self.text_start,
            Segment::Data => self.data_start,
            Segment::Bss => self.bss_start,
        }
    }

    /// Get a line from the source by pointer
    fn get_line(&self, pointer: &LinePointer) -> Result<&Line> {
        self.source
            .files
            .get(pointer.file_index)
            .and_then(|file| file.lines.get(pointer.line_index))
            .ok_or_else(|| EvaluationError::InvalidLinePointer {
                file_index: pointer.file_index,
                line_index: pointer.line_index,
            })
    }
}
```

Note: The cycle detection stack is passed as a parameter to evaluation functions, not stored in the context, because it's local to each evaluation tree.

## Core Functions

### 1. Context Initialization

```rust
/// Create an evaluation context with segment addresses and seed the symbol table
///
/// # Arguments
/// * `source` - The complete parsed source with all symbols resolved
/// * `text_start` - The starting address for the .text segment (default: 0x100e8)
///
/// # Returns
/// A new EvaluationContext ready for expression evaluation
pub fn new_evaluation_context(source: Source, text_start: u64) -> EvaluationContext {
    // Calculate segment addresses
    let text_size = source.text_size;
    let data_size = source.data_size;

    // data_start = next 4K page boundary after (text_start + text_size)
    let data_start = ((text_start + text_size + 4095) / 4096) * 4096;

    // bss_start = immediately after data
    let bss_start = data_start + data_size;

    EvaluationContext {
        source,
        symbol_values: HashMap::new(),
        text_start,
        data_start,
        bss_start,
    }
}
```

**Algorithm**:
1. Calculate segment addresses:
   - `text_start` is provided (default 0x100e8)
   - `data_start` = next 4K page boundary after (text_start + source.text_size)
   - `bss_start` = data_start + source.data_size
2. Create empty symbol_values HashMap
3. Return context

**Note**: `__global_pointer$` is NOT stored in the symbol table. It is special-cased in `resolve_symbol_value` to avoid needing a LinePointer.

### 2. Symbol Value Resolution

```rust
/// Resolve a symbol to its concrete value, recursively evaluating if needed
///
/// This function implements lazy evaluation with memoization and cycle detection.
///
/// # Arguments
/// * `symbol` - The symbol name to resolve
/// * `pointer` - Where the symbol is defined (from symbol resolution phase)
/// * `context` - The evaluation context (contains source, memoization table, segment addresses)
/// * `cycle_stack` - Stack for detecting circular references
///
/// # Returns
/// The evaluated value with its type (Integer or Address)
///
/// # Errors
/// * CircularReference if the symbol depends on itself
/// * TypeError if the symbol definition contains invalid operations
/// * Other evaluation errors from sub-expressions
fn resolve_symbol_value(
    symbol: &str,
    pointer: &LinePointer,
    context: &mut EvaluationContext,
    cycle_stack: &mut Vec<SymbolKey>,
) -> Result<EvaluatedValue>
```

**Algorithm**:
1. **Special case**: If symbol == `SPECIAL_GLOBAL_POINTER`:
   - Return Address with value (context.data_start + 2048)

2. Create `key = SymbolKey { name: symbol.clone(), pointer: pointer.clone() }`

3. **Check memoization**: If key exists in `context.symbol_values`:
   - Return cached value

4. **Check cycles**: If key exists in `cycle_stack`:
   - Return error: "Circular reference in expression involving symbol '{symbol}'"

5. **Push to cycle stack**: `cycle_stack.push(key.clone())`

6. **Lookup definition**: Get line at pointer from `context.source`:
   - `line = &context.source.files[pointer.file_index].lines[pointer.line_index]`

7. **Evaluate based on line content**:
   - **Label**:
     - Calculate absolute address: `segment_start(line.segment) + line.offset`
     - Result: Address type
   - **Directive::Equ(_, expr)**:
     - Recursively evaluate expr: `evaluate_expression(expr, context, line, cycle_stack)?`
     - Result: whatever type the expression evaluates to
   - Other: Error "Symbol points to non-defining line"

8. **Memoize**: Insert key -> result into `context.symbol_values`

9. **Pop from cycle stack**: `cycle_stack.pop()`

10. Return result

**Helper**: `segment_start(segment: &Segment) -> u64` returns text_start, data_start, or bss_start

### 3. Expression Evaluation

```rust
/// Evaluate an expression to a concrete value with type checking
///
/// This is the core recursive evaluator that walks the expression tree,
/// enforcing type rules and precision requirements.
///
/// # Arguments
/// * `expr` - The expression to evaluate
/// * `context` - The evaluation context (mutable for memoization)
/// * `current_line` - The line containing this expression (for error reporting and `.` resolution)
/// * `cycle_stack` - Stack for cycle detection in symbol references
///
/// # Returns
/// The evaluated value with its type
///
/// # Errors
/// * TypeError for invalid type combinations
/// * Overflow/Underflow for arithmetic errors
/// * PrecisionLoss for shift operations that lose bits
/// * DivisionByZero for division/modulo by zero
/// * CircularReference for cycles in symbol definitions
fn evaluate_expression(
    expr: &Expression,
    context: &mut EvaluationContext,
    current_line: &Line,
    cycle_stack: &mut Vec<SymbolKey>,
) -> Result<EvaluatedValue>
```

**Algorithm** (recursive descent on Expression enum):

1. **Literal(i)**: Return Integer with value i

2. **Identifier(name)**:
   - Find symbol in current_line.outgoing_refs with matching name
   - If not found: error "Unresolved symbol (should have been caught in symbol resolution)"
   - Resolve: `resolve_symbol_value(name, &ref.pointer, context, cycle_stack)?`

3. **CurrentAddress**:
   - Calculate: `segment_start(current_line.segment) + current_line.offset`
   - Return Address type

4. **NumericLabelRef(ref)**:
   - Find the reference in current_line.outgoing_refs (it should be there from symbol resolution)
   - Resolve like Identifier

5. **Parenthesized(expr)**:
   - Recursively evaluate inner expression (parentheses don't change semantics)

6. **PlusOp { lhs, rhs }**:
   - Evaluate both sides
   - Type check:
     - `Integer + Integer` → `Integer`
     - `Address + Integer` → `Address`
     - `Integer + Address` → `Address`
     - `Address + Address` → **error**
   - Check overflow: `lhs.checked_add(rhs)`, error if None
   - Return result with appropriate type

7. **MinusOp { lhs, rhs }**:
   - Evaluate both sides
   - Type check:
     - `Integer - Integer` → `Integer`
     - `Address - Integer` → `Address`
     - `Address - Address` → `Integer`
     - `Integer - Address` → **error**
   - Check underflow: `lhs.checked_sub(rhs)`, error if None
   - Return result with appropriate type

8. **MultiplyOp { lhs, rhs }**:
   - Evaluate both sides
   - Type check: both must be `Integer`, error otherwise
   - Check overflow: `lhs.checked_mul(rhs)`, error if None
   - Return Integer

9. **DivideOp { lhs, rhs }**:
   - Evaluate both sides
   - Type check: both must be `Integer`
   - Check division by zero: error if rhs == 0
   - Check overflow: `lhs.checked_div(rhs)` (only overflows for i64::MIN / -1)
   - Return Integer

10. **ModuloOp { lhs, rhs }**:
    - Evaluate both sides
    - Type check: both must be `Integer`
    - Check modulo by zero: error if rhs == 0
    - Compute: `lhs % rhs`
    - Return Integer

11. **LeftShiftOp { lhs, rhs }**:
    - Evaluate both sides
    - Type check: both must be `Integer`
    - Check shift amount: error if rhs < 0 or rhs >= 64
    - **Precision check**: Verify no non-sign-extension bits are lost
      - For signed left shift by N bits, the top N+1 bits must all be the same
      - Check: `(lhs >> (64 - rhs - 1))` must be 0 or -1
    - Compute: `lhs << rhs`
    - Return Integer

12. **RightShiftOp { lhs, rhs }**:
    - Evaluate both sides
    - Type check: both must be `Integer`
    - Check shift amount: error if rhs < 0 or rhs >= 64
    - **Precision check**: Verify no non-zero bits are lost
      - Check: `(lhs << (64 - rhs)) >> (64 - rhs) == lhs`
      - Or: compute mask `(1i64 << rhs) - 1` and check `lhs & mask == 0`
    - Compute: `lhs >> rhs` (arithmetic right shift for i64)
    - Return Integer

13. **BitwiseOrOp, BitwiseAndOp, BitwiseXorOp { lhs, rhs }**:
    - Evaluate both sides
    - Type check: both must be `Integer`
    - Compute: `lhs | rhs` (or `&`, `^`)
    - Return Integer

14. **NegateOp { expr }**:
    - Evaluate expr
    - Type check: must be `Integer`
    - Check overflow: `-i64::MIN` would overflow
    - Compute: `-value`
    - Return Integer

15. **BitwiseNotOp { expr }**:
    - Evaluate expr
    - Type check: must be `Integer`
    - Compute: `!value`
    - Return Integer

### 4. Line Symbol Evaluation (Pre-pass for a line)

```rust
/// Ensure all symbols referenced by a line are evaluated
///
/// This function resolves all symbols that a line depends on before evaluating
/// the line's expressions. This is called before code generation for each line.
///
/// # Arguments
/// * `line` - The line whose symbol dependencies should be resolved
/// * `context` - The evaluation context
///
/// # Returns
/// Ok(()) if all symbols were successfully resolved
///
/// # Errors
/// Propagates any errors from symbol resolution
pub fn evaluate_line_symbols(
    line: &Line,
    context: &mut EvaluationContext,
) -> Result<()>
```

**Algorithm**:
1. Create empty cycle_stack: `Vec::new()`
2. For each symbol_ref in line.outgoing_refs:
   - Call `resolve_symbol_value(&symbol_ref.symbol, &symbol_ref.pointer, context, &mut cycle_stack)?`
3. Cycle stack should be empty at the end (sanity check with debug_assert!)
4. Return Ok(())

**Note**: This ensures all dependencies are resolved before evaluating the line's expressions.

### 5. Public API for Code Generation

```rust
/// Evaluate an expression in the context of a specific line (public API)
///
/// This is the main entry point for code generation to evaluate expressions.
/// It ensures all symbol dependencies are resolved first.
///
/// # Arguments
/// * `expr` - The expression to evaluate
/// * `line` - The line containing this expression
/// * `context` - The evaluation context
///
/// # Returns
/// The evaluated value with its type
///
/// # Errors
/// Propagates all evaluation errors
///
/// # Example
/// ```
/// let value = eval_expr(&instruction.immediate, &line, &mut context)?;
/// let offset = value.value; // Use the i64 value for code generation
/// ```
pub fn eval_expr(
    expr: &Expression,
    line: &Line,
    context: &mut EvaluationContext,
) -> Result<EvaluatedValue>
```

**Algorithm**:
1. Ensure all symbols are evaluated: `evaluate_line_symbols(line, context)?`
2. Create empty cycle_stack: `Vec::new()`
3. Evaluate: `evaluate_expression(expr, context, line, &mut cycle_stack)?`
4. Sanity check: cycle_stack should be empty (debug_assert!)
5. Return result

**Note**: This is the main entry point that code generation will use.

### 6. Helper Functions for Type-Safe Arithmetic

```rust
/// Perform addition with type checking and overflow detection
fn checked_add(
    lhs: EvaluatedValue,
    rhs: EvaluatedValue,
    location: &Location,
) -> Result<EvaluatedValue> {
    match (lhs.value_type, rhs.value_type) {
        (ValueType::Integer, ValueType::Integer) => {
            let result = lhs.value.checked_add(rhs.value)
                .ok_or_else(|| EvaluationError::Overflow {
                    operation: format!("{} + {}", lhs.value, rhs.value),
                    location: location.clone(),
                })?;
            Ok(EvaluatedValue::new_integer(result))
        }
        (ValueType::Address, ValueType::Integer) | (ValueType::Integer, ValueType::Address) => {
            let result = lhs.value.checked_add(rhs.value)
                .ok_or_else(|| EvaluationError::Overflow {
                    operation: "address + offset".to_string(),
                    location: location.clone(),
                })?;
            Ok(EvaluatedValue::new_address(result))
        }
        (ValueType::Address, ValueType::Address) => {
            Err(EvaluationError::TypeError {
                operation: "addition".to_string(),
                expected: "Address + Integer or Integer + Integer".to_string(),
                got: ValueType::Address,
                location: location.clone(),
            })
        }
    }
}

/// Perform subtraction with type checking and underflow detection
fn checked_sub(
    lhs: EvaluatedValue,
    rhs: EvaluatedValue,
    location: &Location,
) -> Result<EvaluatedValue> {
    match (lhs.value_type, rhs.value_type) {
        (ValueType::Integer, ValueType::Integer) => {
            let result = lhs.value.checked_sub(rhs.value)
                .ok_or_else(|| EvaluationError::Underflow {
                    operation: format!("{} - {}", lhs.value, rhs.value),
                    location: location.clone(),
                })?;
            Ok(EvaluatedValue::new_integer(result))
        }
        (ValueType::Address, ValueType::Integer) => {
            let result = lhs.value.checked_sub(rhs.value)
                .ok_or_else(|| EvaluationError::Underflow {
                    operation: "address - offset".to_string(),
                    location: location.clone(),
                })?;
            Ok(EvaluatedValue::new_address(result))
        }
        (ValueType::Address, ValueType::Address) => {
            let result = lhs.value.checked_sub(rhs.value)
                .ok_or_else(|| EvaluationError::Underflow {
                    operation: "address - address".to_string(),
                    location: location.clone(),
                })?;
            Ok(EvaluatedValue::new_integer(result))
        }
        (ValueType::Integer, ValueType::Address) => {
            Err(EvaluationError::TypeError {
                operation: "subtraction".to_string(),
                expected: "Integer - Integer, Address - Integer, or Address - Address".to_string(),
                got: ValueType::Address,
                location: location.clone(),
            })
        }
    }
}

/// Check that a value is an Integer type, return error if not
fn require_integer(value: EvaluatedValue, operation: &str, location: &Location) -> Result<i64> {
    match value.value_type {
        ValueType::Integer => Ok(value.value),
        ValueType::Address => Err(EvaluationError::TypeError {
            operation: operation.to_string(),
            expected: "Integer".to_string(),
            got: ValueType::Address,
            location: location.clone(),
        }),
    }
}

/// Check if left shift would lose precision
fn check_left_shift_precision(value: i64, shift: i64, location: &Location) -> Result<()> {
    // For a left shift by N bits, check that the top N+1 bits are all the same
    // (all 0s or all 1s, i.e., sign-extension bits)
    if shift >= 63 {
        // Shifting by 63 or more always loses precision unless value is 0 or -1
        if value != 0 && value != -1 {
            return Err(EvaluationError::PrecisionLoss {
                operation: format!("{} << {}", value, shift),
                details: "Would shift out non-sign-extension bits".to_string(),
                location: location.clone(),
            });
        }
        return Ok(());
    }

    // Check if the top (shift + 1) bits are all the same
    let bits_to_check = shift + 1;
    let sign_extended = value >> (64 - bits_to_check);

    if sign_extended != 0 && sign_extended != -1 {
        return Err(EvaluationError::PrecisionLoss {
            operation: format!("{} << {}", value, shift),
            details: "Would shift out non-sign-extension bits".to_string(),
            location: location.clone(),
        });
    }

    Ok(())
}

/// Check if right shift would lose precision
fn check_right_shift_precision(value: i64, shift: i64, location: &Location) -> Result<()> {
    // For a right shift, check that no non-zero bits are shifted out
    // Create a mask for the bottom 'shift' bits
    if shift == 0 {
        return Ok(());
    }

    let mask = if shift >= 64 {
        -1i64 // All bits
    } else {
        (1i64 << shift) - 1
    };

    if (value & mask) != 0 {
        return Err(EvaluationError::PrecisionLoss {
            operation: format!("{} >> {}", value, shift),
            details: format!("Would lose {} non-zero bits", (value & mask).count_ones()),
            location: location.clone(),
        });
    }

    Ok(())
}
```

## Error Handling

```rust
use crate::ast::Location;
use crate::error::Error;
use std::fmt;

/// Errors that can occur during expression evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationError {
    /// Type mismatch in operation
    TypeError {
        operation: String,
        expected: String,
        got: ValueType,
        location: Location,
    },

    /// Arithmetic overflow
    Overflow {
        operation: String,
        location: Location,
    },

    /// Arithmetic underflow
    Underflow {
        operation: String,
        location: Location,
    },

    /// Division by zero
    DivisionByZero {
        location: Location,
    },

    /// Precision lost in shift operation
    PrecisionLoss {
        operation: String,
        details: String,
        location: Location,
    },

    /// Circular reference detected in symbol evaluation
    CircularReference {
        symbol: String,
        chain: Vec<String>,
        location: Location,
    },

    /// Symbol not found (should not happen if symbol resolution worked)
    UnresolvedSymbol {
        symbol: String,
        location: Location,
    },

    /// Invalid shift amount (negative or >= 64)
    InvalidShiftAmount {
        amount: i64,
        location: Location,
    },

    /// Invalid line pointer (internal error)
    InvalidLinePointer {
        file_index: usize,
        line_index: usize,
    },

    /// Symbol definition points to wrong kind of line
    InvalidSymbolDefinition {
        symbol: String,
        location: Location,
    },
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvaluationError::TypeError { operation, expected, got, location } => {
                write!(
                    f,
                    "{}: Type error in {}: expected {}, got {}",
                    location, operation, expected, got
                )
            }
            EvaluationError::Overflow { operation, location } => {
                write!(f, "{}: Arithmetic overflow in {}", location, operation)
            }
            EvaluationError::Underflow { operation, location } => {
                write!(f, "{}: Arithmetic underflow in {}", location, operation)
            }
            EvaluationError::DivisionByZero { location } => {
                write!(f, "{}: Division by zero", location)
            }
            EvaluationError::PrecisionLoss { operation, details, location } => {
                write!(f, "{}: Precision loss in {}: {}", location, operation, details)
            }
            EvaluationError::CircularReference { symbol, chain, location } => {
                write!(
                    f,
                    "{}: Circular reference in symbol '{}': {}",
                    location,
                    symbol,
                    chain.join(" -> ")
                )
            }
            EvaluationError::UnresolvedSymbol { symbol, location } => {
                write!(
                    f,
                    "{}: Unresolved symbol '{}' (internal error - should have been caught earlier)",
                    location, symbol
                )
            }
            EvaluationError::InvalidShiftAmount { amount, location } => {
                write!(
                    f,
                    "{}: Invalid shift amount {} (must be 0..64)",
                    location, amount
                )
            }
            EvaluationError::InvalidLinePointer { file_index, line_index } => {
                write!(
                    f,
                    "Internal error: invalid line pointer [{}:{}]",
                    file_index, line_index
                )
            }
            EvaluationError::InvalidSymbolDefinition { symbol, location } => {
                write!(
                    f,
                    "{}: Symbol '{}' definition points to invalid line",
                    location, symbol
                )
            }
        }
    }
}

impl std::error::Error for EvaluationError {}

// Convert EvaluationError to the assembler's main Error type
impl From<EvaluationError> for Error {
    fn from(err: EvaluationError) -> Self {
        Error::Evaluation(err)
    }
}
```

**Note**: This assumes that `src/error.rs` defines a main `Error` enum with an `Evaluation(EvaluationError)` variant. All errors include the Location from the current line for error reporting with source context.

## Module Structure

The implementation will be organized in `src/expressions.rs` with the following structure:

```rust
// src/expressions.rs

//! Expression evaluation with type checking and precision requirements
//!
//! This module implements lazy evaluation of expressions in the RISC-V assembler.
//! It enforces a two-type system (Integer and Address) with strict type checking
//! and precision loss detection.

use std::collections::HashMap;
use crate::ast::{Expression, Line, LineContent, LinePointer, Location, Segment, Source, Directive};
use crate::error::Result;

// ============================================================================
// Public Types
// ============================================================================

pub enum ValueType { ... }
pub struct EvaluatedValue { ... }
pub enum EvaluationError { ... }

// ============================================================================
// Evaluation Context
// ============================================================================

pub struct EvaluationContext { ... }

impl EvaluationContext {
    fn segment_start(&self, segment: &Segment) -> u64 { ... }
    fn get_line(&self, pointer: &LinePointer) -> Result<&Line> { ... }
}

// ============================================================================
// Public API
// ============================================================================

/// Create an evaluation context with segment addresses
pub fn new_evaluation_context(source: Source, text_start: u64) -> EvaluationContext { ... }

/// Evaluate an expression in the context of a specific line
pub fn eval_expr(
    expr: &Expression,
    line: &Line,
    context: &mut EvaluationContext,
) -> Result<EvaluatedValue> { ... }

/// Ensure all symbols referenced by a line are evaluated
pub fn evaluate_line_symbols(
    line: &Line,
    context: &mut EvaluationContext,
) -> Result<()> { ... }

// ============================================================================
// Internal Implementation
// ============================================================================

/// Internal type for cycle detection
struct SymbolKey { ... }

/// Resolve a symbol to its concrete value, recursively evaluating if needed
fn resolve_symbol_value(
    symbol: &str,
    pointer: &LinePointer,
    context: &mut EvaluationContext,
    cycle_stack: &mut Vec<SymbolKey>,
) -> Result<EvaluatedValue> { ... }

/// Evaluate an expression to a concrete value with type checking
fn evaluate_expression(
    expr: &Expression,
    context: &mut EvaluationContext,
    current_line: &Line,
    cycle_stack: &mut Vec<SymbolKey>,
) -> Result<EvaluatedValue> { ... }

// ============================================================================
// Type-Safe Arithmetic Helpers
// ============================================================================

fn checked_add(lhs: EvaluatedValue, rhs: EvaluatedValue, location: &Location) -> Result<EvaluatedValue> { ... }
fn checked_sub(lhs: EvaluatedValue, rhs: EvaluatedValue, location: &Location) -> Result<EvaluatedValue> { ... }
fn require_integer(value: EvaluatedValue, operation: &str, location: &Location) -> Result<i64> { ... }
fn check_left_shift_precision(value: i64, shift: i64, location: &Location) -> Result<()> { ... }
fn check_right_shift_precision(value: i64, shift: i64, location: &Location) -> Result<()> { ... }

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    // Test utilities and test cases
}
```

### Integration with Main Error Type

In `src/error.rs`, add a variant to the main Error enum:

```rust
// src/error.rs

pub enum Error {
    // ... existing variants ...
    Evaluation(expressions::EvaluationError),
}
```

## Integration with Code Generation

### Initialization

At the start of code generation:
1. Create `EvaluationContext` with computed segment addresses
2. Store in code generation state

### During Code Generation

For each line:
1. **Before encoding**: Call `evaluate_line_symbols(line, context)?`
2. **When evaluating expressions**: Call `eval_expr(expr, line, context)?`
3. **For label/equ definitions**: The value is automatically memoized during symbol resolution

### After Code Generation

The `context.symbol_values` table contains all evaluated symbols and can be used to:
- Populate the ELF symbol table with concrete values
- Debug output showing all symbol values

## Display/Output Enhancements

Update the `Display` implementation for `Source` (in src/ast.rs around line 682) to show evaluated values:

### For Each Line

After showing the line content, if the line defines a symbol (Label or Equ):
- Show: `= 0x<address> (<type>)`
- Example: `.text+0: main: = 0x100e8 (Address)`

### For Global Symbols

In the "Exported symbols" section, show:
- Symbol name → definition location → value and type
- Example: `main -> [0:0] = 0x100e8 (Address)`

### For Expressions

When displaying expressions in instructions/directives during debug output, optionally show evaluated value:
- Example: `.text+4: addi a0, zero, counter + 4 [= 0x12004 (Address)]`

**Implementation note**: This requires passing an `Option<&EvaluationContext>` to the Display implementation, or creating a separate debug printer function.

## Testing Strategy

### Test Framework

Create test utilities in `src/expressions.rs` or `tests/expressions.rs`:

```rust
/// Create a minimal Source structure for testing
fn make_test_source(
    lines: Vec<(Segment, u64, LineContent)>,
    segment_sizes: (u64, u64, u64),
) -> Source

/// Create a test evaluation context
fn make_test_context(source: Source) -> EvaluationContext

/// Helper to evaluate an expression string (parse + evaluate)
fn eval_test_expr(expr_str: &str, context: &mut EvaluationContext) -> Result<EvaluatedValue>
```

### Test Categories and Specific Tests

#### 1. Type System Tests

**Test: literal_is_integer**
- Expression: `42`
- Expected: Integer(42)

**Test: label_is_address**
- Setup: Label "foo" at .text+0
- Expression: identifier "foo"
- Expected: Address(0x100e8) (assuming default text_start)

**Test: current_address_is_address**
- Setup: Line at .text+16
- Expression: `.`
- Expected: Address(0x100f8)

**Test: address_plus_integer**
- Setup: Label "foo" at .text+0
- Expression: `foo + 4`
- Expected: Address(0x100ec)

**Test: integer_plus_address**
- Setup: Label "foo" at .text+0
- Expression: `4 + foo`
- Expected: Address(0x100ec)

**Test: address_minus_integer**
- Setup: Label "foo" at .text+16
- Expression: `foo - 8`
- Expected: Address(0x100f0)

**Test: address_minus_address**
- Setup: Label "foo" at .text+0, Label "bar" at .text+16
- Expression: `bar - foo`
- Expected: Integer(16)

**Test: address_plus_address_error**
- Setup: Two labels
- Expression: `foo + bar`
- Expected: Error (TypeError)

**Test: integer_minus_address_error**
- Setup: Label "foo"
- Expression: `8 - foo`
- Expected: Error (TypeError)

#### 2. Arithmetic Operations

**Test: integer_multiply**
- Expression: `6 * 7`
- Expected: Integer(42)

**Test: integer_divide**
- Expression: `42 / 7`
- Expected: Integer(6)

**Test: integer_modulo**
- Expression: `43 % 7`
- Expected: Integer(1)

**Test: multiply_address_error**
- Setup: Label "foo"
- Expression: `foo * 2`
- Expected: Error (TypeError: multiply requires Integer)

**Test: division_by_zero_error**
- Expression: `42 / 0`
- Expected: Error (DivisionByZero)

**Test: modulo_by_zero_error**
- Expression: `42 % 0`
- Expected: Error (ModuloByZero)

#### 3. Bitwise Operations

**Test: bitwise_or**
- Expression: `0x0f | 0xf0`
- Expected: Integer(0xff)

**Test: bitwise_and**
- Expression: `0xff & 0x0f`
- Expected: Integer(0x0f)

**Test: bitwise_xor**
- Expression: `0xff ^ 0x0f`
- Expected: Integer(0xf0)

**Test: bitwise_not**
- Expression: `~0`
- Expected: Integer(-1)

**Test: bitwise_on_address_error**
- Setup: Label "foo"
- Expression: `foo | 0xff`
- Expected: Error (TypeError)

#### 4. Shift Operations

**Test: left_shift_simple**
- Expression: `1 << 4`
- Expected: Integer(16)

**Test: right_shift_simple**
- Expression: `16 >> 2`
- Expected: Integer(4)

**Test: arithmetic_right_shift**
- Expression: `-8 >> 1`
- Expected: Integer(-4) (sign-extended)

**Test: shift_address_error**
- Setup: Label "foo"
- Expression: `foo << 2`
- Expected: Error (TypeError)

**Test: shift_negative_amount_error**
- Expression: `8 << -1`
- Expected: Error (InvalidShiftAmount)

**Test: shift_too_large_error**
- Expression: `8 << 64`
- Expected: Error (InvalidShiftAmount)

#### 5. Precision Loss Detection

**Test: overflow_addition**
- Expression: `9223372036854775807 + 1` (i64::MAX + 1)
- Expected: Error (Overflow)

**Test: underflow_subtraction**
- Expression: `-9223372036854775808 - 1` (i64::MIN - 1)
- Expected: Error (Underflow)

**Test: overflow_multiplication**
- Expression: `9223372036854775807 * 2`
- Expected: Error (Overflow)

**Test: overflow_division_edge_case**
- Expression: `-9223372036854775808 / -1` (i64::MIN / -1 overflows)
- Expected: Error (Overflow)

**Test: overflow_negation**
- Expression: `-(-9223372036854775808)` (negate i64::MIN)
- Expected: Error (Overflow)

**Test: left_shift_precision_loss**
- Expression: `0x4000000000000000 << 2` (shifts out a 1 bit)
- Expected: Error (PrecisionLoss)

**Test: left_shift_sign_extension_ok**
- Expression: `-1 << 4` (all bits are sign bits)
- Expected: Integer(-16)

**Test: right_shift_precision_loss**
- Expression: `15 >> 2` (loses bits: 15 = 0b1111, >> 2 = 0b11, lost 0b11)
- Expected: Error (PrecisionLoss)

**Test: right_shift_no_loss**
- Expression: `16 >> 2` (16 = 0b10000, >> 2 = 0b100, no bits lost)
- Expected: Integer(4)

#### 6. Unary Operations

**Test: negate_positive**
- Expression: `-42`
- Expected: Integer(-42)

**Test: negate_negative**
- Expression: `-(-42)`
- Expected: Integer(42)

**Test: negate_address_error**
- Setup: Label "foo"
- Expression: `-foo`
- Expected: Error (TypeError)

**Test: bitwise_not_simple**
- Expression: `~0xff`
- Expected: Integer(-256)

#### 7. Parentheses and Precedence

**Test: parentheses_override_precedence**
- Expression: `2 + 3 * 4`
- Expected: Integer(14)

**Test: parentheses_explicit**
- Expression: `(2 + 3) * 4`
- Expected: Integer(20)

**Test: nested_parentheses**
- Expression: `((2 + 3) * (4 + 5))`
- Expected: Integer(45)

#### 8. Complex Expressions

**Test: address_arithmetic_complex**
- Setup: Label "foo" at .text+100
- Expression: `foo + (4 * 8) - 12`
- Expected: Address(0x100e8 + 100 + 32 - 12) = Address(0x1014c)

**Test: mixed_operations**
- Expression: `(10 + 20) * 2 - 5`
- Expected: Integer(55)

#### 9. Symbol Resolution and Equ

**Test: equ_integer_value**
- Setup: `.equ counter, 42`
- Expression: `counter`
- Expected: Integer(42)

**Test: equ_address_value**
- Setup: `.equ ptr, foo` where foo is a label at .text+0
- Expression: `ptr`
- Expected: Address(0x100e8)

**Test: equ_expression**
- Setup: `.equ value, 10 + 20`
- Expression: `value * 2`
- Expected: Integer(60)

**Test: equ_forward_reference**
- Setup: `.equ a, b + 1` then `.equ b, 5`
- Expression: `a`
- Expected: Integer(6)

**Test: equ_chained**
- Setup: `.equ a, 1`, `.equ b, a + 1`, `.equ c, b + 1`
- Expression: `c`
- Expected: Integer(3)

#### 10. Cycle Detection

**Test: direct_cycle**
- Setup: `.equ a, a + 1`
- Expression: `a`
- Expected: Error (CircularReference)

**Test: indirect_cycle**
- Setup: `.equ a, b + 1`, `.equ b, a + 1`
- Expression: `a`
- Expected: Error (CircularReference)

**Test: three_way_cycle**
- Setup: `.equ a, b + 1`, `.equ b, c + 1`, `.equ c, a + 1`
- Expression: `a`
- Expected: Error (CircularReference)

**Test: no_false_positive_cycle**
- Setup: `.equ a, 1`, use `a` in two different expressions
- Both expressions evaluate successfully
- Expected: No cycle error

#### 11. Special Symbols

**Test: global_pointer_symbol**
- Setup: .data segment starts at 0x12000
- Expression: `__global_pointer$`
- Expected: Address(0x12000 + 2048) = Address(0x12800)

**Test: global_pointer_in_expression**
- Expression: `__global_pointer$ + 100`
- Expected: Address(data_start + 2048 + 100)

**Test: user_cannot_define_global_pointer**
- Setup: Attempt to create label or .equ named `__global_pointer$`
- Expected: Error (should be caught earlier in symbol resolution, but test here too)

#### 12. Current Address

**Test: current_address_text_segment**
- Setup: Line at .text+32
- Expression: `.`
- Expected: Address(0x100e8 + 32) = Address(0x10108)

**Test: current_address_data_segment**
- Setup: .text size = 256, line at .data+16
- Expression: `.`
- Expected: Address(data_start + 16)

**Test: current_address_in_expression**
- Setup: Line at .text+32
- Expression: `. + 4`
- Expected: Address(text_start + 32 + 4)

**Test: pc_relative_offset**
- Setup: Line at .text+32, label "target" at .text+64
- Expression: `target - .`
- Expected: Integer(32)

#### 13. Numeric Label References

**Test: numeric_label_forward_ref**
- Setup: Numeric label "1" at .text+100, reference "1f" at .text+0
- Expression at .text+0: `1f`
- Expected: Address(0x100e8 + 100)

**Test: numeric_label_backward_ref**
- Setup: Numeric label "1" at .text+0, reference "1b" at .text+100
- Expression at .text+100: `1b`
- Expected: Address(0x100e8)

#### 14. Multiple Segments

**Test: cross_segment_reference**
- Setup: Label "data_var" at .data+0, reference from .text
- Expression: `data_var`
- Expected: Address(data_start)

**Test: bss_segment_label**
- Setup: Label "bss_var" at .bss+16
- Expression: `bss_var`
- Expected: Address(bss_start + 16)

#### 15. Memoization

**Test: multiple_evaluations_cached**
- Setup: `.equ expensive, 100 * 200` used in multiple expressions
- Verify: Symbol is only evaluated once (can track with a counter or debug output)
- Expected: All evaluations return Integer(20000)

#### 16. Integration Tests

**Test: full_program_evaluation**
- Setup: Complete test program with:
  - Multiple labels in different segments
  - Several .equ definitions
  - Instructions with expressions
  - Data directives with expressions
- Verify: All expressions evaluate correctly

**Test: realistic_address_calculations**
- Setup: Typical patterns like:
  - `la a0, __global_pointer$` loads
  - PC-relative offsets for branches
  - Data directive arrays with `.4byte label1, label2, label3`
- Verify: All addresses are correct

## Open Questions and Uncertainties

### 1. Handling of `__global_pointer$` LinePointer

**Question**: How to represent "no definition pointer" for the special `__global_pointer$` symbol?

**Options**:
- a) Special-case in `resolve_symbol_value`, don't put in memoization table
- b) Use sentinel values in LinePointer (file_index: usize::MAX, line_index: usize::MAX)
- c) Change SymbolKey to use Option<LinePointer>

**Recommendation**: Option (a) - special-case in lookup. It's cleaner and the symbol is checked once per program.

**Decision needed before implementation**: Yes

### 2. Shift Operation Semantics

**Question**: Are shifts arithmetic or logical for negative numbers?

**Context**: Rust's `>>` on i64 is arithmetic (sign-extending), which matches RISC-V semantics.

**Resolution**: Use Rust's default i64 shift operations (arithmetic).

**Decision needed**: No, already clear from RISC-V spec

### 3. Right Shift Precision Loss Detection

**Question**: Exact algorithm for detecting precision loss in right shifts?

**Proposed**: For `value >> amount`, precision is lost if `(value << (64 - amount)) >> (64 - amount) != value`, which simplifies to checking if any of the rightmost `amount` bits are non-zero.

**Alternative**: Create mask `(1i64 << amount) - 1` and check `value & mask == 0`

**Recommendation**: Use the mask approach for clarity

**Decision needed**: Implementation detail, can be decided during coding

### 4. Left Shift Precision Loss Detection

**Question**: Exact algorithm for detecting precision loss in left shifts?

**Context**: RISC-V treats addresses as signed. Left shift loses precision if non-sign-extension bits are shifted out.

**Proposed**: For `value << amount`, check that the top `amount + 1` bits are all the same (all 0s or all 1s). This can be checked by verifying that `value >> (63 - amount)` is either 0 or -1.

**Alternative**: After shifting, shift back and compare: `(value << amount) >> amount == value` (but this may overflow)

**Recommendation**: Use the first approach (check top bits before shifting)

**Decision needed**: Implementation detail, can be decided during coding

### 5. Error Location Reporting

**Question**: Should errors report the location of the line being evaluated, or the specific sub-expression?

**Context**: Our Location struct only has file and line number, not column information.

**Resolution**: Report the line where the expression appears. For better error messages, include the full expression in the error text.

**Decision needed**: No, proceed with line-level location

### 6. Testing Without Full Assembler

**Question**: How to test expressions without implementing full code generation?

**Proposed approach**:
1. Create minimal test harness that:
   - Manually constructs Source structures with known offsets
   - Creates EvaluationContext with fixed segment addresses
   - Calls evaluation functions directly
2. Use integration tests later with real assembly files

**Decision needed**: No, proceed with unit testing approach

### 7. Display Implementation Details

**Question**: How to add evaluated values to Display output without breaking existing code?

**Options**:
- a) Add optional EvaluationContext parameter (requires changing function signature)
- b) Create separate debug printer function
- c) Store evaluation results in AST (mutates AST)

**Recommendation**: Option (b) - create `display_with_values(source: &Source, context: &EvaluationContext)` function

**Decision needed**: Implementation detail, can be decided during coding

### 8. Forbidding User Definition of `__global_pointer$`

**Question**: Where to enforce that users cannot define `__global_pointer$`?

**Options**:
- a) During symbol resolution (earlier is better)
- b) During expression evaluation (catches it but later)

**Recommendation**: During symbol resolution (in symbols.rs), add a check when processing label or .equ definitions

**Decision needed**: Yes, this should be enforced in symbol resolution, not expression evaluation

### 9. Expression Evaluation API Design

**Question**: Should the public API take `&Line` or just the specific expression?

**Context**: We need the line for:
- Current address (`.`)
- Looking up outgoing symbol references
- Error reporting

**Resolution**: Public API takes `&Line` and `&Expression`

**Decision needed**: No, already designed above

### 10. Order of Operations for Symbol Resolution

**Question**: When exactly are symbols in the memoization table populated?

**Answer from GUIDE.md**:
- Labels: When their defining line is processed during code generation
- .equ: When first referenced (lazy evaluation)

**Clarification needed**: Should we pre-populate all labels at the start, or evaluate lazily?

**Recommendation**: Lazy evaluation for both - simpler and matches the spec

**Decision needed**: No, proceed with lazy evaluation

## Implementation Order

Suggested order for implementing components:

1. **Data structures** (ValueType, EvaluatedValue, SymbolKey, EvaluationContext, EvaluationError)
2. **Context initialization** (new_evaluation_context)
3. **Basic expression evaluation** (literals, current address, arithmetic on integers)
4. **Type system enforcement** (tests for type checking)
5. **Symbol resolution** (resolve_symbol_value for labels)
6. **Precision checking** (overflow, underflow, shift precision)
7. **.equ support** (evaluate equate expressions)
8. **Cycle detection** (detect circular references)
9. **Special symbols** (`__global_pointer$`)
10. **Public API** (eval_expr, evaluate_line_symbols)
11. **Display enhancements** (show evaluated values in output)
12. **Comprehensive tests** (all test cases listed above)

## Integration Checklist

Before integrating with code generation:

- [ ] All unit tests pass
- [ ] Cycle detection tested with various cases
- [ ] Type checking catches all invalid operations
- [ ] Precision loss detection works for all cases
- [ ] Special symbols handled correctly
- [ ] Memoization prevents redundant evaluation
- [ ] Error messages are clear and include location
- [ ] Display output shows evaluated values
- [ ] Documentation complete
- [ ] GUIDE.md requirements all satisfied

## Summary

This plan provides a complete roadmap for implementing expression evaluation in `src/expressions.rs`.

### Key Components

**Public API** (3 functions):
- `new_evaluation_context(source, text_start)` - Initialize evaluation
- `eval_expr(expr, line, context)` - Evaluate a single expression
- `evaluate_line_symbols(line, context)` - Pre-resolve all symbols for a line

**Core Types**:
- `ValueType` - Integer or Address
- `EvaluatedValue` - A typed value (i64 + ValueType)
- `EvaluationError` - Comprehensive error enum with location info
- `EvaluationContext` - Holds source, memoization table, and segment addresses

**Type System Rules**:
- Address ± Integer = Address
- Address - Address = Integer
- Integer ops on Integers only
- Strict precision checking (no bit loss in shifts, no overflow/underflow)

**Implementation Features**:
1. Lazy evaluation with memoization
2. Cycle detection for `.equ` circular references
3. Special handling for `__global_pointer$`
4. Comprehensive type checking
5. Overflow/underflow detection
6. Precision loss detection for shifts
7. Clear error messages with source locations

### Testing

40+ specific test cases covering:
- Type system (8 tests)
- Arithmetic operations (5 tests)
- Bitwise operations (5 tests)
- Shift operations (4 tests)
- Precision loss detection (9 tests)
- Unary operations (3 tests)
- Parentheses and precedence (3 tests)
- Complex expressions (2 tests)
- Symbol resolution and .equ (4 tests)
- Cycle detection (4 tests)
- Special symbols (3 tests)
- Current address (4 tests)
- Numeric label references (2 tests)
- Multiple segments (2 tests)
- Memoization (1 test)
- Integration (2 tests)

### Integration Points

**With Code Generation**:
1. Initialize context at start of codegen
2. Call `evaluate_line_symbols()` before encoding each line
3. Call `eval_expr()` for each expression that needs a concrete value
4. Use `context.symbol_values` to populate ELF symbol table

**With Error System**:
- Add `Error::Evaluation(EvaluationError)` variant to main error type
- All errors include Location for source context

**With Display**:
- Optionally enhance `Source::Display` to show evaluated values
- Create helper function `display_with_values()` for debug output

### Implementation Order

1. Data structures (types, context, errors)
2. Context initialization
3. Basic expression evaluation (literals, arithmetic)
4. Type system enforcement
5. Symbol resolution (labels, identifiers)
6. Precision checking (overflow, underflow, shifts)
7. .equ support
8. Cycle detection
9. Special symbols (`__global_pointer$`)
10. Public API
11. Display enhancements
12. Comprehensive tests

An implementer should be able to follow this plan step-by-step, implementing each function with the exact signatures provided, confident that it matches the assembler's requirements from GUIDE.md.
