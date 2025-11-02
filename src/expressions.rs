// expressions.rs
//
// Expression evaluation with type checking
//
// This module implements expression evaluation.
// There are two types of values (Integer and Address) with strict type checking.

use crate::ast::{
    Directive, Expression, LineContent, LinePointer, Location, Source,
};
use crate::error::{AssemblerError, Result};
use crate::layout::Layout;
use crate::symbols::{SymbolDefinition, SymbolLinks, SymbolReference};
use std::collections::HashMap;
use std::fmt;

/// A fully evaluated expression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluatedValue {
    Integer(i32),
    Address(u32),
}

impl fmt::Display for EvaluatedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvaluatedValue::Integer(v) => write!(f, "{}", v),
            EvaluatedValue::Address(v) => write!(f, "0x{:x}", v),
        }
    }
}

/// A container for evaluated symbol values
///
/// This type wraps a HashMap of symbol values to provide a clean interface
/// for symbol value lookup. It represents the result of evaluating all symbols
/// in the program (both labels and .equ definitions).
pub struct SymbolValues {
    values: HashMap<SymbolDefinition, EvaluatedValue>,
}

impl SymbolValues {
    /// Create an empty SymbolValues
    pub fn new() -> Self {
        SymbolValues { values: HashMap::new() }
    }

    /// Look up a symbol value by definition
    pub fn get(&self, key: &SymbolDefinition) -> Option<EvaluatedValue> {
        self.values.get(key).copied()
    }

    /// Insert or update a symbol value (internal)
    fn insert(&mut self, key: SymbolDefinition, value: EvaluatedValue) {
        self.values.insert(key, value);
    }

    /// Check if a symbol is already evaluated (internal)
    fn contains_key(&self, key: &SymbolDefinition) -> bool {
        self.values.contains_key(key)
    }
}

impl Default for SymbolValues {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate all symbols (labels and .equ definitions) in the program
///
/// This function computes all symbol values upfront, once per convergence iteration.
/// It replaces the per-line evaluation approach by processing all symbols globally
/// with full dependency resolution and cycle detection.
///
/// Returns a `SymbolValues` containing all evaluated symbols.
pub fn eval_symbol_values(
    source: &Source,
    symbol_links: &SymbolLinks,
    layout: &Layout,
) -> Result<SymbolValues> {
    // Start with empty symbol values
    let mut symbol_values = SymbolValues::new();

    // Iterate all files and lines, evaluating labels and .equ definitions
    for (file_index, file) in source.files.iter().enumerate() {
        for (line_index, line) in file.lines.iter().enumerate() {
            let pointer = LinePointer { file_index, line_index };

            // Only process lines that define symbols (labels and .equ)
            let symbol_name = match &line.content {
                LineContent::Label(name) => Some(name.clone()),
                LineContent::Directive(Directive::Equ(name, _)) => {
                    Some(name.clone())
                }
                _ => None,
            };

            if let Some(name) = symbol_name {
                // Create symbol definition for this definition
                let sym_def = SymbolDefinition { symbol: name, pointer };

                // Recursively evaluate this symbol and its dependencies
                let mut cycle_stack = Vec::new();
                eval_symbol(
                    &sym_def,
                    source,
                    symbol_links,
                    layout,
                    &mut symbol_values,
                    &mut cycle_stack,
                )?;
            }
        }
    }

    Ok(symbol_values)
}

/// Recursively evaluate a symbol and all its dependencies
fn eval_symbol(
    key: &SymbolDefinition,
    source: &Source,
    symbol_links: &SymbolLinks,
    layout: &Layout,
    symbol_values: &mut SymbolValues,
    cycle_stack: &mut Vec<SymbolDefinition>,
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
            let addr = layout.get_line_address(&key.pointer);
            EvaluatedValue::Address(addr)
        }
        LineContent::Directive(Directive::Equ(_, expr)) => {
            // Recursive case: evaluate dependencies first
            cycle_stack.push(key.clone());

            // Get all symbol references from this .equ line
            let sym_refs = symbol_links.get_line_refs(&key.pointer);
            for sym_ref in sym_refs {
                eval_symbol(
                    &sym_ref.definition,
                    source,
                    symbol_links,
                    layout,
                    symbol_values,
                    cycle_stack,
                )?;
            }

            cycle_stack.pop();

            // Now evaluate the expression (all dependencies resolved)
            let address = layout.get_line_address(&key.pointer);
            let refs = symbol_links.get_line_refs(&key.pointer);
            eval_expr(expr, address, refs, symbol_values, source, &key.pointer)?
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

/// Evaluate an expression with explicit context
///
/// This is the new expression evaluator that works with pre-computed symbol values
/// and explicit parameters. It's a pure function that doesn't rely on mutable state.
///
/// Parameters:
/// - `expr`: The expression to evaluate
/// - `address`: The current address (used for `.` operator)
/// - `refs`: Symbol references available at this line
/// - `symbol_values`: Pre-computed symbol values
/// - `source`: Source code (for error reporting)
/// - `pointer`: Current line pointer (for error reporting)
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
                .find(|r| r.outgoing_name == *name)
                .ok_or_else(|| {
                    AssemblerError::from_context(
                        format!(
                            "Unresolved symbol '{}' (internal error - should have been caught earlier)",
                            name
                        ),
                        location.clone(),
                    )
                })?;

            symbol_values.get(&sym_ref.definition).ok_or_else(|| {
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
                .find(|r| r.outgoing_name == label_name)
                .ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Unresolved numeric label '{}' (internal error)",
                        nlr
                    ),
                    location.clone(),
                )
            })?;

            symbol_values.get(&sym_ref.definition).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Numeric label '{}' not resolved (internal error)",
                        label_name
                    ),
                    location.clone(),
                )
            })
        }

        Expression::Parenthesized(inner) => {
            eval_expr(inner, address, refs, symbol_values, source, pointer)
        }

        Expression::PlusOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            checked_add(lhs_val, rhs_val, location)
        }

        Expression::MinusOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            checked_sub(lhs_val, rhs_val, location)
        }

        Expression::MultiplyOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "multiplication", location)?;
            let rhs_int = require_integer(rhs_val, "multiplication", location)?;
            let result = lhs_int.checked_mul(rhs_int).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Arithmetic overflow in multiplication: {} * {}",
                        lhs_int, rhs_int
                    ),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::DivideOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
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
                    format!(
                        "Arithmetic overflow in division: {} / {}",
                        lhs_int, rhs_int
                    ),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::ModuloOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "modulo", location)?;
            let rhs_int = require_integer(rhs_val, "modulo", location)?;
            if rhs_int == 0 {
                return Err(AssemblerError::from_context(
                    "Modulo by zero".to_string(),
                    location.clone(),
                ));
            }
            let result = lhs_int.checked_rem(rhs_int).ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Arithmetic overflow in modulo: {} % {}",
                        lhs_int, rhs_int
                    ),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::LeftShiftOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "left shift", location)?;
            let rhs_int = require_integer(rhs_val, "left shift", location)?;

            if !(0..32).contains(&rhs_int) {
                return Err(AssemblerError::from_context(
                    format!("Invalid shift amount {} (must be 0..32)", rhs_int),
                    location.clone(),
                ));
            }

            let result =
                lhs_int.checked_shl(rhs_int as u32).ok_or_else(|| {
                    AssemblerError::from_context(
                        format!(
                            "Arithmetic overflow in left shift: {} << {}",
                            lhs_int, rhs_int
                        ),
                        location.clone(),
                    )
                })?;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::RightShiftOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "right shift", location)?;
            let rhs_int = require_integer(rhs_val, "right shift", location)?;

            if !(0..32).contains(&rhs_int) {
                return Err(AssemblerError::from_context(
                    format!("Invalid shift amount {} (must be 0..32)", rhs_int),
                    location.clone(),
                ));
            }

            let result = lhs_int >> rhs_int as u32;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseOrOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "bitwise OR", location)?;
            let rhs_int = require_integer(rhs_val, "bitwise OR", location)?;
            let result = lhs_int | rhs_int;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseAndOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "bitwise AND", location)?;
            let rhs_int = require_integer(rhs_val, "bitwise AND", location)?;
            let result = lhs_int & rhs_int;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseXorOp { lhs, rhs } => {
            let lhs_val =
                eval_expr(lhs, address, refs, symbol_values, source, pointer)?;
            let rhs_val =
                eval_expr(rhs, address, refs, symbol_values, source, pointer)?;
            let lhs_int = require_integer(lhs_val, "bitwise XOR", location)?;
            let rhs_int = require_integer(rhs_val, "bitwise XOR", location)?;
            let result = lhs_int ^ rhs_int;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::NegateOp { expr } => {
            let val =
                eval_expr(expr, address, refs, symbol_values, source, pointer)?;
            let int = require_integer(val, "negation", location)?;
            let result = int.checked_neg().ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Arithmetic overflow in negation: -{}", int),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseNotOp { expr } => {
            let val =
                eval_expr(expr, address, refs, symbol_values, source, pointer)?;
            let int = require_integer(val, "bitwise NOT", location)?;
            Ok(EvaluatedValue::Integer(!int))
        }
    }
}

/// Perform addition with type checking and overflow detection
pub fn checked_add(
    lhs: EvaluatedValue,
    rhs: EvaluatedValue,
    location: &Location,
) -> Result<EvaluatedValue> {
    match (lhs, rhs) {
        (EvaluatedValue::Integer(lhs_i), EvaluatedValue::Integer(rhs_i)) => {
            let result = lhs_i.checked_add(rhs_i).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Integer overflow: {} + {}", lhs, rhs),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }
        (EvaluatedValue::Address(lhs_a), EvaluatedValue::Integer(rhs_i))
        | (EvaluatedValue::Integer(rhs_i), EvaluatedValue::Address(lhs_a)) => {
            let result: u32 = {
                let sum: i64 = i64::from(lhs_a) + i64::from(rhs_i);
                sum.try_into().map_err(|_| {
                    // was it an overflow (too large) or underflow (negative)
                    let error_type = if sum < 0 {
                        "underflow" // Negative result
                    } else {
                        "overflow" // Result too large for u32
                    };

                    AssemblerError::from_context(
                        format!("Address {}: {} + {}", error_type, lhs, rhs),
                        location.clone(),
                    )
                })?
            };
            Ok(EvaluatedValue::Address(result))
        }
        (EvaluatedValue::Address(_), EvaluatedValue::Address(_)) => {
            Err(AssemblerError::from_context(
                format!(
                    "Type error: cannot add Address + Address: {} + {}",
                    lhs, rhs
                ),
                location.clone(),
            ))
        }
    }
}

/// Perform subtraction with type checking and underflow detection
pub fn checked_sub(
    lhs: EvaluatedValue,
    rhs: EvaluatedValue,
    location: &Location,
) -> Result<EvaluatedValue> {
    match (lhs, rhs) {
        (EvaluatedValue::Integer(lhs_i), EvaluatedValue::Integer(rhs_i)) => {
            let result = lhs_i.checked_sub(rhs_i).ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Integer wraparound: {} - {}", lhs, rhs),
                    location.clone(),
                )
            })?;
            Ok(EvaluatedValue::Integer(result))
        }
        (EvaluatedValue::Address(lhs_a), EvaluatedValue::Integer(rhs_i)) => {
            let result: u32 = {
                let sum: i64 = i64::from(lhs_a) - i64::from(rhs_i);
                sum.try_into().map_err(|_| {
                    // was it an overflow (too large) or underflow (negative)
                    let error_type = if sum < 0 {
                        "underflow" // Negative result
                    } else {
                        "overflow" // Result too large for u32
                    };

                    AssemblerError::from_context(
                        format!("Address {}: {} - {}", error_type, lhs, rhs),
                        location.clone(),
                    )
                })?
            };
            Ok(EvaluatedValue::Address(result))
        }
        (EvaluatedValue::Address(lhs_a), EvaluatedValue::Address(rhs_a)) => {
            // Address - Address = Integer (distance between addresses)
            let result: i32 = {
                let sum: i64 = i64::from(lhs_a) - i64::from(rhs_a);
                sum.try_into().map_err(|_| {
                    AssemblerError::from_context(
                        format!("Integer out of range: {} - {}", lhs, rhs),
                        location.clone(),
                    )
                })?
            };
            Ok(EvaluatedValue::Integer(result))
        }
        (EvaluatedValue::Integer(_), EvaluatedValue::Address(_)) => {
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
) -> Result<i32> {
    match value {
        EvaluatedValue::Integer(i) => Ok(i),
        EvaluatedValue::Address(_) => Err(AssemblerError::from_context(
            format!(
                "Type error in {}: expected Integer, got Address",
                operation
            ),
            location.clone(),
        )),
    }
}
