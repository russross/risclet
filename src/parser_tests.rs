use crate::ast::*;
use crate::parser::*;
use crate::tokenizer::tokenize;

#[test]
fn test_parse_simple_instruction() {
    let line = "add a0, a1, a2";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::RType(
        RTypeOp::Add,
        Register::X10,
        Register::X11,
        Register::X12,
    )) = ast[0].content
    {
        // ok
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_label() {
    let line = "loop: add a0, a1, a2";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 2);
    assert_eq!(ast[0].content, LineContent::Label("loop".to_string()));
    if let LineContent::Instruction(Instruction::RType(
        RTypeOp::Add,
        Register::X10,
        Register::X11,
        Register::X12,
    )) = ast[1].content
    {
        // ok
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_directive() {
    let line = ".global main";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Directive(Directive::Global(vec)) = &ast[0].content {
        assert_eq!(vec, &["main".to_string()]);
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_expression() {
    let line = "li a0, 1 + 2";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::Pseudo(PseudoOp::Li(
        Register::X10,
        expr,
    ))) = &ast[0].content
    {
        if let Expression::PlusOp { lhs, rhs } = &**expr {
            if let Expression::Literal(1) = **lhs {
                if let Expression::Literal(2) = **rhs {
                    // ok
                } else {
                    panic!("Unexpected RHS");
                }
            } else {
                panic!("Unexpected LHS");
            }
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_numeric_label() {
    let line = "bnez a0, 1f";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::BType(
        BTypeOp::Bne,
        Register::X10,
        Register::X0,
        expr,
    )) = &ast[0].content
    {
        if let Expression::NumericLabelRef(nlr) = &**expr {
            assert_eq!(nlr.num, 1);
            assert!(nlr.is_forward);
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_load_with_expression() {
    let line = "lb a0, 1+2(a1)";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::LoadStore(
        LoadStoreOp::Lb,
        Register::X10,
        expr,
        Register::X11,
    )) = &ast[0].content
    {
        if let Expression::PlusOp { lhs, rhs } = &**expr {
            if let Expression::Literal(1) = **lhs {
                if let Expression::Literal(2) = **rhs {
                    // ok
                } else {
                    panic!("Unexpected RHS");
                }
            } else {
                panic!("Unexpected LHS");
            }
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_branch_with_expression() {
    let line = "beq a0, a1, 1+2";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::BType(
        BTypeOp::Beq,
        Register::X10,
        Register::X11,
        expr,
    )) = &ast[0].content
    {
        if let Expression::PlusOp { lhs, rhs } = &**expr {
            if let Expression::Literal(1) = **lhs {
                if let Expression::Literal(2) = **rhs {
                    // ok
                } else {
                    panic!("Unexpected RHS");
                }
            } else {
                panic!("Unexpected LHS");
            }
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_pseudo_li() {
    let line = "li a0, 0x13579BDF";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::Pseudo(PseudoOp::Li(
        Register::X10,
        expr,
    ))) = &ast[0].content
    {
        if let Expression::Literal(0x13579BDF) = **expr {
            // ok
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_directive_space() {
    let line = ".space 4";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Directive(Directive::Space(expr)) = &ast[0].content {
        if let Expression::Literal(4) = *expr {
            // ok
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_directive_balign() {
    let line = ".balign 8";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Directive(Directive::Balign(expr)) = &ast[0].content {
        if let Expression::Literal(8) = *expr {
            // ok
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_directive_string() {
    let line = ".string \"hello\", \"world\"";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Directive(Directive::String(vec)) = &ast[0].content {
        assert_eq!(*vec, vec!["hello".to_string(), "world".to_string()]);
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_directive_byte() {
    let line = ".byte 1, 2, 3";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Directive(Directive::Byte(vec)) = &ast[0].content {
        assert_eq!(vec.len(), 3);
        if let Expression::Literal(1) = vec[0] {
            if let Expression::Literal(2) = vec[1] {
                if let Expression::Literal(3) = vec[2] {
                    // ok
                } else {
                    panic!("Unexpected third");
                }
            } else {
                panic!("Unexpected second");
            }
        } else {
            panic!("Unexpected first");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_current_address() {
    let line = "li a0, .";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::Pseudo(PseudoOp::Li(
        Register::X10,
        expr,
    ))) = &ast[0].content
    {
        if let Expression::CurrentAddress = **expr {
            // ok
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_complex_expression() {
    let line = "li a0, (1 + 2) << 3";
    let tokens = tokenize(line).unwrap();
    let ast = parse(&tokens, "test".to_string(), 1).unwrap();
    assert_eq!(ast.len(), 1);
    if let LineContent::Instruction(Instruction::Pseudo(PseudoOp::Li(
        Register::X10,
        expr,
    ))) = &ast[0].content
    {
        if let Expression::LeftShiftOp { lhs, rhs } = &**expr {
            if let Expression::Parenthesized(plus_expr) = &**lhs {
                if let Expression::PlusOp { lhs: l, rhs: r } = &**plus_expr {
                    if let Expression::Literal(1) = **l {
                        if let Expression::Literal(2) = **r {
                            if let Expression::Literal(3) = **rhs {
                                // ok
                            } else {
                                panic!("Unexpected shift amount");
                            }
                        } else {
                            panic!("Unexpected plus RHS");
                        }
                    } else {
                        panic!("Unexpected plus LHS");
                    }
                } else {
                    panic!("Unexpected parenthesized");
                }
            } else {
                panic!("Unexpected LHS");
            }
        } else {
            panic!("Unexpected expression");
        }
    } else {
        panic!("Unexpected AST");
    }
}

#[test]
fn test_parse_error_leftover_tokens() {
    let line = "add a0, a1, a2 extra";
    let tokens = tokenize(line).unwrap();
    let result = parse(&tokens, "test".to_string(), 1);
    assert!(result.is_err(), "Should fail with leftover tokens");
    let err = result.unwrap_err();
    assert!(
        err.message().contains("Extra tokens after instruction"),
        "Error should mention extra tokens: {}",
        err
    );
}
