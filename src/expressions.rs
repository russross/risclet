// expressions.rs
//
// Expression evaluation with type checking and precision requirements
//
// This module implements lazy evaluation of expressions in the RISC-V assembler.
// It enforces a two-type system (Integer and Address) with strict type checking
// and precision loss detection.

use crate::ast::{
    Directive, Expression, Line, LineContent, LinePointer, Location, Segment,
    Source,
};
use crate::error::AssemblerError;
use std::collections::HashMap;
use std::fmt;

// Type alias for Result with AssemblerError
type Result<T> = std::result::Result<T, AssemblerError>;

// ============================================================================
// Public Types
// ============================================================================

/// The type of an evaluated expression value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Integer,
    Address,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl fmt::Display for EvaluatedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x} ({})", self.value, self.value_type)
    }
}

// ============================================================================
// Internal Types
// ============================================================================

/// A key for uniquely identifying a symbol definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SymbolKey {
    name: String,
    pointer: LinePointer,
}

/// Special constant for __global_pointer$ (no definition pointer)
const SPECIAL_GLOBAL_POINTER: &str = "__global_pointer$";

// ============================================================================
// Evaluation Context
// ============================================================================

pub struct EvaluationContext {
    /// The complete source with all files, lines, and resolved symbols
    /// Note: This is NOT mut because we only read from Source during evaluation
    source: Source,

    /// Memoization table: (symbol, definition location) -> evaluated value
    symbol_values: HashMap<SymbolKey, EvaluatedValue>,

    /// Segment start addresses (computed from segment sizes)
    pub text_start: i64,
    pub data_start: i64,
    pub bss_start: i64,
}

impl EvaluationContext {
    /// Get the start address of a segment
    fn segment_start(&self, segment: &Segment) -> i64 {
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
            .ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Internal error: invalid line pointer [{}:{}]",
                        pointer.file_index, pointer.line_index
                    ),
                    Location { file: "unknown".to_string(), line: 0 },
                )
            })
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Create an evaluation context with segment addresses and seed the symbol table
///
/// # Arguments
/// * `source` - The complete parsed source with all symbols resolved
/// * `text_start` - The starting address for the .text segment (default: 0x100e8)
///
/// # Returns
/// A new EvaluationContext ready for expression evaluation
pub fn new_evaluation_context(
    source: Source,
    text_start: i64,
) -> EvaluationContext {
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
/// ```ignore
/// let value = eval_expr(&instruction.immediate, &line, &mut context)?;
/// let offset = value.value; // Use the i64 value for code generation
/// ```
pub fn eval_expr(
    expr: &Expression,
    line: &Line,
    context: &mut EvaluationContext,
) -> Result<EvaluatedValue> {
    // Ensure all symbols are evaluated
    evaluate_line_symbols(line, context)?;

    // Create empty cycle stack
    let mut cycle_stack = Vec::new();

    // Evaluate the expression
    let result = evaluate_expression(expr, context, line, &mut cycle_stack)?;

    // Sanity check: cycle_stack should be empty
    debug_assert!(
        cycle_stack.is_empty(),
        "Cycle stack not empty after evaluation"
    );

    Ok(result)
}

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
) -> Result<()> {
    let mut cycle_stack = Vec::new();

    for sym_ref in &line.outgoing_refs {
        resolve_symbol_value(
            &sym_ref.symbol,
            &sym_ref.pointer,
            context,
            &mut cycle_stack,
        )?;
    }

    // Sanity check
    debug_assert!(
        cycle_stack.is_empty(),
        "Cycle stack not empty after line symbol evaluation"
    );

    Ok(())
}

// ============================================================================
// Internal Implementation
// ============================================================================

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
) -> Result<EvaluatedValue> {
    // Special case: __global_pointer$
    if symbol == SPECIAL_GLOBAL_POINTER {
        let gp_value = context.data_start + 2048;
        return Ok(EvaluatedValue::new_address(gp_value));
    }

    // Create key
    let key = SymbolKey { name: symbol.to_string(), pointer: pointer.clone() };

    // Check memoization
    if let Some(&value) = context.symbol_values.get(&key) {
        return Ok(value);
    }

    // Check for cycles
    if cycle_stack.contains(&key) {
        let cycle_chain: Vec<String> =
            cycle_stack.iter().map(|k| k.name.clone()).collect();
        let line = context.get_line(pointer)?;
        return Err(AssemblerError::from_context(
            format!(
                "Circular reference in symbol '{}': {} -> {}",
                symbol,
                cycle_chain.join(" -> "),
                symbol
            ),
            line.location.clone(),
        ));
    }

    // Push to cycle stack
    cycle_stack.push(key.clone());

    // Get the line where this symbol is defined
    let line = context.get_line(pointer)?;

    // Evaluate based on line content
    let result = match &line.content {
        LineContent::Label(_) => {
            // Calculate absolute address
            let absolute_addr =
                context.segment_start(&line.segment) + line.offset;
            EvaluatedValue::new_address(absolute_addr)
        }
        LineContent::Directive(Directive::Equ(_, expr)) => {
            // Clone expr to avoid borrow issues
            let expr_clone = expr.clone();
            let line_clone = line.clone();
            // Recursively evaluate the expression
            evaluate_expression(&expr_clone, context, &line_clone, cycle_stack)?
        }
        _ => {
            return Err(AssemblerError::from_context(
                format!(
                    "Symbol '{}' definition points to invalid line",
                    symbol
                ),
                line.location.clone(),
            ));
        }
    };

    // Memoize
    context.symbol_values.insert(key.clone(), result);

    // Pop from cycle stack
    cycle_stack.pop();

    Ok(result)
}

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
) -> Result<EvaluatedValue> {
    match expr {
        Expression::Literal(i) => Ok(EvaluatedValue::new_integer(*i)),

        Expression::Identifier(name) => {
            // Special case: __global_pointer$ is handled specially and not in outgoing_refs
            if name == SPECIAL_GLOBAL_POINTER {
                let gp_value = context.data_start + 2048;
                return Ok(EvaluatedValue::new_address(gp_value));
            }

            // Find symbol in current_line.outgoing_refs
            let sym_ref = current_line
                .outgoing_refs
                .iter()
                .find(|r| r.symbol == *name)
                .ok_or_else(|| {
                    AssemblerError::from_context(
                        format!(
                            "Unresolved symbol '{}' (internal error - should have been caught earlier)",
                            name
                        ),
                        current_line.location.clone(),
                    )
                })?;

            resolve_symbol_value(name, &sym_ref.pointer, context, cycle_stack)
        }

        Expression::CurrentAddress => {
            let addr = context.segment_start(&current_line.segment)
                + current_line.offset;
            Ok(EvaluatedValue::new_address(addr))
        }

        Expression::NumericLabelRef(nlr) => {
            // Find the numeric label reference in outgoing_refs
            // The symbol name for numeric labels includes the direction suffix (e.g., "3f" or "2b")
            let label_name = format!(
                "{}{}",
                nlr.num,
                if nlr.is_forward { "f" } else { "b" }
            );
            let sym_ref = current_line
                .outgoing_refs
                .iter()
                .find(|r| r.symbol == label_name)
                .ok_or_else(|| {
                    AssemblerError::from_context(
                        format!(
                            "Unresolved numeric label '{}' (internal error)",
                            nlr
                        ),
                        current_line.location.clone(),
                    )
                })?;

            resolve_symbol_value(
                &label_name,
                &sym_ref.pointer,
                context,
                cycle_stack,
            )
        }

        Expression::Parenthesized(inner) => {
            // Parentheses don't change semantics, just evaluate inner
            evaluate_expression(inner, context, current_line, cycle_stack)
        }

        Expression::PlusOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;
            checked_add(lhs_val, rhs_val, &current_line.location)
        }

        Expression::MinusOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;
            checked_sub(lhs_val, rhs_val, &current_line.location)
        }

        Expression::MultiplyOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int = require_integer(
                lhs_val,
                "multiplication",
                &current_line.location,
            )?;
            let rhs_int = require_integer(
                rhs_val,
                "multiplication",
                &current_line.location,
            )?;

            let result = lhs_int.checked_mul(rhs_int).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Arithmetic overflow in multiplication: {} * {}",
                        lhs_int, rhs_int
                    ),
                    current_line.location.clone(),
                )
            })?;

            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::DivideOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int =
                require_integer(lhs_val, "division", &current_line.location)?;
            let rhs_int =
                require_integer(rhs_val, "division", &current_line.location)?;

            if rhs_int == 0 {
                return Err(AssemblerError::from_context(
                    "Division by zero".to_string(),
                    current_line.location.clone(),
                ));
            }

            let result = lhs_int.checked_div(rhs_int).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Arithmetic overflow in division: {} / {}",
                        lhs_int, rhs_int
                    ),
                    current_line.location.clone(),
                )
            })?;

            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::ModuloOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int =
                require_integer(lhs_val, "modulo", &current_line.location)?;
            let rhs_int =
                require_integer(rhs_val, "modulo", &current_line.location)?;

            if rhs_int == 0 {
                return Err(AssemblerError::from_context(
                    "Modulo by zero".to_string(),
                    current_line.location.clone(),
                ));
            }

            let result = lhs_int % rhs_int;
            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::LeftShiftOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int =
                require_integer(lhs_val, "left shift", &current_line.location)?;
            let rhs_int =
                require_integer(rhs_val, "left shift", &current_line.location)?;

            if !(0..64).contains(&rhs_int) {
                return Err(AssemblerError::from_context(
                    format!("Invalid shift amount {} (must be 0..64)", rhs_int),
                    current_line.location.clone(),
                ));
            }

            check_left_shift_precision(
                lhs_int,
                rhs_int,
                &current_line.location,
            )?;

            let result = lhs_int << rhs_int;
            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::RightShiftOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int = require_integer(
                lhs_val,
                "right shift",
                &current_line.location,
            )?;
            let rhs_int = require_integer(
                rhs_val,
                "right shift",
                &current_line.location,
            )?;

            if !(0..64).contains(&rhs_int) {
                return Err(AssemblerError::from_context(
                    format!("Invalid shift amount {} (must be 0..64)", rhs_int),
                    current_line.location.clone(),
                ));
            }

            check_right_shift_precision(
                lhs_int,
                rhs_int,
                &current_line.location,
            )?;

            let result = lhs_int >> rhs_int;
            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::BitwiseOrOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int =
                require_integer(lhs_val, "bitwise OR", &current_line.location)?;
            let rhs_int =
                require_integer(rhs_val, "bitwise OR", &current_line.location)?;

            let result = lhs_int | rhs_int;
            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::BitwiseAndOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int = require_integer(
                lhs_val,
                "bitwise AND",
                &current_line.location,
            )?;
            let rhs_int = require_integer(
                rhs_val,
                "bitwise AND",
                &current_line.location,
            )?;

            let result = lhs_int & rhs_int;
            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::BitwiseXorOp { lhs, rhs } => {
            let lhs_val =
                evaluate_expression(lhs, context, current_line, cycle_stack)?;
            let rhs_val =
                evaluate_expression(rhs, context, current_line, cycle_stack)?;

            let lhs_int = require_integer(
                lhs_val,
                "bitwise XOR",
                &current_line.location,
            )?;
            let rhs_int = require_integer(
                rhs_val,
                "bitwise XOR",
                &current_line.location,
            )?;

            let result = lhs_int ^ rhs_int;
            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::NegateOp { expr: inner } => {
            let val =
                evaluate_expression(inner, context, current_line, cycle_stack)?;
            let int_val =
                require_integer(val, "negation", &current_line.location)?;

            let result = int_val.checked_neg().ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Arithmetic overflow in negation: -{}", int_val),
                    current_line.location.clone(),
                )
            })?;

            Ok(EvaluatedValue::new_integer(result))
        }

        Expression::BitwiseNotOp { expr: inner } => {
            let val =
                evaluate_expression(inner, context, current_line, cycle_stack)?;
            let int_val =
                require_integer(val, "bitwise NOT", &current_line.location)?;

            let result = !int_val;
            Ok(EvaluatedValue::new_integer(result))
        }
    }
}

// ============================================================================
// Type-Safe Arithmetic Helpers
// ============================================================================

/// Perform addition with type checking and overflow detection
fn checked_add(
    lhs: EvaluatedValue,
    rhs: EvaluatedValue,
    location: &Location,
) -> Result<EvaluatedValue> {
    match (lhs.value_type, rhs.value_type) {
        (ValueType::Integer, ValueType::Integer) => {
            let result = lhs.value.checked_add(rhs.value).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Arithmetic overflow in addition: {} + {}",
                        lhs.value, rhs.value
                    ),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::new_integer(result))
        }
        (ValueType::Address, ValueType::Integer)
        | (ValueType::Integer, ValueType::Address) => {
            let result = lhs.value.checked_add(rhs.value).ok_or_else(|| {
                AssemblerError::from_context(
                    "Arithmetic overflow in address + offset".to_string(),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::new_address(result))
        }
        (ValueType::Address, ValueType::Address) => {
            Err(AssemblerError::from_context(
                "Type error in addition: cannot add Address + Address"
                    .to_string(),
                location.clone(),
            ))
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
            let result = lhs.value.checked_sub(rhs.value).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Arithmetic underflow in subtraction: {} - {}",
                        lhs.value, rhs.value
                    ),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::new_integer(result))
        }
        (ValueType::Address, ValueType::Integer) => {
            let result = lhs.value.checked_sub(rhs.value).ok_or_else(|| {
                AssemblerError::from_context(
                    "Arithmetic underflow in address - offset".to_string(),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::new_address(result))
        }
        (ValueType::Address, ValueType::Address) => {
            let result = lhs.value.checked_sub(rhs.value).ok_or_else(|| {
                AssemblerError::from_context(
                    "Arithmetic underflow in address - address".to_string(),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::new_integer(result))
        }
        (ValueType::Integer, ValueType::Address) => {
            Err(AssemblerError::from_context(
                "Type error in subtraction: cannot compute Integer - Address"
                    .to_string(),
                location.clone(),
            ))
        }
    }
}

/// Check that a value is an Integer type, return error if not
fn require_integer(
    value: EvaluatedValue,
    operation: &str,
    location: &Location,
) -> Result<i64> {
    match value.value_type {
        ValueType::Integer => Ok(value.value),
        ValueType::Address => Err(AssemblerError::from_context(
            format!(
                "Type error in {}: expected Integer, got Address",
                operation
            ),
            location.clone(),
        )),
    }
}

/// Check if left shift would lose precision
fn check_left_shift_precision(
    value: i64,
    shift: i64,
    location: &Location,
) -> Result<()> {
    // For a left shift by N bits, check that the top N+1 bits are all the same
    // (all 0s or all 1s, i.e., sign-extension bits)
    if shift >= 63 {
        // Shifting by 63 or more always loses precision unless value is 0 or -1
        if value != 0 && value != -1 {
            return Err(AssemblerError::from_context(
                format!(
                    "Precision loss in left shift: {} << {} would shift out non-sign-extension bits",
                    value, shift
                ),
                location.clone(),
            ));
        }
        return Ok(());
    }

    // Check if the top (shift + 1) bits are all the same
    let bits_to_check = shift + 1;
    let sign_extended = value >> (64 - bits_to_check);

    if sign_extended != 0 && sign_extended != -1 {
        return Err(AssemblerError::from_context(
            format!(
                "Precision loss in left shift: {} << {} would shift out non-sign-extension bits",
                value, shift
            ),
            location.clone(),
        ));
    }

    Ok(())
}

/// Check if right shift would lose precision
fn check_right_shift_precision(
    value: i64,
    shift: i64,
    location: &Location,
) -> Result<()> {
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
        let lost_bits = (value & mask).count_ones();
        return Err(AssemblerError::from_context(
            format!(
                "Precision loss in right shift: {} >> {} would lose {} non-zero bits",
                value, shift, lost_bits
            ),
            location.clone(),
        ));
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceFile;

    /// Helper to create a minimal Source structure for testing
    fn make_test_source() -> Source {
        Source {
            files: vec![SourceFile {
                file: "test.s".to_string(),
                lines: vec![],
                text_size: 0,
                data_size: 0,
                bss_size: 0,
                local_symbols: vec![],
            }],
            text_size: 0,
            data_size: 0,
            bss_size: 0,
            global_symbols: vec![],
        }
    }

    /// Helper to create a test line with an expression
    fn make_test_line(
        segment: Segment,
        offset: i64,
        content: LineContent,
    ) -> Line {
        Line {
            location: Location { file: "test.s".to_string(), line: 1 },
            content,
            segment,
            offset,
            size: 0,
            outgoing_refs: vec![],
        }
    }

    /// Helper to evaluate a simple expression (just a literal for now)
    fn eval_simple(
        expr: Expression,
        context: &mut EvaluationContext,
    ) -> Result<EvaluatedValue> {
        let line = make_test_line(
            Segment::Text,
            0,
            LineContent::Label("test".to_string()),
        );
        eval_expr(&expr, &line, context)
    }

    // ========================================================================
    // Type System Tests
    // ========================================================================

    #[test]
    fn test_literal_is_integer() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::Literal(42);
        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 42);
    }

    #[test]
    fn test_current_address_is_address() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::CurrentAddress;
        let line = make_test_line(
            Segment::Text,
            16,
            LineContent::Label("test".to_string()),
        );

        let result = eval_expr(&expr, &line, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Address);
        assert_eq!(result.value, 0x100e8 + 16);
    }

    #[test]
    fn test_address_plus_integer() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // . + 4 where . = 0x100e8
        let expr = Expression::PlusOp {
            lhs: Box::new(Expression::CurrentAddress),
            rhs: Box::new(Expression::Literal(4)),
        };

        let line = make_test_line(
            Segment::Text,
            0,
            LineContent::Label("test".to_string()),
        );
        let result = eval_expr(&expr, &line, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Address);
        assert_eq!(result.value, 0x100e8 + 4);
    }

    #[test]
    fn test_integer_plus_address() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // 4 + . where . = 0x100e8
        let expr = Expression::PlusOp {
            lhs: Box::new(Expression::Literal(4)),
            rhs: Box::new(Expression::CurrentAddress),
        };

        let line = make_test_line(
            Segment::Text,
            0,
            LineContent::Label("test".to_string()),
        );
        let result = eval_expr(&expr, &line, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Address);
        assert_eq!(result.value, 0x100e8 + 4);
    }

    #[test]
    fn test_address_minus_integer() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // . - 8 where . = 0x100f8 (offset 16)
        let expr = Expression::MinusOp {
            lhs: Box::new(Expression::CurrentAddress),
            rhs: Box::new(Expression::Literal(8)),
        };

        let line = make_test_line(
            Segment::Text,
            16,
            LineContent::Label("test".to_string()),
        );
        let result = eval_expr(&expr, &line, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Address);
        assert_eq!(result.value, 0x100e8 + 16 - 8);
    }

    #[test]
    fn test_address_minus_address() {
        let _source = make_test_source();

        // Create two current address expressions at different offsets
        // This is a bit artificial, but tests the type system
        let addr1 = EvaluatedValue::new_address(0x100e8 + 16);
        let addr2 = EvaluatedValue::new_address(0x100e8);

        let result = checked_sub(
            addr1,
            addr2,
            &Location { file: "test".to_string(), line: 1 },
        )
        .unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 16);
    }

    #[test]
    fn test_address_plus_address_error() {
        let addr1 = EvaluatedValue::new_address(0x100e8);
        let addr2 = EvaluatedValue::new_address(0x100f8);

        let result = checked_add(
            addr1,
            addr2,
            &Location { file: "test".to_string(), line: 1 },
        );

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("cannot add Address + Address"));
    }

    #[test]
    fn test_integer_minus_address_error() {
        let int_val = EvaluatedValue::new_integer(8);
        let addr_val = EvaluatedValue::new_address(0x100e8);

        let result = checked_sub(
            int_val,
            addr_val,
            &Location { file: "test".to_string(), line: 1 },
        );

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("cannot compute Integer - Address"));
    }

    // ========================================================================
    // Arithmetic Operations Tests
    // ========================================================================

    #[test]
    fn test_integer_multiply() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::MultiplyOp {
            lhs: Box::new(Expression::Literal(6)),
            rhs: Box::new(Expression::Literal(7)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 42);
    }

    #[test]
    fn test_integer_divide() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::DivideOp {
            lhs: Box::new(Expression::Literal(42)),
            rhs: Box::new(Expression::Literal(7)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 6);
    }

    #[test]
    fn test_integer_modulo() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::ModuloOp {
            lhs: Box::new(Expression::Literal(43)),
            rhs: Box::new(Expression::Literal(7)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 1);
    }

    #[test]
    fn test_division_by_zero_error() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::DivideOp {
            lhs: Box::new(Expression::Literal(42)),
            rhs: Box::new(Expression::Literal(0)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Division by zero"));
    }

    #[test]
    fn test_modulo_by_zero_error() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::ModuloOp {
            lhs: Box::new(Expression::Literal(42)),
            rhs: Box::new(Expression::Literal(0)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Modulo by zero"));
    }

    // ========================================================================
    // Bitwise Operations Tests
    // ========================================================================

    #[test]
    fn test_bitwise_or() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::BitwiseOrOp {
            lhs: Box::new(Expression::Literal(0x0f)),
            rhs: Box::new(Expression::Literal(0xf0)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 0xff);
    }

    #[test]
    fn test_bitwise_and() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::BitwiseAndOp {
            lhs: Box::new(Expression::Literal(0xff)),
            rhs: Box::new(Expression::Literal(0x0f)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 0x0f);
    }

    #[test]
    fn test_bitwise_xor() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::BitwiseXorOp {
            lhs: Box::new(Expression::Literal(0xff)),
            rhs: Box::new(Expression::Literal(0x0f)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 0xf0);
    }

    #[test]
    fn test_bitwise_not() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr =
            Expression::BitwiseNotOp { expr: Box::new(Expression::Literal(0)) };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, -1);
    }

    // ========================================================================
    // Shift Operations Tests
    // ========================================================================

    #[test]
    fn test_left_shift_simple() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(1)),
            rhs: Box::new(Expression::Literal(4)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 16);
    }

    #[test]
    fn test_right_shift_simple() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(16)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 4);
    }

    #[test]
    fn test_arithmetic_right_shift() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(-8)),
            rhs: Box::new(Expression::Literal(1)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, -4);
    }

    #[test]
    fn test_shift_negative_amount_error() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(8)),
            rhs: Box::new(Expression::Literal(-1)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Invalid shift amount"));
    }

    #[test]
    fn test_shift_too_large_error() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(8)),
            rhs: Box::new(Expression::Literal(64)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Invalid shift amount"));
    }

    // ========================================================================
    // Precision Loss Detection Tests
    // ========================================================================

    #[test]
    fn test_overflow_addition() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::PlusOp {
            lhs: Box::new(Expression::Literal(i64::MAX)),
            rhs: Box::new(Expression::Literal(1)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("overflow"));
    }

    #[test]
    fn test_underflow_subtraction() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::MinusOp {
            lhs: Box::new(Expression::Literal(i64::MIN)),
            rhs: Box::new(Expression::Literal(1)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("underflow"));
    }

    #[test]
    fn test_overflow_multiplication() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::MultiplyOp {
            lhs: Box::new(Expression::Literal(i64::MAX)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("overflow"));
    }

    #[test]
    fn test_overflow_negation() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::NegateOp {
            expr: Box::new(Expression::Literal(i64::MIN)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("overflow"));
    }

    #[test]
    fn test_left_shift_precision_loss() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // 0x4000000000000000 << 2 would shift out a 1 bit
        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(0x4000000000000000)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Precision loss")
                || err_msg.contains("precision loss")
        );
    }

    #[test]
    fn test_left_shift_sign_extension_ok() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // -1 << 4 should work (all bits are sign bits)
        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(-1)),
            rhs: Box::new(Expression::Literal(4)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, -16);
    }

    #[test]
    fn test_right_shift_precision_loss() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // 15 >> 2 loses bits (15 = 0b1111, >> 2 = 0b11, loses 0b11)
        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(15)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Precision loss")
                || err_msg.contains("precision loss")
        );
    }

    #[test]
    fn test_right_shift_no_loss() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // 16 >> 2 = 4, no bits lost (16 = 0b10000, >> 2 = 0b100)
        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(16)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 4);
    }

    // ========================================================================
    // Unary Operations Tests
    // ========================================================================

    #[test]
    fn test_negate_positive() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr =
            Expression::NegateOp { expr: Box::new(Expression::Literal(42)) };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, -42);
    }

    #[test]
    fn test_negate_negative() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        let expr = Expression::NegateOp {
            expr: Box::new(Expression::NegateOp {
                expr: Box::new(Expression::Literal(42)),
            }),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 42);
    }

    // ========================================================================
    // Parentheses and Precedence Tests
    // ========================================================================

    #[test]
    fn test_parentheses_explicit() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // (2 + 3) * 4 = 20
        let expr = Expression::MultiplyOp {
            lhs: Box::new(Expression::Parenthesized(Box::new(
                Expression::PlusOp {
                    lhs: Box::new(Expression::Literal(2)),
                    rhs: Box::new(Expression::Literal(3)),
                },
            ))),
            rhs: Box::new(Expression::Literal(4)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 20);
    }

    #[test]
    fn test_complex_expression() {
        let source = make_test_source();
        let mut context = new_evaluation_context(source, 0x100e8);

        // (10 + 20) * 2 - 5 = 55
        let expr = Expression::MinusOp {
            lhs: Box::new(Expression::MultiplyOp {
                lhs: Box::new(Expression::Parenthesized(Box::new(
                    Expression::PlusOp {
                        lhs: Box::new(Expression::Literal(10)),
                        rhs: Box::new(Expression::Literal(20)),
                    },
                ))),
                rhs: Box::new(Expression::Literal(2)),
            }),
            rhs: Box::new(Expression::Literal(5)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        assert_eq!(result.value_type, ValueType::Integer);
        assert_eq!(result.value, 55);
    }

    // ========================================================================
    // Context Tests
    // ========================================================================

    #[test]
    fn test_context_segment_addresses() {
        let source = Source {
            files: vec![],
            text_size: 1000, // Will cause data to be on next 4K boundary
            data_size: 500,
            bss_size: 200,
            global_symbols: vec![],
        };

        let context = new_evaluation_context(source, 0x100e8);

        assert_eq!(context.text_start, 0x100e8);
        // data_start should be next 4K boundary after (0x100e8 + 1000)
        // 0x100e8 + 1000 = 0x104d0
        // Next 4K boundary is 0x11000
        assert_eq!(context.data_start, 0x11000);
        // bss_start should be data_start + data_size
        assert_eq!(context.bss_start, 0x11000 + 500);
    }

    #[test]
    fn test_global_pointer_symbol() {
        let source = Source {
            files: vec![],
            text_size: 100,
            data_size: 500,
            bss_size: 0,
            global_symbols: vec![],
        };

        let mut context = new_evaluation_context(source, 0x100e8);

        // Manually resolve __global_pointer$
        let gp_value = resolve_symbol_value(
            "__global_pointer$",
            &LinePointer { file_index: 0, line_index: 0 },
            &mut context,
            &mut Vec::new(),
        )
        .unwrap();

        assert_eq!(gp_value.value_type, ValueType::Address);
        // __global_pointer$ = data_start + 2048
        // data_start = next 4K after (0x100e8 + 100) = 0x11000
        assert_eq!(gp_value.value, 0x11000 + 2048);
    }
}
