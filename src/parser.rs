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

    fn parse_integer(&mut self) -> Result<i64, String> {
        if let Some(Token::Integer(i)) = self.next() {
            Ok(i)
        } else {
            Err("Expected integer".to_string())
        }
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        if let Some(Token::Identifier(s)) = self.next() {
            Ok(s)
        } else {
            Err("Expected identifier".to_string())
        }
    }

    fn parse_register(&mut self) -> Result<Register, String> {
        if let Some(Token::Register(r)) = self.next() {
            Ok(r)
        } else {
            Err("Expected register".to_string())
        }
    }



    fn parse_label_ref(&mut self) -> Result<LabelRef, String> {
        if let Some(Token::Integer(num)) = self.peek() {
            let num = *num as u32; // assume positive
            self.next();
            if let Some(Token::Identifier(s)) = self.peek() {
                if s == "f" {
                    self.next();
                    Ok(LabelRef::Numeric(NumericLabelRef { num, is_forward: true }))
                } else if s == "b" {
                    self.next();
                    Ok(LabelRef::Numeric(NumericLabelRef { num, is_forward: false }))
                } else {
                    Err("Numeric label must be followed by 'f' or 'b'".to_string())
                }
            } else {
                Err("Numeric label must be followed by 'f' or 'b'".to_string())
            }
        } else {
            let ident = self.parse_identifier()?;
            Ok(LabelRef::Symbolic(ident))
        }
    }

    fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_bitwise_or()
    }

    fn parse_bitwise_or(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_bitwise_xor()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::BitwiseOr) => {
                    self.next();
                    let right = self.parse_bitwise_xor()?;
                    left = Expression::BitwiseOrOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_bitwise_and()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::BitwiseXor) => {
                    self.next();
                    let right = self.parse_bitwise_and()?;
                    left = Expression::BitwiseXorOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_shift()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::BitwiseAnd) => {
                    self.next();
                    let right = self.parse_shift()?;
                    left = Expression::BitwiseAndOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_additive()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::LeftShift) => {
                    self.next();
                    let right = self.parse_additive()?;
                    left = Expression::LeftShiftOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                Token::Operator(OperatorOp::RightShift) => {
                    self.next();
                    let right = self.parse_additive()?;
                    left = Expression::RightShiftOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_multiplicative()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::Plus) => {
                    self.next();
                    let right = self.parse_multiplicative()?;
                    left = Expression::PlusOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                Token::Operator(OperatorOp::Minus) => {
                    self.next();
                    let right = self.parse_multiplicative()?;
                    left = Expression::MinusOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_unary()?;
        while let Some(op) = self.peek() {
            match op {
                Token::Operator(OperatorOp::Multiply) => {
                    self.next();
                    let right = self.parse_unary()?;
                    left = Expression::MultiplyOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                Token::Operator(OperatorOp::Divide) => {
                    self.next();
                    let right = self.parse_unary()?;
                    left = Expression::DivideOp { lhs: Box::new(left), rhs: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expression, String> {
        if let Some(Token::Operator(OperatorOp::Minus)) = self.peek() {
            self.next();
            let expr = self.parse_unary()?;
            Ok(Expression::NegateOp { expr: Box::new(expr) })
        } else if let Some(Token::Operator(OperatorOp::Tilde)) = self.peek() {
            self.next();
            let expr = self.parse_unary()?;
            Ok(Expression::BitwiseNotOp { expr: Box::new(expr) })
        } else {
            self.parse_operand()
        }
    }

    fn parse_operand(&mut self) -> Result<Expression, String> {
        if let Some(t) = self.peek().cloned() {
            match t {
                Token::Integer(i) => {
                    self.next();
                    Ok(Expression::Literal(i))
                }
                Token::CharacterLiteral(c) => {
                    self.next();
                    Ok(Expression::Literal(c as i64))
                }
                Token::Identifier(s) => {
                    self.next();
                    Ok(Expression::Identifier(s))
                }
                Token::OpenParen => {
                    self.next();
                    let expr = self.parse_expression()?;
                    self.expect(&Token::CloseParen)?;
                    Ok(Expression::Parenthesized(Box::new(expr)))
                }
                _ => Err("Expected operand".to_string()),
            }
        } else {
            Err("Expected operand".to_string())
        }
    }

    fn parse_line(&mut self) -> Result<Vec<Line>, String> {
        let location = Location { file: self.file.clone(), line: self.line };
        let mut lines = Vec::new();
        // Check for label
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
            lines.push(Line { location: location.clone(), content: LineContent::Label(l) });
        }
        if let Some(c) = content {
            lines.push(Line { location, content: c });
        }
        if lines.is_empty() {
            return Err("Empty line".to_string());
        }
        Ok(lines)
    }

    fn parse_directive(&mut self) -> Result<Directive, String> {
        if let Some(Token::Directive(d)) = self.next() {
            match d {
                DirectiveOp::Global => {
                    let name = self.parse_identifier()?;
                    Ok(Directive::Global(name))
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
                let label = self.parse_label_ref()?;
                Ok(Instruction::BType(BTypeOp::Bge, rs1, Register::X0, label))
            }
            "bnez" => {
                let rs1 = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let label = self.parse_label_ref()?;
                Ok(Instruction::BType(BTypeOp::Bne, rs1, Register::X0, label))
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
                let rd = if let Some(Token::Register(_)) = self.peek() {
                    self.parse_register()?
                } else {
                    Register::X1 // ra
                };
                if let Some(Token::Comma) = self.peek() {
                    self.next();
                }
                let label = self.parse_label_ref()?;
                Ok(Instruction::JType(JTypeOp::Jal, rd, label))
            }
            // Special
            "ecall" => Ok(Instruction::Special(SpecialOp::Ecall)),
            "ebreak" => Ok(Instruction::Special(SpecialOp::Ebreak)),
            // Load/store
            "lb" => self.parse_load_store(LoadStoreOp::Lb),
            "lh" => self.parse_load_store(LoadStoreOp::Lh),
            "lw" => self.parse_load_store(LoadStoreOp::Lw),
            "ld" => self.parse_load_store(LoadStoreOp::Ld),
            "lbu" => self.parse_load_store(LoadStoreOp::Lbu),
            "lhu" => self.parse_load_store(LoadStoreOp::Lhu),
            "lwu" => self.parse_load_store(LoadStoreOp::Lwu),
            "sb" => self.parse_load_store(LoadStoreOp::Sb),
            "sh" => self.parse_load_store(LoadStoreOp::Sh),
            "sw" => self.parse_load_store(LoadStoreOp::Sw),
            "sd" => self.parse_load_store(LoadStoreOp::Sd),
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
                let sym = self.parse_identifier()?;
                Ok(Instruction::Pseudo(PseudoOp::La(rd, sym)))
            }
            "call" => {
                let label = self.parse_label_ref()?;
                Ok(Instruction::Pseudo(PseudoOp::Call(label)))
            }
            "tail" => {
                let label = self.parse_label_ref()?;
                Ok(Instruction::Pseudo(PseudoOp::Tail(label)))
            }
            "mv" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(ITypeOp::Addi, rd, rs, Box::new(Expression::Literal(0))))
            }
            "ret" => Ok(Instruction::IType(ITypeOp::Jalr, Register::X0, Register::X1, Box::new(Expression::Literal(0)))),
            "nop" => Ok(Instruction::IType(ITypeOp::Addi, Register::X0, Register::X0, Box::new(Expression::Literal(0)))),
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
                Ok(Instruction::IType(ITypeOp::Addiw, rd, rs, Box::new(Expression::Literal(0))))
            }
            "seqz" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(ITypeOp::Sltiu, rd, rs, Box::new(Expression::Literal(1))))
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
                 let label = self.parse_label_ref()?;
                 Ok(Instruction::BType(BTypeOp::Beq, rs, Register::X0, label))
             }
             "blez" => {
                 let rs = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let label = self.parse_label_ref()?;
                 Ok(Instruction::BType(BTypeOp::Bge, Register::X0, rs, label))
             }
             "bltz" => {
                 let rs = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let label = self.parse_label_ref()?;
                 Ok(Instruction::BType(BTypeOp::Blt, rs, Register::X0, label))
             }
             "bgtz" => {
                 let rs = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let label = self.parse_label_ref()?;
                 Ok(Instruction::BType(BTypeOp::Blt, Register::X0, rs, label))
             }
             "bgt" => {
                 let rs1 = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let rs2 = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let label = self.parse_label_ref()?;
                 Ok(Instruction::BType(BTypeOp::Blt, rs2, rs1, label))
             }
             "ble" => {
                 let rs1 = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let rs2 = self.parse_register()?;
                 self.expect(&Token::Comma)?;
                 let label = self.parse_label_ref()?;
                 Ok(Instruction::BType(BTypeOp::Bge, rs2, rs1, label))
             }
            "j" => {
                let label = self.parse_label_ref()?;
                Ok(Instruction::JType(JTypeOp::Jal, Register::X0, label))
            }
            "jr" => {
                let rs = self.parse_register()?;
                Ok(Instruction::IType(ITypeOp::Jalr, Register::X0, rs, Box::new(Expression::Literal(0))))
            }
            "not" => {
                let rd = self.parse_register()?;
                self.expect(&Token::Comma)?;
                let rs = self.parse_register()?;
                Ok(Instruction::IType(ITypeOp::Xori, rd, rs, Box::new(Expression::Literal(-1))))
            }
            _ => Err(format!("Unknown instruction {}", opcode)),
        }
    }

    fn parse_rtype(&mut self, op: RTypeOp) -> Result<Instruction, String> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs2 = self.parse_register()?;
        Ok(Instruction::RType(op, rd, rs1, rs2))
    }

    fn parse_itype(&mut self, op: ITypeOp) -> Result<Instruction, String> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let imm = self.parse_expression()?;
        Ok(Instruction::IType(op, rd, rs1, Box::new(imm)))
    }

    fn parse_btype(&mut self, op: BTypeOp) -> Result<Instruction, String> {
        let rs1 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let rs2 = self.parse_register()?;
        self.expect(&Token::Comma)?;
        let label = self.parse_label_ref()?;
        Ok(Instruction::BType(op, rs1, rs2, label))
    }

    fn parse_load_store(&mut self, op: LoadStoreOp) -> Result<Instruction, String> {
        let rd = self.parse_register()?;
        self.expect(&Token::Comma)?;
        // Check if integer
        let offset = if let Some(Token::Integer(_)) = self.peek() {
            self.parse_integer()?
        } else {
            0
        };
        self.expect(&Token::OpenParen)?;
        let rs = self.parse_register()?;
        self.expect(&Token::CloseParen)?;
        Ok(Instruction::LoadStore(op, rd, offset, rs))
    }
}

pub fn parse(tokens: &[Token], file: String, line: u32) -> Result<Vec<Line>, String> {
    let mut parser = Parser::new(tokens, file, line);
    parser.parse_line()
}
