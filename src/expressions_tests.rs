// expressions_tests.rs

use crate::ast::*;
use crate::error::Result;
use crate::expressions::*;
use crate::symbols::SymbolLinks;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a minimal Source structure for testing
    fn make_test_source() -> Source {
        Source {
            files: vec![SourceFile {
                file: "test.s".to_string(),
                lines: vec![],
            }],
        }
    }

    /// Helper to create minimal Symbols structure for testing
    fn make_test_symbols(num_files: usize) -> SymbolLinks {
        let mut line_refs = Vec::new();
        for _ in 0..num_files {
            line_refs.push(vec![vec![]]);
        }
        SymbolLinks {
            line_refs,
            local_symbols_by_file: vec![vec![]; num_files],
            global_symbols: vec![],
        }
    }

    /// Helper to create a test line with an expression
    fn make_test_line(
        _segment: Segment,
        _offset: u32,
        content: LineContent,
    ) -> Line {
        // Note: segment and offset are no longer part of Line; they're in Layout
        // The _segment and _offset parameters are kept for backwards compatibility with existing tests
        Line {
            location: Location { file: "test.s".to_string(), line: 1 },
            content,
        }
    }

    /// Helper to create a layout with a test line entry
    fn make_test_layout_with_line(
        segment: Segment,
        offset: u32,
        size: u32,
    ) -> crate::layout::Layout {
        let mut layout = crate::layout::Layout::new();
        layout.set(
            LinePointer { file_index: 0, line_index: 0 },
            crate::layout::LineLayout { segment, offset, size },
        );
        layout
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
        eval_expr(
            &expr,
            &line,
            &LinePointer { file_index: 0, line_index: 0 },
            context,
        )
    }

    // ========================================================================
    // Type System Tests
    // ========================================================================

    #[test]
    fn test_literal_is_integer() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x10000,
        );

        let expr = Expression::Literal(42);
        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 42),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_current_address_is_address() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 16, 0),
            0x10000,
        );

        let expr = Expression::CurrentAddress;
        let line = make_test_line(
            Segment::Text,
            16,
            LineContent::Label("test".to_string()),
        );

        let result = eval_expr(
            &expr,
            &line,
            &LinePointer { file_index: 0, line_index: 0 },
            &mut context,
        )
        .unwrap();

        match result {
            EvaluatedValue::Address(a) => assert_eq!(a, 0x10000u32 + 16),
            _ => panic!("Expected Address"),
        }
    }

    #[test]
    fn test_address_plus_integer() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x10000,
        );

        // . + 4 where . = 0x10000
        let expr = Expression::PlusOp {
            lhs: Box::new(Expression::CurrentAddress),
            rhs: Box::new(Expression::Literal(4)),
        };

        let line = make_test_line(
            Segment::Text,
            0,
            LineContent::Label("test".to_string()),
        );
        let result = eval_expr(
            &expr,
            &line,
            &LinePointer { file_index: 0, line_index: 0 },
            &mut context,
        )
        .unwrap();

        match result {
            EvaluatedValue::Address(a) => assert_eq!(a, 0x10000u32 + 4),
            _ => panic!("Expected Address"),
        }
    }

    #[test]
    fn test_integer_plus_address() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x10000,
        );

        // 4 + . where . = 0x10000
        let expr = Expression::PlusOp {
            lhs: Box::new(Expression::Literal(4)),
            rhs: Box::new(Expression::CurrentAddress),
        };

        let line = make_test_line(
            Segment::Text,
            0,
            LineContent::Label("test".to_string()),
        );
        let result = eval_expr(
            &expr,
            &line,
            &LinePointer { file_index: 0, line_index: 0 },
            &mut context,
        )
        .unwrap();

        match result {
            EvaluatedValue::Address(a) => assert_eq!(a, 0x10000u32 + 4),
            _ => panic!("Expected Address"),
        }
    }

    #[test]
    fn test_address_minus_integer() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 16, 0),
            0x10000,
        );

        // . - 8 where . = 0x10000 + 16
        let expr = Expression::MinusOp {
            lhs: Box::new(Expression::CurrentAddress),
            rhs: Box::new(Expression::Literal(8)),
        };

        let line = make_test_line(
            Segment::Text,
            16,
            LineContent::Label("test".to_string()),
        );
        let result = eval_expr(
            &expr,
            &line,
            &LinePointer { file_index: 0, line_index: 0 },
            &mut context,
        )
        .unwrap();

        match result {
            EvaluatedValue::Address(a) => {
                assert_eq!(a, (0x10000u32).wrapping_add(16).wrapping_sub(8))
            }
            _ => panic!("Expected Address"),
        }
    }

    #[test]
    fn test_address_minus_address() {
        let _source = make_test_source();

        // Create two address values at different locations
        let addr1 = EvaluatedValue::Address(0x10000u32 + 16);
        let addr2 = EvaluatedValue::Address(0x10000u32);

        let result = checked_sub(
            addr1,
            addr2,
            &Location { file: "test".to_string(), line: 1 },
        )
        .unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 16),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_address_plus_address_error() {
        let addr1 = EvaluatedValue::Address(0x10000u32);
        let addr2 = EvaluatedValue::Address(0x10008u32);

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
        let int_val = EvaluatedValue::Integer(8);
        let addr_val = EvaluatedValue::Address(0x10000u32);

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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::MultiplyOp {
            lhs: Box::new(Expression::Literal(6)),
            rhs: Box::new(Expression::Literal(7)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 42),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_integer_divide() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::DivideOp {
            lhs: Box::new(Expression::Literal(42)),
            rhs: Box::new(Expression::Literal(7)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 6),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_integer_modulo() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::ModuloOp {
            lhs: Box::new(Expression::Literal(43)),
            rhs: Box::new(Expression::Literal(7)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 1),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_division_by_zero_error() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::BitwiseOrOp {
            lhs: Box::new(Expression::Literal(0x0f)),
            rhs: Box::new(Expression::Literal(0xf0)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 0xff),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_bitwise_and() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::BitwiseAndOp {
            lhs: Box::new(Expression::Literal(0xff)),
            rhs: Box::new(Expression::Literal(0x0f)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 0x0f),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_bitwise_xor() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::BitwiseXorOp {
            lhs: Box::new(Expression::Literal(0xff)),
            rhs: Box::new(Expression::Literal(0x0f)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 0xf0),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_bitwise_not() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr =
            Expression::BitwiseNotOp { expr: Box::new(Expression::Literal(0)) };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, -1),
            _ => panic!("Expected Integer"),
        }
    }

    // ========================================================================
    // Shift Operations Tests
    // ========================================================================

    #[test]
    fn test_left_shift_simple() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(1)),
            rhs: Box::new(Expression::Literal(4)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 16),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_right_shift_simple() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(16)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 4),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_arithmetic_right_shift() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(-8)),
            rhs: Box::new(Expression::Literal(1)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, -4),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_shift_negative_amount_error() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(8)),
            rhs: Box::new(Expression::Literal(32)),
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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::PlusOp {
            lhs: Box::new(Expression::Literal(i32::MAX)),
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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::MinusOp {
            lhs: Box::new(Expression::Literal(i32::MIN)),
            rhs: Box::new(Expression::Literal(1)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("wraparound"));
    }

    #[test]
    fn test_overflow_multiplication() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::MultiplyOp {
            lhs: Box::new(Expression::Literal(i32::MAX)),
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
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::NegateOp {
            expr: Box::new(Expression::Literal(i32::MIN)),
        };

        let result = eval_simple(expr, &mut context);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("overflow"));
    }

    #[test]
    fn test_left_shift_sign_extension_ok() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        // -1 << 4 should work (all bits are sign bits)
        let expr = Expression::LeftShiftOp {
            lhs: Box::new(Expression::Literal(-1)),
            rhs: Box::new(Expression::Literal(4)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, -16),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_right_shift_no_loss() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        // 16 >> 2 = 4, no bits lost (16 = 0b10000, >> 2 = 0b100)
        let expr = Expression::RightShiftOp {
            lhs: Box::new(Expression::Literal(16)),
            rhs: Box::new(Expression::Literal(2)),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 4),
            _ => panic!("Expected Integer"),
        }
    }

    // ========================================================================
    // Unary Operations Tests
    // ========================================================================

    #[test]
    fn test_negate_positive() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr =
            Expression::NegateOp { expr: Box::new(Expression::Literal(42)) };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, -42),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_negate_negative() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

        let expr = Expression::NegateOp {
            expr: Box::new(Expression::NegateOp {
                expr: Box::new(Expression::Literal(42)),
            }),
        };

        let result = eval_simple(expr, &mut context).unwrap();

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 42),
            _ => panic!("Expected Integer"),
        }
    }

    // ========================================================================
    // Parentheses and Precedence Tests
    // ========================================================================

    #[test]
    fn test_parentheses_explicit() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

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

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 20),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_complex_expression() {
        let source = make_test_source();
        let mut context = new_evaluation_context(
            source,
            make_test_symbols(1),
            make_test_layout_with_line(Segment::Text, 0, 0),
            0x100e8,
        );

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

        match result {
            EvaluatedValue::Integer(i) => assert_eq!(i, 55),
            _ => panic!("Expected Integer"),
        }
    }

    // ========================================================================
    // Context Tests
    // ========================================================================

    #[test]
    fn test_context_segment_addresses() {
        let source = Source { files: vec![] };

        // Create a layout with the desired sizes (manually, since source has no files)
        let mut layout = crate::layout::Layout::new();
        layout.text_size = 1000; // Will cause data to be boundary
        layout.data_size = 500;
        layout.bss_size = 200;
        layout.header_size = 0;

        let context = new_evaluation_context(
            source,
            make_test_symbols(1),
            layout,
            0x100e8,
        );

        assert_eq!(context.text_start, 0x100e8);
        // data_start should be next 4K boundary after (0x100e8 + 1000)
        // 0x100e8 + 1000 = 0x104d0
        // Next 4K boundary is 0x11000
        assert_eq!(context.data_start, 0x11000);
        // bss_start should be data_start + data_size
        assert_eq!(context.bss_start, 0x11000 + 500);
    }
}
