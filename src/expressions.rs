// expressions.rs
//
// Expression evaluation with type checking
//
// This module implements lazy evaluation of expressions.
// There are two types of values (Integer and Address) with strict type checking.

use crate::ast::*;
use crate::error::{AssemblerError, Result};
use crate::symbols::Symbols;
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

pub struct EvaluationContext {
    /// The complete source with all files, lines, and linked symbols
    source: Source,

    /// Symbol information extracted during linking
    symbols: Symbols,

    /// Memoization table: (symbol, definition location) -> evaluated value
    symbol_values: HashMap<SymbolReference, EvaluatedValue>,

    /// Segment start addresses
    pub text_start: u32,
    pub data_start: u32,
    pub bss_start: u32,

    /// Current line context for looking up symbol references
    current_line_pointer: LinePointer,
}

impl EvaluationContext {
    /// Get the start address of a segment
    fn segment_start(&self, segment: Segment) -> u32 {
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
                AssemblerError::no_context(format!(
                    "Internal error: invalid line pointer [{}:{}]",
                    pointer.file_index, pointer.line_index
                ))
            })
    }
}

/// Create an evaluation context with segment addresses and seed the symbol table
pub fn new_evaluation_context(
    source: Source,
    symbols: Symbols,
    text_start: u32,
) -> EvaluationContext {
    // The first instruction in the text segment is pushed back
    // by the ELF header + program header table
    let text_first_instruction = text_start + source.header_size;

    // Calculate segment addresses
    let text_size = source.text_size;
    let data_size = source.data_size;

    // data_start = next 4K page boundary after (text_start + text_size)
    let data_start = (text_first_instruction + text_size + 4095) & !(4096 - 1);

    // bss_start = immediately after data
    let bss_start = data_start + data_size;

    EvaluationContext {
        source,
        symbols,
        symbol_values: HashMap::new(),
        text_start: text_first_instruction,
        data_start,
        bss_start,
        current_line_pointer: LinePointer { file_index: 0, line_index: 0 },
    }
}

/// Evaluate an expression in the context of a specific line
///
/// This is the main entry point for code generation. It implements the two-phase evaluation:
/// 1. Forward phase: Ensure all symbol dependencies are resolved and memoized
/// 2. Backward phase: Evaluate the expression using cached values
pub fn eval_expr(
    expr: &Expression,
    line: &Line,
    pointer: &LinePointer,
    context: &mut EvaluationContext,
) -> Result<EvaluatedValue> {
    // Set the current line context for symbol lookups
    context.current_line_pointer = pointer.clone();

    // Resolve all symbol dependencies
    evaluate_line_symbols(line, pointer, context)?;

    // Evaluate the expression now that all symbol values are cached
    evaluate_expression(expr, context, line)
}

/// Ensure all symbols referenced by a line are evaluated
///
/// This function resolves all symbols that a line depends on before evaluating
/// the line's expressions. This is called before code generation for each line.
///
/// This is the forward phase of recursive evaluation: all symbol dependencies are
/// computed and memoized before any expressions are evaluated.
pub fn evaluate_line_symbols(
    _line: &Line,
    pointer: &LinePointer,
    context: &mut EvaluationContext,
) -> Result<()> {
    let mut cycle_stack = Vec::new();

    let sym_refs = context.symbols.get_line_refs(pointer).to_vec();
    for sym_ref in sym_refs {
        resolve_symbol_dependencies(&sym_ref, context, &mut cycle_stack)?;
    }

    Ok(())
}

/// Forward phase: Recursively resolve a symbol and all its dependencies
///
/// This function ensures that a symbol and all symbols it transitively depends on
/// are computed and memoized. It uses cycle detection to prevent infinite recursion.
fn resolve_symbol_dependencies(
    key: &SymbolReference,
    context: &mut EvaluationContext,
    cycle_stack: &mut Vec<SymbolReference>,
) -> Result<()> {
    // Already resolved?
    if context.symbol_values.contains_key(key) {
        return Ok(());
    }

    // Detect cycles
    if cycle_stack.contains(key) {
        let cycle_chain: Vec<String> =
            cycle_stack.iter().map(|k| k.symbol.clone()).collect();
        let line = context.get_line(&key.pointer)?;
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

    // Push to cycle stack
    cycle_stack.push(key.clone());

    // Get the line where this symbol is defined
    let line = context.get_line(&key.pointer)?;

    // Compute the symbol's value based on line content
    let result = match &line.content {
        LineContent::Label(_) => {
            // Label: address is computed directly from position
            let absolute_addr =
                context.segment_start(line.segment).wrapping_add(line.offset);
            EvaluatedValue::Address(absolute_addr)
        }
        LineContent::Directive(Directive::Equ(_, expr)) => {
            // .equ: First resolve all referenced symbols (via Symbols struct), then evaluate
            // Clone everything to avoid borrow conflicts during recursive resolution
            let sym_refs = context.symbols.get_line_refs(&key.pointer).to_vec();
            let expr_clone = expr.clone();
            let line_clone = line.clone();
            let equ_pointer = key.pointer.clone();

            for sym_ref in sym_refs {
                resolve_symbol_dependencies(&sym_ref, context, cycle_stack)?;
            }
            
            // Set current_line_pointer to the .equ line so that evaluate_expression
            // can correctly look up symbol references from this line
            let saved_pointer = context.current_line_pointer.clone();
            context.current_line_pointer = equ_pointer;
            
            // All dependencies are now resolved; evaluate the expression
            let result = evaluate_expression(&expr_clone, context, &line_clone)?;
            
            // Restore the previous current_line_pointer
            context.current_line_pointer = saved_pointer;
            
            result
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

    // Memoize result
    context.symbol_values.insert(key.clone(), result);

    // Pop from cycle stack
    cycle_stack.pop();

    Ok(())
}

/// Backward phase: Evaluate an expression using cached symbol values
///
/// This evaluates the expression tree after all symbol dependencies have been
/// resolved and memoized. Symbol lookups are simple cache hits with assertions,
/// so this phase never recurses into symbol resolution.
fn evaluate_expression(
    expr: &Expression,
    context: &EvaluationContext,
    current_line: &Line,
) -> Result<EvaluatedValue> {
    match expr {
        Expression::Literal(i) => Ok(EvaluatedValue::Integer(*i)),

        Expression::Identifier(name) => {
            // Find symbol in Symbols using current line context and look up cached value
            let sym_refs =
                context.symbols.get_line_refs(&context.current_line_pointer);
            let sym_ref = sym_refs
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

            let value = context.symbol_values.get(sym_ref).copied().ok_or_else(|| {
                AssemblerError::from_context(
                    format!(
                        "Symbol '{}' not resolved (internal error - should have been resolved in forward phase)",
                        name
                    ),
                    current_line.location.clone(),
                )
            })?;
            Ok(value)
        }

        Expression::CurrentAddress => {
            let addr = context.segment_start(current_line.segment)
                + current_line.offset;
            Ok(EvaluatedValue::Address(addr))
        }

        Expression::NumericLabelRef(nlr) => {
            let label_name = format!(
                "{}{}",
                nlr.num,
                if nlr.is_forward { "f" } else { "b" }
            );
            let sym_refs =
                context.symbols.get_line_refs(&context.current_line_pointer);
            let sym_ref = sym_refs
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

            let value = context
                .symbol_values
                .get(sym_ref)
                .copied()
                .ok_or_else(|| {
                    AssemblerError::from_context(
                        format!(
                            "Numeric label '{}' not resolved (internal error)",
                            label_name
                        ),
                        current_line.location.clone(),
                    )
                })?;
            Ok(value)
        }

        Expression::Parenthesized(inner) => {
            // Parentheses don't change semantics, just evaluate inner
            evaluate_expression(inner, context, current_line)
        }

        Expression::PlusOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;
            checked_add(lhs_val, rhs_val, &current_line.location)
        }

        Expression::MinusOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;
            checked_sub(lhs_val, rhs_val, &current_line.location)
        }

        Expression::MultiplyOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

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

            Ok(EvaluatedValue::Integer(result))
        }

        Expression::DivideOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

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

            Ok(EvaluatedValue::Integer(result))
        }

        Expression::ModuloOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

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
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::LeftShiftOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

            let lhs_int =
                require_integer(lhs_val, "left shift", &current_line.location)?;
            let rhs_int =
                require_integer(rhs_val, "left shift", &current_line.location)?;

            if !(0..32).contains(&rhs_int) {
                return Err(AssemblerError::from_context(
                    format!("Invalid shift amount {} (must be 0..32)", rhs_int),
                    current_line.location.clone(),
                ));
            }

            let result = lhs_int << rhs_int as u32;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::RightShiftOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

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

            if !(0..32).contains(&rhs_int) {
                return Err(AssemblerError::from_context(
                    format!("Invalid shift amount {} (must be 0..32)", rhs_int),
                    current_line.location.clone(),
                ));
            }

            let result = lhs_int >> rhs_int as u32;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseOrOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

            let lhs_int =
                require_integer(lhs_val, "bitwise OR", &current_line.location)?;
            let rhs_int =
                require_integer(rhs_val, "bitwise OR", &current_line.location)?;

            let result = lhs_int | rhs_int;
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseAndOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

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
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseXorOp { lhs, rhs } => {
            let lhs_val = evaluate_expression(lhs, context, current_line)?;
            let rhs_val = evaluate_expression(rhs, context, current_line)?;

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
            Ok(EvaluatedValue::Integer(result))
        }

        Expression::NegateOp { expr: inner } => {
            let val = evaluate_expression(inner, context, current_line)?;
            let int_val =
                require_integer(val, "negation", &current_line.location)?;

            let result = int_val.checked_neg().ok_or_else(|| {
                AssemblerError::from_context(
                    format!("Arithmetic overflow in negation: -{}", int_val),
                    current_line.location.clone(),
                )
            })?;

            Ok(EvaluatedValue::Integer(result))
        }

        Expression::BitwiseNotOp { expr: inner } => {
            let val = evaluate_expression(inner, context, current_line)?;
            let int_val =
                require_integer(val, "bitwise NOT", &current_line.location)?;

            let result = !int_val;
            Ok(EvaluatedValue::Integer(result))
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
