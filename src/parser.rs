use crate::ast::{
    AtomicOp, BTypeOp, CompressedOp, CompressedOperands, Directive,
    DirectiveOp, Expression, ITypeOp, Instruction, JTypeOp, Line, LineContent,
    LoadStoreOp, Location, MemoryOrdering, NumericLabelRef, OperatorOp,
    PseudoOp, RTypeOp, Register, SpecialOp, Token, UTypeOp,
};
use crate::error::{Result, RiscletError};

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    file: String,
    line: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], file: String, line: usize) -> Self {
        Parser { tokens, pos: 0, file, line }
    }

    fn location(&self) -> Location {
        Location { file: self.file.clone(), line: self.line }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        if let Some(t) = self.peek() {
            if t == expected {
                self.next();
                Ok(())
            } else {
                let expected_name = match expected {
                    Token::Colon => "colon (:)",
                    Token::Comma => "comma (,)",
                    Token::OpenParen => "left parenthesis (()",
                    Token::CloseParen => "right parenthesis ())",
                    _ => "expected token",
                };
                let found_name = match t {
                    Token::Colon => "colon (:)",
                    Token::Comma => "comma (,)",
                    Token::OpenParen => "left parenthesis (()",
                    Token::CloseParen => "right parenthesis ())",
                    Token::Identifier(s) => &format!("identifier '{}'", s),
                    Token::Register(r) => &format!("register {}", r),
                    Token::Integer(n) => &format!("number {}", n),
                    _ => "unexpected token",
                };
                Err(RiscletError::from_context(
                    format!(
                        "Expected {} but found {}",
                        expected_name, found_name
                    ),
                    self.location(),
                ))
            }
        } else {
            let expected_name = match expected {
                Token::Colon => "colon (:)",
                Token::Comma => "comma (,)",
                Token::OpenParen => "left parenthesis (()",
                Token::CloseParen => "right parenthesis ())",
                _ => "expected token",
            };
            Err(RiscletError::from_context(
                format!("Expected {} but reached end of line", expected_name),
                self.location(),
            ))
        }
    }

    // Grammar: ident
    // Example: foo
    fn parse_identifier(&mut self) -> Result<String> {
        if let Some(Token::Identifier(s)) = self.next() {
            Ok(s)
        } else {
            Err(RiscletError::from_context(
                "Expected an identifier (label, symbol, or directive name)"
                    .to_string(),
                self.location(),
            ))
        }
    }

    // Grammar: reg
    // Example: a0
    fn parse_register(&mut self) -> Result<Register> {
        if let Some(Token::Register(r)) = self.next() {
            Ok(r)
        } else {
            Err(RiscletError::from_context(
                "Expected a register name (x0-x31, a0-a7, sp, ra, etc.)"
                    .to_string(),
                self.location(),
            ))
        }
    }

    // Grammar: exp (calls parse_bitwise_or) (part of expression grammar)
    // Example: a + b * c
    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_bitwise_or()
    }

    // Grammar: bitwise_or ::= bitwise_xor ( | bitwise_xor )* (part of expression grammar)
    // Example: a | b | c
    fn parse_bitwise_or(&mut self) -> Result<Expression> {
        let mut left = self.parse_bitwise_xor()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::BitwiseOr) => {
                    self.next();
                    let right = self.parse_bitwise_xor()?;
                    left = Expression::BitwiseOrOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // Grammar: bitwise_xor ::= bitwise_and ( ^ bitwise_and )* (part of expression grammar)
    // Example: a ^ b
    fn parse_bitwise_xor(&mut self) -> Result<Expression> {
        let mut left = self.parse_bitwise_and()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::BitwiseXor) => {
                    self.next();
                    let right = self.parse_bitwise_and()?;
                    left = Expression::BitwiseXorOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // Grammar: bitwise_and ::= shift ( & shift )* (part of expression grammar)
    // Example: a & b & c
    fn parse_bitwise_and(&mut self) -> Result<Expression> {
        let mut left = self.parse_shift()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::BitwiseAnd) => {
                    self.next();
                    let right = self.parse_shift()?;
                    left = Expression::BitwiseAndOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // Grammar: shift ::= additive ( << additive | >> additive )* (part of expression grammar)
    // Examples: a << 1, b >> 2, c << 1 >> 2
    fn parse_shift(&mut self) -> Result<Expression> {
        let mut left = self.parse_additive()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::LeftShift) => {
                    self.next();
                    let right = self.parse_additive()?;
                    left = Expression::LeftShiftOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                Token::Operator(OperatorOp::RightShift) => {
                    self.next();
                    let right = self.parse_additive()?;
                    left = Expression::RightShiftOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // Grammar: additive ::= multiplicative ( + multiplicative | - multiplicative )* (part of expression grammar)
    // Examples: a + b, c - d, e + f - g
    fn parse_additive(&mut self) -> Result<Expression> {
        let mut left = self.parse_multiplicative()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::Plus) => {
                    self.next();
                    let right = self.parse_multiplicative()?;
                    left = Expression::PlusOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                Token::Operator(OperatorOp::Minus) => {
                    self.next();
                    let right = self.parse_multiplicative()?;
                    left = Expression::MinusOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // Grammar: multiplicative ::= unary ( * unary | / unary | % unary )* (part of expression grammar)
    // Examples: a * b, c / d, e % f, g * h / i % j
    fn parse_multiplicative(&mut self) -> Result<Expression> {
        let mut left = self.parse_unary()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::Multiply) => {
                    self.next();
                    let right = self.parse_unary()?;
                    left = Expression::MultiplyOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                Token::Operator(OperatorOp::Divide) => {
                    self.next();
                    let right = self.parse_unary()?;
                    left = Expression::DivideOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                Token::Operator(OperatorOp::Modulo) => {
                    self.next();
                    let right = self.parse_unary()?;
                    left = Expression::ModuloOp {
                        lhs: Box::new(left),
                        rhs: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // Grammar: unary ::= - unary | ~ unary | operand (part of expression grammar)
    // Examples: -a (negation), ~b (bitwise not), c (no unary op)
    fn parse_unary(&mut self) -> Result<Expression> {
        if let Some(Token::Operator(OperatorOp::Minus)) = self.peek() {
            self.next();
            let expr = self.parse_unary()?;
            Ok(Expression::NegateOp { expr: Box::new(expr) })
        } else if let Some(Token::Operator(OperatorOp::BitwiseNot)) =
            self.peek()
        {
            self.next();
            let expr = self.parse_unary()?;
            Ok(Expression::BitwiseNotOp { expr: Box::new(expr) })
        } else {
            self.parse_operand()
        }
    }

    // Grammar: operand ::= int | ident | label_ref | ( exp ) (part of expression grammar)
    // Examples: 42 (integer literal), foo (identifier), 1f (numeric label), (a + b) (parenthesized expression)
    fn parse_operand(&mut self) -> Result<Expression> {
        if let Some(t) = self.peek().cloned() {
            match t {
                Token::Integer(i) => {
                    let num = i as u32; // assume positive for labels
                    self.next();
                    // Check for numeric label: int f | int b
                    if let Some(Token::Identifier(s)) = self.peek() {
                        if s == "f" {
                            self.next();
                            Ok(Expression::NumericLabelRef(NumericLabelRef {
                                num,
                                is_forward: true,
                            }))
                        } else if s == "b" {
                            self.next();
                            Ok(Expression::NumericLabelRef(NumericLabelRef {
                                num,
                                is_forward: false,
                            }))
                        } else {
                            Ok(Expression::Literal(i)) // plain int
                        }
                    } else {
                        Ok(Expression::Literal(i)) // plain int
                    }
                }

                Token::Identifier(s) => {
                    self.next();
                    Ok(Expression::Identifier(s)) // ident or symbolic label
                }
                Token::OpenParen => {
                    self.next();
                    let expr = self.parse_expression()?;
                    self.expect(&Token::CloseParen)?;
                    Ok(Expression::Parenthesized(Box::new(expr))) // ( exp )
                }
                Token::Dot => {
                    self.next();
                    Ok(Expression::CurrentAddress) // .
                }
                _ => Err(RiscletError::from_context(
                    "Expected an operand (number, label, register, or parenthesized expression)".to_string(),
                    self.location(),
                )),
            }
        } else {
            Err(RiscletError::from_context(
                "Expected an operand but reached end of line".to_string(),
                self.location(),
            ))
        }
    }

    // Grammar: [ident | int :] [directive | instruction]
    // Examples: loop: add a0, a1, a2 (labeled instruction), .global foo (directive), add a0, a1, a2 (unlabeled instruction)
    fn parse_line(&mut self) -> Result<Vec<Line>> {
        let location = Location { file: self.file.clone(), line: self.line };
        let mut lines = Vec::new();
        // Check for label: [ident | int :] (peekahead and backtrack if no colon)
        let mut label = None;
        let pos_backup = self.pos;
        if let Some(t) = self.peek().cloned() {
            let l = match t {
                Token::Identifier(s) => s,
                Token::Integer(i) => i.to_string(),
                _ => {
                    self.pos = pos_backup;
                    "".to_string()
                }
            };
            if !l.is_empty() {
                self.next(); // consume the token
                if self.expect(&Token::Colon).is_ok() {
                    label = Some(l);
                } else {
                    self.pos = pos_backup; // backtrack
                }
            } else {
                self.pos = pos_backup;
            }
        }
        // Parse content if any
        let content = if self.pos < self.tokens.len() {
            Some(if let Some(Token::Directive(_)) = self.peek() {
                LineContent::Directive(self.parse_directive()?)
            } else {
                LineContent::Instruction(self.parse_instruction()?)
            })
        } else {
            None
        };
        // Emit lines
        if let Some(l) = label {
            lines.push(Line {
                location: location.clone(),
                content: LineContent::Label(l),
            });
        }
        if let Some(c) = content {
            lines.push(Line { location, content: c });
        }
        if lines.is_empty() {
            return Err(RiscletError::from_context(
                "Empty line: a label must be followed by an instruction or directive".to_string(),
                self.location(),
            ));
        }
        Ok(lines)
    }

    // Grammar: .global ident | .equ ident , exp | .text | .data | .bss | .space exp | .balign exp | .string string [, string]* | .asciz string [, string]* | .byte exp [, exp]* | .2byte exp [, exp]* | .4byte exp [, exp]* | .8byte exp [, exp]*
    // Examples: .global main, .equ SIZE, 100, .text, .data, .bss, .space 4, .balign 8, .string "hello", "world", .asciz "foo", .byte 1, 2, 3, .2byte 10, 20, .4byte 100, .8byte 1000
    fn parse_directive(&mut self) -> Result<Directive> {
        if let Some(Token::Directive(d)) = self.next() {
            match d {
                DirectiveOp::Global => {
                    let mut names = Vec::new();
                    names.push(self.parse_identifier()?);
                    while let Some(Token::Comma) = self.peek() {
                        self.next();
                        names.push(self.parse_identifier()?);
                    }
                    Ok(Directive::Global(names))
                }
                DirectiveOp::Equ => {
                    let name = self.parse_identifier()?;
                    self.expect(&Token::Comma)?;
                    let expr = self.parse_expression()?;
                    Ok(Directive::Equ(name, expr))
                }
                DirectiveOp::Text => Ok(Directive::Text),
                DirectiveOp::Data => Ok(Directive::Data),
                DirectiveOp::Bss => Ok(Directive::Bss),
                DirectiveOp::Space => {
                    let expr = self.parse_expression()?;
                    Ok(Directive::Space(expr))
                }
                DirectiveOp::Balign => {
                    let expr = self.parse_expression()?;
                    Ok(Directive::Balign(expr))
                }
                DirectiveOp::String => {
                    let mut strings = Vec::new();
                    while let Some(Token::StringLiteral(s)) = self.peek() {
                        strings.push(s.clone());
                        self.next();
                        if let Some(Token::Comma) = self.peek() {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    Ok(Directive::String(strings))
                }
                DirectiveOp::Asciz => {
                    let mut strings = Vec::new();
                    while let Some(Token::StringLiteral(s)) = self.peek() {
                        strings.push(s.clone());
                        self.next();
                        if let Some(Token::Comma) = self.peek() {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    Ok(Directive::Asciz(strings))
                }
                DirectiveOp::Byte => {
                    let mut exprs = Vec::new();
                    exprs.push(self.parse_expression()?);
                    while let Some(Token::Comma) = self.peek() {
                        self.next();
                        exprs.push(self.parse_expression()?);
                    }
                    Ok(Directive::Byte(exprs))
                }
                DirectiveOp::TwoByte => {
                    let mut exprs = Vec::new();
                    exprs.push(self.parse_expression()?);
                    while let Some(Token::Comma) = self.peek() {
                        self.next();
                        exprs.push(self.parse_expression()?);
                    }
                    Ok(Directive::TwoByte(exprs))
                }
                DirectiveOp::FourByte => {
                    let mut exprs = Vec::new();
                    exprs.push(self.parse_expression()?);
                    while let Some(Token::Comma) = self.peek() {
                        self.next();
                        exprs.push(self.parse_expression()?);
                    }
                    Ok(Directive::FourByte(exprs))
                }
            }
        } else {
            Err(RiscletError::from_context(
                "Expected a directive (like .global, .text, .data, .bss, .space, .byte, etc.)".to_string(),
                self.location(),
            ))
        }
    }

    // Grammar: opcode-specific (see below for each type)
    fn parse_instruction(&mut self) -> Result<Instruction> {
        let opcode = self.parse_identifier()?;

        // Check for compressed instructions (c.* prefix)
        if let Some(c_op) = opcode.strip_prefix("c.") {
            return self.parse_compressed_instruction(c_op);
        }

        match opcode.as_str() {
            // R-type
            "add" => self.parse_rtype(RTypeOp::Add),
            "sub" => self.parse_rtype(RTypeOp::Sub),
            "sll" => self.parse_rtype(RTypeOp::Sll),
            "slt" => self.parse_rtype(RTypeOp::Slt),
            "sltu" => self.parse_rtype(RTypeOp::Sltu),
            "xor" => self.parse_rtype(RTypeOp::Xor),
            "srl" => self.parse_rtype(RTypeOp::Srl),
            "sra" => self.parse_rtype(RTypeOp::Sra),
            "or" => self.parse_rtype(RTypeOp::Or),
            "and" => self.parse_rtype(RTypeOp::And),
            "mul" => self.parse_rtype(RTypeOp::Mul),
            "mulh" => self.parse_rtype(RTypeOp::Mulh),
            "mulhsu" => self.parse_rtype(RTypeOp::Mulhsu),
            "mulhu" => self.parse_rtype(RTypeOp::Mulhu),
            "div" => self.parse_rtype(RTypeOp::Div),
            "divu" => self.parse_rtype(RTypeOp::Divu),
            "rem" => self.parse_rtype(RTypeOp::Rem),
            "remu" => self.parse_rtype(RTypeOp::Remu),
            // I-type
            "addi" => self.parse_itype(ITypeOp::Addi),
            "slli" => self.parse_itype(ITypeOp::Slli),
            "slti" => self.parse_itype(ITypeOp::Slti),
            "sltiu" => self.parse_itype(ITypeOp::Sltiu),
            "xori" => self.parse_itype(ITypeOp::Xori),
            "ori" => self.parse_itype(ITypeOp::Ori),
            "andi" => self.parse_itype(ITypeOp::Andi),
            "srli" => self.parse_itype(ITypeOp::Srli),
            "srai" => self.parse_itype(ITypeOp::Srai),
            "jalr" => self.parse_jalr(),
            // B-type
            "beq" => self.parse_btype(BTypeOp::Beq),
            "bne" => self.parse_btype(BTypeOp::Bne),
            "blt" => self.parse_btype(BTypeOp::Blt),
            "bge" => self.parse_btype(BTypeOp::Bge),
            "bltu" => self.parse_btype(BTypeOp::Bltu),
            "bgeu" => self.parse_btype(BTypeOp::Bgeu),
            "bgez" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(
                    BTypeOp::Bge,
                    rs1,
                    Register::X0,
                    Box::new(expr),
                )) // Special: rs2 is x0
            }
            "bnez" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(
                    BTypeOp::Bne,
                    rs1,
                    Register::X0,
                    Box::new(expr),
                )) // Special: rs2 is x0
            }
            // U-type
            "lui" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                Ok(Instruction::UType(UTypeOp::Lui, rd, Box::new(imm)))
            }
            "auipc" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                Ok(Instruction::UType(UTypeOp::Auipc, rd, Box::new(imm)))
            }
            // J-type
            "jal" => {
                // Optional rd: [reg ,] expression
                let rd = if let Some(Token::Register(_)) = self.peek() {
                    self.parse_register()?
                } else {
                    Register::X1 // ra
                };
                if let Some(Token::Comma) = self.peek() {
                    self.next();
                }
                let expr = self.parse_expression()?;
                Ok(Instruction::JType(JTypeOp::Jal, rd, Box::new(expr)))
            }
            // Special
            "fence" => self.parse_fence(),
            "fence.tso" => Ok(Instruction::Special(SpecialOp::FenceTso)),
            "fence.i" => Ok(Instruction::Special(SpecialOp::FenceI)),
            "ecall" => Ok(Instruction::Special(SpecialOp::Ecall)),
            "ebreak" => Ok(Instruction::Special(SpecialOp::Ebreak)),
            // Load/store
            "lb" => self.parse_load(LoadStoreOp::Lb),
            "lh" => self.parse_load(LoadStoreOp::Lh),
            "lw" => self.parse_load(LoadStoreOp::Lw),
            "lbu" => self.parse_load(LoadStoreOp::Lbu),
            "lhu" => self.parse_load(LoadStoreOp::Lhu),
            "sb" => self.parse_store(LoadStoreOp::Sb),
            "sh" => self.parse_store(LoadStoreOp::Sh),
            "sw" => self.parse_store(LoadStoreOp::Sw),
            // Pseudo
            "li" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                Ok(Instruction::Pseudo(PseudoOp::Li(rd, Box::new(imm))))
            }
            "la" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::Pseudo(PseudoOp::La(rd, Box::new(expr))))
            }
            "call" => {
                let expr = self.parse_expression()?;
                Ok(Instruction::Pseudo(PseudoOp::Call(Box::new(expr))))
            }
            "tail" => {
                let expr = self.parse_expression()?;
                Ok(Instruction::Pseudo(PseudoOp::Tail(Box::new(expr))))
            }
            "mv" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(
                    ITypeOp::Addi,
                    rd,
                    rs,
                    Box::new(Expression::Literal(0)),
                ))
            }
            "ret" => Ok(Instruction::IType(
                ITypeOp::Jalr,
                Register::X0,
                Register::X1,
                Box::new(Expression::Literal(0)),
            )),
            "nop" => Ok(Instruction::IType(
                ITypeOp::Addi,
                Register::X0,
                Register::X0,
                Box::new(Expression::Literal(0)),
            )),
            "neg" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::RType(RTypeOp::Sub, rd, Register::X0, rs))
            }
            "seqz" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(
                    ITypeOp::Sltiu,
                    rd,
                    rs,
                    Box::new(Expression::Literal(1)),
                ))
            }
            "snez" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::RType(RTypeOp::Sltu, rd, Register::X0, rs))
            }
            "sltz" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::RType(RTypeOp::Slt, rd, rs, Register::X0))
            }
            "sgtz" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::RType(RTypeOp::Slt, rd, Register::X0, rs))
            }
            "beqz" => {
                let rs = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(
                    BTypeOp::Beq,
                    rs,
                    Register::X0,
                    Box::new(expr),
                ))
            }
            "blez" => {
                let rs = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(
                    BTypeOp::Bge,
                    Register::X0,
                    rs,
                    Box::new(expr),
                ))
            }
            "bltz" => {
                let rs = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(
                    BTypeOp::Blt,
                    rs,
                    Register::X0,
                    Box::new(expr),
                ))
            }
            "bgtz" => {
                let rs = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(
                    BTypeOp::Blt,
                    Register::X0,
                    rs,
                    Box::new(expr),
                ))
            }
            "bgt" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(BTypeOp::Blt, rs2, rs1, Box::new(expr)))
            }
            "ble" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(BTypeOp::Bge, rs2, rs1, Box::new(expr)))
            }
            "bgtu" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(BTypeOp::Bltu, rs2, rs1, Box::new(expr)))
            }
            "bleu" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let expr = self.parse_expression()?;
                Ok(Instruction::BType(BTypeOp::Bgeu, rs2, rs1, Box::new(expr)))
            }
            "j" => {
                let expr = self.parse_expression()?;
                Ok(Instruction::JType(
                    JTypeOp::Jal,
                    Register::X0,
                    Box::new(expr),
                ))
            }
            "jr" => {
                let rs = self.parse_register()?;
                Ok(Instruction::IType(
                    ITypeOp::Jalr,
                    Register::X0,
                    rs,
                    Box::new(Expression::Literal(0)),
                ))
            }
            "not" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(
                    ITypeOp::Xori,
                    rd,
                    rs,
                    Box::new(Expression::Literal(-1)),
                ))
            }
            _ => {
                // Try to parse as atomic instruction (A extension)
                if let Some((op, ordering)) = Self::parse_atomic_name(&opcode) {
                    return self.parse_atomic(op, ordering);
                }

                Err(RiscletError::from_context(
                    format!(
                        "Unknown instruction '{}': check spelling or consult the RISC-V ISA reference",
                        opcode
                    ),
                    self.location(),
                ))
            }
        }
    }

    // Grammar: reg , reg , reg
    // Example: add a0, a1, a2
    fn parse_rtype(&mut self, op: RTypeOp) -> Result<Instruction> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs2 = self.parse_register()?;
        Ok(Instruction::RType(op, rd, rs1, rs2))
    }

    // Grammar: reg , reg , exp
    // Examples: addi a0, a1, 1, addi a0, a1, 'z' - 'a'
    fn parse_itype(&mut self, op: ITypeOp) -> Result<Instruction> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let imm = self.parse_expression()?;
        Ok(Instruction::IType(op, rd, rs1, Box::new(imm)))
    }

    // Grammar: reg , [offset] ( reg ) | reg , reg [, offset]
    // Examples: jalr ra, 0(t0), jalr ra, (t0), jalr ra, t0, jalr ra, t0, 0
    fn parse_jalr(&mut self) -> Result<Instruction> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;

        // Lookahead for ( reg ) to handle zero offset: jalr rd, (rs1)
        if let Some(Token::OpenParen) = self.peek() {
            let pos_backup = self.pos;
            self.next();
            if let Ok(rs1) = self.parse_register()
                && self.expect(&Token::CloseParen).is_ok()
            {
                return Ok(Instruction::IType(
                    ITypeOp::Jalr,
                    rd,
                    rs1,
                    Box::new(Expression::Literal(0)),
                ));
            }
            self.pos = pos_backup;
        }

        // Try to parse as register first (for alternate format or just "jalr rd, rs1")
        let first_token_pos = self.pos;
        if let Ok(rs1) = self.parse_register() {
            // Check what follows the register
            match self.peek() {
                Some(Token::Comma) => {
                    // jalr rd, rs1, offset
                    self.next();
                    let offset = self.parse_expression()?;
                    return Ok(Instruction::IType(ITypeOp::Jalr, rd, rs1, Box::new(offset)));
                }
                _ => {
                    // jalr rd, rs1 (offset defaults to 0)
                    return Ok(Instruction::IType(
                        ITypeOp::Jalr,
                        rd,
                        rs1,
                        Box::new(Expression::Literal(0)),
                    ));
                }
            }
        }

        // Not a register, so must be offset(rs1) format
        self.pos = first_token_pos;
        let offset = self.parse_expression()?;
        if let Some(Token::OpenParen) = self.peek() {
            self.next();
            let rs1 = self.parse_register()?;
            self.expect(&Token::CloseParen)?;
            Ok(Instruction::IType(ITypeOp::Jalr, rd, rs1, Box::new(offset)))
        } else {
            Err(RiscletError::from_context(
                "jalr expects offset(rs1), (rs1), rs1, or rs1, offset syntax".to_string(),
                self.location(),
            ))
        }
    }

    // Grammar: reg , reg , expression
    // Examples: beq a0, a1, loop (symbolic), beq a0, a1, . + 8 (expression)
    fn parse_btype(&mut self, op: BTypeOp) -> Result<Instruction> {
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs2 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let expr = self.parse_expression()?;
        Ok(Instruction::BType(op, rs1, rs2, Box::new(expr)))
    }

    // Grammar: fence | fence pred, succ
    // Examples: fence, fence iorw,iorw, fence r,w, fence i,o
    fn parse_fence(&mut self) -> Result<Instruction> {
        // Parse optional pred, succ parameters
        let (pred, succ) = if let Some(Token::Identifier(_)) = self.peek() {
            let pred_str = self.parse_identifier()?;
            self.expect(&Token::Comma)?;
            let succ_str = self.parse_identifier()?;
            (
                self.parse_fence_bits(&pred_str)?,
                self.parse_fence_bits(&succ_str)?,
            )
        } else {
            // Default: iorw, iorw (0xF, 0xF)
            (0xF, 0xF)
        };
        Ok(Instruction::Special(SpecialOp::Fence { pred, succ }))
    }

    // Helper: parse fence ordering bits from string or numeric literal
    // Valid: "i" (input), "o" (output), "r" (read), "w" (write), or combinations "iorw", "rw", etc.
    // Also accepts numeric values like "15" or "0xf"
    fn parse_fence_bits(&self, s: &str) -> Result<u8> {
        // Try parsing as integer first (for numeric values in identifier form)
        if let Ok(val) = s.parse::<u8>() {
            return Ok(val & 0x0F); // Mask to 4 bits
        }

        // Parse letter combinations
        let mut bits = 0u8;
        for ch in s.chars() {
            match ch {
                'i' => bits |= 0x08, // input (bit 3)
                'o' => bits |= 0x04, // output (bit 2)
                'r' => bits |= 0x02, // read (bit 1)
                'w' => bits |= 0x01, // write (bit 0)
                _ => {
                    return Err(RiscletError::from_context(
                        format!(
                            "Invalid fence ordering character '{}': must be 'i' (input), 'o' (output), 'r' (read), or 'w' (write)",
                            ch
                        ),
                        self.location(),
                    ));
                }
            }
        }
        Ok(bits)
    }

    // Grammar: reg , [exp] ( reg ) | reg , exp (global load, exp not followed by '(')
    // Examples: lb a0, 0(sp) (immediate offset), lb a0, (sp) (zero offset via peekahead), lb a0, label (global load, no parens; parentheses in exp like (label + 4) are part of the expression)
    fn parse_load(&mut self, op: LoadStoreOp) -> Result<Instruction> {
        let reg = self.parse_register()?;
        self.expect(&Token::Comma)?;
        // Peekahead for ( reg ) to handle zero offset: reg , ( reg )
        if let Some(Token::OpenParen) = self.peek() {
            let pos_backup = self.pos;
            self.next();
            if let Ok(rs) = self.parse_register()
                && self.expect(&Token::CloseParen).is_ok()
            {
                return Ok(Instruction::LoadStore(
                    op,
                    reg,
                    Box::new(Expression::Literal(0)),
                    rs,
                ));
            }
            self.pos = pos_backup;
        }
        let expr = self.parse_expression()?;
        // If followed by ( reg ), it's reg , exp ( reg ); else global load
        if let Some(Token::OpenParen) = self.peek() {
            self.next();
            let rs = self.parse_register()?;
            self.expect(&Token::CloseParen)?;
            Ok(Instruction::LoadStore(op, reg, Box::new(expr), rs))
        } else {
            Ok(Instruction::Pseudo(PseudoOp::LoadGlobal(
                op,
                reg,
                Box::new(expr),
            )))
        }
    }

    // Grammar: reg , [exp] ( reg ) | reg , exp , reg (global store, exp not followed by '(')
    // Examples: sb a0, 0(sp) (immediate offset), sb a0, (sp) (zero offset via peekahead), sb a0, label, t0 (global store, no parens; parentheses in exp like (label + 4) are part of the expression)
    fn parse_store(&mut self, op: LoadStoreOp) -> Result<Instruction> {
        let reg = self.parse_register()?;
        self.expect(&Token::Comma)?;
        // Peekahead for ( reg ) to handle zero offset: reg , ( reg )
        if let Some(Token::OpenParen) = self.peek() {
            let pos_backup = self.pos;
            self.next();
            if let Ok(rs) = self.parse_register()
                && self.expect(&Token::CloseParen).is_ok()
            {
                return Ok(Instruction::LoadStore(
                    op,
                    reg,
                    Box::new(Expression::Literal(0)),
                    rs,
                ));
            }
            self.pos = pos_backup;
        }
        let expr = self.parse_expression()?;
        // If followed by ( reg ), it's reg , exp ( reg ); else global store with temp reg
        if let Some(Token::OpenParen) = self.peek() {
            self.next();
            let rs = self.parse_register()?;
            self.expect(&Token::CloseParen)?;
            Ok(Instruction::LoadStore(op, reg, Box::new(expr), rs))
        } else {
            self.expect(&Token::Comma)?;
            let temp = self.parse_register()?;
            Ok(Instruction::Pseudo(PseudoOp::StoreGlobal(
                op,
                reg,
                Box::new(expr),
                temp,
            )))
        }
    }

    // Grammar: atomic_op[.aq|.rel|.aqrl] rd, (rs1) | atomic_op[.aq|.rel|.aqrl] rd, rs2, (rs1)
    // Examples:
    //   lr.w a0, (a1)
    //   lr.w.aq a0, (a1)
    //   sc.w a0, a2, (a1)
    //   amoswap.w.aqrl a0, a2, (a1)
    fn parse_atomic(
        &mut self,
        op: AtomicOp,
        ordering: MemoryOrdering,
    ) -> Result<Instruction> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;

        // Check for LR format: rd, (rs1)
        if matches!(op, AtomicOp::LrW) {
            self.expect(&Token::OpenParen)?;
            let rs1 = self.parse_register()?;
            self.expect(&Token::CloseParen)?;
            return Ok(Instruction::Atomic(
                op,
                rd,
                rs1,
                Register::X0,
                ordering,
            ));
        }

        // SC/AMO format: rd, rs2, (rs1)
        let rs2 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        self.expect(&Token::OpenParen)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::CloseParen)?;

        Ok(Instruction::Atomic(op, rd, rs1, rs2, ordering))
    }

    /// Parse atomic instruction name and extract operation + ordering
    /// Examples: "lr.w" -> (LrW, None), "amoswap.w.aqrl" -> (AmoswapW, AqRl)
    fn parse_atomic_name(name: &str) -> Option<(AtomicOp, MemoryOrdering)> {
        // Split by dots: ["lr", "w", "aq"] or ["amoswap", "w"]
        let parts: Vec<&str> = name.split('.').collect();

        if parts.len() < 2 {
            return None;
        }

        // Parse ordering suffix (.aq, .rel, .aqrl)
        let has_ordering =
            matches!(parts.last(), Some(&"aqrl") | Some(&"aq") | Some(&"rel"));
        let ordering = if has_ordering {
            match parts.last() {
                Some(&"aqrl") => MemoryOrdering::AqRl,
                Some(&"aq") => MemoryOrdering::Aq,
                Some(&"rel") => MemoryOrdering::Rel,
                _ => MemoryOrdering::None, // Should not reach here given has_ordering check
            }
        } else {
            MemoryOrdering::None
        };

        // Determine base instruction (before ordering suffix)
        let base_parts =
            if has_ordering { &parts[..parts.len() - 1] } else { &parts[..] };

        // Match instruction: base + width
        let op = match base_parts {
            ["lr", "w"] => AtomicOp::LrW,
            ["sc", "w"] => AtomicOp::ScW,
            ["amoswap", "w"] => AtomicOp::AmoswapW,
            ["amoadd", "w"] => AtomicOp::AmoaddW,
            ["amoxor", "w"] => AtomicOp::AmoxorW,
            ["amoand", "w"] => AtomicOp::AmoandW,
            ["amoor", "w"] => AtomicOp::AmoorW,
            ["amomin", "w"] => AtomicOp::AmominW,
            ["amomax", "w"] => AtomicOp::AmomaxW,
            ["amominu", "w"] => AtomicOp::AmominuW,
            ["amomaxu", "w"] => AtomicOp::AmomaxuW,
            _ => return None,
        };

        Some((op, ordering))
    }

    /// Parse compressed instruction (called with opcode after "c." prefix stripped)
    /// Examples: "add" in "c.add", "li" in "c.li"
    fn parse_compressed_instruction(
        &mut self,
        op: &str,
    ) -> Result<Instruction> {
        let (c_op, operands) = match op {
            // CR format (rd, rs2)
            "add" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                (CompressedOp::CAdd, CompressedOperands::CR { rd, rs2 })
            }

            "mv" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                (CompressedOp::CMv, CompressedOperands::CR { rd, rs2 })
            }

            // CR format single register
            "jr" => {
                let rs1 = self.parse_register()?;
                (CompressedOp::CJr, CompressedOperands::CRSingle { rs1 })
            }

            "jalr" => {
                let rs1 = self.parse_register()?;
                (CompressedOp::CJalr, CompressedOperands::CRSingle { rs1 })
            }

            // CI format (rd, imm)
            "li" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CLi,
                    CompressedOperands::CI { rd, imm: Box::new(imm) },
                )
            }

            "lui" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CLui,
                    CompressedOperands::CI { rd, imm: Box::new(imm) },
                )
            }

            "addi" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CAddi,
                    CompressedOperands::CI { rd, imm: Box::new(imm) },
                )
            }

            "addi16sp" => {
                let rd = self.parse_register()?;
                if rd != Register::X2 {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.addi16sp destination must be sp (x2), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CAddi16sp,
                    CompressedOperands::CI { rd, imm: Box::new(imm) },
                )
            }

            "addi4spn" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.addi4spn destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let base = self.parse_register()?;
                if base != Register::X2 {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.addi4spn base register must be sp (x2), got {}",
                            base
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CAddi4spn,
                    CompressedOperands::CIW {
                        rd_prime: rd,
                        imm: Box::new(imm),
                    },
                )
            }

            "slli" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CSlli,
                    CompressedOperands::CI { rd, imm: Box::new(imm) },
                )
            }

            // CI format stack-relative load: c.lwsp rd, offset(sp)
            "lwsp" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let offset = self.parse_expression()?;
                self.expect(&Token::OpenParen)?;
                let base = self.parse_register()?;
                if base != Register::X2 {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.lwsp base register must be sp (x2), got {}",
                            base
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::CloseParen)?;
                (
                    CompressedOp::CLwsp,
                    CompressedOperands::CIStackLoad {
                        rd,
                        offset: Box::new(offset),
                    },
                )
            }

            // CSS format stack-relative store: c.swsp rs2, offset(sp)
            "swsp" => {
                let rs2 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let offset = self.parse_expression()?;
                self.expect(&Token::OpenParen)?;
                let base = self.parse_register()?;
                if base != Register::X2 {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.swsp base register must be sp (x2), got {}",
                            base
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::CloseParen)?;
                (
                    CompressedOp::CSwsp,
                    CompressedOperands::CSSStackStore {
                        rs2,
                        offset: Box::new(offset),
                    },
                )
            }

            // CL format: c.lw rd', offset(rs1')
            "lw" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.lw destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let offset = self.parse_expression()?;
                self.expect(&Token::OpenParen)?;
                let rs1 = self.parse_register()?;
                if !rs1.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.lw base register must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs1
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::CloseParen)?;
                (
                    CompressedOp::CLw,
                    CompressedOperands::CL {
                        rd_prime: rd,
                        rs1_prime: rs1,
                        offset: Box::new(offset),
                    },
                )
            }

            // CS format: c.sw rs2', offset(rs1')
            "sw" => {
                let rs2 = self.parse_register()?;
                if !rs2.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.sw source must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs2
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let offset = self.parse_expression()?;
                self.expect(&Token::OpenParen)?;
                let rs1 = self.parse_register()?;
                if !rs1.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.sw base register must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs1
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::CloseParen)?;
                (
                    CompressedOp::CSw,
                    CompressedOperands::CS {
                        rs2_prime: rs2,
                        rs1_prime: rs1,
                        offset: Box::new(offset),
                    },
                )
            }

            // CA format: c.and, c.or, c.xor, c.sub
            "and" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.and destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                if !rs2.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.and source must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs2
                        ),
                        self.location(),
                    ));
                }
                (
                    CompressedOp::CAnd,
                    CompressedOperands::CA { rd_prime: rd, rs2_prime: rs2 },
                )
            }

            "or" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.or destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                if !rs2.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.or source must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs2
                        ),
                        self.location(),
                    ));
                }
                (
                    CompressedOp::COr,
                    CompressedOperands::CA { rd_prime: rd, rs2_prime: rs2 },
                )
            }

            "xor" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.xor destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                if !rs2.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.xor source must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs2
                        ),
                        self.location(),
                    ));
                }
                (
                    CompressedOp::CXor,
                    CompressedOperands::CA { rd_prime: rd, rs2_prime: rs2 },
                )
            }

            "sub" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.sub destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let rs2 = self.parse_register()?;
                if !rs2.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.sub source must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs2
                        ),
                        self.location(),
                    ));
                }
                (
                    CompressedOp::CSub,
                    CompressedOperands::CA { rd_prime: rd, rs2_prime: rs2 },
                )
            }

            // CB format shift/immediate: c.srli, c.srai, c.andi
            "srli" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.srli destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CSrli,
                    CompressedOperands::CBImm {
                        rd_prime: rd,
                        imm: Box::new(imm),
                    },
                )
            }

            "srai" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.srai destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CSrai,
                    CompressedOperands::CBImm {
                        rd_prime: rd,
                        imm: Box::new(imm),
                    },
                )
            }

            "andi" => {
                let rd = self.parse_register()?;
                if !rd.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.andi destination must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rd
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let imm = self.parse_expression()?;
                (
                    CompressedOp::CAndi,
                    CompressedOperands::CBImm {
                        rd_prime: rd,
                        imm: Box::new(imm),
                    },
                )
            }

            // CB format branch: c.beqz, c.bnez
            "beqz" => {
                let rs1 = self.parse_register()?;
                if !rs1.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.beqz operand must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs1
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let offset = self.parse_expression()?;
                (
                    CompressedOp::CBeqz,
                    CompressedOperands::CBBranch {
                        rs1_prime: rs1,
                        offset: Box::new(offset),
                    },
                )
            }

            "bnez" => {
                let rs1 = self.parse_register()?;
                if !rs1.is_compressed_register() {
                    return Err(RiscletError::from_context(
                        format!(
                            "c.bnez operand must be in compressed set (x8-x15/s0-s1 and a0-a5), got {}",
                            rs1
                        ),
                        self.location(),
                    ));
                }
                self.expect(&Token::Comma)?;
                let offset = self.parse_expression()?;
                (
                    CompressedOp::CBnez,
                    CompressedOperands::CBBranch {
                        rs1_prime: rs1,
                        offset: Box::new(offset),
                    },
                )
            }

            // CJ format: c.j, c.jal
            "j" => {
                let offset = self.parse_expression()?;
                (
                    CompressedOp::CJComp,
                    CompressedOperands::CJOpnd { offset: Box::new(offset) },
                )
            }

            "jal" => {
                let offset = self.parse_expression()?;
                (
                    CompressedOp::CJalComp,
                    CompressedOperands::CJOpnd { offset: Box::new(offset) },
                )
            }

            // Special
            "nop" => (CompressedOp::CNop, CompressedOperands::None),
            "ebreak" => (CompressedOp::CEbreak, CompressedOperands::None),

            _ => {
                return Err(RiscletError::from_context(
                    format!(
                        "Unknown compressed instruction 'c.{}': check spelling or consult the RISC-V ISA reference",
                        op
                    ),
                    self.location(),
                ));
            }
        };

        Ok(Instruction::Compressed(c_op, operands))
    }
}

pub fn parse(tokens: &[Token], file: String, line: usize) -> Result<Vec<Line>> {
    let mut parser = Parser::new(tokens, file.clone(), line);
    let lines = parser.parse_line()?;

    // Check for leftover tokens
    if parser.pos < parser.tokens.len() {
        let remaining: Vec<String> = parser.tokens[parser.pos..]
            .iter()
            .map(|t| format!("{:?}", t))
            .collect();
        return Err(RiscletError::from_context(
            format!(
                "Extra tokens after instruction: '{}' (each line should have at most one instruction or directive)",
                remaining.join(" ")
            ),
            Location { file: file.clone(), line },
        ));
    }

    Ok(lines)
}
