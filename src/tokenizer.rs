use crate::ast::{DirectiveOp, OperatorOp, Register, Token};

pub fn tokenize(line: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {
                chars.next();
            }
            '#' => {
                // Comment to end of line
                for _c in chars.by_ref() {
                    // discard comment
                }
            }
            ':' => {
                tokens.push(Token::Colon);
                chars.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                chars.next();
            }
            '(' => {
                tokens.push(Token::OpenParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::CloseParen);
                chars.next();
            }
            '+' => {
                tokens.push(Token::Operator(OperatorOp::Plus));
                chars.next();
            }
            '-' => {
                tokens.push(Token::Operator(OperatorOp::Minus));
                chars.next();
            }
            '*' => {
                tokens.push(Token::Operator(OperatorOp::Multiply));
                chars.next();
            }
            '/' => {
                tokens.push(Token::Operator(OperatorOp::Divide));
                chars.next();
            }
            '%' => {
                tokens.push(Token::Operator(OperatorOp::Modulo));
                chars.next();
            }
            '|' => {
                tokens.push(Token::Operator(OperatorOp::BitwiseOr));
                chars.next();
            }
            '&' => {
                tokens.push(Token::Operator(OperatorOp::BitwiseAnd));
                chars.next();
            }
            '^' => {
                tokens.push(Token::Operator(OperatorOp::BitwiseXor));
                chars.next();
            }
            '~' => {
                tokens.push(Token::Operator(OperatorOp::BitwiseNot));
                chars.next();
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'<') {
                    chars.next();
                    tokens.push(Token::Operator(OperatorOp::LeftShift));
                } else {
                    return Err("Unexpected '<'".to_string());
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::Operator(OperatorOp::RightShift));
                } else {
                    return Err("Unexpected '>'".to_string());
                }
            }
            '\'' => {
                chars.next();
                let ch = chars.next().ok_or("Unexpected end in char literal")?;
                let c = if ch == '\\' {
                    let esc = chars.next().ok_or("Unexpected end in escape sequence")?;
                    match esc {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        '0' => '\0',
                        _ => {
                            return Err(format!("Unknown escape sequence \\{}", esc));
                        }
                    }
                } else {
                    ch
                };
                if chars.next() != Some('\'') {
                    return Err("Unclosed char literal".to_string());
                }
                tokens.push(Token::Integer(c as i32));
            }
            '"' => {
                chars.next();
                let s = parse_string_literal(&mut chars)?;
                tokens.push(Token::StringLiteral(s));
            }
            '.' => {
                chars.next();
                if chars.peek().is_some() && chars.peek().unwrap().is_alphanumeric() {
                    let ident = parse_identifier(&mut chars)?;
                    let dir = match ident.as_str() {
                        "global" => DirectiveOp::Global,
                        "globl" => DirectiveOp::Global,
                        "equ" => DirectiveOp::Equ,
                        "set" => DirectiveOp::Equ,
                        "text" => DirectiveOp::Text,
                        "data" => DirectiveOp::Data,
                        "bss" => DirectiveOp::Bss,
                        "space" => DirectiveOp::Space,
                        "balign" => DirectiveOp::Balign,
                        "string" => DirectiveOp::String,
                        "asciz" => DirectiveOp::Asciz,
                        "byte" => DirectiveOp::Byte,
                        "2byte" => DirectiveOp::TwoByte,
                        "4byte" => DirectiveOp::FourByte,
                        _ => {
                            return Err(format!("Unknown directive .{}", ident));
                        }
                    };
                    tokens.push(Token::Directive(dir));
                } else {
                    tokens.push(Token::Dot);
                }
            }
            '0'..='9' => {
                let num = parse_number(&mut chars)?;
                tokens.push(Token::Integer(num));
            }
            'a'..='z' | 'A'..='Z' | '_' | '$' => {
                let ident = parse_identifier(&mut chars)?;
                if let Some(reg) = parse_register(&ident) {
                    tokens.push(Token::Register(reg));
                } else {
                    tokens.push(Token::Identifier(ident));
                }
            }
            _ => return Err(format!("Unexpected character '{}'", ch)),
        }
    }
    Ok(tokens)
}

fn parse_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    let mut s = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' || ch == '.' || ch == '$' {
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    if s.is_empty() { Err("Empty identifier".to_string()) } else { Ok(s) }
}

fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<i32, String> {
    let mut s = String::new();
    let mut base = 10;
    if chars.peek() == Some(&'0') {
        s.push('0');
        chars.next();
        match chars.peek() {
            Some('x') | Some('X') => {
                s.push('x');
                chars.next();
                base = 16;
            }
            Some('b') | Some('B') => {
                s.push('b');
                chars.next();
                base = 2;
            }
            Some('o') | Some('O') => {
                s.push('o');
                chars.next();
                base = 8;
            }
            Some(&ch) if ch.is_ascii_digit() => {
                // Leading 0 followed by digits -> octal (traditional C-style)
                base = 8;
            }
            _ => {}
        }
    }
    while let Some(&ch) = chars.peek() {
        if (base == 10 && ch.is_ascii_digit())
            || (base == 16 && ch.is_ascii_hexdigit())
            || (base == 8 && ch.is_digit(8))
            || (base == 2 && (ch == '0' || ch == '1'))
        {
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    let num_str = if (base == 16 && (s.starts_with("0x") || s.starts_with("0X")))
        || (base == 2 && (s.starts_with("0b") || s.starts_with("0B")))
        || (base == 8 && (s.starts_with("0o") || s.starts_with("0O")))
    {
        &s[2..]
    } else {
        &s
    };
    match i32::from_str_radix(num_str, base) {
        Ok(val) => Ok(val),
        Err(_) => match u32::from_str_radix(num_str, base) {
            Ok(val) => Ok(val as i32),
            Err(_) => Err(format!("Invalid number {}", s)),
        },
    }
}

fn parse_string_literal(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    let mut s = String::new();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            return Ok(s);
        } else if ch == '\\' {
            let esc = chars.next().ok_or("Unexpected end in escape sequence")?;
            let c = match esc {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                '0' => '\0',
                _ => return Err(format!("Unknown escape sequence \\{}", esc)),
            };
            s.push(c);
        } else {
            s.push(ch);
        }
    }
    Err("Unclosed string literal".to_string())
}

fn parse_register(ident: &str) -> Option<Register> {
    match ident {
        "zero" | "x0" => Some(Register::X0),
        "ra" | "x1" => Some(Register::X1),
        "sp" | "x2" => Some(Register::X2),
        "gp" | "x3" => Some(Register::X3),
        "tp" | "x4" => Some(Register::X4),
        "t0" | "x5" => Some(Register::X5),
        "t1" | "x6" => Some(Register::X6),
        "t2" | "x7" => Some(Register::X7),
        "fp" | "s0" | "x8" => Some(Register::X8),
        "s1" | "x9" => Some(Register::X9),
        "a0" | "x10" => Some(Register::X10),
        "a1" | "x11" => Some(Register::X11),
        "a2" | "x12" => Some(Register::X12),
        "a3" | "x13" => Some(Register::X13),
        "a4" | "x14" => Some(Register::X14),
        "a5" | "x15" => Some(Register::X15),
        "a6" | "x16" => Some(Register::X16),
        "a7" | "x17" => Some(Register::X17),
        "s2" | "x18" => Some(Register::X18),
        "s3" | "x19" => Some(Register::X19),
        "s4" | "x20" => Some(Register::X20),
        "s5" | "x21" => Some(Register::X21),
        "s6" | "x22" => Some(Register::X22),
        "s7" | "x23" => Some(Register::X23),
        "s8" | "x24" => Some(Register::X24),
        "s9" | "x25" => Some(Register::X25),
        "s10" | "x26" => Some(Register::X26),
        "s11" | "x27" => Some(Register::X27),
        "t3" | "x28" => Some(Register::X28),
        "t4" | "x29" => Some(Register::X29),
        "t5" | "x30" => Some(Register::X30),
        "t6" | "x31" => Some(Register::X31),
        _ => None,
    }
}
