// encoder_tests.rs
//
// Unit tests for the RISC-V instruction encoder
// These tests compare our encoder output against GNU assembler output

use crate::assembler::{NoOpCallback, converge_and_encode};
use crate::ast::{Source, SourceFile, create_builtin_symbols_file};
use crate::encoder::Relax;
use crate::parser::parse;
use crate::symbols::link_symbols;
use crate::tokenizer::tokenize;

/// Helper function to assemble a source string and return the encoded bytes
fn assemble(source_text: &str) -> Result<(Vec<u8>, Vec<u8>, u32), String> {
    use crate::assembler::guess_line_size;
    use crate::ast::{Directive, LineContent, Segment};

    // Process each line
    let mut all_lines = Vec::new();
    let mut current_segment = Segment::Text;

    for (line_num, line_text) in source_text.lines().enumerate() {
        let line_text = line_text.trim();
        if line_text.is_empty() || line_text.starts_with('#') {
            continue;
        }

        // Tokenize
        let tokens = tokenize(line_text).map_err(|e| {
            format!("Tokenize error on line {}: {}", line_num + 1, e)
        })?;

        if tokens.is_empty() {
            continue;
        }

        // Parse
        let lines = parse(&tokens, "test.s".to_string(), line_num + 1)
            .map_err(|e| {
                format!("Parse error on line {}: {}", line_num + 1, e)
            })?;

        for mut line in lines {
            // Update segment if directive changes it
            if let LineContent::Directive(ref dir) = line.content {
                match dir {
                    Directive::Text => current_segment = Segment::Text,
                    Directive::Data => current_segment = Segment::Data,
                    Directive::Bss => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            // Segment and size will be set in the layout phase
            all_lines.push(line);
        }
    }

    // Build Source structure
    let mut source = Source {
        files: vec![SourceFile {
            file: "test.s".to_string(),
            lines: all_lines,
        }],
    };

    // Add builtin symbols file (provides __global_pointer$ definition)
    source.files.push(create_builtin_symbols_file());

    // Resolve symbols
    let symbols = link_symbols(&mut source)
        .map_err(|e| format!("Symbol resolution error: {:?}", e))?;

    // Create relaxation settings (disable compression in tests to keep instruction sizes predictable)
    let relax = Relax { gp: true, pseudo: true, compressed: false };

    // Converge: repeatedly compute offsets, evaluate expressions, and encode
    // until line sizes stabilize. Returns the final encoded segments.
    let mut layout = crate::layout::create_initial_layout(&source);
    converge_and_encode(
        &mut source,
        &symbols,
        &mut layout,
        0x10000,
        &relax,
        &NoOpCallback,
        false,
    )
    .map_err(|e| e.with_source_context())
}

/// Helper to format bytes as hex for debugging
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
}

/// Helper to compare encoded data segment with expected bytes
fn assert_data_match(source: &str, expected_data: &[u8]) {
    let (text, data, bss_size) =
        assemble(source).expect("Assembly should succeed");

    assert_eq!(text.len(), 0, "Expected no text segment output");
    assert_eq!(bss_size, 0, "Expected no BSS segment");

    if data != expected_data {
        panic!(
            "Data segment differs:\n  Expected: {}\n  Got:      {}",
            bytes_to_hex(expected_data),
            bytes_to_hex(&data)
        );
    }
}

/// Helper to compare encoded instructions with expected bytes
fn assert_instructions_match(source: &str, expected_text: &[u8]) {
    let (text, data, bss_size) =
        assemble(source).expect("Assembly should succeed");

    assert_eq!(data.len(), 0, "Expected no data segment output");
    assert_eq!(bss_size, 0, "Expected no BSS segment");

    if text != expected_text {
        // Find which instruction differs
        let chunk_size = 4; // Instructions are 4 bytes
        let text_chunks: Vec<_> = text.chunks(chunk_size).collect();
        let expected_chunks: Vec<_> =
            expected_text.chunks(chunk_size).collect();

        for (i, (actual, expected)) in
            text_chunks.iter().zip(expected_chunks.iter()).enumerate()
        {
            if actual != expected {
                panic!(
                    "Instruction {} differs:\n  Expected: {}\n  Got:      {}\n\nFull output:\n  Expected: {}\n  Got:      {}",
                    i,
                    bytes_to_hex(expected),
                    bytes_to_hex(actual),
                    bytes_to_hex(expected_text),
                    bytes_to_hex(&text)
                );
            }
        }

        // If we get here, lengths differ
        panic!(
            "Instruction count differs:\n  Expected {} bytes ({} instructions)\n  Got {} bytes ({} instructions)\n\nExpected: {}\nGot:      {}",
            expected_text.len(),
            expected_text.len() / 4,
            text.len(),
            text.len() / 4,
            bytes_to_hex(expected_text),
            bytes_to_hex(&text)
        );
    }
}

// ============================================================================
// R-Type Instruction Tests
// ============================================================================

#[test]
fn test_r_type_base() {
    let source = r#"
.text
add x1, x2, x3
sub x4, x5, x6
sll x7, x8, x9
slt x10, x11, x12
sltu x13, x14, x15
xor x16, x17, x18
srl x19, x20, x21
sra x22, x23, x24
or x25, x26, x27
and x28, x29, x30
"#;

    // From GNU assembler disassembly
    let expected = &[
        0xb3, 0x00, 0x31, 0x00, // add ra,sp,gp
        0x33, 0x82, 0x62, 0x40, // sub tp,t0,t1
        0xb3, 0x13, 0x94, 0x00, // sll t2,s0,s1
        0x33, 0xa5, 0xc5, 0x00, // slt a0,a1,a2
        0xb3, 0x36, 0xf7, 0x00, // sltu a3,a4,a5
        0x33, 0xc8, 0x28, 0x01, // xor a6,a7,s2
        0xb3, 0x59, 0x5a, 0x01, // srl s3,s4,s5
        0x33, 0xdb, 0x8b, 0x41, // sra s6,s7,s8
        0xb3, 0x6c, 0xbd, 0x01, // or s9,s10,s11
        0x33, 0xfe, 0xee, 0x01, // and t3,t4,t5
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Auto-Relaxation Tests
// ============================================================================
// These tests verify that base instructions are automatically relaxed to their
// compressed equivalents when relaxation is enabled.

#[test]
fn test_relax_addi_to_c_addi() {
    // addi rd, rd, imm → c.addi rd, imm (when imm is 6-bit signed)
    // Tests that the instruction count is reduced due to relaxation
    let source = r#"
.global _start
_start:
addi a0, a0, 5
addi a1, a1, -10
"#;

    use crate::assembler::{self, guess_line_size};
    use crate::ast::{Directive, LineContent, Segment};
    use crate::symbols;

    let mut all_lines = Vec::new();
    let mut current_segment = Segment::Text;

    for (line_num, line_text) in source.lines().enumerate() {
        let line_text = line_text.trim();
        if line_text.is_empty() || line_text.starts_with('#') {
            continue;
        }

        let tokens = tokenize(line_text).expect("Tokenize should succeed");
        if tokens.is_empty() {
            continue;
        }

        let lines = parse(&tokens, "test.s".to_string(), line_num + 1)
            .expect("Parse should succeed");

        for mut line in lines {
            if let LineContent::Directive(ref dir) = line.content {
                match dir {
                    Directive::Text => current_segment = Segment::Text,
                    Directive::Data => current_segment = Segment::Data,
                    Directive::Bss => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            
            
            all_lines.push(line);
        }
    }

    // Build Source with relaxation ENABLED
    let mut source_struct = Source {
        files: vec![SourceFile {
            file: "test.s".to_string(),
            lines: all_lines,
        }],
    };

    let text = symbols::link_symbols(&mut source_struct)
        .and_then(|symbols| {
            let callback = assembler::NoOpCallback;
            let relax = Relax { gp: true, pseudo: true, compressed: true };
            let mut layout = crate::layout::create_initial_layout(&source_struct);
            assembler::converge_and_encode(
                &mut source_struct,
                &symbols,
                &mut layout,
                0x10000,
                &relax,
                &callback,
                false,
            )
            .map(|(text, _, _)| text)
        })
        .expect("Assembly should succeed");

    // With relaxation, 2 addi instructions should compile to 4 bytes (2x2) instead of 8 bytes (2x4)
    assert_eq!(
        text.len(),
        4,
        "Relaxed 2 addi instructions should be 4 bytes total"
    );
}

#[test]
fn test_relax_add_to_c_add() {
    // add rd, rd, rs2 → c.add rd, rs2 (rd != x0, rs2 != x0)
    let source = r#"
.global _start
_start:
add a0, a0, a1
"#;

    use crate::assembler::{self, guess_line_size};
    use crate::ast::{Directive, LineContent, Segment};
    use crate::symbols;

    let mut all_lines = Vec::new();
    let mut current_segment = Segment::Text;

    for (line_num, line_text) in source.lines().enumerate() {
        let line_text = line_text.trim();
        if line_text.is_empty() || line_text.starts_with('#') {
            continue;
        }

        let tokens = tokenize(line_text).expect("Tokenize should succeed");
        if tokens.is_empty() {
            continue;
        }

        let lines = parse(&tokens, "test.s".to_string(), line_num + 1)
            .expect("Parse should succeed");

        for mut line in lines {
            if let LineContent::Directive(ref dir) = line.content {
                match dir {
                    Directive::Text => current_segment = Segment::Text,
                    Directive::Data => current_segment = Segment::Data,
                    Directive::Bss => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            
            
            all_lines.push(line);
        }
    }

    // Build Source with relaxation ENABLED
    let mut source_struct = Source {
        files: vec![SourceFile {
            file: "test.s".to_string(),
            lines: all_lines,
        }],
    };

    let text = symbols::link_symbols(&mut source_struct)
        .and_then(|symbols| {
            let callback = assembler::NoOpCallback;
            let relax = Relax { gp: true, pseudo: true, compressed: true };
            let mut layout = crate::layout::create_initial_layout(&source_struct);
            assembler::converge_and_encode(
                &mut source_struct,
                &symbols,
                &mut layout,
                0x10000,
                &relax,
                &callback,
                false,
            )
            .map(|(text, _, _)| text)
        })
        .expect("Assembly should succeed");

    // With relaxation, 1 add instruction should compile to 2 bytes instead of 4 bytes
    assert_eq!(text.len(), 2, "Relaxed add instruction should be 2 bytes");
}

#[test]
fn test_relax_no_compression_large_immediate() {
    // addi with immediate > 31 should NOT relax
    let source = r#"
.global _start
_start:
addi a0, a0, 50
"#;

    use crate::assembler::{self, guess_line_size};
    use crate::ast::{Directive, LineContent, Segment};
    use crate::symbols;

    let mut all_lines = Vec::new();
    let mut current_segment = Segment::Text;

    for (line_num, line_text) in source.lines().enumerate() {
        let line_text = line_text.trim();
        if line_text.is_empty() || line_text.starts_with('#') {
            continue;
        }

        let tokens = tokenize(line_text).expect("Tokenize should succeed");
        if tokens.is_empty() {
            continue;
        }

        let lines = parse(&tokens, "test.s".to_string(), line_num + 1)
            .expect("Parse should succeed");

        for mut line in lines {
            if let LineContent::Directive(ref dir) = line.content {
                match dir {
                    Directive::Text => current_segment = Segment::Text,
                    Directive::Data => current_segment = Segment::Data,
                    Directive::Bss => current_segment = Segment::Bss,
                    _ => {}
                }
            }

            
            
            all_lines.push(line);
        }
    }

    // Build Source with relaxation ENABLED
    let mut source_struct = Source {
        files: vec![SourceFile {
            file: "test.s".to_string(),
            lines: all_lines,
        }],
    };

    let text = symbols::link_symbols(&mut source_struct)
        .and_then(|symbols| {
            let callback = assembler::NoOpCallback;
            let relax = Relax { gp: true, pseudo: true, compressed: true };
            let mut layout = crate::layout::create_initial_layout(&source_struct);
            assembler::converge_and_encode(
                &mut source_struct,
                &symbols,
                &mut layout,
                0x10000,
                &relax,
                &callback,
                false,
            )
            .map(|(text, _, _)| text)
        })
        .expect("Assembly should succeed");

    // Should NOT be compressed because 50 doesn't fit in 6-bit signed
    // So it should be 4 bytes (base instruction)
    assert_eq!(
        text.len(),
        4,
        "Large immediate should NOT be compressed and remain 4 bytes"
    );
}

// ============================================================================
// I-Type Instruction Tests
// ============================================================================

#[test]
fn test_i_type_arithmetic() {
    let source = r#"
.text
addi x1, x2, 100
slti x3, x4, -50
sltiu x5, x6, 200
xori x7, x8, 0x555
ori x9, x10, 0x333
andi x11, x12, 0x7ff
"#;

    let expected = &[
        0x93, 0x00, 0x41, 0x06, // addi ra,sp,100
        0x93, 0x21, 0xe2, 0xfc, // slti gp,tp,-50
        0x93, 0x32, 0x83, 0x0c, // sltiu t0,t1,200
        0x93, 0x43, 0x54, 0x55, // xori t2,s0,1365
        0x93, 0x64, 0x35, 0x33, // ori s1,a0,819
        0x93, 0x75, 0xf6, 0x7f, // andi a1,a2,2047
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_i_type_shifts() {
    let source = r#"
.text
slli x13, x14, 5
srli x15, x16, 10
srai x17, x18, 15
"#;

    let expected = &[
        0x93, 0x16, 0x57, 0x00, // slli a3,a4,0x5
        0x93, 0x57, 0xa8, 0x00, // srli a5,a6,0xa
        0x93, 0x58, 0xf9, 0x40, // srai a7,s2,0xf
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_jalr() {
    let source = r#"
.text
jalr x27, x28, 0
"#;

    let expected = &[
        0xe7, 0x0d, 0x0e, 0x00, // jalr s11,t3,0
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Load/Store Instruction Tests
// ============================================================================

#[test]
fn test_load_instructions() {
    let source = r#"
.text
lb x1, 0(x2)
lh x3, 4(x4)
lw x5, 8(x6)
lbu x9, 20(x10)
lhu x11, 24(x12)
"#;

    let expected = &[
        0x83, 0x00, 0x01, 0x00, // lb ra,0(sp)
        0x83, 0x11, 0x42, 0x00, // lh gp,4(tp)
        0x83, 0x22, 0x83, 0x00, // lw t0,8(t1)
        0x83, 0x44, 0x45, 0x01, // lbu s1,20(a0)
        0x83, 0x55, 0x86, 0x01, // lhu a1,24(a2)
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_store_instructions() {
    let source = r#"
.text
sb x15, 32(x16)
sh x17, 36(x18)
sw x19, 40(x20)
"#;

    let expected = &[
        0x23, 0x00, 0xf8, 0x02, // sb a5,32(a6)
        0x23, 0x12, 0x19, 0x03, // sh a7,36(s2)
        0x23, 0x24, 0x3a, 0x03, // sw s3,40(s4)
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Branch Instruction Tests
// ============================================================================

#[test]
fn test_branch_instructions() {
    let source = r#"
.text
beq x1, x2, target
bne x3, x4, target
blt x5, x6, target
bge x7, x8, target
bltu x9, x10, target
bgeu x11, x12, target
target:
nop
"#;

    // Branches to offset 0x18 (24 bytes forward)
    let expected = &[
        0x63, 0x8c, 0x20, 0x00, // beq ra,sp,18 <target>
        0x63, 0x9a, 0x41, 0x00, // bne gp,tp,14 <target>
        0x63, 0xc8, 0x62, 0x00, // blt t0,t1,10 <target>
        0x63, 0xd6, 0x83, 0x00, // bge t2,s0,c <target>
        0x63, 0xe4, 0xa4, 0x00, // bltu s1,a0,8 <target>
        0x63, 0xf2, 0xc5, 0x00, // bgeu a1,a2,4 <target>
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Jump and U-Type Instruction Tests
// ============================================================================

#[test]
fn test_jal() {
    let source = r#"
.text
jal x13, target
target:
nop
"#;

    let expected = &[
        0xef, 0x06, 0x40, 0x00, // jal a3,4 <target>
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_u_type() {
    let source = r#"
.text
lui x14, 0x12345
auipc x15, 0x100
"#;

    let expected = &[
        0x37, 0x57, 0x34, 0x12, // lui a4,0x12345
        0x97, 0x07, 0x10, 0x00, // auipc a5,0x100
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Special Instruction Tests
// ============================================================================

#[test]
fn test_special_instructions() {
    let source = r#"
.text
ecall
ebreak
"#;

    let expected = &[
        0x73, 0x00, 0x00, 0x00, // ecall
        0x73, 0x00, 0x10, 0x00, // ebreak
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Pseudo-Instruction Tests
// ============================================================================

#[test]
fn test_li_small_immediate() {
    let source = r#"
.text
li x1, 0
li x2, 100
li x3, -50
li x4, 2047
"#;

    // Small immediates expand to: addi rd, x0, imm
    let expected = &[
        0x93, 0x00, 0x00, 0x00, // li ra,0  -> addi ra,zero,0
        0x13, 0x01, 0x40, 0x06, // li sp,100  -> addi sp,zero,100
        0x93, 0x01, 0xe0, 0xfc, // li gp,-50  -> addi gp,zero,-50
        0x13, 0x02, 0xf0, 0x7f, // li tp,2047  -> addi tp,zero,2047
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_li_large_immediate() {
    let source = r#"
.text
li x5, 0x12345678
li x6, -0x7fffffff
"#;

    // Large 32-bit immediates expand to: lui + addi
    let expected = &[
        0xb7, 0x52, 0x34, 0x12, // lui t0,0x12345
        0x93, 0x82, 0x82, 0x67, // addi t0,t0,1656
        0x37, 0x03, 0x00, 0x80, // lui t1,0x80000
        0x13, 0x03, 0x13, 0x00, // addi t1,t1,1
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_la_pseudo() {
    let source = r#"
.text
.global _start
_start:
la x7, target
nop
target:
nop
"#;

    // la expands to: auipc + addi
    // Target is at offset 12 (after 8-byte la + 4-byte nop)
    // PC-relative offset from la (at offset 0) to target is 12
    let expected = &[
        0x97, 0x03, 0x00, 0x00, // auipc t2,0x0
        0x93, 0x83, 0xc3, 0x00, // addi t2,t2,12
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_call_pseudo() {
    let source = r#"
.text
.global _start
_start:
call target
nop
target:
nop
"#;

    // call optimizes to: jal ra, offset (single 4-byte instruction when in range)
    // Target is at offset 8 (after 4-byte jal + 4-byte nop)
    // PC-relative offset from call (at offset 0) to target is 8
    // GNU assembler produces: 008000ef = jal ra, 8
    let expected = &[
        0xef, 0x00, 0x80, 0x00, // jal ra,8 <target>
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_tail_pseudo() {
    let source = r#"
.text
.global _start
_start:
tail target
nop
target:
nop
"#;

    // tail optimizes to: j offset (jal x0, offset - single 4-byte instruction when in range)
    // Target is at offset 8 (after 4-byte j + 4-byte nop)
    // PC-relative offset from tail (at offset 0) to target is 8
    // GNU assembler produces: 0080006f = j 8 <target>
    let expected = &[
        0x6f, 0x00, 0x80, 0x00, // j 8 <target>
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// Data Directive Tests
// ============================================================================

#[test]
fn test_byte_directive() {
    let source = r#"
.data
.byte 0x42, 0x43, 0x44
"#;

    let expected = &[0x42, 0x43, 0x44];

    assert_data_match(source, expected);
}

#[test]
fn test_twobyte_directive() {
    let source = r#"
.data
.2byte 0x1234, 0x5678
"#;

    // Little-endian: 0x1234 -> 34 12, 0x5678 -> 78 56
    let expected = &[0x34, 0x12, 0x78, 0x56];

    assert_data_match(source, expected);
}

#[test]
fn test_fourbyte_directive() {
    let source = r#"
.data
.4byte 0xDEADBEEF, 0xCAFEBABE
"#;

    // Little-endian: 0xDEADBEEF -> ef be ad de, 0xCAFEBABE -> be ba fe ca
    let expected = &[
        0xef, 0xbe, 0xad, 0xde, // 0xDEADBEEF
        0xbe, 0xba, 0xfe, 0xca, // 0xCAFEBABE
    ];

    assert_data_match(source, expected);
}

#[test]
fn test_string_directive() {
    let source = r#"
.data
.string "hello"
"#;

    // "hello" without null terminator
    let expected = &[0x68, 0x65, 0x6c, 0x6c, 0x6f]; // "hello"

    assert_data_match(source, expected);
}

#[test]
fn test_asciz_directive() {
    let source = r#"
.data
.asciz "world"
"#;

    // "world" with null terminator
    let expected = &[0x77, 0x6f, 0x72, 0x6c, 0x64, 0x00]; // "world\0"

    assert_data_match(source, expected);
}

#[test]
fn test_mixed_data_directives() {
    let source = r#"
.data
.byte 0x42, 0x43, 0x44
.2byte 0x1234, 0x5678
.4byte 0xDEADBEEF, 0xCAFEBABE
.string "hello"
.asciz "world"
"#;

    // All directives together
    let expected = &[
        0x42, 0x43, 0x44, // .byte
        0x34, 0x12, 0x78, 0x56, // .2byte
        0xef, 0xbe, 0xad, 0xde, 0xbe, 0xba, 0xfe, 0xca, // .4byte
        0x68, 0x65, 0x6c, 0x6c, 0x6f, // .string "hello"
        0x77, 0x6f, 0x72, 0x6c, 0x64, 0x00, // .asciz "world"
    ];

    assert_data_match(source, expected);
}

// ============================================================================
// BSS Segment Tests
// ============================================================================

#[test]
fn test_bss_space_directive() {
    let source = r#"
.bss
.space 64
"#;

    let (text, data, bss_size) =
        assemble(source).expect("Assembly should succeed");

    assert_eq!(text.len(), 0, "Expected no text segment output");
    assert_eq!(data.len(), 0, "Expected no data segment output");
    assert_eq!(bss_size, 64, "Expected BSS size of 64");
}

#[test]
fn test_bss_multiple_space() {
    let source = r#"
.bss
buffer1: .space 128
buffer2: .space 256
"#;

    let (text, data, bss_size) =
        assemble(source).expect("Assembly should succeed");

    assert_eq!(text.len(), 0, "Expected no text segment output");
    assert_eq!(data.len(), 0, "Expected no data segment output");
    assert_eq!(bss_size, 384, "Expected BSS size of 384 (128 + 256)");
}

#[test]
fn test_bss_rejects_byte_directive() {
    let source = r#"
.bss
.byte 0x42
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for .byte in .bss");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains(".byte") && err_msg.contains(".bss"),
        "Error should mention .byte not allowed in .bss, got: {}",
        err_msg
    );
}

#[test]
fn test_bss_rejects_string_directive() {
    let source = r#"
.bss
.string "test"
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for .string in .bss");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains(".string") && err_msg.contains(".bss"),
        "Error should mention .string not allowed in .bss, got: {}",
        err_msg
    );
}

#[test]
fn test_bss_rejects_instructions() {
    let source = r#"
.bss
add x1, x2, x3
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for instruction in .bss");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Instruction") && err_msg.contains(".bss"),
        "Error should mention instructions not allowed in .bss, got: {}",
        err_msg
    );
}

// ============================================================================
// Negative Tests: Type Mismatches
// ============================================================================

#[test]
fn test_li_rejects_address() {
    let source = r#"
.text
target:
li x1, target
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for li with address");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("li")
            && err_msg.contains("Integer")
            && err_msg.contains("Address"),
        "Error should mention type mismatch (expected Integer, got Address), got: {}",
        err_msg
    );
}

#[test]
fn test_la_rejects_integer() {
    let source = r#"
.text
la x1, 42
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for la with integer");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("la")
            && err_msg.contains("Address")
            && err_msg.contains("Integer"),
        "Error should mention type mismatch (expected Address, got Integer), got: {}",
        err_msg
    );
}

#[test]
fn test_call_rejects_integer() {
    let source = r#"
.text
call 100
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for call with integer");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("call")
            && err_msg.contains("Address")
            && err_msg.contains("Integer"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_tail_rejects_integer() {
    let source = r#"
.text
tail 200
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for tail with integer");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("tail")
            && err_msg.contains("Address")
            && err_msg.contains("Integer"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_jal_rejects_integer() {
    let source = r#"
.text
jal x1, 42
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for jal with integer");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Jump")
            && err_msg.contains("Address")
            && err_msg.contains("Integer"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_branch_rejects_integer() {
    let source = r#"
.text
beq x1, x2, 16
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for branch with integer");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Branch")
            && err_msg.contains("Address")
            && err_msg.contains("Integer"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_addi_rejects_address() {
    let source = r#"
.text
target:
addi x1, x2, target
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for addi with address");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("I-type")
            && err_msg.contains("Integer")
            && err_msg.contains("Address"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_lui_rejects_address() {
    let source = r#"
.text
target:
lui x1, target
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for lui with address");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("U-type")
            && err_msg.contains("Integer")
            && err_msg.contains("Address"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_load_offset_rejects_address() {
    let source = r#"
.text
target:
lw x1, target(x2)
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for load with address offset");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Load/Store")
            && err_msg.contains("Integer")
            && err_msg.contains("Address"),
        "Error should mention type mismatch, got: {}",
        err_msg
    );
}

// ============================================================================
// Negative Tests: Out of Range Values
// ============================================================================

#[test]
fn test_addi_immediate_out_of_range_positive() {
    let source = r#"
.text
addi x1, x2, 2048
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for addi immediate out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("2048") && err_msg.contains("12-bit"),
        "Error should mention immediate out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_addi_immediate_out_of_range_negative() {
    let source = r#"
.text
addi x1, x2, -2049
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for addi immediate out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("-2049") && err_msg.contains("12-bit"),
        "Error should mention immediate out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_lui_immediate_out_of_range() {
    let source = r#"
.text
lui x1, 0x100000
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for lui immediate out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("1048576") && err_msg.contains("20"),
        "Error should mention immediate out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_lui_negative_immediate() {
    let source = r#"
.text
lui x1, -1
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for lui with negative immediate");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("-1") && err_msg.contains("20"),
        "Error should mention immediate out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_shift_amount_out_of_range() {
    let source = r#"
.text
slli x1, x2, 64
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for shift amount out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("64")
            && (err_msg.contains("0-31") || err_msg.contains("RV32")),
        "Error should mention shift amount out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_branch_offset_out_of_range() {
    // Branch offset is 13-bit signed (±4 KiB range = ±4096 bytes)
    // beq is at offset 0, target is at .space size + 4
    // To exceed range: need target at > 4096, so .space > 4092
    // Using .space 8192 puts target at 8196, well out of range
    let source = r#"
.text
beq x1, x2, target
.space 8192
target:
nop
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for branch offset out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Branch")
            && (err_msg.contains("13-bit")
                || err_msg.contains("range")
                || err_msg.contains("4096")),
        "Error should mention branch offset out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_branch_offset_misaligned() {
    let source = r#"
.text
.global _start
_start:
beq x1, x2, target
.byte 1
target:
nop
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for misaligned branch offset");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Branch")
            && (err_msg.contains("even") || err_msg.contains("aligned")),
        "Error should mention branch offset must be even, got: {}",
        err_msg
    );
}

#[test]
fn test_jal_offset_misaligned() {
    let source = r#"
.text
.global _start
_start:
jal x1, target
.byte 1
target:
nop
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for misaligned jal offset");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Jump")
            && (err_msg.contains("even") || err_msg.contains("aligned")),
        "Error should mention jump offset must be even, got: {}",
        err_msg
    );
}

// ============================================================================
// Negative Tests: Data Directive Validation
// ============================================================================

#[test]
fn test_space_negative_size() {
    let source = r#"
.bss
.space -10
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for .space with negative size");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains(".space") && err_msg.contains("negative"),
        "Error should mention .space size cannot be negative, got: {}",
        err_msg
    );
}

// ============================================================================
// Negative Tests: Invalid Register Numbers
// ============================================================================

#[test]
fn test_register_out_of_range() {
    let source = r#"
.text
add x32, x1, x2
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for invalid register x32");
}

#[test]
fn test_load_store_offset_out_of_range() {
    let source = r#"
.text
lw x1, 2048(x2)
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for load offset out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("2048") && err_msg.contains("12-bit"),
        "Error should mention 12-bit range, got: {}",
        err_msg
    );
}

#[test]
fn test_undefined_symbol() {
    let source = r#"
.text
jal x1, undefined_label
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for undefined symbol");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("undefined") || err_msg.contains("Undefined"),
        "Error should mention undefined symbol, got: {}",
        err_msg
    );
}

#[test]
fn test_duplicate_label() {
    let source = r#"
.text
foo:
    nop
foo:
    nop
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for duplicate label");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("duplic")
            || err_msg.contains("Duplic")
            || err_msg.contains("already"),
        "Error should mention duplicate label, got: {}",
        err_msg
    );
}

#[test]
fn test_jal_offset_out_of_range() {
    // JAL (not call pseudo) offset is 21-bit signed (±1 MiB range = ±1048576 bytes)
    // Using .space to create a distance greater than 1 MiB
    // Note: call pseudo-instruction never fails because it relaxes to auipc+jalr,
    // but raw jal instruction has a fixed range
    let source = r#"
.text
jal x1, target
.space 1048580
target:
    nop
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for jal offset out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Jump")
            || err_msg.contains("range")
            || err_msg.contains("21-bit"),
        "Error should mention jump out of range, got: {}",
        err_msg
    );
}

#[test]
fn test_auipc_immediate_value() {
    // auipc should accept 20-bit immediate (but only upper 20 bits)
    let source = r#"
.text
auipc x1, 0x100000
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for auipc immediate out of range");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("1048576") && err_msg.contains("20"),
        "Error should mention 20-bit range, got: {}",
        err_msg
    );
}

#[test]
fn test_data_in_text_section() {
    // Data directives in .text should work (they're just bytes)
    let source = r#"
.text
nop
.byte 0x42
nop
"#;

    let (text, _data, _bss) =
        assemble(source).expect("Data directives in .text should be allowed");
    assert_eq!(text.len(), 9); // 4 (nop) + 1 (byte) + 4 (nop)
}

#[test]
fn test_invalid_expression() {
    let source = r#"
.text
label1:
    nop
label2:
    nop
.data
    .4byte label1 / label2
"#;

    let result = assemble(source);
    assert!(result.is_err(), "Expected error for division of addresses");
}

#[test]
fn test_call_pseudo_with_far_target() {
    // Verify that call pseudo-instruction successfully relaxes to auipc+jalr
    // for targets beyond ±1 MiB (which jal cannot reach)
    // This is a positive test showing call handles what jal cannot
    let source = r#"
.text
call target
.space 1048580
target:
    nop
"#;

    let (text, _data, _bss) = assemble(source)
        .expect("call should relax to auipc+jalr for far targets");

    // call should expand to 8-byte auipc+jalr sequence
    // Plus .space 1048580 bytes, plus 4-byte nop = 1048592 bytes total
    assert_eq!(text.len(), 1048592);

    // First 8 bytes should be auipc+jalr (0x97 for auipc, 0xe7 for jalr)
    assert_eq!(text[0], 0x97, "First instruction should be auipc");
    assert_eq!(text[4], 0xe7, "Second instruction should be jalr");
}

#[test]
fn test_tail_pseudo_with_far_target() {
    // Verify that tail pseudo-instruction successfully relaxes to auipc+jalr
    let source = r#"
.text
tail target
.space 1048580
target:
    nop
"#;

    let (text, _data, _bss) = assemble(source)
        .expect("tail should relax to auipc+jalr for far targets");

    // tail should expand to 8-byte auipc+jalr sequence
    assert_eq!(text.len(), 1048592);

    // First 8 bytes should be auipc t1 + jalr x0, t1
    // auipc t1 has opcode 0x17, jalr has opcode 0x67
    assert_eq!(
        text[0] & 0x7f,
        0x17,
        "First instruction should be auipc (opcode 0x17)"
    );
    assert_eq!(
        text[4] & 0x7f,
        0x67,
        "Second instruction should be jalr (opcode 0x67)"
    );
}

#[test]
fn test_string_escapes() {
    // Test that string directives handle escape sequences
    let source = r#"
.data
.string "hello\nworld\t\"\\"
"#;

    let (text, data, _bss) =
        assemble(source).expect("String escapes should be supported");
    assert_eq!(text.len(), 0);

    // Expected: h e l l o \n w o r l d \t " \
    let expected = &[
        0x68, 0x65, 0x6c, 0x6c, 0x6f, // hello
        0x0a, // \n
        0x77, 0x6f, 0x72, 0x6c, 0x64, // world
        0x09, // \t
        0x22, // \"
        0x5c, // \\
    ];

    assert_eq!(&data[..], expected);
}

#[test]
fn test_convergence_cascading_relaxation() {
    // Test a scenario where relaxation of one instruction affects another
    // This tests the iterative convergence loop
    let source = r#"
.text
start:
    call mid1
    nop
mid1:
    call mid2
    nop
mid2:
    call end
    nop
end:
    nop
"#;

    let (text, _data, _bss) =
        assemble(source).expect("Cascading relaxation should converge");

    // All three calls should relax to 4-byte jal
    // Total: 3x(4-byte call + 4-byte nop) + 4-byte nop = 28 bytes
    assert_eq!(text.len(), 28);

    // Each call should be encoded as jal (opcode 0x6f or 0xef)
    assert!(text[0] == 0xef || text[0] == 0x6f, "First call should be jal");
    assert!(text[8] == 0xef || text[8] == 0x6f, "Second call should be jal");
    assert!(text[16] == 0xef || text[16] == 0x6f, "Third call should be jal");
}

// ============================================================================
// Convergence/Relaxation Tests
// ============================================================================
// These tests verify that the relaxation loop correctly handles cases where
// instruction sizes change during convergence, requiring multiple passes.

#[test]
fn test_convergence_call_relaxation() {
    // Test that call gets relaxed from 8 bytes to 4 bytes
    // This requires convergence because the initial guess is 8 bytes,
    // but after encoding we discover it can be 4 bytes
    let source = r#"
.text
_start:
    call nearby_target
    nop
    nop
nearby_target:
    nop
"#;

    // call should relax to single jal (4 bytes)
    // Target at offset 12 (4-byte call + 2x 4-byte nop)
    let expected = &[
        0xef, 0x00, 0xc0, 0x00, // jal ra,12 <nearby_target>
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_convergence_tail_relaxation() {
    // Test that tail gets relaxed from 8 bytes to 4 bytes
    let source = r#"
.text
_start:
    tail nearby_target
    nop
    nop
nearby_target:
    nop
"#;

    // tail should relax to single j (4 bytes)
    // Target at offset 12 (4-byte tail + 2x 4-byte nop)
    let expected = &[
        0x6f, 0x00, 0xc0, 0x00, // j 12 <nearby_target>
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_convergence_multiple_calls() {
    // Test multiple calls that all get relaxed
    // This verifies that offsets are correctly updated when multiple
    // instructions change size
    let source = r#"
.text
_start:
    call func1
    call func2
    call func3
    nop
func1:
    nop
func2:
    nop
func3:
    nop
"#;

    // All three calls should relax to jal (4 bytes each)
    // Each call has the same offset (16 bytes) from its own PC to its target:
    // - First call at 0x0 to func1 at 0x10: offset = 0x10
    // - Second call at 0x4 to func2 at 0x14: offset = 0x10
    // - Third call at 0x8 to func3 at 0x18: offset = 0x10
    let expected = &[
        0xef, 0x00, 0x00, 0x01, // jal ra,16 <func1>
        0xef, 0x00, 0x00, 0x01, // jal ra,16 <func2>
        0xef, 0x00, 0x00, 0x01, // jal ra,16 <func3>
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop (func1)
        0x13, 0x00, 0x00, 0x00, // nop (func2)
        0x13, 0x00, 0x00, 0x00, // nop (func3)
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_convergence_forward_backward_references() {
    // Test a mix of forward and backward references
    let source = r#"
.text
func1:
    nop
    call func2
    tail func1
func2:
    call func1
    nop
"#;

    // func1 at offset 0
    // nop at offset 0
    // call func2 at offset 4, target at 12 (offset = 8)
    // tail func1 at offset 8, target at 0 (offset = -8)
    // func2 at offset 12
    // call func1 at offset 12, target at 0 (offset = -12)
    // nop at offset 16
    let expected = &[
        0x13, 0x00, 0x00, 0x00, // nop (func1)
        0xef, 0x00, 0x80, 0x00, // jal ra,8 <func2>
        0x6f, 0xf0, 0x9f, 0xff, // j -8 <func1>
        0xef, 0xf0, 0x5f, 0xff, // jal ra,-12 <func1>
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_convergence_la_with_data_section() {
    // Test that la correctly computes offset to data section
    // after all text section sizes have converged
    let source = r#"
.text
_start:
    la x5, data_label
    call func
    nop
func:
    nop
.data
data_label:
    .4byte 0x12345678
"#;

    let (text, data, _bss) = assemble(source).expect("Assembly should succeed");

    // Verify text section
    // With relax_gp enabled (default), data_label fits within ±2KiB of gp
    // la at offset 0, optimizes to addi rd, gp, offset (4 bytes via GP-relative addressing)
    // call at offset 4, relaxes to jal (4 bytes)
    // nop at offset 8 (func label)
    // nop at offset 12
    assert_eq!(text.len(), 16, "Text section should be 16 bytes");

    // Verify data section exists
    assert_eq!(data.len(), 4, "Data section should be 4 bytes");
    assert_eq!(&data[..], &[0x78, 0x56, 0x34, 0x12]);
}

#[test]
fn test_convergence_expression_with_symbols() {
    // Test that expressions using symbols work correctly after convergence
    let source = r#"
.text
start:
    call middle
    nop
middle:
    call end
    nop
end:
    nop
.data
    .4byte end - start
"#;

    let (text, data, _bss) = assemble(source).expect("Assembly should succeed");

    // All calls relax to 4 bytes
    // start at 0, middle at 8, end at 16
    // end - start = 16
    assert_eq!(text.len(), 20);
    assert_eq!(data.len(), 4);
    assert_eq!(&data[..], &[0x10, 0x00, 0x00, 0x00]); // 16 in little-endian
}

#[test]
fn test_global_pointer_optimization_enabled() {
    // Test that when __global_pointer$ is referenced, GP-relative optimization is enabled
    let source = r#"
.text
.global _start
_start:
    la a0, __global_pointer$
    nop
"#;

    let expected = &[
        0x13, 0x85, 0x01, 0x00, // addi a0, gp, 0
        0x13, 0x00, 0x00, 0x00, // nop
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_global_pointer_optimization_disabled() {
    // Test that when __global_pointer$ is NOT referenced, GP-relative optimization is disabled
    let source = r#"
.text
.global _start
_start:
    la a0, target
    nop
target:
    nop
"#;

    let (text, _, _) = assemble(source).expect("Assembly should succeed");

    // Should use auipc + addi (8 bytes for la) instead of addi rd, gp, offset (4 bytes)
    assert_eq!(
        text.len(),
        16,
        "Should use auipc + addi (8 bytes) + nop + nop = 16 bytes"
    );

    // First instruction should be auipc (opcode 0b0010111), proving no GP optimization
    assert_eq!(
        text[0] & 0x7F,
        0b0010111,
        "First instruction should be auipc, not addi rd, gp, offset"
    );
}

// ============================================================================
// Atomic Instructions (A Extension) Tests
// ============================================================================
// These tests verify encoding of RISC-V A extension atomic instructions.
// Reference encodings are from GNU riscv64-unknown-elf-as with -march=rv32ia

#[test]
fn test_lr_w() {
    // lr.w a0, (a1)
    // GNU: 1005a52f
    let source = "lr.w a0, (a1)";
    let expected = &[0x2f, 0xa5, 0x05, 0x10];
    assert_instructions_match(source, expected);
}

#[test]
fn test_lr_w_aq() {
    // lr.w.aq a0, (a1)
    // GNU: 1405a52f
    let source = "lr.w.aq a0, (a1)";
    let expected = &[0x2f, 0xa5, 0x05, 0x14];
    assert_instructions_match(source, expected);
}

#[test]
fn test_lr_w_aqrl() {
    // lr.w.aqrl a0, (a1)
    // GNU: 1605a52f
    let source = "lr.w.aqrl a0, (a1)";
    let expected = &[0x2f, 0xa5, 0x05, 0x16];
    assert_instructions_match(source, expected);
}

#[test]
fn test_sc_w() {
    // sc.w a0, a2, (a1)
    // GNU: 18c5a52f
    let source = "sc.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x18];
    assert_instructions_match(source, expected);
}

#[test]
fn test_sc_w_aq() {
    // sc.w.aq a0, a2, (a1)
    // GNU: 1cc5a52f
    let source = "sc.w.aq a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x1c];
    assert_instructions_match(source, expected);
}

#[test]
fn test_sc_w_aqrl() {
    // sc.w.aqrl a0, a2, (a1)
    // GNU: 1ec5a52f
    let source = "sc.w.aqrl a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x1e];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amoswap_w() {
    // amoswap.w a0, a2, (a1)
    // GNU: 08c5a52f
    let source = "amoswap.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x08];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amoadd_w() {
    // amoadd.w a0, a2, (a1)
    // GNU: 00c5a52f
    let source = "amoadd.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x00];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amoxor_w() {
    // amoxor.w a0, a2, (a1)
    // GNU: 20c5a52f
    let source = "amoxor.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x20];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amoand_w() {
    // amoand.w a0, a2, (a1)
    // GNU: 60c5a52f
    let source = "amoand.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x60];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amoor_w() {
    // amoor.w a0, a2, (a1)
    // GNU: 40c5a52f
    let source = "amoor.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x40];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amomin_w() {
    // amomin.w a0, a2, (a1)
    // GNU: 80c5a52f
    let source = "amomin.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0x80];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amomax_w() {
    // amomax.w a0, a2, (a1)
    // GNU: a0c5a52f
    let source = "amomax.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0xa0];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amominu_w() {
    // amominu.w a0, a2, (a1)
    // GNU: c0c5a52f
    let source = "amominu.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0xc0];
    assert_instructions_match(source, expected);
}

#[test]
fn test_amomaxu_w() {
    // amomaxu.w a0, a2, (a1)
    // GNU: e0c5a52f
    let source = "amomaxu.w a0, a2, (a1)";
    let expected = &[0x2f, 0xa5, 0xc5, 0xe0];
    assert_instructions_match(source, expected);
}

#[test]
fn test_all_atomic_instructions() {
    // Test all atomic instructions assembled together
    let source = r#"
lr.w a0, (a1)
lr.w.aq a0, (a1)
lr.w.aqrl a0, (a1)
sc.w a0, a2, (a1)
sc.w.aq a0, a2, (a1)
sc.w.aqrl a0, a2, (a1)
amoswap.w a0, a2, (a1)
amoadd.w a0, a2, (a1)
amoxor.w a0, a2, (a1)
amoand.w a0, a2, (a1)
amoor.w a0, a2, (a1)
amomin.w a0, a2, (a1)
amomax.w a0, a2, (a1)
amominu.w a0, a2, (a1)
amomaxu.w a0, a2, (a1)
"#;

    let expected = &[
        0x2f, 0xa5, 0x05, 0x10, // lr.w a0, (a1)
        0x2f, 0xa5, 0x05, 0x14, // lr.w.aq a0, (a1)
        0x2f, 0xa5, 0x05, 0x16, // lr.w.aqrl a0, (a1)
        0x2f, 0xa5, 0xc5, 0x18, // sc.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x1c, // sc.w.aq a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x1e, // sc.w.aqrl a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x08, // amoswap.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x00, // amoadd.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x20, // amoxor.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x60, // amoand.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x40, // amoor.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0x80, // amomin.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0xa0, // amomax.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0xc0, // amominu.w a0, a2, (a1)
        0x2f, 0xa5, 0xc5, 0xe0, // amomaxu.w a0, a2, (a1)
    ];

    assert_instructions_match(source, expected);
}

// ============================================================================
// RV32C Compressed Instruction Tests
// ============================================================================
// Tests for RISC-V compressed instruction extension (RV32C)
// Encodings validated against GNU riscv64-unknown-elf-as

#[test]
fn test_c_nop() {
    // c.nop (special encoding of c.addi x0, 0)
    // GNU: 0x0001
    let source = "c.nop";
    let expected = &[0x01, 0x00];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_ebreak() {
    // c.ebreak
    // GNU: 0x9002
    let source = "c.ebreak";
    let expected = &[0x02, 0x90];
    assert_instructions_match(source, expected);
}

// CR format: register-register operations
#[test]
fn test_c_add() {
    // c.add a0, a1
    // GNU: 0x952e
    let source = "c.add a0, a1";
    let expected = &[0x2e, 0x95];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_mv() {
    // c.mv a0, a1
    // GNU: 0x852e
    let source = "c.mv a0, a1";
    let expected = &[0x2e, 0x85];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_jr() {
    // c.jr ra
    // GNU: 0x8082
    let source = "c.jr ra";
    let expected = &[0x82, 0x80];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_jalr() {
    // c.jalr ra
    // GNU: 0x9082
    let source = "c.jalr ra";
    let expected = &[0x82, 0x90];
    assert_instructions_match(source, expected);
}

// CI format: register with 6-bit signed immediate
#[test]
fn test_c_li_positive() {
    // c.li a0, 5
    // GNU: 0x4515
    let source = "c.li a0, 5";
    let expected = &[0x15, 0x45];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_li_negative() {
    // c.li a0, -1
    // GNU: 0x557d
    let source = "c.li a0, -1";
    let expected = &[0x7d, 0x55];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_addi_positive() {
    // c.addi a0, 5
    // GNU: 0x0515
    let source = "c.addi a0, 5";
    let expected = &[0x15, 0x05];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_addi_negative() {
    // c.addi a0, -5
    // GNU: 0x156d
    let source = "c.addi a0, -5";
    let expected = &[0x6d, 0x15];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_slli() {
    // c.slli a0, 2
    // GNU: 0x050a
    let source = "c.slli a0, 2";
    let expected = &[0x0a, 0x05];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_addi16sp_positive() {
    // c.addi16sp sp, 16
    // GNU: 0x6141
    let source = "c.addi16sp sp, 16";
    let expected = &[0x41, 0x61];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_addi16sp_negative() {
    // c.addi16sp sp, -16
    // GNU: 0x717d
    let source = "c.addi16sp sp, -16";
    let expected = &[0x7d, 0x71];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_addi4spn() {
    // c.addi4spn s0, sp, 4
    // GNU: 0x0040
    let source = "c.addi4spn s0, sp, 4";
    let expected = &[0x40, 0x00];
    assert_instructions_match(source, expected);
}

// CA format: compressed register arithmetic
#[test]
fn test_c_and() {
    // c.and s0, s1
    // GNU: 0x8c65
    let source = "c.and s0, s1";
    let expected = &[0x65, 0x8c];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_or() {
    // c.or s0, s1
    // GNU: 0x8c45
    let source = "c.or s0, s1";
    let expected = &[0x45, 0x8c];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_xor() {
    // c.xor s0, s1
    // GNU: 0x8c25
    let source = "c.xor s0, s1";
    let expected = &[0x25, 0x8c];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_sub() {
    // c.sub s0, s1
    // GNU: 0x8c05
    let source = "c.sub s0, s1";
    let expected = &[0x05, 0x8c];
    assert_instructions_match(source, expected);
}

// CBImm format: compressed register with immediate
#[test]
fn test_c_srli() {
    // c.srli s0, 4
    // GNU: 0x8011
    let source = "c.srli s0, 4";
    let expected = &[0x11, 0x80];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_srai() {
    // c.srai s0, 4
    // GNU: 0x8411
    let source = "c.srai s0, 4";
    let expected = &[0x11, 0x84];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_andi() {
    // c.andi s0, -5
    // GNU: 0x986d
    let source = "c.andi s0, -5";
    let expected = &[0x6d, 0x98];
    assert_instructions_match(source, expected);
}

// CL/CS format: load/store with immediate offset (compressed registers)
#[test]
fn test_c_lw() {
    // c.lw s0, 4(a0)
    // GNU: 0x4140
    let source = "c.lw s0, 4(a0)";
    let expected = &[0x40, 0x41];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_sw() {
    // c.sw s1, 4(a0)
    // GNU: 0xc144
    let source = "c.sw s1, 4(a0)";
    let expected = &[0x44, 0xc1];
    assert_instructions_match(source, expected);
}

// CBBranch format: branch with PC-relative offset
#[test]
fn test_c_beqz_forward() {
    // Forward branch: c.beqz a0, forward
    // Creates: c.beqz a0, forward (2 bytes) at offset 0
    //          nop (4 bytes) at offset 2
    //          forward: nop (4 bytes) at offset 6
    // offset = 6 bytes from c.beqz to forward
    let source = r#"
c.beqz a0, forward
nop
forward:
nop
"#;
    let expected = &[
        0x19, 0xc1, // c.beqz a0, forward (offset=6)
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop (forward label)
    ];
    assert_instructions_match(source, expected);
}

#[test]
fn test_c_bnez_backward() {
    // Backward branch: loop: c.bnez a0, loop
    // Creates: nop
    //          loop: c.bnez a0, loop (offset -2 back to loop)
    //          nop
    // GNU: 0xe101
    let source = r#"
nop
loop:
c.bnez a0, loop
nop
"#;
    let expected = &[
        0x13, 0x00, 0x00, 0x00, // nop
        0x01, 0xe1, // c.bnez a0, loop (backward offset)
        0x13, 0x00, 0x00, 0x00, // nop
    ];
    assert_instructions_match(source, expected);
}

// CJ format: unconditional jump with PC-relative offset
#[test]
fn test_c_j_forward() {
    // Forward jump: c.j target
    // Creates: c.j target (2 bytes) at offset 0
    //          nop (4 bytes) at offset 2
    //          target: nop (4 bytes) at offset 6
    // offset = 6 bytes from c.j to target
    let source = r#"
c.j target
nop
target:
nop
"#;
    let expected = &[
        0x19, 0xa0, // c.j target (offset=6)
        0x13, 0x00, 0x00, 0x00, // nop
        0x13, 0x00, 0x00, 0x00, // nop (target label)
    ];
    assert_instructions_match(source, expected);
}

// Test all compressed instructions together
#[test]
fn test_all_compressed_instructions() {
    let source = r#"
c.nop
c.addi a0, 5
c.add s0, s1
c.li a1, 10
c.and s0, s1
c.or a2, a3
c.xor a4, a5
c.sub a0, a1
c.jr ra
c.ebreak
"#;

    let expected = &[
        0x01, 0x00, // c.nop
        0x15, 0x05, // c.addi a0, 5
        0x26, 0x94, // c.add s0, s1
        0xa9, 0x45, // c.li a1, 10
        0x65, 0x8c, // c.and s0, s1
        0x55, 0x8e, // c.or a2, a3
        0x3d, 0x8f, // c.xor a4, a5
        0x0d, 0x8d, // c.sub a0, a1
        0x82, 0x80, // c.jr ra
        0x02, 0x90, // c.ebreak
    ];

    assert_instructions_match(source, expected);
}

#[test]
fn test_equ_referencing_label_in_expression() {
    // Regression test for bug where .equ could not reference labels
    // in expressions. The issue was that context.current_line_pointer
    // was not being updated when evaluating .equ expressions.
    let source = r#"
.text
start_func:
    nop
    nop
end_func:
    nop

.equ func_size, end_func - start_func

_start:
    li a0, func_size
"#;

    // func_size = end_func - start_func = 0x8 - 0x0 = 8
    // li a0, 8 expands to: addi a0, zero, 8
    let expected = &[
        0x13, 0x00, 0x00, 0x00, // nop (addi zero, zero, 0)
        0x13, 0x00, 0x00, 0x00, // nop (addi zero, zero, 0)
        0x13, 0x00, 0x00, 0x00, // nop (addi zero, zero, 0)
        0x13, 0x05, 0x80, 0x00, // li a0, 8 (addi a0, zero, 8)
    ];

    assert_instructions_match(source, expected);
}
