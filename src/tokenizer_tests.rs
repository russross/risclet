use crate::ast::*;
use crate::tokenizer::*;

#[test]
fn internal_names_preserve_the_dot_and_directive_distinction() {
    for name in [".L", ".L1", ".Lloop", ".Lfoo.bar_$"] {
        assert_eq!(
            tokenize(&format!("{}:", name)).unwrap(),
            vec![Token::Identifier(name.into()), Token::Colon]
        );
    }

    // The current address and known directives retain their own token kinds.
    assert_eq!(tokenize(".").unwrap(), vec![Token::Dot]);
    assert_eq!(
        tokenize(".globl .L1").unwrap(),
        vec![
            Token::Directive(DirectiveOp::Global),
            Token::Identifier(".L1".into())
        ]
    );
    assert!(tokenize(".l1").unwrap_err().contains("Unknown directive"));
    assert!(tokenize(".wordx").unwrap_err().contains("Unknown directive"));
}

#[test]
fn test_tokenize_simple_instruction() {
    let line = "add a0, a1, a2";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens.len(), 6);
    assert_eq!(tokens[0], Token::Identifier("add".to_string()));
    assert_eq!(tokens[1], Token::Register(Register::X10));
    assert_eq!(tokens[2], Token::Comma);
    assert_eq!(tokens[3], Token::Register(Register::X11));
    assert_eq!(tokens[4], Token::Comma);
    assert_eq!(tokens[5], Token::Register(Register::X12));
}

#[test]
fn test_tokenize_number() {
    let line = "li a0, 42";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(42));
}

#[test]
fn test_tokenize_hex() {
    let line = "li a0, 0x10";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(16));
}

#[test]
fn test_tokenize_hex_uppercase() {
    let line = "li a0, 0XFF";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(255));
}

#[test]
fn test_tokenize_binary() {
    let line = "li a0, 0b1010";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(10));
}

#[test]
fn test_tokenize_binary_uppercase() {
    let line = "li a0, 0B11111111";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(255));
}

#[test]
fn test_tokenize_octal_prefix() {
    let line = "li a0, 0o77";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(63));
}

#[test]
fn test_tokenize_octal_uppercase_prefix() {
    let line = "li a0, 0O755";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(493));
}

#[test]
fn test_tokenize_octal_leading_zero() {
    // Leading 0 without o/O suffix treated as octal in traditional assembly
    let line = "li a0, 0777";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[3], Token::Integer(511));
}

#[test]
fn test_tokenize_string() {
    let line = ".string \"hello\"";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens[1], Token::StringLiteral("hello".to_string()));
}

#[test]
fn test_tokenize_comment() {
    let line = "add a0, a1, a2 # comment";
    let tokens = tokenize(line).unwrap();
    assert_eq!(tokens.len(), 6); // comment removed
}
