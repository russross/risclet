use crate::ast::*;

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    file: String,
    line: u32,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], file: String, line: u32) -> Self {
        Parser { tokens, pos: 0, file, line }
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

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        if let Some(t) = self.peek() {
            if t == expected {
                self.next();
                Ok(())
            } else {
                Err(format!("Expected {:?}, found {:?}", expected, t))
            }
        } else {
            Err(format!("Expected {:?}, found EOF", expected))
        }
    }

    // Grammar: ident
    // Example: foo
    fn parse_identifier(&mut self) -> Result<String, String> {
        if let Some(Token::Identifier(s)) = self.next() {
            Ok(s)
        } else {
            Err("Expected identifier".to_string())
        }
    }

    // Grammar: reg
    // Example: a0
    fn parse_register(&mut self) -> Result<Register, String> {
        if let Some(Token::Register(r)) = self.next() {
            Ok(r)
        } else {
            Err("Expected register".to_string())
        }
    }

    // Grammar: exp (calls parse_bitwise_or) (part of expression grammar)
    // Example: a + b * c
    fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_bitwise_or()
    }

    // Grammar: bitwise_or ::= bitwise_xor ( | bitwise_xor )* (part of expression grammar)
    // Example: a | b | c
    fn parse_bitwise_or(&mut self) -> Result<Expression, String> {
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
    fn parse_bitwise_xor(&mut self) -> Result<Expression, String> {
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
    fn parse_bitwise_and(&mut self) -> Result<Expression, String> {
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
    fn parse_shift(&mut self) -> Result<Expression, String> {
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
    fn parse_additive(&mut self) -> Result<Expression, String> {
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
    fn parse_multiplicative(&mut self) -> Result<Expression, String> {
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
    fn parse_unary(&mut self) -> Result<Expression, String> {
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
    fn parse_operand(&mut self) -> Result<Expression, String> {
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
                _ => Err("Expected operand".to_string()),
            }
        } else {
            Err("Expected operand".to_string())
        }
    }

    // Grammar: [ident | int :] [directive | instruction]
    // Examples: loop: add a0, a1, a2 (labeled instruction), .global foo (directive), add a0, a1, a2 (unlabeled instruction)
    fn parse_line(&mut self) -> Result<Vec<Line>, String> {
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
                segment: Segment::Text, // Default, will be overridden
                offset: 0,              // Default, will be overridden
                size: 0,                // Default, will be set
                outgoing_refs: Vec::new(),
            });
        }
        if let Some(c) = content {
            lines.push(Line {
                location,
                content: c,
                segment: Segment::Text, // Default, will be overridden
                offset: 0,              // Default, will be overridden
                size: 0,                // Default, will be set
                outgoing_refs: Vec::new(),
            });
        }
        if lines.is_empty() {
            return Err("Empty line".to_string());
        }
        Ok(lines)
    }

    // Grammar: .global ident | .equ ident , exp | .text | .data | .bss | .space exp | .balign exp | .string string [, string]* | .asciz string [, string]* | .byte exp [, exp]* | .2byte exp [, exp]* | .4byte exp [, exp]* | .8byte exp [, exp]*
    // Examples: .global main, .equ SIZE, 100, .text, .data, .bss, .space 4, .balign 8, .string "hello", "world", .asciz "foo", .byte 1, 2, 3, .2byte 10, 20, .4byte 100, .8byte 1000
    fn parse_directive(&mut self) -> Result<Directive, String> {
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
                DirectiveOp::EightByte => {
                    let mut exprs = Vec::new();
                    exprs.push(self.parse_expression()?);
                    while let Some(Token::Comma) = self.peek() {
                        self.next();
                        exprs.push(self.parse_expression()?);
                    }
                    Ok(Directive::EightByte(exprs))
                }
            }
        } else {
            Err("Expected directive".to_string())
        }
    }

    // Grammar: opcode-specific (see below for each type)
    fn parse_instruction(&mut self) -> Result<Instruction, String> {
        let opcode = self.parse_identifier()?;
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
            "addw" => self.parse_rtype(RTypeOp::Addw),
            "subw" => self.parse_rtype(RTypeOp::Subw),
            "sllw" => self.parse_rtype(RTypeOp::Sllw),
            "srlw" => self.parse_rtype(RTypeOp::Srlw),
            "sraw" => self.parse_rtype(RTypeOp::Sraw),
            "mulw" => self.parse_rtype(RTypeOp::Mulw),
            "divw" => self.parse_rtype(RTypeOp::Divw),
            "divuw" => self.parse_rtype(RTypeOp::Divuw),
            "remw" => self.parse_rtype(RTypeOp::Remw),
            "remuw" => self.parse_rtype(RTypeOp::Remuw),
            // I-type
            "addi" => self.parse_itype(ITypeOp::Addi),
            "addiw" => self.parse_itype(ITypeOp::Addiw),
            "slli" => self.parse_itype(ITypeOp::Slli),
            "slti" => self.parse_itype(ITypeOp::Slti),
            "sltiu" => self.parse_itype(ITypeOp::Sltiu),
            "xori" => self.parse_itype(ITypeOp::Xori),
            "ori" => self.parse_itype(ITypeOp::Ori),
            "andi" => self.parse_itype(ITypeOp::Andi),
            "srli" => self.parse_itype(ITypeOp::Srli),
            "srai" => self.parse_itype(ITypeOp::Srai),
            "jalr" => self.parse_itype(ITypeOp::Jalr),
            "slliw" => self.parse_itype(ITypeOp::Slliw),
            "srliw" => self.parse_itype(ITypeOp::Srliw),
            "sraiw" => self.parse_itype(ITypeOp::Sraiw),
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
            "ecall" => Ok(Instruction::Special(SpecialOp::Ecall)),
            "ebreak" => Ok(Instruction::Special(SpecialOp::Ebreak)),
            // Load/store
            "lb" => self.parse_load(LoadStoreOp::Lb),
            "lh" => self.parse_load(LoadStoreOp::Lh),
            "lw" => self.parse_load(LoadStoreOp::Lw),
            "ld" => self.parse_load(LoadStoreOp::Ld),
            "lbu" => self.parse_load(LoadStoreOp::Lbu),
            "lhu" => self.parse_load(LoadStoreOp::Lhu),
            "lwu" => self.parse_load(LoadStoreOp::Lwu),
            "sb" => self.parse_store(LoadStoreOp::Sb),
            "sh" => self.parse_store(LoadStoreOp::Sh),
            "sw" => self.parse_store(LoadStoreOp::Sw),
            "sd" => self.parse_store(LoadStoreOp::Sd),
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
            "negw" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::RType(RTypeOp::Subw, rd, Register::X0, rs))
            }
            "sext.w" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(
                    ITypeOp::Addiw,
                    rd,
                    rs,
                    Box::new(Expression::Literal(0)),
                ))
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
            _ => Err(format!("Unknown instruction {}", opcode)),
        }
    }

    // Grammar: reg , reg , reg
    // Example: add a0, a1, a2
    fn parse_rtype(&mut self, op: RTypeOp) -> Result<Instruction, String> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs2 = self.parse_register()?;
        Ok(Instruction::RType(op, rd, rs1, rs2))
    }

    // Grammar: reg , reg , exp
    // Examples: addi a0, a1, 1, addi a0, a1, 'z' - 'a'
    fn parse_itype(&mut self, op: ITypeOp) -> Result<Instruction, String> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let imm = self.parse_expression()?;
        Ok(Instruction::IType(op, rd, rs1, Box::new(imm)))
    }

    // Grammar: reg , reg , expression
    // Examples: beq a0, a1, loop (symbolic), beq a0, a1, . + 8 (expression)
    fn parse_btype(&mut self, op: BTypeOp) -> Result<Instruction, String> {
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs2 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let expr = self.parse_expression()?;
        Ok(Instruction::BType(op, rs1, rs2, Box::new(expr)))
    }

    // Grammar: reg , [exp] ( reg ) | reg , exp (global load, exp not followed by '(')
    // Examples: lb a0, 0(sp) (immediate offset), lb a0, (sp) (zero offset via peekahead), lb a0, label (global load, no parens; parentheses in exp like (label + 4) are part of the expression)
    fn parse_load(&mut self, op: LoadStoreOp) -> Result<Instruction, String> {
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
    fn parse_store(&mut self, op: LoadStoreOp) -> Result<Instruction, String> {
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
}

pub fn parse(
    tokens: &[Token],
    file: String,
    line: u32,
) -> Result<Vec<Line>, String> {
    let mut parser = Parser::new(tokens, file, line);
    let lines = parser.parse_line()?;

    // Check for leftover tokens
    if parser.pos < parser.tokens.len() {
        let remaining: Vec<String> = parser.tokens[parser.pos..]
            .iter()
            .map(|t| format!("{:?}", t))
            .collect();
        return Err(format!("Unexpected tokens after parsing: {}", remaining.join(" ")));
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::tokenizer;

    #[test]
    fn test_parse_simple_instruction() {
        let line = "add a0, a1, a2";
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
        let ast = parse(&tokens, "test".to_string(), 1).unwrap();
        assert_eq!(ast.len(), 1);
        if let LineContent::Directive(Directive::Global(vec)) = &ast[0].content
        {
            assert_eq!(vec, &["main".to_string()]);
        } else {
            panic!("Unexpected AST");
        }
    }

    #[test]
    fn test_parse_expression() {
        let line = "li a0, 1 + 2";
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let line = "li a0, 0x123456789ABCDEF";
        let tokens = tokenizer::tokenize(line).unwrap();
        let ast = parse(&tokens, "test".to_string(), 1).unwrap();
        assert_eq!(ast.len(), 1);
        if let LineContent::Instruction(Instruction::Pseudo(PseudoOp::Li(
            Register::X10,
            expr,
        ))) = &ast[0].content
        {
            if let Expression::Literal(0x123456789ABCDEF) = **expr {
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
        let tokens = tokenizer::tokenize(line).unwrap();
        let ast = parse(&tokens, "test".to_string(), 1).unwrap();
        assert_eq!(ast.len(), 1);
        if let LineContent::Directive(Directive::Space(expr)) = &ast[0].content
        {
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
        let tokens = tokenizer::tokenize(line).unwrap();
        let ast = parse(&tokens, "test".to_string(), 1).unwrap();
        assert_eq!(ast.len(), 1);
        if let LineContent::Directive(Directive::Balign(expr)) = &ast[0].content
        {
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
        let tokens = tokenizer::tokenize(line).unwrap();
        let ast = parse(&tokens, "test".to_string(), 1).unwrap();
        assert_eq!(ast.len(), 1);
        if let LineContent::Directive(Directive::String(vec)) = &ast[0].content
        {
            assert_eq!(*vec, vec!["hello".to_string(), "world".to_string()]);
        } else {
            panic!("Unexpected AST");
        }
    }

    #[test]
    fn test_parse_directive_byte() {
        let line = ".byte 1, 2, 3";
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
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
        let tokens = tokenizer::tokenize(line).unwrap();
        let ast = parse(&tokens, "test".to_string(), 1).unwrap();
        assert_eq!(ast.len(), 1);
        if let LineContent::Instruction(Instruction::Pseudo(PseudoOp::Li(
            Register::X10,
            expr,
        ))) = &ast[0].content
        {
            if let Expression::LeftShiftOp { lhs, rhs } = &**expr {
                if let Expression::Parenthesized(plus_expr) = &**lhs {
                    if let Expression::PlusOp { lhs: l, rhs: r } = &**plus_expr
                    {
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
        let tokens = tokenizer::tokenize(line).unwrap();
        let result = parse(&tokens, "test".to_string(), 1);
        assert!(result.is_err(), "Should fail with leftover tokens");
        let err = result.unwrap_err();
        assert!(err.contains("Unexpected tokens"), "Error should mention unexpected tokens: {}", err);
    }
}
