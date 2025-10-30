// symbols.rs
//
// This file implements the symbol linking phase for the RISC-V assembler.
// It connects symbol references to their definitions, handling local and global symbols,
// numeric labels, and cross-file references.

use crate::ast::*;
use crate::error::AssemblerError;
use std::collections::HashMap;

/// New symbol linking result structure.
/// Contains all symbol-related information extracted from the Source during
/// resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbols {
    /// Outgoing symbol references for each line, indexed by (file_index, line_index).
    ///
    pub line_refs: Vec<Vec<Vec<SymbolReference>>>,

    /// Local symbols per file, indexed by file_index.
    /// Includes non-numeric (non-local) labels
    /// and the final value of .equ symbols
    pub local_symbols_by_file: Vec<Vec<SymbolDefinition>>,

    /// Cross-file global symbols.
    /// Includes exported symbols, which must be globally unique
    /// These are the symbols that are recorded in the global section of the ELF
    /// binary
    pub global_symbols: Vec<GlobalDefinition>,
}

impl Symbols {
    /// Get the symbol references for a specific line
    pub fn get_line_refs(&self, pointer: &LinePointer) -> &[SymbolReference] {
        self.line_refs
            .get(pointer.file_index)
            .and_then(|file| file.get(pointer.line_index))
            .map(|refs| refs.as_slice())
            .unwrap_or(&[])
    }
}

/// Temporary struct for building global symbols during file processing.
#[derive(Debug, Clone)]
struct UnfinalizedGlobal {
    pub _symbol: String,
    pub definition: Option<LinePointer>,
    pub declaration_pointer: LinePointer,
}

/// Temporary struct for unresolved references during file processing.
#[derive(Debug, Clone)]
struct UnresolvedReference {
    pub symbol: String,
    pub referencing_pointer: LinePointer,
}

/// Links all symbols in the source, connecting references with definition.
/// The linking happening here is at the source/AST level, not at the concrete
/// value level.
/// Calls link_symbols to get the Symbols object, then distributes
/// the data into the Source structure for backward compatibility.
pub fn link_symbols_old(source: &mut Source) -> Result<Symbols, AssemblerError> {
    let symbols = link_symbols(source)?;

    // Distribute line references to Source structure
    for (file_index, file_refs) in symbols.line_refs.iter().enumerate() {
        for (line_index, refs) in file_refs.iter().enumerate() {
            if file_index < source.files.len()
                && line_index < source.files[file_index].lines.len()
            {
                source.files[file_index].lines[line_index].outgoing_refs =
                    refs.clone();
            }
        }
    }

    // Distribute local symbols to Source structure
    for (file_index, local_symbols) in
        symbols.local_symbols_by_file.iter().enumerate()
    {
        if file_index < source.files.len() {
            source.files[file_index].local_symbols = local_symbols.clone();
        }
    }

    // Distribute global symbols to Source structure
    source.global_symbols = symbols.global_symbols.clone();

    Ok(symbols)
}

pub fn link_symbols(
    source: &Source,
) -> Result<Symbols, AssemblerError> {
    let mut globals: HashMap<String, GlobalDefinition> = HashMap::new();
    let mut unresolved: Vec<UnresolvedReference> = Vec::new();

    // Build parallel Symbols structures
    let mut line_refs: Vec<Vec<Vec<SymbolReference>>> = Vec::new();
    let mut local_symbols_by_file: Vec<Vec<SymbolDefinition>> = Vec::new();

    for (file_index, file) in source.files.iter().enumerate() {
        let (file_globals, file_unresolved, file_local_symbols, file_line_refs) =
            link_file(file_index, file)?;

        // Store line references for this file
        line_refs.push(file_line_refs);

        // Store local symbols
        local_symbols_by_file.push(file_local_symbols);

        // Merge globals
        for gd in file_globals {
            if globals.contains_key(&gd.symbol) {
                let old_gd_pointer =
                    &globals.get(&gd.symbol).unwrap().declaration_pointer;
                let old_location = source.files[old_gd_pointer.file_index]
                    .lines[old_gd_pointer.line_index]
                    .location
                    .to_string();
                return Err(AssemblerError::from_source_pointer(
                    format!(
                        "Duplicate global symbol: {} (previously declared at {})",
                        gd.symbol, old_location,
                    ),
                    source,
                    &gd.declaration_pointer,
                ));
            }
            globals.insert(gd.symbol.clone(), gd.clone());
        }

        // Merge unresolved
        for u_r in file_unresolved {
            unresolved.push(u_r);
        }
    }

    // Now resolve cross-file references and collect final line references
    let cross_file_refs = resolve_cross_file(source, &globals, unresolved)?;

    // Merge cross-file references into line_refs
    for (file_index, line_index, sym_ref) in cross_file_refs {
        if file_index < line_refs.len()
            && line_index < line_refs[file_index].len()
        {
            line_refs[file_index][line_index].push(sym_ref);
        }
    }

    let global_symbols: Vec<GlobalDefinition> = globals.into_values().collect();

    Ok(Symbols { line_refs, local_symbols_by_file, global_symbols })
}

/// Helper function to check if a symbol is a backward numeric label reference (e.g., "1b").
fn is_numeric_backward_ref(symbol: &str) -> Option<u32> {
    if symbol.ends_with('b') && symbol.len() > 1 {
        let num_str = &symbol[..symbol.len() - 1];
        num_str.parse::<u32>().ok()
    } else {
        None
    }
}

/// Helper function to check if a symbol is a forward numeric label reference (e.g., "1f").
fn is_numeric_forward_ref(symbol: &str) -> Option<u32> {
    if symbol.ends_with('f') && symbol.len() > 1 {
        let num_str = &symbol[..symbol.len() - 1];
        num_str.parse::<u32>().ok()
    } else {
        None
    }
}

/// Helper function to flush numeric labels from definitions and return any unresolved numeric references.
fn flush_numeric_labels(
    locations: &[Location],
    definitions: &mut HashMap<String, LinePointer>,
    unresolved: &mut Vec<UnresolvedReference>,
) -> Result<(), AssemblerError> {
    // Remove numeric labels from definitions
    definitions.retain(|k, _| {
        if is_numeric_backward_ref(k).is_some() {
            false
        } else if is_numeric_forward_ref(k).is_some() {
            panic!("Forward numeric label in definitions: {}", k);
        } else {
            true
        }
    });

    // Check unresolved for numeric labels and return the first bad one
    let mut to_remove = Vec::new();
    for (i, unref) in unresolved.iter().enumerate() {
        if is_numeric_backward_ref(&unref.symbol).is_some() {
            panic!("Backward numeric label in unresolved: {}", unref.symbol);
        } else if is_numeric_forward_ref(&unref.symbol).is_some() {
            to_remove.push(i);
        }
    }
    if let Some(&i) = to_remove.first() {
        let unref = unresolved.remove(i);
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
/// Returns global definitions, unresolved references, local symbol definitions, and line references.
/// Does not mutate the SourceFile.
fn link_file(
    file_index: usize,
    file: &SourceFile,
) -> Result<
    (
        Vec<GlobalDefinition>,
        Vec<UnresolvedReference>,
        Vec<SymbolDefinition>,
        Vec<Vec<SymbolReference>>,
    ),
    AssemblerError,
> {
    let locations: Vec<Location> =
        file.lines.iter().map(|line| line.location.clone()).collect();
    let mut definitions: HashMap<String, LinePointer> = HashMap::new();
    let mut unresolved: Vec<UnresolvedReference> = Vec::new();
    let mut unfinalized_globals: HashMap<String, UnfinalizedGlobal> =
        HashMap::new();

    // Track symbol references for each line
    let mut line_outgoing_refs: Vec<Vec<SymbolReference>> =
        vec![Vec::new(); file.lines.len()];

    // Track resolved references that need to be added after the loop
    let mut patches: Vec<(usize, SymbolReference)> = Vec::new();

    for (line_index, line) in file.lines.iter().enumerate() {
        let line_ptr = LinePointer { file_index, line_index };

        // Extract symbol references from the line
        // This must happen before symbol definitions so .equ can redefine symbols
        let refs = extract_references_from_line(line);
        for symbol in refs {
            if let Some(_num) = is_numeric_backward_ref(&symbol) {
                // Backward reference
                if let Some(def_ptr) = definitions.get(&symbol) {
                    line_outgoing_refs[line_index].push(SymbolReference {
                        symbol: symbol.clone(),
                        pointer: def_ptr.clone(),
                    });
                } else {
                    // Error immediately
                    return Err(AssemblerError::from_context(
                        format!(
                            "Unresolved backward numeric label reference: {}",
                            symbol
                        ),
                        line.location.clone(),
                    ));
                }
            } else if let Some(_num) = is_numeric_forward_ref(&symbol) {
                // Forward reference
                unresolved.push(UnresolvedReference {
                    symbol: symbol.clone(),
                    referencing_pointer: line_ptr.clone(),
                });
            } else {
                // Regular symbol
                if let Some(def_ptr) = definitions.get(&symbol) {
                    line_outgoing_refs[line_index].push(SymbolReference {
                        symbol: symbol.clone(),
                        pointer: def_ptr.clone(),
                    });
                } else {
                    unresolved.push(UnresolvedReference {
                        symbol: symbol.clone(),
                        referencing_pointer: line_ptr.clone(),
                    });
                }
            }
        }

        // Handle definitions
        let mut new_definition: Option<String> = None;
        if let LineContent::Label(ref label) = line.content {
            if label.parse::<u32>().is_ok() {
                // Numeric label
                let forward_symbol = format!("{}f", label);
                // Resolve any matching forward references
                let mut i = 0;
                while i < unresolved.len() {
                    if unresolved[i].symbol == forward_symbol {
                        let unref = unresolved.remove(i);
                        // Schedule a patch to add the reference later
                        patches.push((
                            unref.referencing_pointer.line_index,
                            SymbolReference {
                                symbol: forward_symbol.clone(),
                                pointer: line_ptr.clone(),
                            },
                        ));
                    } else {
                        i += 1;
                    }
                }
                // Insert with 'b' suffix
                let backward_symbol = format!("{}b", label);
                definitions.insert(backward_symbol.clone(), line_ptr.clone());
                new_definition = Some(backward_symbol);
            } else {
                // Non-numeric label: flush numeric labels
                flush_numeric_labels(
                    &locations,
                    &mut definitions,
                    &mut unresolved,
                )?;
                // Check if label already exists
                if definitions.contains_key(label) {
                    return Err(AssemblerError::from_context(
                        format!("Duplicate label: {}", label),
                        line.location.clone(),
                    ));
                }
                definitions.insert(label.clone(), line_ptr.clone());
                new_definition = Some(label.clone());
                // Update global if present
                if let Some(global) = unfinalized_globals.get_mut(label) {
                    global.definition = Some(line_ptr.clone());
                }
            }
        } else if let LineContent::Directive(Directive::Equ(name, _)) =
            &line.content
        {
            // .equ definition
            if name.parse::<u32>().is_ok() {
                return Err(AssemblerError::from_context(
                    format!(
                        "Numeric label cannot be defined in .equ: {}",
                        name
                    ),
                    line.location.clone(),
                ));
            }
            definitions.insert(name.clone(), line_ptr.clone());
            new_definition = Some(name.clone());
            // Update global if present
            if let Some(global) = unfinalized_globals.get_mut(name) {
                global.definition = Some(line_ptr.clone());
            }
        }

        // Consolidated check and resolve unresolved references for new definition
        if let Some(sym) = new_definition {
            let mut i = 0;
            while i < unresolved.len() {
                if unresolved[i].symbol == sym {
                    let unref = unresolved.remove(i);
                    // Schedule a patch to add the reference later
                    patches.push((
                        unref.referencing_pointer.line_index,
                        SymbolReference {
                            symbol: sym.clone(),
                            pointer: line_ptr.clone(),
                        },
                    ));
                } else {
                    i += 1;
                }
            }
        }

        // Handle segment changes
        if let LineContent::Directive(
            Directive::Text | Directive::Data | Directive::Bss,
        ) = line.content
        {
            flush_numeric_labels(
                &locations,
                &mut definitions,
                &mut unresolved,
            )?;
        }

        // Handle .global declarations
        if let LineContent::Directive(Directive::Global(symbols)) =
            &line.content
        {
            for sym in symbols {
                if sym.parse::<u32>().is_ok() {
                    return Err(AssemblerError::from_context(
                        format!(
                            "Numeric label cannot be declared global: {}",
                            sym
                        ),
                        line.location.clone(),
                    ));
                }
                if unfinalized_globals.contains_key(sym) {
                    return Err(AssemblerError::from_context(
                        format!("Symbol already declared global: {}", sym),
                        line.location.clone(),
                    ));
                }
                unfinalized_globals.insert(
                    sym.clone(),
                    UnfinalizedGlobal {
                        _symbol: sym.clone(),
                        definition: definitions.get(sym).cloned(),
                        declaration_pointer: line_ptr.clone(),
                    },
                );
            }
        }
    }

    // Apply all the patches now that we're done iterating
    for (line_index, sym_ref) in patches {
        line_outgoing_refs[line_index].push(sym_ref);
    }

    // Convert unfinalized globals to GlobalDefinition with validation
    let mut global_definitions = Vec::new();
    for (symbol, ug) in unfinalized_globals {
        if ug.definition.is_none() {
            let decl_location =
                file.lines[ug.declaration_pointer.line_index].location.clone();
            return Err(AssemblerError::from_context(
                format!("Global symbol declared but not defined: {}", symbol),
                decl_location,
            ));
        }
        global_definitions.push(GlobalDefinition {
            symbol,
            definition_pointer: ug.definition.unwrap(),
            declaration_pointer: ug.declaration_pointer,
        });
    }

    // Flush any remaining numeric labels at the end of the file
    if let Some(_last_line) = file.lines.last() {
        flush_numeric_labels(&locations, &mut definitions, &mut unresolved)?;
    }

    // Convert definitions HashMap to Vec<SymbolDefinition> for local symbols
    let local_symbols: Vec<SymbolDefinition> = definitions
        .into_iter()
        .map(|(symbol, pointer)| SymbolDefinition { symbol, pointer })
        .collect();

    Ok((global_definitions, unresolved, local_symbols, line_outgoing_refs))
}

/// Extracts all symbol references from a line's AST
pub fn extract_references_from_line(line: &Line) -> Vec<String> {
    let mut refs = Vec::new();
    match &line.content {
        LineContent::Instruction(inst) => {
            // Walk expressions in instructions
            match inst {
                Instruction::RType(_, _, _, _) => {}
                Instruction::IType(_, _, _, expr) => {
                    refs.extend(extract_from_expression(expr));
                }
                Instruction::BType(_, _, _, expr) => {
                    refs.extend(extract_from_expression(expr));
                }
                Instruction::UType(_, _, expr) => {
                    refs.extend(extract_from_expression(expr));
                }
                Instruction::JType(_, _, expr) => {
                    refs.extend(extract_from_expression(expr));
                }
                Instruction::Special(_) => {}
                Instruction::LoadStore(_, _, expr, _) => {
                    refs.extend(extract_from_expression(expr));
                }
                Instruction::Atomic(_, _, _, _, _) => {
                    // Atomic instructions don't have expressions
                }
                Instruction::Compressed(_, operands) => {
                    // Extract expressions from compressed instruction operands
                    use crate::ast::CompressedOperands::*;
                    match operands {
                        CR { .. } | CRSingle { .. } | CA { .. } => {}
                        CI { imm, .. } => {
                            refs.extend(extract_from_expression(imm))
                        }
                        CIStackLoad { offset, .. } => {
                            refs.extend(extract_from_expression(offset))
                        }
                        CSSStackStore { offset, .. } => {
                            refs.extend(extract_from_expression(offset))
                        }
                        CIW { imm, .. } => {
                            refs.extend(extract_from_expression(imm))
                        }
                        CL { offset, .. } => {
                            refs.extend(extract_from_expression(offset))
                        }
                        CS { offset, .. } => {
                            refs.extend(extract_from_expression(offset))
                        }
                        CBImm { imm, .. } => {
                            refs.extend(extract_from_expression(imm))
                        }
                        CBBranch { offset, .. } => {
                            refs.extend(extract_from_expression(offset))
                        }
                        CJOpnd { offset } => {
                            refs.extend(extract_from_expression(offset))
                        }
                        crate::ast::CompressedOperands::None => {}
                    }
                }
                Instruction::Pseudo(pseudo) => match pseudo {
                    PseudoOp::Li(_, expr) => {
                        refs.extend(extract_from_expression(expr));
                    }
                    PseudoOp::La(_, expr) => {
                        refs.extend(extract_from_expression(expr));
                    }
                    PseudoOp::LoadGlobal(_, _, expr) => {
                        refs.extend(extract_from_expression(expr));
                    }
                    PseudoOp::StoreGlobal(_, _, expr, _) => {
                        refs.extend(extract_from_expression(expr));
                    }
                    PseudoOp::Call(expr) => {
                        refs.extend(extract_from_expression(expr));
                    }
                    PseudoOp::Tail(expr) => {
                        refs.extend(extract_from_expression(expr));
                    }
                },
            }
        }
        LineContent::Directive(dir) => match dir {
            Directive::Equ(_, expr) => {
                refs.extend(extract_from_expression(expr));
            }
            Directive::Space(expr) => {
                refs.extend(extract_from_expression(expr));
            }
            Directive::Balign(expr) => {
                refs.extend(extract_from_expression(expr));
            }
            Directive::Byte(exprs)
            | Directive::TwoByte(exprs)
            | Directive::FourByte(exprs) => {
                for expr in exprs {
                    refs.extend(extract_from_expression(expr));
                }
            }
            _ => {}
        },
        _ => {}
    }
    refs
}

/// Recursively extracts symbol references from an expression.
fn extract_from_expression(expr: &Expression) -> Vec<String> {
    let mut refs = Vec::new();
    match expr {
        Expression::Identifier(s) => {
            refs.push(s.clone());
        }
        Expression::Literal(_) => {}
        Expression::PlusOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::MinusOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::MultiplyOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::DivideOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::ModuloOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::LeftShiftOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::RightShiftOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::BitwiseOrOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::BitwiseAndOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::BitwiseXorOp { lhs, rhs } => {
            refs.extend(extract_from_expression(lhs));
            refs.extend(extract_from_expression(rhs));
        }
        Expression::NegateOp { expr } => {
            refs.extend(extract_from_expression(expr));
        }
        Expression::BitwiseNotOp { expr } => {
            refs.extend(extract_from_expression(expr));
        }
        Expression::Parenthesized(expr) => {
            refs.extend(extract_from_expression(expr));
        }
        Expression::CurrentAddress => {}
        Expression::NumericLabelRef(nlr) => {
            refs.push(nlr.to_string());
        }
    }
    refs
}

/// Resolves cross-file references using the global symbols map.
/// Returns a list of (file_index, line_index, SymbolReference) tuples to be added.
fn resolve_cross_file(
    source: &Source,
    globals: &HashMap<String, GlobalDefinition>,
    unresolved: Vec<UnresolvedReference>,
) -> Result<Vec<(usize, usize, SymbolReference)>, AssemblerError> {
    let mut cross_file_refs = Vec::new();

    for unref in unresolved {
        if let Some(gd) = globals.get(&unref.symbol) {
            cross_file_refs.push((
                unref.referencing_pointer.file_index,
                unref.referencing_pointer.line_index,
                SymbolReference {
                    symbol: unref.symbol.clone(),
                    pointer: gd.definition_pointer.clone(),
                },
            ));
        } else {
            // Get the location of the referencing line for the error
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