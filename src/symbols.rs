//! Symbol linking phase for the RISC-V assembler.
//!
//! This module connects symbol references to their definitions across all source files.
//! It handles:
//!
//! - **Regular symbols**: Named labels and .equ definitions
//! - **Numeric labels**: Special labels (1:, 2:, etc.) with limited scope
//! - **Local symbols**: Visible within a single file
//! - **Global symbols**: Exported via .global and visible across all files
//!
//! # Symbol Scoping Rules
//!
//! ## Regular Symbols
//! - Cannot be redefined (duplicate label error)
//! - .equ can redefine previous .equ definitions
//! - Can be declared global with .global directive
//!
//! ## Numeric Labels
//! - Can be reused (e.g., multiple "1:" labels in a file)
//! - Referenced as "Nf" (forward) or "Nb" (backward)
//! - Scope is limited: flushed when crossing non-numeric labels or segment boundaries
//! - Cannot be declared global
//!
//! # Linking Process
//!
//! The linking happens in two phases:
//!
//! 1. **Per-file linking**: Process each file independently, resolving local references
//!    and collecting global declarations
//! 2. **Cross-file linking**: Resolve references between files using the global symbol table

use crate::ast::{
    CompressedOperands, Directive, Expression, Instruction, Line, LineContent,
    LinePointer, Location, PseudoOp, Source, SourceFile,
};
use crate::error::AssemblerError;
use std::collections::HashMap;

// ==============================================================================
// Symbol Types and Constants
// ==============================================================================

/// Special global pointer symbol name used for GP-relative addressing
pub const SPECIAL_GLOBAL_POINTER: &str = "__global_pointer$";

/// Name of the builtin symbols file that's injected at the start of assembly
pub const BUILTIN_FILE_NAME: &str = "<builtin>";

/// A struct representing a symbol reference that has been linked to its definition.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SymbolReference {
    pub symbol: String,
    pub pointer: LinePointer,
}

/// A struct representing a symbol definition site in a source file.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolDefinition {
    pub symbol: String,
    pub pointer: LinePointer,
}

/// A struct representing a global symbol definition, including where it was defined and declared.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalDefinition {
    pub symbol: String,
    pub definition_pointer: LinePointer,
    pub declaration_pointer: LinePointer,
}

/// Symbol linking results for the entire source.
///
/// Contains all symbol-related information needed for later encoding phases:
/// - Line references: Which symbols does each line reference?
/// - Local symbols: Symbol definitions visible within each file
/// - Global symbols: Symbol definitions visible across all files
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLinks {
    /// List of every symbol referenced by each line of source code,
    /// indexed by [file_index][line_index].
    /// Each line can reference zero or more symbols.
    pub line_refs: Vec<Vec<Vec<SymbolReference>>>,

    /// Local symbol definitions per file, indexed by [file_index].
    /// Includes regular labels and .equ definitions that are file-local.
    /// Does not include numeric labels (they are resolved during linking).
    pub local_symbols_by_file: Vec<Vec<SymbolDefinition>>,

    /// Global symbol definitions visible across all files.
    /// These are symbols declared with .global and appear in the ELF symbol table.
    /// Each global symbol must be unique across all files.
    pub global_symbols: Vec<GlobalDefinition>,
}

impl SymbolLinks {
    /// Get the symbol references for a specific line
    pub fn get_line_refs(&self, pointer: &LinePointer) -> &[SymbolReference] {
        self.line_refs
            .get(pointer.file_index)
            .and_then(|file| file.get(pointer.line_index))
            .map(|refs| refs.as_slice())
            .unwrap_or(&[])
    }
}

/// Temporary structure for tracking global symbol declarations during file processing.
///
/// When a .global directive is encountered, we may not yet have seen the symbol's
/// definition. This struct tracks the declaration until we can finalize it.
#[derive(Debug, Clone)]
struct UnfinalizedGlobal {
    /// Location where the symbol is defined (None if not yet defined)
    pub definition: Option<LinePointer>,
    /// Location where .global directive appears
    pub declaration_pointer: LinePointer,
}

/// Temporary structure for tracking unresolved symbol references during file processing.
///
/// When a reference to a symbol is encountered before its definition, or when
/// it references a symbol in another file, we track it here for later resolution.
#[derive(Debug, Clone)]
struct UnresolvedReference {
    /// The symbol name being referenced
    pub symbol: String,
    /// Location of the line containing the reference
    pub referencing_pointer: LinePointer,
}

/// Return type for link_file function: (globals, unresolved, locals, line_refs)
type LinkFileResult = (
    Vec<GlobalDefinition>,
    Vec<UnresolvedReference>,
    Vec<SymbolDefinition>,
    Vec<Vec<SymbolReference>>,
);

/// Links symbols across all source files.
///
/// This is the main entry point for symbol linking. It performs a two-phase process:
///
/// Phase 1 - Per-File Linking:
/// - Process each file independently
/// - Resolve local symbol references within each file
/// - Collect global symbol declarations
/// - Gather unresolved references for cross-file resolution
///
/// Phase 2 - Cross-File Linking:
/// - Build a unified global symbols table
/// - Resolve remaining references using global symbols
/// - Report errors for duplicate globals and undefined symbols
pub fn link_symbols(source: &Source) -> Result<SymbolLinks, AssemblerError> {
    let mut globals: HashMap<String, GlobalDefinition> = HashMap::new();
    let mut all_unresolved: Vec<UnresolvedReference> = Vec::new();
    let mut line_refs: Vec<Vec<Vec<SymbolReference>>> = Vec::new();
    let mut local_symbols_by_file: Vec<Vec<SymbolDefinition>> = Vec::new();

    // Phase 1: Process each file independently
    for (file_index, file) in source.files.iter().enumerate() {
        let (file_globals, file_unresolved, file_local_symbols, file_line_refs) =
            link_file(file_index, file)?;

        line_refs.push(file_line_refs);
        local_symbols_by_file.push(file_local_symbols);

        // Merge global symbols, checking for duplicates
        for global_def in file_globals {
            if let Some(existing) = globals.get(&global_def.symbol) {
                let old_location = source.files
                    [existing.declaration_pointer.file_index]
                    .lines[existing.declaration_pointer.line_index]
                    .location
                    .to_string();
                return Err(AssemblerError::from_source_pointer(
                    format!(
                        "Duplicate global symbol: {} (previously declared at {})",
                        global_def.symbol, old_location
                    ),
                    source,
                    &global_def.declaration_pointer,
                ));
            }
            globals.insert(global_def.symbol.clone(), global_def);
        }

        // Accumulate unresolved references
        all_unresolved.extend(file_unresolved);
    }

    // Phase 2: Resolve cross-file references
    let cross_file_refs = link_cross_file(source, &globals, all_unresolved)?;

    // Merge cross-file references into the line references
    for (file_index, line_index, sym_ref) in cross_file_refs {
        line_refs[file_index][line_index].push(sym_ref);
    }

    Ok(SymbolLinks {
        line_refs,
        local_symbols_by_file,
        global_symbols: globals.into_values().collect(),
    })
}

/// Checks if a symbol is a backward numeric label reference (e.g., "1b").
/// Returns the numeric value if it's a valid backward reference.
fn is_numeric_backward_ref(symbol: &str) -> Option<u32> {
    symbol.strip_suffix('b').and_then(|num_str| num_str.parse::<u32>().ok())
}

/// Checks if a symbol is a forward numeric label reference (e.g., "1f").
/// Returns the numeric value if it's a valid forward reference.
fn is_numeric_forward_ref(symbol: &str) -> Option<u32> {
    symbol.strip_suffix('f').and_then(|num_str| num_str.parse::<u32>().ok())
}

/// Flushes numeric labels when crossing a non-numeric label or segment boundary.
///
/// Numeric labels (e.g., "1:", "2:") have limited scope and are cleared when:
/// - A non-numeric label is encountered
/// - A segment boundary (.text, .data, .bss) is crossed
///
/// This function removes all backward references ("1b", "2b") from the definitions
/// and checks for any unresolved forward references ("1f", "2f"). If any forward
/// references remain unresolved, it returns an error for the first one found.
fn flush_numeric_labels(
    locations: &[Location],
    definitions: &mut HashMap<String, LinePointer>,
    unresolved: &mut Vec<UnresolvedReference>,
) -> Result<(), AssemblerError> {
    // Remove all backward numeric label definitions (e.g., "1b", "2b")
    definitions.retain(|symbol, _| is_numeric_backward_ref(symbol).is_none());

    // Find the first unresolved forward numeric reference, if any
    if let Some(pos) = unresolved
        .iter()
        .position(|unref| is_numeric_forward_ref(&unref.symbol).is_some())
    {
        let unref = unresolved.remove(pos);
        let error_location =
            locations[unref.referencing_pointer.line_index].clone();
        return Err(AssemblerError::from_context(
            format!("Unresolved numeric label reference: {}", unref.symbol),
            error_location,
        ));
    }

    Ok(())
}

/// Processes a single file for symbol linking.
///
/// This function performs a single-pass scan through the file, handling:
/// - Symbol references (backward, forward, and numeric labels)
/// - Symbol definitions (labels and .equ directives)
/// - Global symbol declarations
/// - Numeric label scoping (flushed at non-numeric labels and segment boundaries)
///
/// Returns:
/// - Global definitions exported from this file
/// - Unresolved references (to be resolved cross-file)
/// - Local symbol definitions
/// - Line-by-line symbol references
fn link_file(
    file_index: usize,
    file: &SourceFile,
) -> Result<LinkFileResult, AssemblerError> {
    // Precompute locations for error reporting
    let locations: Vec<Location> =
        file.lines.iter().map(|line| line.location.clone()).collect();

    // Symbol tracking state
    let mut definitions: HashMap<String, LinePointer> = HashMap::new();
    let mut unresolved: Vec<UnresolvedReference> = Vec::new();
    let mut unfinalized_globals: HashMap<String, UnfinalizedGlobal> =
        HashMap::new();
    let mut line_outgoing_refs: Vec<Vec<SymbolReference>> =
        vec![Vec::new(); file.lines.len()];

    // Deferred patches for forward references that get resolved during the pass.
    // We can't modify line_outgoing_refs while iterating, so we collect patches
    // and apply them afterward.
    let mut patches: Vec<(usize, SymbolReference)> = Vec::new();

    // Single-pass processing
    for (line_index, line) in file.lines.iter().enumerate() {
        let line_ptr = LinePointer { file_index, line_index };

        // Phase 1: Extract and resolve symbol references
        process_symbol_references(
            line,
            &line_ptr,
            &definitions,
            &mut unresolved,
            &mut line_outgoing_refs[line_index],
        )?;

        // Phase 2: Handle symbol definitions
        let new_definition = process_symbol_definitions(
            line,
            &line_ptr,
            &locations,
            &mut definitions,
            &mut unresolved,
            &mut unfinalized_globals,
            &mut patches,
        )?;

        // Phase 3: Resolve any forward references to the newly defined symbol
        if let Some(symbol) = new_definition {
            resolve_forward_references(
                &symbol,
                &line_ptr,
                &mut unresolved,
                &mut patches,
            );
        }

        // Phase 4: Handle segment boundaries (flush numeric labels)
        if matches!(
            line.content,
            LineContent::Directive(
                Directive::Text | Directive::Data | Directive::Bss
            )
        ) {
            flush_numeric_labels(
                &locations,
                &mut definitions,
                &mut unresolved,
            )?;
        }

        // Phase 5: Handle .global declarations
        if let LineContent::Directive(Directive::Global(symbols)) =
            &line.content
        {
            process_global_declarations(
                symbols,
                &line_ptr,
                &line.location,
                &definitions,
                &mut unfinalized_globals,
            )?;
        }
    }

    // Apply all deferred patches
    for (line_index, sym_ref) in patches {
        line_outgoing_refs[line_index].push(sym_ref);
    }

    // Finalize globals and validate they have definitions
    let global_definitions = finalize_globals(unfinalized_globals, file)?;

    // Flush remaining numeric labels at end of file
    if file.lines.last().is_some() {
        flush_numeric_labels(&locations, &mut definitions, &mut unresolved)?;
    }

    // Convert local definitions to output format
    let local_symbols: Vec<SymbolDefinition> = definitions
        .into_iter()
        .map(|(symbol, pointer)| SymbolDefinition { symbol, pointer })
        .collect();

    Ok((global_definitions, unresolved, local_symbols, line_outgoing_refs))
}

/// Processes symbol references found in a line.
///
/// Handles three types of references:
/// - Backward numeric references ("1b"): Must already be defined
/// - Forward numeric references ("1f"): Added to unresolved list
/// - Regular symbols: Resolved if defined, otherwise added to unresolved list
fn process_symbol_references(
    line: &Line,
    line_ptr: &LinePointer,
    definitions: &HashMap<String, LinePointer>,
    unresolved: &mut Vec<UnresolvedReference>,
    outgoing_refs: &mut Vec<SymbolReference>,
) -> Result<(), AssemblerError> {
    let symbols = extract_references_from_line(line);

    for symbol in symbols {
        if is_numeric_backward_ref(&symbol).is_some() {
            // Backward numeric reference must already exist
            if let Some(def_ptr) = definitions.get(&symbol) {
                outgoing_refs
                    .push(SymbolReference { symbol, pointer: def_ptr.clone() });
            } else {
                return Err(AssemblerError::from_context(
                    format!(
                        "Unresolved backward numeric label reference: {}",
                        symbol
                    ),
                    line.location.clone(),
                ));
            }
        } else if is_numeric_forward_ref(&symbol).is_some() {
            // Forward numeric reference to be resolved later
            unresolved.push(UnresolvedReference {
                symbol,
                referencing_pointer: line_ptr.clone(),
            });
        } else {
            // Regular symbol reference
            if let Some(def_ptr) = definitions.get(&symbol) {
                outgoing_refs
                    .push(SymbolReference { symbol, pointer: def_ptr.clone() });
            } else {
                unresolved.push(UnresolvedReference {
                    symbol,
                    referencing_pointer: line_ptr.clone(),
                });
            }
        }
    }

    Ok(())
}

/// Processes symbol definitions (labels and .equ directives).
///
/// Returns the newly defined symbol name, if any.
fn process_symbol_definitions(
    line: &Line,
    line_ptr: &LinePointer,
    locations: &[Location],
    definitions: &mut HashMap<String, LinePointer>,
    unresolved: &mut Vec<UnresolvedReference>,
    unfinalized_globals: &mut HashMap<String, UnfinalizedGlobal>,
    patches: &mut Vec<(usize, SymbolReference)>,
) -> Result<Option<String>, AssemblerError> {
    match &line.content {
        LineContent::Label(label) => {
            if label.parse::<u32>().is_ok() {
                // Numeric label (e.g., "1:")
                process_numeric_label(
                    label,
                    line_ptr,
                    unresolved,
                    definitions,
                    patches,
                )
            } else {
                // Non-numeric label
                process_regular_label(
                    label,
                    line_ptr,
                    &line.location,
                    locations,
                    definitions,
                    unresolved,
                    unfinalized_globals,
                )
            }
        }
        LineContent::Directive(Directive::Equ(name, _)) => {
            process_equ_definition(
                name,
                line_ptr,
                &line.location,
                definitions,
                unfinalized_globals,
            )
        }
        _ => Ok(None),
    }
}

/// Processes a numeric label definition (e.g., "1:").
fn process_numeric_label(
    label: &str,
    line_ptr: &LinePointer,
    unresolved: &mut Vec<UnresolvedReference>,
    definitions: &mut HashMap<String, LinePointer>,
    patches: &mut Vec<(usize, SymbolReference)>,
) -> Result<Option<String>, AssemblerError> {
    let forward_symbol = format!("{}f", label);
    let backward_symbol = format!("{}b", label);

    // Resolve all forward references to this numeric label
    unresolved.retain(|unref| {
        if unref.symbol == forward_symbol {
            patches.push((
                unref.referencing_pointer.line_index,
                SymbolReference {
                    symbol: forward_symbol.clone(),
                    pointer: line_ptr.clone(),
                },
            ));
            false // Remove from unresolved
        } else {
            true // Keep in unresolved
        }
    });

    // Define the backward reference for future use
    definitions.insert(backward_symbol.clone(), line_ptr.clone());
    Ok(Some(backward_symbol))
}

/// Processes a regular (non-numeric) label definition.
fn process_regular_label(
    label: &str,
    line_ptr: &LinePointer,
    line_location: &Location,
    locations: &[Location],
    definitions: &mut HashMap<String, LinePointer>,
    unresolved: &mut Vec<UnresolvedReference>,
    unfinalized_globals: &mut HashMap<String, UnfinalizedGlobal>,
) -> Result<Option<String>, AssemblerError> {
    // Non-numeric labels flush all numeric label scopes
    flush_numeric_labels(locations, definitions, unresolved)?;

    // Check for duplicate label
    if definitions.contains_key(label) {
        return Err(AssemblerError::from_context(
            format!("Duplicate label: {}", label),
            line_location.clone(),
        ));
    }

    // Define the label
    definitions.insert(label.to_string(), line_ptr.clone());

    // Update global definition pointer if this symbol was declared global
    if let Some(global) = unfinalized_globals.get_mut(label) {
        global.definition = Some(line_ptr.clone());
    }

    Ok(Some(label.to_string()))
}

/// Processes an .equ directive definition.
fn process_equ_definition(
    name: &str,
    line_ptr: &LinePointer,
    line_location: &Location,
    definitions: &mut HashMap<String, LinePointer>,
    unfinalized_globals: &mut HashMap<String, UnfinalizedGlobal>,
) -> Result<Option<String>, AssemblerError> {
    // .equ cannot define numeric labels
    if name.parse::<u32>().is_ok() {
        return Err(AssemblerError::from_context(
            format!("Numeric label cannot be defined in .equ: {}", name),
            line_location.clone(),
        ));
    }

    // .equ can redefine existing symbols (including previous .equ definitions)
    definitions.insert(name.to_string(), line_ptr.clone());

    // Update global definition pointer if this symbol was declared global
    if let Some(global) = unfinalized_globals.get_mut(name) {
        global.definition = Some(line_ptr.clone());
    }

    Ok(Some(name.to_string()))
}

/// Resolves forward references to a newly defined symbol.
fn resolve_forward_references(
    symbol: &str,
    definition_ptr: &LinePointer,
    unresolved: &mut Vec<UnresolvedReference>,
    patches: &mut Vec<(usize, SymbolReference)>,
) {
    unresolved.retain(|unref| {
        if unref.symbol == symbol {
            patches.push((
                unref.referencing_pointer.line_index,
                SymbolReference {
                    symbol: symbol.to_string(),
                    pointer: definition_ptr.clone(),
                },
            ));
            false // Remove from unresolved
        } else {
            true // Keep in unresolved
        }
    });
}

/// Processes .global declarations.
fn process_global_declarations(
    symbols: &[String],
    line_ptr: &LinePointer,
    line_location: &Location,
    definitions: &HashMap<String, LinePointer>,
    unfinalized_globals: &mut HashMap<String, UnfinalizedGlobal>,
) -> Result<(), AssemblerError> {
    for symbol in symbols {
        // Cannot declare numeric labels as global
        if symbol.parse::<u32>().is_ok() {
            return Err(AssemblerError::from_context(
                format!("Numeric label cannot be declared global: {}", symbol),
                line_location.clone(),
            ));
        }

        // Cannot declare the same symbol global twice
        if unfinalized_globals.contains_key(symbol) {
            return Err(AssemblerError::from_context(
                format!("Symbol already declared global: {}", symbol),
                line_location.clone(),
            ));
        }

        // Record the global declaration
        unfinalized_globals.insert(
            symbol.clone(),
            UnfinalizedGlobal {
                definition: definitions.get(symbol).cloned(),
                declaration_pointer: line_ptr.clone(),
            },
        );
    }

    Ok(())
}

/// Finalizes global symbols and validates they all have definitions.
fn finalize_globals(
    unfinalized_globals: HashMap<String, UnfinalizedGlobal>,
    file: &SourceFile,
) -> Result<Vec<GlobalDefinition>, AssemblerError> {
    let mut global_definitions = Vec::new();

    for (symbol, ug) in unfinalized_globals {
        let Some(definition_pointer) = ug.definition else {
            let decl_location =
                file.lines[ug.declaration_pointer.line_index].location.clone();
            return Err(AssemblerError::from_context(
                format!("Global symbol declared but not defined: {}", symbol),
                decl_location,
            ));
        };

        global_definitions.push(GlobalDefinition {
            symbol,
            definition_pointer,
            declaration_pointer: ug.declaration_pointer,
        });
    }

    Ok(global_definitions)
}

/// Extracts all symbol references from a line's AST.
///
/// Scans the line's content (instruction or directive) and collects all symbol
/// references found in expressions. This includes:
/// - Regular symbol names (e.g., "main", "loop")
/// - Numeric label references (e.g., "1f", "2b")
///
/// Does not include symbol definitions (labels, .equ names, etc.).
pub fn extract_references_from_line(line: &Line) -> Vec<String> {
    let mut refs = Vec::new();

    match &line.content {
        LineContent::Instruction(inst) => {
            extract_refs_from_instruction(inst, &mut refs);
        }
        LineContent::Directive(dir) => {
            extract_refs_from_directive(dir, &mut refs);
        }
        _ => {}
    }

    refs
}

/// Extracts symbol references from an instruction.
fn extract_refs_from_instruction(inst: &Instruction, refs: &mut Vec<String>) {
    match inst {
        // Instructions with no expressions
        Instruction::RType(_, _, _, _)
        | Instruction::Special(_)
        | Instruction::Atomic(_, _, _, _, _) => {}

        // Instructions with a single expression
        Instruction::IType(_, _, _, expr)
        | Instruction::BType(_, _, _, expr)
        | Instruction::UType(_, _, expr)
        | Instruction::JType(_, _, expr)
        | Instruction::LoadStore(_, _, expr, _) => {
            refs.extend(extract_from_expression(expr));
        }

        // Compressed instructions
        Instruction::Compressed(_, operands) => {
            extract_refs_from_compressed_operands(operands, refs);
        }

        // Pseudo-instructions
        Instruction::Pseudo(pseudo) => {
            extract_refs_from_pseudo(pseudo, refs);
        }
    }
}

/// Extracts symbol references from compressed instruction operands.
fn extract_refs_from_compressed_operands(
    operands: &CompressedOperands,
    refs: &mut Vec<String>,
) {
    match operands {
        // Operands with no expressions
        CompressedOperands::CR { .. }
        | CompressedOperands::CRSingle { .. }
        | CompressedOperands::CA { .. }
        | CompressedOperands::None => {}

        // Operands with expressions
        CompressedOperands::CI { imm, .. }
        | CompressedOperands::CIStackLoad { offset: imm, .. }
        | CompressedOperands::CSSStackStore { offset: imm, .. }
        | CompressedOperands::CIW { imm, .. }
        | CompressedOperands::CL { offset: imm, .. }
        | CompressedOperands::CS { offset: imm, .. }
        | CompressedOperands::CBImm { imm, .. }
        | CompressedOperands::CBBranch { offset: imm, .. }
        | CompressedOperands::CJOpnd { offset: imm } => {
            refs.extend(extract_from_expression(imm));
        }
    }
}

/// Extracts symbol references from pseudo-instructions.
fn extract_refs_from_pseudo(pseudo: &PseudoOp, refs: &mut Vec<String>) {
    match pseudo {
        PseudoOp::Li(_, expr)
        | PseudoOp::La(_, expr)
        | PseudoOp::LoadGlobal(_, _, expr)
        | PseudoOp::Call(expr)
        | PseudoOp::Tail(expr) => {
            refs.extend(extract_from_expression(expr));
        }
        PseudoOp::StoreGlobal(_, _, expr, _) => {
            refs.extend(extract_from_expression(expr));
        }
    }
}

/// Extracts symbol references from a directive.
fn extract_refs_from_directive(dir: &Directive, refs: &mut Vec<String>) {
    match dir {
        // Directives with a single expression
        Directive::Equ(_, expr)
        | Directive::Space(expr)
        | Directive::Balign(expr) => {
            refs.extend(extract_from_expression(expr));
        }

        // Directives with multiple expressions
        Directive::Byte(exprs)
        | Directive::TwoByte(exprs)
        | Directive::FourByte(exprs) => {
            for expr in exprs {
                refs.extend(extract_from_expression(expr));
            }
        }

        // Directives with no expressions
        _ => {}
    }
}

/// Recursively extracts symbol references from an expression tree.
///
/// Traverses the expression AST and collects all identifiers and numeric label
/// references (e.g., "1f", "2b"). Literals and the current address marker (".")
/// are not considered symbol references.
fn extract_from_expression(expr: &Expression) -> Vec<String> {
    let mut refs = Vec::new();

    match expr {
        // Base cases: symbols
        Expression::Identifier(symbol) => {
            refs.push(symbol.clone());
        }
        Expression::NumericLabelRef(label_ref) => {
            refs.push(label_ref.to_string());
        }

        // Base cases: non-symbols
        Expression::Literal(_) | Expression::CurrentAddress => {}

        // Binary operations: recurse on both operands
        Expression::PlusOp { lhs, rhs }
        | Expression::MinusOp { lhs, rhs }
        | Expression::MultiplyOp { lhs, rhs }
        | Expression::DivideOp { lhs, rhs }
        | Expression::ModuloOp { lhs, rhs }
        | Expression::LeftShiftOp { lhs, rhs }
        | Expression::RightShiftOp { lhs, rhs }
        | Expression::BitwiseOrOp { lhs, rhs }
        | Expression::BitwiseAndOp { lhs, rhs }
        | Expression::BitwiseXorOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }

        // Unary operations: recurse on operand
        Expression::NegateOp { expr }
        | Expression::BitwiseNotOp { expr }
        | Expression::Parenthesized(expr) => {
            refs.extend(extract_from_expression(expr));
        }
    }

    refs
}

/// Links cross-file symbol references using the global symbols table.
///
/// After all files have been processed individually, any remaining unresolved
/// references must be resolved against global symbols. This function:
/// - Matches unresolved references with global symbol definitions
/// - Returns cross-file references as tuples: (file_index, line_index, SymbolReference)
/// - Reports errors for any truly undefined symbols
fn link_cross_file(
    source: &Source,
    globals: &HashMap<String, GlobalDefinition>,
    unresolved: Vec<UnresolvedReference>,
) -> Result<Vec<(usize, usize, SymbolReference)>, AssemblerError> {
    let mut cross_file_refs = Vec::new();

    for unref in unresolved {
        if let Some(global_def) = globals.get(&unref.symbol) {
            // Found a global definition for this reference
            cross_file_refs.push((
                unref.referencing_pointer.file_index,
                unref.referencing_pointer.line_index,
                SymbolReference {
                    symbol: unref.symbol,
                    pointer: global_def.definition_pointer.clone(),
                },
            ));
        } else {
            // Symbol is truly undefined
            let file = &source.files[unref.referencing_pointer.file_index];
            let line = &file.lines[unref.referencing_pointer.line_index];
            return Err(AssemblerError::from_context(
                format!("Undefined symbol: {}", unref.symbol),
                line.location.clone(),
            ));
        }
    }

    Ok(cross_file_refs)
}

// ==============================================================================
// Builtin Symbols File Generation
// ==============================================================================

/// Creates a synthetic source file containing builtin symbol definitions.
/// This file is appended to the source file list after parsing and provides
/// definitions for linker-provided symbols like __global_pointer$.
///
/// The builtin file contains:
/// - .data directive to switch to data segment
/// - .global declaration for __global_pointer$
/// - Label definition for __global_pointer$ at offset 2048 (data_start + 0x800)
///
/// This file is excluded from normal processing in several places:
/// - compute_offsets: skipped to preserve hardcoded offset
/// - convergence loop: skipped as it generates no code
/// - ELF symbol table: filtered out, symbols emitted specially
/// - dump output: hidden from user
pub fn create_builtin_symbols_file() -> SourceFile {
    SourceFile {
        file: BUILTIN_FILE_NAME.to_string(),
        lines: vec![
            // .data directive
            Line {
                location: Location {
                    file: BUILTIN_FILE_NAME.to_string(),
                    line: 1,
                },
                content: LineContent::Directive(Directive::Data),
            },
            // .global __global_pointer$
            Line {
                location: Location {
                    file: BUILTIN_FILE_NAME.to_string(),
                    line: 2,
                },
                content: LineContent::Directive(Directive::Global(vec![
                    SPECIAL_GLOBAL_POINTER.to_string(),
                ])),
            },
            // __global_pointer$: label at offset 2048
            Line {
                location: Location {
                    file: BUILTIN_FILE_NAME.to_string(),
                    line: 3,
                },
                content: LineContent::Label(SPECIAL_GLOBAL_POINTER.to_string()),
            },
        ],
    }
}
