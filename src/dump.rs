// dump.rs
//
// Debug dump functionality for the assembler.
// Provides visibility into intermediate states at various stages of assembly.

use crate::ast::*;
use crate::elf::ElfBuilder;
use crate::expressions::{self, EvaluationContext, ValueType};

// ============================================================================
// Configuration Data Structures
// ============================================================================

/// Specifies which relaxation passes to dump
#[derive(Debug, Clone, PartialEq)]
pub enum PassRange {
    Final,               // Only the final pass (default)
    Specific(usize),     // A specific pass number (1, 2, etc.)
    Range(usize, usize), // Range of passes (1-3)
    From(usize),         // From pass N to end (1-)
    UpTo(usize),         // From start to pass N (-2)
    All,                 // All passes (*)
}

/// Specifies which files to include in the dump
#[derive(Debug, Clone, PartialEq)]
pub enum FileSelection {
    All,
    Specific(Vec<String>),
}

/// Specification for value/code dumps
#[derive(Debug, Clone, PartialEq)]
pub struct DumpSpec {
    pub passes: PassRange,
    pub files: FileSelection,
}

/// Parts of ELF to dump
#[derive(Debug, Clone, PartialEq)]
pub struct ElfDumpParts {
    pub headers: bool,
    pub symbols: bool,
    pub sections: bool,
}

impl Default for ElfDumpParts {
    fn default() -> Self {
        Self { headers: true, symbols: true, sections: true }
    }
}

/// Complete dump configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DumpConfig {
    pub dump_ast: Option<DumpSpec>,
    pub dump_symbols: Option<DumpSpec>,
    pub dump_values: Option<DumpSpec>,
    pub dump_code: Option<DumpSpec>,
    pub dump_elf: Option<ElfDumpParts>,
}

impl Default for DumpConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl DumpConfig {
    pub fn new() -> Self {
        Self {
            dump_ast: None,
            dump_symbols: None,
            dump_values: None,
            dump_code: None,
            dump_elf: None,
        }
    }

    /// Returns true if any dump option is enabled
    pub fn has_dumps(&self) -> bool {
        self.dump_ast.is_some()
            || self.dump_symbols.is_some()
            || self.dump_values.is_some()
            || self.dump_code.is_some()
            || self.dump_elf.is_some()
    }
}

// ============================================================================
// Parsing Functions
// ============================================================================

/// Parse a pass range string
/// Formats: empty/"" (final), "1" (specific), "1-3" (range), "1-" (from), "-2" (up to), "*"/"all" (all)
pub fn parse_pass_range(s: &str) -> Result<PassRange, String> {
    if s.is_empty() {
        return Ok(PassRange::Final);
    }

    if s == "*" || s == "all" {
        return Ok(PassRange::All);
    }

    // Check for range patterns
    if let Some(dash_pos) = s.find('-') {
        let before = &s[..dash_pos];
        let after = &s[dash_pos + 1..];

        if before.is_empty() && !after.is_empty() {
            // "-N" format (up to N)
            let n = after
                .parse::<usize>()
                .map_err(|_| format!("Invalid pass number: {}", after))?;
            return Ok(PassRange::UpTo(n));
        } else if !before.is_empty() && after.is_empty() {
            // "N-" format (from N)
            let n = before
                .parse::<usize>()
                .map_err(|_| format!("Invalid pass number: {}", before))?;
            return Ok(PassRange::From(n));
        } else if !before.is_empty() && !after.is_empty() {
            // "N-M" format (range)
            let start = before
                .parse::<usize>()
                .map_err(|_| format!("Invalid pass number: {}", before))?;
            let end = after
                .parse::<usize>()
                .map_err(|_| format!("Invalid pass number: {}", after))?;
            if start > end {
                return Err(format!(
                    "Invalid pass range: start {} > end {}",
                    start, end
                ));
            }
            return Ok(PassRange::Range(start, end));
        }
    }

    // Try parsing as a single number
    let n = s
        .parse::<usize>()
        .map_err(|_| format!("Invalid pass specification: {}", s))?;
    Ok(PassRange::Specific(n))
}

/// Parse file selection string
/// Formats: "*" (all), "file1.s,file2.s" (specific files)
pub fn parse_file_selection(s: &str) -> FileSelection {
    if s == "*" || s.is_empty() {
        return FileSelection::All;
    }

    let files: Vec<String> = s
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();

    if files.is_empty() {
        FileSelection::All
    } else {
        FileSelection::Specific(files)
    }
}

/// Check if a file matches the selection criteria
fn matches_file_selection(
    selection: &FileSelection,
    filename: &str,
    index: usize,
) -> bool {
    match selection {
        FileSelection::All => true,
        FileSelection::Specific(files) => {
            files.iter().any(|f| f == filename || f == &format!("{}", index))
        }
    }
}

/// Parse a dump specification string "PASSES[:FILES]"
pub fn parse_dump_spec(s: &str) -> Result<DumpSpec, String> {
    if s.is_empty() {
        // Default: final pass, all files
        return Ok(DumpSpec {
            passes: PassRange::Final,
            files: FileSelection::All,
        });
    }

    let parts: Vec<&str> = s.split(':').collect();

    let passes = parse_pass_range(parts[0])?;
    let files = if parts.len() > 1 {
        parse_file_selection(parts[1])
    } else {
        FileSelection::All
    };

    Ok(DumpSpec { passes, files })
}

/// Parse ELF dump parts string
/// Formats: empty/"" (all), "headers", "headers,symbols", etc.
pub fn parse_elf_parts(s: &str) -> Result<ElfDumpParts, String> {
    if s.is_empty() || s == "all" {
        return Ok(ElfDumpParts::default());
    }

    let mut parts =
        ElfDumpParts { headers: false, symbols: false, sections: false };

    for part in s.split(',') {
        match part.trim() {
            "headers" => parts.headers = true,
            "symbols" => parts.symbols = true,
            "sections" => parts.sections = true,
            other => return Err(format!("Unknown ELF part: {}", other)),
        }
    }

    Ok(parts)
}

/// Check if a pass should be included based on PassRange
pub fn should_include_pass(
    pass: usize,
    range: &PassRange,
    is_final: bool,
) -> bool {
    match range {
        PassRange::Final => is_final,
        PassRange::Specific(n) => pass == *n,
        PassRange::Range(start, end) => pass >= *start && pass <= *end,
        PassRange::From(n) => pass >= *n,
        PassRange::UpTo(n) => pass <= *n,
        PassRange::All => true,
    }
}

/// Check if a file should be included based on FileSelection
pub fn should_include_file(file: &str, selection: &FileSelection) -> bool {
    match selection {
        FileSelection::All => true,
        FileSelection::Specific(files) => {
            // Check if file matches any in the list (basename comparison)
            let file_basename = std::path::Path::new(file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file);

            files.iter().any(|f| {
                let f_basename = std::path::Path::new(f)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(f);
                file_basename == f_basename
            })
        }
    }
}

// ============================================================================
// AST Dump (S-Expression Format)
// ============================================================================

pub fn dump_ast(source: &Source, spec: &DumpSpec) {
    println!("AST Dump:\n");

    for (i, file) in source.files.iter().enumerate() {
        if matches_file_selection(&spec.files, &file.file, i) {
            println!("File: {}", file.file);
            println!("{}", "=".repeat(79));

            let max_line_width = calculate_max_line_width_for_file(file);

            for line in &file.lines {
                let (loc_str, padding) =
                    format_location_aligned(&line.location, max_line_width);
                print!("{}{} ", loc_str, padding);
                dump_line_content_ast(&line.content);
                println!();
            }

            println!();
        }
    }
}

fn dump_line_content_ast(content: &LineContent) {
    match content {
        LineContent::Label(name) => {
            print!("(label \"{}\")", name);
        }
        LineContent::Instruction(inst) => {
            print!("{:16}", "");
            dump_instruction_ast(inst);
        }
        LineContent::Directive(dir) => {
            print!("{:16}", "");
            dump_directive_ast(dir);
        }
    }
}

fn dump_instruction_ast(inst: &Instruction) {
    match inst {
        Instruction::RType(op, rd, rs1, rs2) => {
            print!("(r-type {} {} {} {})", op, rd, rs1, rs2);
        }
        Instruction::IType(op, rd, rs1, imm) => {
            print!("(i-type {} {} {} ", op, rd, rs1);
            dump_expression_ast(imm);
            print!(")");
        }
        Instruction::BType(op, rs1, rs2, target) => {
            print!("(b-type {} {} {} ", op, rs1, rs2);
            dump_expression_ast(target);
            print!(")");
        }
        Instruction::UType(op, rd, imm) => {
            print!("(u-type {} {} ", op, rd);
            dump_expression_ast(imm);
            print!(")");
        }
        Instruction::JType(op, rd, target) => {
            print!("(j-type {} {} ", op, rd);
            dump_expression_ast(target);
            print!(")");
        }
        Instruction::Special(op) => {
            print!("(special {})", op);
        }
        Instruction::LoadStore(op, rd, offset, rs) => {
            print!("(load-store {} {} ", op, rd);
            dump_expression_ast(offset);
            print!(" {})", rs);
        }
        Instruction::Pseudo(pseudo) => {
            dump_pseudo_ast(pseudo);
        }
    }
}

fn dump_pseudo_ast(pseudo: &PseudoOp) {
    match pseudo {
        PseudoOp::Li(rd, imm) => {
            print!("(pseudo li {} ", rd);
            dump_expression_ast(imm);
            print!(")");
        }
        PseudoOp::La(rd, addr) => {
            print!("(pseudo la {} ", rd);
            dump_expression_ast(addr);
            print!(")");
        }
        PseudoOp::LoadGlobal(op, rd, addr) => {
            print!("(pseudo load-global {} {} ", op, rd);
            dump_expression_ast(addr);
            print!(")");
        }
        PseudoOp::StoreGlobal(op, rs, addr, temp) => {
            print!("(pseudo store-global {} {} ", op, rs);
            dump_expression_ast(addr);
            print!(" {})", temp);
        }
        PseudoOp::Call(target) => {
            print!("(pseudo call ");
            dump_expression_ast(target);
            print!(")");
        }
        PseudoOp::Tail(target) => {
            print!("(pseudo tail ");
            dump_expression_ast(target);
            print!(")");
        }
    }
}

fn dump_directive_ast(dir: &Directive) {
    match dir {
        Directive::Global(symbols) => {
            print!("(directive global");
            for sym in symbols {
                print!(" \"{}\"", sym);
            }
            print!(")");
        }
        Directive::Equ(name, expr) => {
            print!("(directive equ \"{}\" ", name);
            dump_expression_ast(expr);
            print!(")");
        }
        Directive::Text => print!("(directive text)"),
        Directive::Data => print!("(directive data)"),
        Directive::Bss => print!("(directive bss)"),
        Directive::Space(expr) => {
            print!("(directive space ");
            dump_expression_ast(expr);
            print!(")");
        }
        Directive::Balign(expr) => {
            print!("(directive balign ");
            dump_expression_ast(expr);
            print!(")");
        }
        Directive::String(strings) => {
            print!("(directive string");
            for s in strings {
                print!(" {:?}", s);
            }
            print!(")");
        }
        Directive::Asciz(strings) => {
            print!("(directive asciz");
            for s in strings {
                print!(" {:?}", s);
            }
            print!(")");
        }
        Directive::Byte(exprs) => {
            print!("(directive byte");
            for expr in exprs {
                print!(" ");
                dump_expression_ast(expr);
            }
            print!(")");
        }
        Directive::TwoByte(exprs) => {
            print!("(directive 2byte");
            for expr in exprs {
                print!(" ");
                dump_expression_ast(expr);
            }
            print!(")");
        }
        Directive::FourByte(exprs) => {
            print!("(directive 4byte");
            for expr in exprs {
                print!(" ");
                dump_expression_ast(expr);
            }
            print!(")");
        }
        Directive::EightByte(exprs) => {
            print!("(directive 8byte");
            for expr in exprs {
                print!(" ");
                dump_expression_ast(expr);
            }
            print!(")");
        }
    }
}

fn dump_expression_ast(expr: &Expression) {
    match expr {
        Expression::Identifier(id) => print!("(id \"{}\")", id),
        Expression::Literal(val) => print!("(lit {})", val),
        Expression::PlusOp { lhs, rhs } => {
            print!("(+ ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::MinusOp { lhs, rhs } => {
            print!("(- ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::MultiplyOp { lhs, rhs } => {
            print!("(* ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::DivideOp { lhs, rhs } => {
            print!("(/ ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::ModuloOp { lhs, rhs } => {
            print!("(% ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::LeftShiftOp { lhs, rhs } => {
            print!("(<< ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::RightShiftOp { lhs, rhs } => {
            print!("(>> ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::BitwiseOrOp { lhs, rhs } => {
            print!("(| ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::BitwiseAndOp { lhs, rhs } => {
            print!("(& ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::BitwiseXorOp { lhs, rhs } => {
            print!("(^ ");
            dump_expression_ast(lhs);
            print!(" ");
            dump_expression_ast(rhs);
            print!(")");
        }
        Expression::NegateOp { expr } => {
            print!("(neg ");
            dump_expression_ast(expr);
            print!(")");
        }
        Expression::BitwiseNotOp { expr } => {
            print!("(~ ");
            dump_expression_ast(expr);
            print!(")");
        }
        Expression::Parenthesized(expr) => {
            print!("(paren ");
            dump_expression_ast(expr);
            print!(")");
        }
        Expression::CurrentAddress => print!("(current-address)"),
        Expression::NumericLabelRef(ref_item) => {
            print!(
                "(numeric-label {} {})",
                ref_item.num,
                if ref_item.is_forward { "f" } else { "b" }
            );
        }
    }
}

// ============================================================================
// Symbol Resolution Dump
// ============================================================================

pub fn dump_symbols(source: &Source, spec: &DumpSpec) {
    println!("========== SYMBOL RESOLUTION DUMP ==========\n");

    for (i, file) in source.files.iter().enumerate() {
        if matches_file_selection(&spec.files, &file.file, i) {
            println!("File: {}", file.file);
            println!("{}", "=".repeat(79));

            let max_line_width = calculate_max_line_width_for_file(file);

            for line in file.lines.iter() {
                // Format: [file:line] content
                let (loc_str, padding) =
                    format_location_aligned(&line.location, max_line_width);
                print!("{}{} {}", loc_str, padding, line.content);

                // If this line has outgoing references, show them
                if !line.outgoing_refs.is_empty() {
                    print!("  →");
                    for (j, ref_item) in line.outgoing_refs.iter().enumerate() {
                        if j > 0 {
                            print!(",");
                        }
                        let def_file =
                            &source.files[ref_item.pointer.file_index];
                        let def_line =
                            &def_file.lines[ref_item.pointer.line_index];
                        print!(" {}@{}", ref_item.symbol, def_line.location);
                    }
                }

                println!();
            }

            println!();
        }
    }

    // Show global symbols
    if !source.global_symbols.is_empty() {
        println!("Global Symbols:");
        println!("{}", "=".repeat(79));
        for global in &source.global_symbols {
            let def_file = &source.files[global.definition_pointer.file_index];
            let def_line =
                &def_file.lines[global.definition_pointer.line_index];
            let decl_file =
                &source.files[global.declaration_pointer.file_index];
            let decl_line =
                &decl_file.lines[global.declaration_pointer.line_index];

            println!(
                "  {} → defined at {}, declared at {}",
                global.symbol, def_line.location, decl_line.location
            );
        }
        println!();
    }
}

// ============================================================================
// Symbol Values Dump
// ============================================================================

pub fn dump_values(
    pass_number: usize,
    is_final: bool,
    source: &Source,
    eval_context: &mut EvaluationContext,
    spec: &DumpSpec,
) {
    if !should_include_pass(pass_number, &spec.passes, is_final) {
        return;
    }

    println!(
        "========== SYMBOL VALUES DUMP (Pass {}{}) ==========\n",
        pass_number,
        if is_final { " - FINAL" } else { "" }
    );

    let addr_width = calculate_address_width(eval_context.text_start);

    for file in &source.files {
        if !should_include_file(&file.file, &spec.files) {
            continue;
        }

        println!("File: {}", file.file);
        println!("{}", "=".repeat(79));

        let max_line_width = calculate_max_line_width_for_file(file);

        for line in &file.lines {
            // Get absolute address
            let segment_base = get_segment_base(line.segment, eval_context);
            let abs_addr = segment_base + line.offset;

            let (loc_str, padding) =
                format_location_aligned(&line.location, max_line_width);
            print!(
                "{}{} {}: {}",
                loc_str,
                padding,
                format_address(abs_addr as u64, addr_width, line.segment),
                line.content
            );

            // Collect and show evaluated expression values
            let expr_values = collect_expression_values(line, eval_context);
            if !expr_values.is_empty() {
                print!("  # ");
                for (i, val_str) in expr_values.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    print!("{}", val_str);
                }
            }

            println!();
        }

        println!();
    }
}

// ============================================================================
// Code Generation Dump
// ============================================================================

pub fn dump_code(
    pass_number: usize,
    is_final: bool,
    source: &Source,
    eval_context: &mut EvaluationContext,
    text_bytes: &[u8],
    data_bytes: &[u8],
    spec: &DumpSpec,
) {
    if !should_include_pass(pass_number, &spec.passes, is_final) {
        return;
    }

    println!(
        "========== CODE GENERATION DUMP (Pass {}{}) ==========\n",
        pass_number,
        if is_final { " - FINAL" } else { "" }
    );

    let addr_width = calculate_address_width(eval_context.text_start);

    for file in &source.files {
        if !should_include_file(&file.file, &spec.files) {
            continue;
        }

        println!("File: {}", file.file);
        println!("{}", "=".repeat(79));

        let max_line_width = calculate_max_line_width_for_file(file);

        for line in &file.lines {
            // Get absolute address and encoded bytes
            let segment_base = get_segment_base(line.segment, eval_context);
            let abs_addr = segment_base + line.offset;
            let encoded_bytes = get_encoded_bytes(line, text_bytes, data_bytes);

            match &line.content {
                LineContent::Label(name) => {
                    // For labels, print with location and address prefix
                    let formatted_addr = format_address(
                        abs_addr as u64,
                        addr_width,
                        line.segment,
                    );
                    let (loc_str, padding) =
                        format_location_aligned(&line.location, max_line_width);
                    println!(
                        "{}{} {}: {}:",
                        loc_str, padding, formatted_addr, name
                    );
                }
                LineContent::Instruction(inst) => {
                    // For instructions: print location, address, first 4 bytes, then instruction at column 16
                    let formatted_addr = format_address(
                        abs_addr as u64,
                        addr_width,
                        line.segment,
                    );
                    let (loc_str, padding) =
                        format_location_aligned(&line.location, max_line_width);
                    // print 2 spaces to offset bytes from labels
                    print!("{}{} {}:   ", loc_str, padding, formatted_addr);

                    // Print first 4 bytes (or fewer if instruction is shorter)
                    if !encoded_bytes.is_empty() {
                        let first_chunk = &encoded_bytes
                            [..std::cmp::min(4, encoded_bytes.len())];
                        for b in first_chunk {
                            print!("{:02x} ", b);
                        }
                    }

                    // Pad to column 16 before printing instruction
                    let bytes_printed = if encoded_bytes.is_empty() {
                        2
                    } else {
                        2 + std::cmp::min(4, encoded_bytes.len()) * 3 // each byte is "xx " (3 chars)
                    };
                    let instruction_padding =
                        if bytes_printed < 16 { 16 - bytes_printed } else { 1 };
                    print!("{:<width$}", "", width = instruction_padding);
                    print!("{}", inst);

                    // If more than 4 bytes, print continuation lines with 4 bytes each
                    if encoded_bytes.len() > 4 {
                        println!();
                        for (i, chunk) in
                            encoded_bytes[4..].chunks(4).enumerate()
                        {
                            let chunk_addr = abs_addr + ((i + 1) as i64 * 4);
                            let chunk_formatted_addr = format_address(
                                chunk_addr as u64,
                                addr_width,
                                line.segment,
                            );
                            print!(
                                "{}{} {}:   ",
                                loc_str, padding, chunk_formatted_addr
                            );
                            for b in chunk {
                                print!("{:02x} ", b);
                            }
                            if i < (encoded_bytes.len() - 4).div_ceil(4) - 1 {
                                println!();
                            }
                        }
                    }
                    println!();
                }
                LineContent::Directive(_dir) => {
                    // For directives: print 8 bytes per line with proper alignment
                    let (loc_str, padding) =
                        format_location_aligned(&line.location, max_line_width);
                    if encoded_bytes.is_empty() {
                        // Directives with no encoded bytes (e.g., .text, .data, .global)
                        let formatted_addr = format_address(
                            abs_addr as u64,
                            addr_width,
                            line.segment,
                        );
                        println!(
                            "{}{} {}: {}",
                            loc_str, padding, formatted_addr, line.content
                        );
                    } else {
                        // Directives with bytes: handle alignment
                        println!(
                            "{}{} {}: {}",
                            loc_str,
                            padding,
                            format_address(
                                abs_addr as u64,
                                addr_width,
                                line.segment
                            ),
                            line.content
                        );

                        // Calculate alignment: how many bytes to print on first line to reach 8-byte alignment
                        let alignment_offset = abs_addr & 0x7; // offset within 8-byte boundary
                        let bytes_to_align = if alignment_offset == 0 {
                            8
                        } else {
                            8 - alignment_offset
                        };

                        let mut byte_offset = 0;

                        // First line: print bytes until we reach 8-byte alignment or run out
                        if byte_offset < encoded_bytes.len() {
                            let first_line_count = std::cmp::min(
                                bytes_to_align as usize,
                                encoded_bytes.len(),
                            );
                            let first_addr = abs_addr;
                            let first_formatted_addr = format_address(
                                first_addr as u64,
                                addr_width,
                                line.segment,
                            );
                            print!(
                                "{}{} {}:   ",
                                loc_str, padding, first_formatted_addr
                            );

                            // Right-flush bytes: print padding spaces first if not 8 bytes
                            if first_line_count < 8 {
                                let padding_bytes = 8 - first_line_count;
                                for _ in 0..padding_bytes {
                                    print!("   "); // 3 spaces per byte (for "xx ")
                                }
                            }

                            for b in &encoded_bytes
                                [byte_offset..byte_offset + first_line_count]
                            {
                                print!("{:02x} ", b);
                            }
                            println!();
                            byte_offset += first_line_count;
                        }

                        // Middle lines: print 8 bytes per line starting on 8-byte aligned addresses
                        while byte_offset + 8 <= encoded_bytes.len() {
                            let line_addr = abs_addr + byte_offset as i64;
                            let line_formatted_addr = format_address(
                                line_addr as u64,
                                addr_width,
                                line.segment,
                            );
                            print!(
                                "{}{} {}:   ",
                                loc_str, padding, line_formatted_addr
                            );
                            for b in
                                &encoded_bytes[byte_offset..byte_offset + 8]
                            {
                                print!("{:02x} ", b);
                            }
                            println!();
                            byte_offset += 8;
                        }

                        // Last line: print remaining bytes (if any) on next 8-byte aligned address
                        if byte_offset < encoded_bytes.len() {
                            // Round up to next 8-byte boundary for the address
                            let aligned_offset = byte_offset.div_ceil(8) * 8;
                            let last_addr = abs_addr + aligned_offset as i64;
                            let last_formatted_addr = format_address(
                                last_addr as u64,
                                addr_width,
                                line.segment,
                            );
                            print!(
                                "{}{} {}:   ",
                                loc_str, padding, last_formatted_addr
                            );
                            for b in &encoded_bytes[byte_offset..] {
                                print!("{:02x} ", b);
                            }
                            println!();
                        }
                    }
                }
            }
        }

        println!();
    }
}

// ============================================================================
// ELF Dump
// ============================================================================

pub fn dump_elf(builder: &ElfBuilder, source: &Source, parts: &ElfDumpParts) {
    println!("========== ELF DUMP ==========\n");

    if parts.headers {
        dump_elf_headers(builder);
    }

    if parts.sections {
        dump_elf_sections(builder);
    }

    if parts.symbols {
        dump_elf_symbols(builder, source);
    }
}

fn dump_elf_headers(builder: &ElfBuilder) {
    println!("ELF Header:");
    println!("{}", "-".repeat(79));

    let h = &builder.header;

    // Magic
    print!("  Magic:   ");
    for (i, &b) in h.e_ident.iter().enumerate() {
        if i > 0 && i % 16 == 0 {
            println!();
            print!("           ");
        }
        print!("{:02x} ", b);
    }
    println!();

    // Class, data, version
    println!("  Class:                           ELF64");
    println!(
        "  Data:                            2's complement, little endian"
    );
    println!("  Version:                         {}", h.e_ident[6]);
    println!("  OS/ABI:                          UNIX - System V");
    println!("  ABI Version:                     {}", h.e_ident[8]);

    // File type
    let type_str = match h.e_type {
        2 => "EXEC (Executable file)",
        _ => "Unknown",
    };
    println!("  Type:                            {}", type_str);

    // Machine
    println!("  Machine:                         RISC-V");
    println!("  Version:                         0x{:x}", h.e_version);
    println!("  Entry point address:             0x{:x}", h.e_entry);
    println!(
        "  Start of program headers:        {} (bytes into file)",
        h.e_phoff
    );
    println!(
        "  Start of section headers:        {} (bytes into file)",
        h.e_shoff
    );
    println!("  Flags:                           0x{:x}", h.e_flags);
    println!("  Size of this header:             {} (bytes)", h.e_ehsize);
    println!("  Size of program headers:         {} (bytes)", h.e_phentsize);
    println!("  Number of program headers:       {}", h.e_phnum);
    println!("  Size of section headers:         {} (bytes)", h.e_shentsize);
    println!("  Number of section headers:       {}", h.e_shnum);
    println!("  Section header string table index: {}", h.e_shstrndx);
    println!();

    // Program headers
    println!("Program Headers:");
    println!("{}", "-".repeat(79));
    println!("  Type           Offset             VirtAddr           PhysAddr");
    println!(
        "                 FileSiz            MemSiz             Flags  Align"
    );

    for ph in &builder.program_headers {
        let type_str = match ph.p_type {
            1 => "LOAD",
            0x70000003 => "RISCV_ATTRIBUTES",
            _ => "UNKNOWN",
        };

        let flags_str = format!(
            "{}{}{}",
            if ph.p_flags & 4 != 0 { "R" } else { " " },
            if ph.p_flags & 2 != 0 { "W" } else { " " },
            if ph.p_flags & 1 != 0 { "E" } else { " " }
        );

        println!(
            "  {:14} 0x{:016x} 0x{:016x} 0x{:016x}",
            type_str, ph.p_offset, ph.p_vaddr, ph.p_paddr
        );
        println!(
            "                 0x{:016x} 0x{:016x} {}  0x{:x}",
            ph.p_filesz, ph.p_memsz, flags_str, ph.p_align
        );
    }
    println!();
}

fn dump_elf_sections(builder: &ElfBuilder) {
    println!("Section Headers:");
    println!("{}", "-".repeat(79));
    println!(
        "  [Nr] Name              Type            Address          Off    Size   Flg Lk"
    );

    for (i, sh) in builder.section_headers.iter().enumerate() {
        // Get section name from string table
        let name = if sh.sh_name == 0 {
            ""
        } else {
            // Extract name from section_names string table
            let strtab = builder.section_names.data();
            let start = sh.sh_name as usize;
            if start < strtab.len() {
                let end = strtab[start..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|pos| start + pos)
                    .unwrap_or(strtab.len());
                std::str::from_utf8(&strtab[start..end]).unwrap_or("")
            } else {
                ""
            }
        };

        let type_str = match sh.sh_type {
            0 => "NULL",
            1 => "PROGBITS",
            2 => "SYMTAB",
            3 => "STRTAB",
            8 => "NOBITS",
            0x70000003 => "RISCV_ATTRIBUTES",
            _ => "UNKNOWN",
        };

        let flags_str = format!(
            "{}{}{}",
            if sh.sh_flags & 1 != 0 { "W" } else { "" },
            if sh.sh_flags & 2 != 0 { "A" } else { "" },
            if sh.sh_flags & 4 != 0 { "X" } else { "" }
        );

        println!(
            "  [{:3}] {:17} {:15} {:016x} {:06x} {:06x} {:3} {:2}",
            i,
            name,
            type_str,
            sh.sh_addr,
            sh.sh_offset,
            sh.sh_size,
            flags_str,
            sh.sh_link
        );
    }

    println!();
    println!("Key to Flags:");
    println!("  W (write), A (alloc), X (execute)");
    println!();
}

fn dump_elf_symbols(builder: &ElfBuilder, _source: &Source) {
    println!("Symbol Table:");
    println!("{}", "-".repeat(79));
    println!("  Num:    Value          Size Type    Bind   Ndx Name");

    for (i, sym) in builder.symbol_table.iter().enumerate() {
        // Get symbol name from string table
        let name = if sym.st_name == 0 {
            ""
        } else {
            let strtab = builder.symbol_names.data();
            let start = sym.st_name as usize;
            if start < strtab.len() {
                let end = strtab[start..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|pos| start + pos)
                    .unwrap_or(strtab.len());
                std::str::from_utf8(&strtab[start..end]).unwrap_or("")
            } else {
                ""
            }
        };

        let bind = sym.st_info >> 4;
        let typ = sym.st_info & 0xf;

        let bind_str = match bind {
            0 => "LOCAL",
            1 => "GLOBAL",
            _ => "UNKNOWN",
        };

        let type_str = match typ {
            0 => "NOTYPE",
            3 => "SECTION",
            4 => "FILE",
            _ => "UNKNOWN",
        };

        let ndx_str = match sym.st_shndx {
            0 => "UND".to_string(),
            0xfff1 => "ABS".to_string(),
            n => format!("{}", n),
        };

        println!(
            "  {:4}:  {:016x} {:5} {:7} {:6} {:>3} {}",
            i, sym.st_value, sym.st_size, type_str, bind_str, ndx_str, name
        );
    }

    println!();
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate the maximum line number width in a single source file for alignment purposes
fn calculate_max_line_width_for_file(file: &SourceFile) -> usize {
    let mut max_line_num = 0u32;
    for line in &file.lines {
        if line.location.line > max_line_num {
            max_line_num = line.location.line;
        }
    }

    // Count digits in max_line_num
    if max_line_num == 0 {
        1
    } else {
        ((max_line_num as f64).log10().floor() as usize) + 1
    }
}

/// Format location with alignment padding
/// Returns (formatted_location, padding_spaces) where padding_spaces aligns all locations
fn format_location_aligned(
    location: &Location,
    max_line_width: usize,
) -> (String, String) {
    let formatted = format!("[{}:{}]", location.file, location.line);
    let line_num_str = location.line.to_string();
    let current_width = line_num_str.len();
    let padding_needed = max_line_width.saturating_sub(current_width);
    let padding = " ".repeat(padding_needed);
    (formatted, padding)
}

/// Calculate the number of hex digits needed to represent an address
/// Given a base address, determines how many hex digits it needs, then adds 1.
/// Examples: 0x10000 needs 5 digits, so use 6; 0x1000000 needs 7 digits, so use 8
fn calculate_address_width(text_start: i64) -> usize {
    if text_start <= 0 {
        return 6; // Fallback: 0x0 needs 1 digit, so use 2 as minimum, but we'll use 6 for consistency
    }

    // Calculate number of hex digits needed to represent text_start
    let unsigned_addr = text_start.unsigned_abs();
    let bits_needed = 64 - unsigned_addr.leading_zeros() as usize;
    let hex_digits_for_addr = bits_needed.div_ceil(4); // Round up to nearest hex digit

    // Add 1 to the digit count as per requirements
    hex_digits_for_addr + 1
}

/// Format an address with segment suffix
/// addr_width: number of hex digits to use
/// addr: the address to format
/// segment_suffix: ".t", ".d", or ".b"
fn format_address(addr: u64, addr_width: usize, segment: Segment) -> String {
    let suffix = match segment {
        Segment::Text => ".t",
        Segment::Data => ".d",
        Segment::Bss => ".b",
    };

    format!("{:0width$x}{}", addr, suffix, width = addr_width)
}

fn get_segment_base(segment: Segment, eval_context: &EvaluationContext) -> i64 {
    match segment {
        Segment::Text => eval_context.text_start,
        Segment::Data => eval_context.data_start,
        Segment::Bss => eval_context.bss_start,
    }
}

fn get_encoded_bytes(
    line: &Line,
    text_bytes: &[u8],
    data_bytes: &[u8],
) -> Vec<u8> {
    if line.size == 0 {
        return Vec::new();
    }

    let offset = line.offset as usize;
    let size = line.size as usize;

    match line.segment {
        Segment::Text => {
            if offset + size <= text_bytes.len() {
                text_bytes[offset..offset + size].to_vec()
            } else {
                Vec::new()
            }
        }
        Segment::Data => {
            if offset + size <= data_bytes.len() {
                data_bytes[offset..offset + size].to_vec()
            } else {
                Vec::new()
            }
        }
        Segment::Bss => {
            // BSS segment has no encoded bytes (zero-initialized)
            Vec::new()
        }
    }
}

fn collect_expression_values(
    line: &Line,
    eval_context: &mut EvaluationContext,
) -> Vec<String> {
    let mut values = Vec::new();

    // Helper to format an evaluated expression value
    let mut format_value = |expr: &Expression| -> String {
        match expressions::eval_expr(expr, line, eval_context) {
            Ok(value) => match value.value_type {
                ValueType::Integer => format!("{}", value.value),
                ValueType::Address => {
                    format!("0x{:x}", value.value)
                }
            },
            Err(_) => "ERROR".to_string(),
        }
    };

    match &line.content {
        LineContent::Label(_label) => {}
        LineContent::Directive(dir) => match dir {
            Directive::Equ(_, expr) => {
                values.push(format_value(expr));
            }
            Directive::Byte(exprs)
            | Directive::TwoByte(exprs)
            | Directive::FourByte(exprs)
            | Directive::EightByte(exprs) => {
                for expr in exprs.iter() {
                    values.push(format_value(expr));
                }
            }
            Directive::Space(expr) => {
                values.push(format_value(expr));
            }
            Directive::Balign(expr) => {
                values.push(format_value(expr));
            }
            _ => {}
        },
        LineContent::Instruction(inst) => {
            // Show expressions in instruction operands
            let exprs = extract_instruction_expressions(inst);
            for expr in exprs.iter() {
                values.push(format_value(expr));
            }
        }
    }

    values
}

fn extract_instruction_expressions(inst: &Instruction) -> Vec<&Expression> {
    let mut exprs = Vec::new();

    match inst {
        Instruction::RType(..) => {}
        Instruction::IType(_, _, _, expr) => {
            exprs.push(expr.as_ref());
        }
        Instruction::BType(_, _, _, expr) => {
            exprs.push(expr.as_ref());
        }
        Instruction::UType(_, _, expr) => {
            exprs.push(expr.as_ref());
        }
        Instruction::JType(_, _, expr) => {
            exprs.push(expr.as_ref());
        }
        Instruction::LoadStore(_, _, expr, _) => {
            exprs.push(expr.as_ref());
        }
        Instruction::Special(_) => {}
        Instruction::Pseudo(pseudo) => match pseudo {
            PseudoOp::La(_, expr)
            | PseudoOp::LoadGlobal(_, _, expr)
            | PseudoOp::StoreGlobal(_, _, expr, _)
            | PseudoOp::Li(_, expr)
            | PseudoOp::Call(expr)
            | PseudoOp::Tail(expr) => {
                exprs.push(expr.as_ref());
            }
        },
    }

    exprs
}
