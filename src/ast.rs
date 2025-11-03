// ast.rs
//
// This file defines the data structures for the tokenizer and the Abstract Syntax Tree (AST)
// for the RISC-V 32 assembler. Each structure and enum represents a specific component
// of the assembly language syntax and is designed to be directly filled by the parser.

use crate::error::{AssemblerError, Result};
use std::fmt;

// ==============================================================================
// Top-Level Parsing and Error Handling
// ==============================================================================
//
// The overall parsing process will work as follows:
// 1. Read the entire source file.
// 2. Split the file content into individual lines.
// 3. For each non-empty line, tokenize it into a `Vec<Token>`.
// 4. The parser then consumes this `Vec<Token>` to generate one or more `Line` AST nodes.
//    A single source line containing both a label and an instruction/directive will
//    produce two `Line` nodes: one for the label and one for the instruction/directive.
//    Some pseudo-instructions are desugared directly into multiple instructions as well.
// 5. If any error is detected during tokenization or parsing, the process will abort
//    immediately and return the error along with its `Location`.
// 6. The final output is a `Vec<Line>`, representing the complete program AST.
//

/// A single location in the source file, used for error reporting and AST annotation.
/// The parser will attach this to every `Line` to provide context for errors.
///
/// **Grammar Rule:** N/A (this is a data structure for parser context)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    pub file: String,
    pub line: usize,
}

/// An enum representing the three segments in the assembler output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Segment {
    Text,
    Data,
    Bss,
}

/// A structure representing a single source file's parsed content, including lines and segment sizes.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub file: String,
    pub lines: Vec<Line>,
}

/// The top-level structure containing all source files.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub files: Vec<SourceFile>,
}

impl Source {
    /// Get a line from the source by pointer
    pub fn get_line(&self, pointer: LinePointer) -> Result<&Line> {
        self.files
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

/// A struct representing a pointer to a specific line in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinePointer {
    pub file_index: usize,
    pub line_index: usize,
}

// ==============================================================================
// Tokenization Data Types
// ==============================================================================
// These are the raw components that the tokenizer will produce from the input
// assembly code.

/// An enum representing the 32 general-purpose registers.
/// The tokenizer will be responsible for mapping register names (e.g., "sp", "x2")
/// to a concrete `Register` variant. This approach moves the register-name-to-number
/// lookup logic out of the parser and into the tokenizer, simplifying the parser's job.
///
/// **Grammar Rule:** N/A (Tokenizer maps raw identifiers to this concrete type)
#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Register {
    X0,  X1,  X2,  X3,  X4,  X5,  X6,  X7,  X8,  X9,  X10, X11, X12, X13, X14, X15,
    X16, X17, X18, X19, X20, X21, X22, X23, X24, X25, X26, X27, X28, X29, X30, X31,
}

impl Register {
    /// Check if register is in the compressed register set (x8-x15: s0, s1, a0-a5)
    pub fn is_compressed_register(self) -> bool {
        matches!(
            self,
            Register::X8
                | Register::X9
                | Register::X10
                | Register::X11
                | Register::X12
                | Register::X13
                | Register::X14
                | Register::X15
        )
    }
}

/// An enum for all supported assembler directives.
///
/// **Grammar Rule:** N/A (Tokenizer maps raw directive names to this concrete type)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum DirectiveOp {
    Global,
    Equ,
    Text,
    Data,
    Bss,
    Space,
    String,
    Asciz,
    Byte,
    TwoByte,
    FourByte,
    Balign,
}

/// An enum for all supported operators.
///
///**Grammar Rule:** N/A (Tokenizer maps raw symbols to this concrete type)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum OperatorOp {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    LeftShift,
    RightShift,
    BitwiseOr,
    BitwiseAnd,
    BitwiseXor,
    BitwiseNot,
}

/// Represents a single token produced by the tokenizer.
/// Comments are removed by the tokenizer.
/// Each input line is tokenized and parsed independently, so no EOL or EOF
/// markers are necessary. The tokenizer also handles all operator tokens directly.
///
///**Grammar Rule:** N/A (Tokenizer output)
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A general identifier (e.g., a symbol, label, or instruction name).
    Identifier(String),
    /// A reserved register name (e.g., "a0", "x10").
    Register(Register),
    /// An integer literal, including binary, octal, decimal, and hexadecimal. Single-quoted
    /// character literals (including \n and other scale sequences) are also tokenized as integer
    /// literals.
    Integer(i32),
    /// A string literal. We accept valid UTF-8 strings with \n and other standard escape sequences.
    StringLiteral(String),
    /// A directive (e.g., ".text", ".global"). The tokenizer accepts anything
    /// with a dot (.) followed by alphanumeric ASCII characters
    Directive(DirectiveOp),
    /// A simple token for syntax.
    Colon,
    Comma,
    OpenParen,
    CloseParen,
    /// An operator.
    Operator(OperatorOp),
    /// The current address (.) in expressions.
    Dot,
}

// ==============================================================================
// Abstract Syntax Tree (AST) Data Types
// ==============================================================================
// These structures represent the parsed, hierarchical view of the assembly code,
// simplifying later stages like semantic analysis and code generation.

/// The top-level structure of a line in the assembly file
/// Every non-blank line of source code will be parsed into an instance of `Line`.
///
/// **Grammar Rule:**
/// `line: label_declaration? line_content`
/// `label_declaration: label_identifier Colon`
///
/// **Parsing Notes:**
/// The parser will check for a label at the start of a line. If present, it will consume the
/// label and its colon, then parse the remaining line content. If an instruction/directive
/// is also present, the parser must create and emit a second `Line` object for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The location of this line in the source file.
    pub location: Location,
    /// The content of the line, which can be an instruction or a directive.
    pub content: LineContent,
}

/// The content of a line, which is either a label, an instruction, or a directive.
/// This enum is the core of the line-based parsing strategy. When a label and
/// an instruction are on the same line, the parser should emit two `Line`
/// objects: one for the `Label` and one for the `Instruction`.
///
/// **Grammar Rule:**
/// `line_content: instruction | directive`
#[derive(Debug, Clone, PartialEq)]
pub enum LineContent {
    /// **Grammar Rule:**
    /// `label_declaration: label_identifier Colon`
    /// A label can be an identifier or a positive integer.
    Label(String),
    Instruction(Instruction),
    Directive(Directive),
}

/// A reference to a numeric local label (e.g., `1f` or `2b`).
#[derive(Debug, Clone, PartialEq)]
pub struct NumericLabelRef {
    pub num: u32,
    pub is_forward: bool,
}

/// Compressed instruction operations (C extension - RV32C)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CompressedOp {
    // CR format: c.add, c.mv, c.jr, c.jalr
    CAdd,
    CMv,
    CJr,
    CJalr,

    // CI format: c.li, c.lui, c.addi, c.slli
    CLi,
    CLui,
    CAddi,
    CAddi16sp,
    CAddi4spn,
    CSlli,

    // CI format (stack-relative): c.lwsp, c.swsp
    CLwsp,
    CSwsp,

    // CL format: c.lw
    CLw,

    // CS format: c.sw
    CSw,

    // CA format: c.and, c.or, c.xor, c.sub
    CAnd,
    COr,
    CXor,
    CSub,

    // CB format: c.srli, c.srai, c.andi, c.beqz, c.bnez
    CSrli,
    CSrai,
    CAndi,
    CBeqz,
    CBnez,

    // CJ format: c.j, c.jal
    CJComp,
    CJalComp,

    // Special
    CNop,
    CEbreak,
}

/// Operands for compressed instructions
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum CompressedOperands {
    /// CR format with two registers: c.add rd, rs2
    CR { rd: Register, rs2: Register },
    /// CR format with single register: c.jr rs1
    CRSingle { rs1: Register },
    /// CI format with register and immediate: c.addi rd, imm
    CI { rd: Register, imm: Box<Expression> },
    /// CI format stack-relative load: c.lwsp rd, offset(sp)
    CIStackLoad { rd: Register, offset: Box<Expression> },
    /// CSS format stack-relative store: c.swsp rs2, offset(sp)
    CSSStackStore { rs2: Register, offset: Box<Expression> },
    /// CIW format: c.addi4spn rd', imm (CIW = Compressed Immediate Wide, standard RISC-V term)
    #[allow(clippy::upper_case_acronyms)]
    CIW { rd_prime: Register, imm: Box<Expression> },
    /// CL format: c.lw rd', offset(rs1')
    CL { rd_prime: Register, rs1_prime: Register, offset: Box<Expression> },
    /// CS format: c.sw rs2', offset(rs1')
    CS { rs2_prime: Register, rs1_prime: Register, offset: Box<Expression> },
    /// CA format: c.and rd', rs2'
    CA { rd_prime: Register, rs2_prime: Register },
    /// CB format with immediate: c.srli rd', shamt
    CBImm { rd_prime: Register, imm: Box<Expression> },
    /// CB format with branch: c.beqz rs1', offset
    CBBranch { rs1_prime: Register, offset: Box<Expression> },
    /// CJ format: c.j offset
    CJOpnd { offset: Box<Expression> },
    /// No operands: c.nop, c.ebreak
    None,
}

/// A node in the Abstract Syntax Tree representing a single assembly instruction.
/// Each variant corresponds to a specific instruction format, making it easy for
/// later stages (e.g., code generation) to pattern match on the instruction type.
///
/// **Grammar Rule:**
/// `instruction:`
/// `| RTypeOp Register Comma Register Comma Register`
/// `| ITypeOp Register Comma Register Comma expression`
/// `| BTypeOp Register Comma Register Comma expression`
/// `| UTypeOp Register Comma expression`
/// `| JTypeOp [ Register Comma ] expression`
/// `| SpecialOp`
/// `| LoadStoreOp Register Comma [ expression ] OpenParen Register CloseParen`
/// `| AtomicOp [ MemoryOrdering ] Register Comma [ Register Comma ] OpenParen Register CloseParen`
/// `| PseudoOp`
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// R-type instructions (opcode, rd, rs1, rs2).
    RType(RTypeOp, Register, Register, Register),
    /// I-type instructions (opcode, rd, rs1, immediate).
    IType(ITypeOp, Register, Register, Box<Expression>),
    /// B-type instructions (opcode, rs1, rs2, target expression).
    BType(BTypeOp, Register, Register, Box<Expression>),
    /// U-type instructions (opcode, rd, immediate).
    UType(UTypeOp, Register, Box<Expression>),
    /// J-type instructions (opcode, rd, target expression).
    JType(JTypeOp, Register, Box<Expression>),
    /// Special instructions (opcode, no operands).
    Special(SpecialOp),
    /// The special case for all load and store instructions. The offset is an expression.
    /// If omitted in source, it is filled in as zero by the parser.
    LoadStore(LoadStoreOp, Register, Box<Expression>, Register),
    /// Atomic instructions (A extension): (op, rd, rs1, rs2, ordering)
    /// - For LR instructions: rs2 must be x0 (unused)
    /// - For SC/AMO instructions: all registers are used
    /// - Syntax: lr.w[.aq|.rel|.aqrl] rd, (rs1)
    ///   sc.w[.aq|.rel|.aqrl] rd, rs2, (rs1)
    ///   amo*.w[.aq|.rel|.aqrl] rd, rs2, (rs1)
    Atomic(AtomicOp, Register, Register, Register, MemoryOrdering),
    /// Compressed instructions (C extension) - 16-bit encoding
    Compressed(CompressedOp, CompressedOperands),
    /// A pseudo-instruction that will be desugared by a later pass.
    Pseudo(PseudoOp),
}

/// An enum for the instruction opcodes in the I, M, and base instruction sets.
///
/// **Grammar Rule and Example:**
/// `RTypeOp Register Comma Register Comma Register`
///
/// - `add`: `add a0, a1, a2`
///
/// **Pseudo-ops and Desugaring:**
/// - `neg rd, rs` desugars to `sub rd, x0, rs`.
///
/// **Variants:**
/// - `M` extension variants like `mul`, `div`, `rem`: these are part of the optional M extension.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum RTypeOp {
    Add,
    Sub,
    Sll,
    Slt,
    Sltu,
    Xor,
    Srl,
    Sra,
    Or,
    And,
    Mul,
    Mulh,
    Mulhsu,
    Mulhu,
    Div,
    Divu,
    Rem,
    Remu,
}

/// The ITypeOp enum for instructions that take an immediate value.
///
/// **Grammar Rule and Example:**
/// `ITypeOp Register Comma Register Comma expression`
///
/// - `addi`: `addi a0, a1, 0`
///
/// **Pseudo-ops and Desugaring:**
/// - `mv rs, rt` desugars to `addi rs, rt, 0`.
/// - `nop` desugars to `addi x0, x0, 0`.
/// - `not rs, rt` desugars to `xori rs, rt, -1`.
/// - `jr rs` desugars to `jalr x0, rs, 0`.
/// - `ret` desugars to `jalr x0, ra, 0`.
/// - `zext.b rd, rs` desugars to `andi rd, rs, 0xff`.
/// - `zext.h rd, rs` desugars to `andi rd, rs, 0xffff`.
/// - `seqz rd, rs` desugars to `sltiu rd, rs, 1`.
/// - `snez rd, rs` desugars to `sltu rd, x0, rs`.
/// - `sltz rd, rs` desugars to `slt rd, rs, x0`.
/// - `sgtz rd, rs` desugars to `slt rd, x0, rs`.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ITypeOp {
    Addi,
    Slli,
    Slti,
    Sltiu,
    Xori,
    Ori,
    Andi,
    Srli,
    Srai,
    Jalr,
}

/// An enum for the B-type branch instructions.
///
/// **Grammar Rule and Example:**
/// `BTypeOp Register Comma Register Comma expression`
///
/// - `beq`: `beq a1, a2, loop_start` or `beq a1, a2, . + 8`
///
/// **Pseudo-ops and Desugaring:**
/// - `beqz rs, label` desugars to `beq rs, x0, label`.
/// - `bnez rs, label` desugars to `bne rs, x0, label`.
/// - `blez rs, label` desugars to `bge x0, rs, label`.
/// - `bgez rs, label` desugars to `bge rs, x0, label`.
/// - `bltz rs, label` desugars to `blt rs, x0, label`.
/// - `bgtz rs, label` desugars to `blt x0, rs, label`.
/// - `bgt rs1, rs2, label` desugars to `blt rs2, rs1, label`.
/// - `ble rs1, rs2, label` desugars to `bge rs2, rs1, label`.
/// - `bgtu rs1, rs2, label` desugars to `bltu rs2, rs1, label`.
/// - `bleu rs1, rs2, label` desugars to `bgeu rs2, rs1, label`.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum BTypeOp {
    Beq,
    Bne,
    Blt,
    Bge,
    Bltu,
    Bgeu,
}

/// An enum for the U-type instructions.
///
/// **Grammar Rule and Example:**
/// `UTypeOp Register Comma expression`
///
/// - `lui`: `lui a0, 0x10000`
///
/// **Variants:**
/// - `auipc` is the only other U-type instruction.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum UTypeOp {
    Lui,
    Auipc,
}

/// An enum for the J-type jump instructions.
///
/// **Grammar Rule and Example:**
/// `JTypeOp [ Register Comma ] expression`
///
/// - `jal`: `jal a0, my_function` or `jal a0, . + 16`
///
/// **Variants:**
/// - The return register is optional: `jal my_function` desugars to `jal ra, my_function`.
/// - `j label` is a pseudo-op that desugars to `jal x0, label`.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum JTypeOp {
    Jal,
}

/// An enum for special zero-operand instructions.
///
/// **Grammar Rule and Example:**
/// `SpecialOp`
///
/// - `fence`
/// - `fence.tso`
/// - `fence.i`
/// - `ecall`
/// - `ebreak`
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum SpecialOp {
    Fence { pred: u8, succ: u8 },
    FenceTso,
    FenceI,
    Ecall,
    Ebreak,
}

/// The `LoadStoreOp` enum covers all load and store instructions.
///
/// **Grammar Rule and Example:**
/// `LoadStoreOp Register Comma [ expression ] OpenParen Register CloseParen`
///
/// - `lb`: `lb a0, 0(a1)`
///
/// **Parsing Notes:**
/// The syntax `offset(register)` presents a parsing challenge because the `offset`
/// expression is optional. A form like `lw a0, (t4)` is valid, as is `lw a0, 16(t4)`.
/// This cannot be parsed with a single token of lookahead, as the parser doesn't know
/// if a `(` is the start of a parenthesized expression or the start of the final
/// `(register)` block. A backtracking or multi-path parsing strategy within the
/// load/store parsing function will be required to handle this ambiguity.
///
/// **Variants:**
/// - The expression for the offset is optional, e.g., `lb a0, (a1)`.
/// - The other load/store instructions are similar (`lh`, `lw`, `ld`, `lbu`, etc.).
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum LoadStoreOp {
    Lb,
    Lh,
    Lw,
    Lbu,
    Lhu,
    Sb,
    Sh,
    Sw,
}

/// Atomic instruction operations (A extension).
///
/// **Grammar Rule and Example:**
/// `AtomicOp [ Ordering ] Register Comma Register Comma OpenParen Register CloseParen`
///
/// - `lr.w`: `lr.w a0, (a1)` - Load reserved word
/// - `sc.w`: `sc.w a0, a2, (a1)` - Store conditional word
/// - `amoswap.w`: `amoswap.w a0, a2, (a1)` - Atomic swap
///
/// **Variants:**
/// Load-Reserved / Store-Conditional (word):
/// - `LrW`: Load reserved word
/// - `ScW`: Store conditional word
///
/// Atomic Memory Operations (word):
/// - `AmoswapW`: Atomic swap
/// - `AmoaddW`: Atomic add
/// - `AmoxorW`: Atomic XOR
/// - `AmoandW`: Atomic AND
/// - `AmoorW`: Atomic OR
/// - `AmominW`: Atomic min (signed)
/// - `AmomaxW`: Atomic max (signed)
/// - `AmominuW`: Atomic min (unsigned)
/// - `AmomaxuW`: Atomic max (unsigned)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum AtomicOp {
    // Load-Reserved / Store-Conditional (word)
    LrW,
    ScW,

    // Atomic Memory Operations (word)
    AmoswapW,
    AmoaddW,
    AmoxorW,
    AmoandW,
    AmoorW,
    AmominW,
    AmomaxW,
    AmominuW,
    AmomaxuW,
}

/// Memory ordering constraints for atomic instructions.
///
/// All atomic operations in the A extension support optional memory ordering annotations:
/// - `.aq` (acquire): Load-acquire semantics
/// - `.rel` (release): Store-release semantics
/// - `.aqrl` (both): Full memory barrier
///
/// **Examples:**
/// - `lr.w.aq a0, (a1)` - Load with acquire semantics
/// - `sc.w.rel a0, a2, (a1)` - Store with release semantics
/// - `amoswap.w.aqrl a0, a2, (a1)` - Atomic operation with full barrier
#[derive(Debug, Clone, PartialEq, Copy, Eq, Hash)]
pub enum MemoryOrdering {
    None, // No ordering constraint
    Aq,   // Acquire semantics
    Rel,  // Release semantics
    AqRl, // Both acquire and release
}

/// An enum to represent pseudo-instructions that desugar into multiple base instructions.
/// The parser will recognize these, and a later pass will expand them.
#[derive(Debug, Clone, PartialEq)]
pub enum PseudoOp {
    /// `li rd, imm`: Desugars to one of several different sequences depending on
    /// the size of the immediate.
    Li(Register, Box<Expression>),
    /// `la rd, symbol`: Generic load address. Desugars to `addi` using `gp` or `auipc` and `addi`.
    La(Register, Box<Expression>),
    /// `l(b|h|w|d|bu|hu|wu) rd, expression`: Desugars to `auipc` and the corresponding load instruction.
    LoadGlobal(LoadStoreOp, Register, Box<Expression>),
    /// `s(b|h|w|d) rs, expression, t0`: Desugars to `auipc` and the corresponding store instruction.  Requires a temporary register.
    StoreGlobal(LoadStoreOp, Register, Box<Expression>, Register),
    /// `call expression`: Desugars to `auipc` and `jalr`.
    Call(Box<Expression>),
    /// `tail expression`: Desugars to `auipc` and `jalr x0, ...`.
    Tail(Box<Expression>),
}

/// A node in the AST representing an assembler directive.
/// Each variant represents a different directive and its required operands.
///
/// **Grammar Rule:**
/// `directive:`
/// `| Global Identifier`
/// `| Equ Identifier Comma expression`
/// `| Text | Data | Bss`
/// `| Space expression`
/// `| String list_of_strings`
/// `| Asciz list_of_strings`
/// `| Byte list_of_expressions`
/// `| TwoByte list_of_expressions`
/// `| FourByte list_of_expressions`
///
/// **Parsing Notes:**
/// The parser must check for a label preceding a directive. A label can only precede
/// `.space`, `.string`, `.asciz`, `.byte`, `.2byte`, or `.4byte`. It cannot
/// precede `.global`, `.equ`, `.text`, `.data`, or `.bss`.
#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    /// .global symbol
    Global(Vec<String>),
    /// .equ symbol, expression
    Equ(String, Expression),
    /// .text
    Text,
    /// .data
    Data,
    /// .bss
    Bss,
    /// .space expression
    Space(Expression),
    /// .balign expression
    Balign(Expression),
    /// Data directives that can take a list of values.
    String(Vec<String>),
    Asciz(Vec<String>),
    Byte(Vec<Expression>),
    TwoByte(Vec<Expression>),
    FourByte(Vec<Expression>),
}

/// An expression, which can be a single literal or a complex combination of
/// literals, identifiers, and operators. The parser should build this tree
/// respecting standard C operator precedence.
///
/// **Grammar Rule (with C-style precedence from low to high):**
/// `expression:          bitwise_or_expr`
/// `bitwise_or_expr:     bitwise_xor_expr ( '|' bitwise_xor_expr )*`
/// `bitwise_xor_expr:    bitwise_and_expr ( '^' bitwise_and_expr )*`
/// `bitwise_and_expr:    shift_expr ( '&' shift_expr )*`
/// `shift_expr:          additive_expr ( ('<<' | '>>') additive_expr )*`
/// `additive_expr:       multiplicative_expr ( ('+' | '-') multiplicative_expr )*`
/// `multiplicative_expr: unary ( ('*' | '/') unary )*`
/// `unary:               '-' operand | '~' operand | operand`
/// `operand:             Literal | Identifier | Register | '(' expression ')'`
///
/// **Parsing Notes:**
/// The parser will use the grammar's structure to handle operator precedence automatically.
/// The `Minus` and `Tilde` operators have the highest precedence, followed by multiplication/division,
/// addition/subtraction, bitwise shifts, and finally the bitwise logical operators.
/// Parentheses override the default precedence.
///
/// The parser will distinguish between a binary minus and a unary minus based on context.
/// A `Token::Operator(OperatorOp::Minus)` is considered a unary minus if it is at the beginning of an expression
/// or immediately follows another operator or an open parenthesis.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Identifier(String),
    /// An integer literal. The tokenizer will convert any literal that fits in a
    /// u32 into a bit-equivalent i32.
    Literal(i32),
    /// Binary operations with a left and right-hand side.
    PlusOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    MinusOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    MultiplyOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    DivideOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    ModuloOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    LeftShiftOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    RightShiftOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    BitwiseOrOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    BitwiseAndOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    BitwiseXorOp {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
    /// Unary operations with a single operand.
    NegateOp {
        expr: Box<Expression>,
    },
    BitwiseNotOp {
        expr: Box<Expression>,
    },
    /// A parenthesized sub-expression. `Box` is used to prevent infinite recursion
    /// and to store the expression on the heap.
    Parenthesized(Box<Expression>),
    /// The current address (.) in expressions, resolved to the address of the current instruction or directive.
    CurrentAddress,
    /// A numeric label reference, subsumed into expressions for flexibility.
    NumericLabelRef(NumericLabelRef),
}

// ==============================================================================
// Display Implementations
// ==============================================================================

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}:{}]", self.file, self.line)
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Segment::Text => ".text",
            Segment::Data => ".data",
            Segment::Bss => ".bss",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ABI names
        let s = match self {
            Register::X0 => "zero",
            Register::X1 => "ra",
            Register::X2 => "sp",
            Register::X3 => "gp",
            Register::X4 => "tp",
            Register::X5 => "t0",
            Register::X6 => "t1",
            Register::X7 => "t2",
            Register::X8 => "s0",
            Register::X9 => "s1",
            Register::X10 => "a0",
            Register::X11 => "a1",
            Register::X12 => "a2",
            Register::X13 => "a3",
            Register::X14 => "a4",
            Register::X15 => "a5",
            Register::X16 => "a6",
            Register::X17 => "a7",
            Register::X18 => "s2",
            Register::X19 => "s3",
            Register::X20 => "s4",
            Register::X21 => "s5",
            Register::X22 => "s6",
            Register::X23 => "s7",
            Register::X24 => "s8",
            Register::X25 => "s9",
            Register::X26 => "s10",
            Register::X27 => "s11",
            Register::X28 => "t3",
            Register::X29 => "t4",
            Register::X30 => "t5",
            Register::X31 => "t6",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for OperatorOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            OperatorOp::Plus => "+",
            OperatorOp::Minus => "-",
            OperatorOp::Multiply => "*",
            OperatorOp::Divide => "/",
            OperatorOp::LeftShift => "<<",
            OperatorOp::RightShift => ">>",
            OperatorOp::BitwiseOr => "|",
            OperatorOp::BitwiseAnd => "&",
            OperatorOp::BitwiseXor => "^",
            OperatorOp::BitwiseNot => "~",
            OperatorOp::Modulo => "%",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for DirectiveOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DirectiveOp::Global => ".global",
            DirectiveOp::Equ => ".equ",
            DirectiveOp::Text => ".text",
            DirectiveOp::Data => ".data",
            DirectiveOp::Bss => ".bss",
            DirectiveOp::Space => ".space",
            DirectiveOp::Balign => ".balign",
            DirectiveOp::String => ".string",
            DirectiveOp::Asciz => ".asciz",
            DirectiveOp::Byte => ".byte",
            DirectiveOp::TwoByte => ".2byte",
            DirectiveOp::FourByte => ".4byte",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Identifier(s) => write!(f, "{}", s),
            Token::Register(r) => write!(f, "{}", r),
            Token::Integer(i) => write!(f, "{}", i),

            Token::StringLiteral(s) => write!(f, "{:?}", s),
            Token::Directive(d) => write!(f, "{}", d),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::OpenParen => write!(f, "("),
            Token::CloseParen => write!(f, ")"),
            Token::Operator(o) => write!(f, "{}", o),
            Token::Dot => write!(f, "."),
        }
    }
}

impl fmt::Display for LineContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineContent::Label(l) => write!(f, "{}:", l),
            LineContent::Instruction(i) => write!(f, "{:16}{}", "", i),
            LineContent::Directive(d) => write!(f, "{:16}{}", "", d),
        }
    }
}

impl fmt::Display for NumericLabelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.num, if self.is_forward { "f" } else { "b" })
    }
}

impl fmt::Display for RTypeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
impl fmt::Display for ITypeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
impl fmt::Display for BTypeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
impl fmt::Display for UTypeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
impl fmt::Display for JTypeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
impl fmt::Display for SpecialOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
impl fmt::Display for LoadStoreOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

impl fmt::Display for AtomicOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

impl fmt::Display for MemoryOrdering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryOrdering::None => Ok(()),
            MemoryOrdering::Aq => write!(f, ".aq"),
            MemoryOrdering::Rel => write!(f, ".rel"),
            MemoryOrdering::AqRl => write!(f, ".aqrl"),
        }
    }
}

impl fmt::Display for PseudoOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PseudoOp::Li(rd, imm) => write!(f, "{:<7} {}, {}", "li", rd, imm),
            PseudoOp::La(rd, s) => write!(f, "{:<7} {}, {}", "la", rd, s),
            PseudoOp::LoadGlobal(op, rd, expr) => {
                write!(f, "{:<7} {}, {}", op.to_string(), rd, expr)
            }
            PseudoOp::StoreGlobal(op, rs, expr, temp) => {
                write!(f, "{:<7} {}, {}, {}", op.to_string(), rs, expr, temp)
            }
            PseudoOp::Call(expr) => write!(f, "{:<7} {}", "call", expr),
            PseudoOp::Tail(expr) => write!(f, "{:<7} {}", "tail", expr),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::RType(op, rd, rs1, rs2) => {
                write!(f, "{:<7} {}, {}, {}", op.to_string(), rd, rs1, rs2)
            }
            Instruction::IType(op, rd, rs1, imm) => {
                write!(f, "{:<7} {}, {}, {}", op.to_string(), rd, rs1, imm)
            }
            Instruction::BType(op, rs1, rs2, expr) => {
                write!(f, "{:<7} {}, {}, {}", op.to_string(), rs1, rs2, expr)
            }
            Instruction::UType(op, rd, imm) => {
                write!(f, "{:<7} {}, {}", op.to_string(), rd, imm)
            }
            Instruction::JType(op, rd, expr) => {
                write!(f, "{:<7} {}, {}", op.to_string(), rd, expr)
            }
            Instruction::Special(op) => write!(f, "{}", op),
            Instruction::LoadStore(op, rd, offset, rs) => {
                write!(f, "{:<7} {}, {}({})", op.to_string(), rd, offset, rs)
            }
            Instruction::Atomic(op, rd, rs1, rs2, ordering) => {
                // LR instructions only use rd and rs1
                let combined = format!("{}{}", op, ordering);
                if matches!(op, AtomicOp::LrW) {
                    write!(f, "{:<7} {}, ({})", combined, rd, rs1)
                } else {
                    write!(f, "{:<7} {}, {}, ({})", combined, rd, rs2, rs1)
                }
            }
            Instruction::Compressed(op, operands) => {
                write!(f, "{}", format_compressed_instruction(op, operands))
            }
            Instruction::Pseudo(p) => write!(f, "{}", p),
        }
    }
}

/// Helper function to format compressed instructions
fn format_compressed_instruction(
    op: &CompressedOp,
    operands: &CompressedOperands,
) -> String {
    match (op, operands) {
        (CompressedOp::CAdd, CompressedOperands::CR { rd, rs2 }) => {
            format!("c.add       {}, {}", rd, rs2)
        }
        (CompressedOp::CMv, CompressedOperands::CR { rd, rs2 }) => {
            format!("c.mv        {}, {}", rd, rs2)
        }
        (CompressedOp::CJr, CompressedOperands::CRSingle { rs1 }) => {
            format!("c.jr        {}", rs1)
        }
        (CompressedOp::CJalr, CompressedOperands::CRSingle { rs1 }) => {
            format!("c.jalr      {}", rs1)
        }
        (CompressedOp::CLi, CompressedOperands::CI { rd, imm }) => {
            format!("c.li        {}, {}", rd, imm)
        }
        (CompressedOp::CLui, CompressedOperands::CI { rd, imm }) => {
            format!("c.lui       {}, {}", rd, imm)
        }
        (CompressedOp::CAddi, CompressedOperands::CI { rd, imm }) => {
            format!("c.addi      {}, {}", rd, imm)
        }
        (CompressedOp::CAddi16sp, CompressedOperands::CI { rd: _, imm }) => {
            format!("c.addi16sp  {}", imm)
        }
        (CompressedOp::CSlli, CompressedOperands::CI { rd, imm }) => {
            format!("c.slli      {}, {}", rd, imm)
        }
        (
            CompressedOp::CLwsp,
            CompressedOperands::CIStackLoad { rd, offset },
        ) => {
            format!("c.lwsp      {}, {}(sp)", rd, offset)
        }
        (
            CompressedOp::CSwsp,
            CompressedOperands::CSSStackStore { rs2, offset },
        ) => {
            format!("c.swsp      {}, {}(sp)", rs2, offset)
        }
        (
            CompressedOp::CLw,
            CompressedOperands::CL { rd_prime, rs1_prime, offset },
        ) => {
            format!("c.lw        {}, {}({})", rd_prime, offset, rs1_prime)
        }
        (
            CompressedOp::CSw,
            CompressedOperands::CS { rs2_prime, rs1_prime, offset },
        ) => {
            format!("c.sw        {}, {}({})", rs2_prime, offset, rs1_prime)
        }
        (
            CompressedOp::CAnd,
            CompressedOperands::CA { rd_prime, rs2_prime },
        ) => {
            format!("c.and       {}, {}", rd_prime, rs2_prime)
        }
        (CompressedOp::COr, CompressedOperands::CA { rd_prime, rs2_prime }) => {
            format!("c.or        {}, {}", rd_prime, rs2_prime)
        }
        (
            CompressedOp::CXor,
            CompressedOperands::CA { rd_prime, rs2_prime },
        ) => {
            format!("c.xor       {}, {}", rd_prime, rs2_prime)
        }
        (
            CompressedOp::CSub,
            CompressedOperands::CA { rd_prime, rs2_prime },
        ) => {
            format!("c.sub       {}, {}", rd_prime, rs2_prime)
        }
        (CompressedOp::CSrli, CompressedOperands::CBImm { rd_prime, imm }) => {
            format!("c.srli      {}, {}", rd_prime, imm)
        }
        (CompressedOp::CSrai, CompressedOperands::CBImm { rd_prime, imm }) => {
            format!("c.srai      {}, {}", rd_prime, imm)
        }
        (CompressedOp::CAndi, CompressedOperands::CBImm { rd_prime, imm }) => {
            format!("c.andi      {}, {}", rd_prime, imm)
        }
        (
            CompressedOp::CBeqz,
            CompressedOperands::CBBranch { rs1_prime, offset },
        ) => {
            format!("c.beqz      {}, {}", rs1_prime, offset)
        }
        (
            CompressedOp::CBnez,
            CompressedOperands::CBBranch { rs1_prime, offset },
        ) => {
            format!("c.bnez      {}, {}", rs1_prime, offset)
        }
        (CompressedOp::CJComp, CompressedOperands::CJOpnd { offset }) => {
            format!("c.j         {}", offset)
        }
        (CompressedOp::CJalComp, CompressedOperands::CJOpnd { offset }) => {
            format!("c.jal       {}", offset)
        }
        (CompressedOp::CNop, CompressedOperands::None) => "c.nop".to_string(),
        (CompressedOp::CEbreak, CompressedOperands::None) => {
            "c.ebreak".to_string()
        }
        _ => format!("c.<unknown> {:?} {:?}", op, operands),
    }
}

impl fmt::Display for Directive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Directive::Global(s) => {
                write!(f, "{:<7} {}", ".global", s.join(", "))
            }
            Directive::Equ(name, expr) => {
                write!(f, "{:<7} {}, {}", ".equ", name, expr)
            }
            Directive::Text => write!(f, ".text"),
            Directive::Data => write!(f, ".data"),
            Directive::Bss => write!(f, ".bss"),
            Directive::Space(expr) => write!(f, "{:<7} {}", ".space", expr),
            Directive::Balign(expr) => write!(f, "{:<7} {}", ".balign", expr),
            Directive::String(items) => {
                let formatted = items
                    .iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{:<7} {}", ".string", formatted)
            }
            Directive::Asciz(items) => {
                let formatted = items
                    .iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{:<7} {}", ".asciz", formatted)
            }
            Directive::Byte(items) => {
                let formatted = items
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{:<7} {}", ".byte", formatted)
            }
            Directive::TwoByte(items) => {
                let formatted = items
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{:<7} {}", ".2byte", formatted)
            }
            Directive::FourByte(items) => {
                let formatted = items
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{:<7} {}", ".4byte", formatted)
            }
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Identifier(s) => write!(f, "{}", s),
            Expression::Literal(i) => write!(f, "{}", i),
            Expression::PlusOp { lhs, rhs } => write!(f, "{} + {}", lhs, rhs),
            Expression::MinusOp { lhs, rhs } => write!(f, "{} - {}", lhs, rhs),
            Expression::MultiplyOp { lhs, rhs } => {
                write!(f, "{} * {}", lhs, rhs)
            }
            Expression::DivideOp { lhs, rhs } => write!(f, "{} / {}", lhs, rhs),
            Expression::ModuloOp { lhs, rhs } => write!(f, "{} % {}", lhs, rhs),
            Expression::LeftShiftOp { lhs, rhs } => {
                write!(f, "{} << {}", lhs, rhs)
            }
            Expression::RightShiftOp { lhs, rhs } => {
                write!(f, "{} >> {}", lhs, rhs)
            }
            Expression::BitwiseOrOp { lhs, rhs } => {
                write!(f, "{} | {}", lhs, rhs)
            }
            Expression::BitwiseAndOp { lhs, rhs } => {
                write!(f, "{} & {}", lhs, rhs)
            }
            Expression::BitwiseXorOp { lhs, rhs } => {
                write!(f, "{} ^ {}", lhs, rhs)
            }
            Expression::NegateOp { expr } => write!(f, "-{}", expr),
            Expression::BitwiseNotOp { expr } => write!(f, "~{}", expr),
            Expression::Parenthesized(expr) => write!(f, "({})", expr),
            Expression::CurrentAddress => write!(f, "."),
            Expression::NumericLabelRef(nlr) => write!(f, "{}", nlr),
        }
    }
}
