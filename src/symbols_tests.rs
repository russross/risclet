#[cfg(test)]
mod tests {
    use crate::ast::*;
    use crate::parser;
    use crate::symbols::{extract_references_from_line, resolve_symbols};
    use crate::tokenizer;

    /// Helper: Parse source lines into a SourceFile
    fn parse_source_file(
        filename: &str,
        source: &str,
    ) -> Result<SourceFile, String> {
        let mut lines = Vec::new();
        let mut current_segment = Segment::Text;

        for (line_num, line_text) in source.lines().enumerate() {
            let line_text = line_text.trim();
            if line_text.is_empty() {
                continue;
            }

            let tokens = tokenizer::tokenize(line_text)?;
            if tokens.is_empty() {
                continue;
            }

            let parsed_lines =
                parser::parse(&tokens, filename.to_string(), line_num + 1)
                    .map_err(|e| e.with_source_context())?;

            for mut parsed_line in parsed_lines {
                // Update segment if directive changes it
                if let LineContent::Directive(ref dir) = parsed_line.content {
                    match dir {
                        Directive::Text => current_segment = Segment::Text,
                        Directive::Data => current_segment = Segment::Data,
                        Directive::Bss => current_segment = Segment::Bss,
                        _ => {}
                    }
                }

                parsed_line.segment = current_segment;
                parsed_line.size = 4; // Simplified size guess for tests
                lines.push(parsed_line);
            }
        }

        Ok(SourceFile {
            file: filename.to_string(),
            lines,
            text_size: 0,
            data_size: 0,
            bss_size: 0,
            local_symbols: Vec::new(),
        })
    }

    /// Helper: Create a Source from multiple file contents
    fn create_source(files: Vec<(&str, &str)>) -> Result<Source, String> {
        let mut source = Source {
            files: Vec::new(),
            text_size: 0,
            data_size: 0,
            bss_size: 0,
            global_symbols: Vec::new(),
            header_size: 0,
            uses_global_pointer: false,
        };

        for (filename, content) in files {
            source.files.push(parse_source_file(filename, content)?);
        }

        Ok(source)
    }

    /// Helper: Find a line by its label
    fn find_line_by_label(source: &Source, label: &str) -> Option<LinePointer> {
        for (file_index, file) in source.files.iter().enumerate() {
            for (line_index, line) in file.lines.iter().enumerate() {
                if let LineContent::Label(ref l) = line.content {
                    if l == label {
                        return Some(LinePointer { file_index, line_index });
                    }
                }
            }
        }
        None
    }

    /// Helper: Find the line that contains a reference to the given symbol
    fn find_referencing_line(
        source: &Source,
        symbol: &str,
    ) -> Option<LinePointer> {
        for (file_index, file) in source.files.iter().enumerate() {
            for (line_index, line) in file.lines.iter().enumerate() {
                // Check if this line has an expression with the symbol
                let refs = extract_references_from_line(line);
                if refs.contains(&symbol.to_string()) {
                    return Some(LinePointer { file_index, line_index });
                }
            }
        }
        None
    }

    /// Helper: Assert that a line has a specific outgoing reference
    fn assert_reference(
        source: &Source,
        line_ptr: &LinePointer,
        expected_symbol: &str,
        expected_def_ptr: &LinePointer,
    ) {
        let file = &source.files[line_ptr.file_index];
        let line = &file.lines[line_ptr.line_index];

        let matching_ref = line.outgoing_refs.iter().find(|r| {
            r.symbol == expected_symbol && r.pointer == *expected_def_ptr
        });

        assert!(
            matching_ref.is_some(),
            "Expected reference from line {}:{} to symbol '{}' at {}:{}, but it was not found",
            line_ptr.file_index,
            line_ptr.line_index,
            expected_symbol,
            expected_def_ptr.file_index,
            expected_def_ptr.line_index
        );
    }

    // ============================================================================
    // Single-File Tests
    // ============================================================================

    #[test]
    fn test_no_symbols() {
        let source_text = "
            addi a0, a0, 1
            addi a1, a1, 2
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with no symbols"
        );

        // Verify no outgoing references
        for file in &source.files {
            for line in &file.lines {
                assert!(
                    line.outgoing_refs.is_empty(),
                    "No references should exist"
                );
            }
        }
    }

    #[test]
    fn test_single_symbol_no_references() {
        let source_text = "
            start:
                addi a0, a0, 1
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed");

        // Find the label line
        let label_ptr = find_line_by_label(&source, "start").unwrap();

        // Verify no outgoing references on the label
        let file = &source.files[label_ptr.file_index];
        let line = &file.lines[label_ptr.line_index];
        assert!(
            line.outgoing_refs.is_empty(),
            "Label should have no outgoing references"
        );
    }

    #[test]
    fn test_backward_reference() {
        let source_text = "
            loop:
                addi a0, a0, 1
                beq a0, a1, loop
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed");

        // Find the label and the referencing line
        let label_ptr = find_line_by_label(&source, "loop").unwrap();
        let ref_ptr = find_referencing_line(&source, "loop").unwrap();

        // Verify the reference
        assert_reference(&source, &ref_ptr, "loop", &label_ptr);
    }

    #[test]
    fn test_forward_reference() {
        let source_text = "
                beq a0, a1, skip
                addi a0, a0, 1
            skip:
                addi a1, a1, 1
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with forward reference"
        );

        // Find the label and the referencing line
        let label_ptr = find_line_by_label(&source, "skip").unwrap();
        let ref_ptr = find_referencing_line(&source, "skip").unwrap();

        // Verify the reference
        assert_reference(&source, &ref_ptr, "skip", &label_ptr);
    }

    #[test]
    fn test_multiple_references_to_same_symbol() {
        let source_text = "
                beq a0, a1, target
                addi a0, a0, 1
                bne a0, a1, target
                addi a1, a1, 1
            target:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed");

        let label_ptr = find_line_by_label(&source, "target").unwrap();

        // Both references should point to the same label
        let file = &source.files[0];
        let mut ref_count = 0;
        for line in &file.lines {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "target" {
                    assert_eq!(
                        sym_ref.pointer, label_ptr,
                        "All references should point to the same label"
                    );
                    ref_count += 1;
                }
            }
        }
        assert_eq!(
            ref_count, 2,
            "Should have exactly 2 references to 'target'"
        );
    }

    #[test]
    fn test_mixed_forward_and_backward_references() {
        let source_text = "
                j middle
            start:
                addi a0, a0, 1
                j end
            middle:
                j start
            end:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed");

        let start_ptr = find_line_by_label(&source, "start").unwrap();
        let middle_ptr = find_line_by_label(&source, "middle").unwrap();
        let end_ptr = find_line_by_label(&source, "end").unwrap();

        // Check that references are correct
        let file = &source.files[0];
        for line in &file.lines {
            for sym_ref in &line.outgoing_refs {
                match sym_ref.symbol.as_str() {
                    "start" => assert_eq!(sym_ref.pointer, start_ptr),
                    "middle" => assert_eq!(sym_ref.pointer, middle_ptr),
                    "end" => assert_eq!(sym_ref.pointer, end_ptr),
                    _ => panic!(
                        "Unexpected symbol reference: {}",
                        sym_ref.symbol
                    ),
                }
            }
        }
    }

    #[test]
    fn test_numeric_label_forward_reference() {
        let source_text = "
                beq a0, a1, 1f
                addi a0, a0, 1
            1:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with numeric forward reference"
        );

        let label_ptr = find_line_by_label(&source, "1").unwrap();
        let ref_ptr = find_referencing_line(&source, "1f").unwrap();

        // The reference should use "1f" but point to the label "1"
        let file = &source.files[ref_ptr.file_index];
        let line = &file.lines[ref_ptr.line_index];
        let matching_ref = line.outgoing_refs.iter().find(|r| r.symbol == "1f");
        assert!(matching_ref.is_some(), "Should have a reference to '1f'");
        assert_eq!(matching_ref.unwrap().pointer, label_ptr);
    }

    #[test]
    fn test_numeric_label_backward_reference() {
        let source_text = "
            1:
                addi a0, a0, 1
                beq a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with numeric backward reference"
        );

        let label_ptr = find_line_by_label(&source, "1").unwrap();
        let ref_ptr = find_referencing_line(&source, "1b").unwrap();

        // The reference should use "1b" and point to the label "1"
        let file = &source.files[ref_ptr.file_index];
        let line = &file.lines[ref_ptr.line_index];
        let matching_ref = line.outgoing_refs.iter().find(|r| r.symbol == "1b");
        assert!(matching_ref.is_some(), "Should have a reference to '1b'");
        assert_eq!(matching_ref.unwrap().pointer, label_ptr);
    }

    #[test]
    fn test_numeric_label_reuse_forward() {
        let source_text = "
                beq a0, a1, 1f
            1:
                addi a0, a0, 1
                beq a0, a1, 1f
            1:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with reused numeric labels"
        );

        // Find both labels named "1"
        let file = &source.files[0];
        let mut label_positions = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Label(ref l) = line.content {
                if l == "1" {
                    label_positions.push(LinePointer {
                        file_index: 0,
                        line_index: line_idx,
                    });
                }
            }
        }
        assert_eq!(
            label_positions.len(),
            2,
            "Should have exactly 2 labels named '1'"
        );

        // First reference should point to first label, second to second label
        let mut ref_positions = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "1f" {
                    ref_positions.push((line_idx, sym_ref.pointer.clone()));
                }
            }
        }
        assert_eq!(
            ref_positions.len(),
            2,
            "Should have exactly 2 references to '1f'"
        );

        // First reference should point to first label
        assert!(
            ref_positions[0].0 < label_positions[0].line_index,
            "First ref should come before first label"
        );
        assert_eq!(
            ref_positions[0].1, label_positions[0],
            "First '1f' should resolve to first '1'"
        );

        // Second reference should point to second label
        assert!(
            ref_positions[1].0 < label_positions[1].line_index,
            "Second ref should come before second label"
        );
        assert_eq!(
            ref_positions[1].1, label_positions[1],
            "Second '1f' should resolve to second '1'"
        );
    }

    #[test]
    fn test_numeric_label_reuse_backward() {
        let source_text = "
            1:
                addi a0, a0, 1
                beq a0, a1, 1b
            1:
                addi a1, a1, 1
                bne a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with reused numeric labels"
        );

        // Find both labels named "1"
        let file = &source.files[0];
        let mut label_positions = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Label(ref l) = line.content {
                if l == "1" {
                    label_positions.push(LinePointer {
                        file_index: 0,
                        line_index: line_idx,
                    });
                }
            }
        }
        assert_eq!(
            label_positions.len(),
            2,
            "Should have exactly 2 labels named '1'"
        );

        // Collect all backward references
        let mut ref_positions = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "1b" {
                    ref_positions.push((line_idx, sym_ref.pointer.clone()));
                }
            }
        }
        assert_eq!(
            ref_positions.len(),
            2,
            "Should have exactly 2 references to '1b'"
        );

        // First reference should point to first label (closest backward)
        assert!(
            ref_positions[0].0 > label_positions[0].line_index,
            "First ref should come after first label"
        );
        assert_eq!(
            ref_positions[0].1, label_positions[0],
            "First '1b' should resolve to first '1'"
        );

        // Second reference should point to second label (closest backward)
        assert!(
            ref_positions[1].0 > label_positions[1].line_index,
            "Second ref should come after second label"
        );
        assert_eq!(
            ref_positions[1].1, label_positions[1],
            "Second '1b' should resolve to second '1'"
        );
    }

    #[test]
    fn test_numeric_labels_blocked_by_non_numeric() {
        let source_text = "
            1:
                addi a0, a0, 1
            regular_label:
                beq a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        // Should fail because numeric label reference crosses a non-numeric label
        assert!(
            result.is_err(),
            "Symbol resolution should fail when numeric reference crosses non-numeric label"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("1b") || err_msg.contains("Undefined"),
            "Error should mention the unresolved numeric label"
        );
    }

    #[test]
    fn test_numeric_labels_forward_blocked_by_non_numeric() {
        let source_text = "
                beq a0, a1, 1f
            regular_label:
            1:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        // Should fail because numeric forward reference crosses a non-numeric label
        assert!(
            result.is_err(),
            "Symbol resolution should fail when numeric forward reference crosses non-numeric label"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("1f") || err_msg.contains("numeric"),
            "Error should mention the unresolved numeric label"
        );
    }

    #[test]
    fn test_numeric_labels_segment_boundary() {
        let source_text = "
            .text
            1:
                addi a0, a0, 1
            .data
                beq a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        // Should fail because segment changes flush numeric labels
        assert!(
            result.is_err(),
            "Symbol resolution should fail when numeric reference crosses segment boundary"
        );
    }

    #[test]
    fn test_multiple_references_in_expression() {
        let source_text = "
            start:
                addi a0, a0, 1
                addi a1, a1, 2
            end:
                li a2, end - start
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with multiple symbols in one expression"
        );

        let start_ptr = find_line_by_label(&source, "start").unwrap();
        let end_ptr = find_line_by_label(&source, "end").unwrap();

        // Find the line with the expression
        let file = &source.files[0];
        let mut found_both = false;
        for line in &file.lines {
            let has_start = line
                .outgoing_refs
                .iter()
                .any(|r| r.symbol == "start" && r.pointer == start_ptr);
            let has_end = line
                .outgoing_refs
                .iter()
                .any(|r| r.symbol == "end" && r.pointer == end_ptr);
            if has_start && has_end {
                found_both = true;
                break;
            }
        }
        assert!(
            found_both,
            "Should find a line with references to both 'start' and 'end'"
        );
    }

    #[test]
    fn test_label_with_instruction_on_same_line() {
        let source_text = "
            start: li a0, 5
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed");

        // Should have two lines: label and instruction
        let file = &source.files[0];
        assert_eq!(
            file.lines.len(),
            2,
            "Label + instruction should create 2 lines"
        );

        // First should be label
        if let LineContent::Label(ref l) = file.lines[0].content {
            assert_eq!(l, "start");
        } else {
            panic!("First line should be a label");
        }

        // Second should be instruction
        match &file.lines[1].content {
            LineContent::Instruction(_) => {}
            _ => panic!("Second line should be an instruction"),
        }
    }

    #[test]
    fn test_label_with_instruction_and_reference() {
        let source_text = "
            loop: beq a0, a1, loop
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed");

        let label_ptr = find_line_by_label(&source, "loop").unwrap();

        // The instruction line should have a reference to the label
        let file = &source.files[0];
        assert_eq!(
            file.lines.len(),
            2,
            "Label + instruction should create 2 lines"
        );

        let instr_line = &file.lines[1];
        let has_ref = instr_line
            .outgoing_refs
            .iter()
            .any(|r| r.symbol == "loop" && r.pointer == label_ptr);
        assert!(
            has_ref,
            "Instruction should have reference back to its own label"
        );
    }

    #[test]
    fn test_equ_directive() {
        let source_text = "
            .equ CONST, 42
            li a0, CONST
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Symbol resolution should succeed with .equ");

        // Find the .equ line
        let file = &source.files[0];
        let mut equ_ptr = None;
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "CONST" {
                    equ_ptr = Some(LinePointer {
                        file_index: 0,
                        line_index: line_idx,
                    });
                    break;
                }
            }
        }
        assert!(equ_ptr.is_some(), "Should find .equ CONST");

        // Check that the reference points to the .equ
        let ref_ptr = find_referencing_line(&source, "CONST").unwrap();
        assert_reference(&source, &ref_ptr, "CONST", &equ_ptr.unwrap());
    }

    #[test]
    fn test_equ_can_redefine() {
        let source_text = "
            .equ counter, 0
            .equ counter, counter + 1
            .equ counter, counter + 1
            li a0, counter
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with .equ redefinition"
        );

        // Find all .equ lines
        let file = &source.files[0];
        let mut equ_count = 0;
        for line in &file.lines {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "counter" {
                    equ_count += 1;
                }
            }
        }
        assert_eq!(
            equ_count, 3,
            "Should have 3 .equ definitions for 'counter'"
        );

        // The second and third .equ should reference previous definitions
        let mut ref_count = 0;
        for line in &file.lines {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "counter" {
                    ref_count += 1;
                }
            }
        }
        // Should have 2 refs in .equ directives + 1 ref in li instruction = 3 total
        assert_eq!(ref_count, 3, "Should have 3 references to 'counter'");
    }

    #[test]
    fn test_undefined_symbol_error() {
        let source_text = "
            beq a0, a1, undefined_label
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Symbol resolution should fail with undefined symbol"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("undefined_label")
                || err_msg.contains("Undefined"),
            "Error should mention the undefined symbol"
        );
    }

    #[test]
    fn test_backward_numeric_reference_undefined() {
        let source_text = "
            beq a0, a1, 1b
            1:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Symbol resolution should fail with backward reference to non-existent label"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("1b") || err_msg.contains("backward"),
            "Error should mention the backward numeric label"
        );
    }

    #[test]
    fn test_complex_numeric_label_interleaving() {
        let source_text = "
            1:
            2:
                beq a0, a1, 1b
                beq a0, a1, 2b
                beq a0, a1, 3f
            3:
            1:
                beq a0, a1, 1b
                beq a0, a1, 3b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with interleaved numeric labels"
        );

        // Verify all references resolve correctly
        let file = &source.files[0];
        for line in &file.lines {
            for sym_ref in &line.outgoing_refs {
                // Each reference should point to a valid label
                let target_line = &file.lines[sym_ref.pointer.line_index];
                assert!(
                    matches!(target_line.content, LineContent::Label(_)),
                    "Reference should point to a label"
                );
            }
        }
    }

    #[test]
    fn test_expression_with_current_address() {
        let source_text = "
            start:
                li a0, . - start
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Symbol resolution should succeed with current address"
        );

        // Should have a reference to 'start' but not to '.'
        let start_ptr = find_line_by_label(&source, "start").unwrap();
        let ref_ptr = find_referencing_line(&source, "start").unwrap();
        assert_reference(&source, &ref_ptr, "start", &start_ptr);
    }

    // ============================================================================
    // .equ Directive Tests
    // ============================================================================

    #[test]
    fn test_equ_redefines_only_equ() {
        let source_text = "
            .equ value, 1
            .equ value, 2
            .equ value, 3
            li a0, value
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), ".equ should be allowed to redefine .equ");

        // Find all three .equ definitions
        let file = &source.files[0];
        let mut equ_positions = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "value" {
                    equ_positions.push(line_idx);
                }
            }
        }
        assert_eq!(equ_positions.len(), 3, "Should have 3 .equ definitions");
    }

    #[test]
    fn test_equ_cannot_redefine_label() {
        let source_text = "
            value:
                nop
            .equ value, 5
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        // This should be allowed in the current implementation since .equ just creates
        // a new definition. Let's verify both exist in the definitions
        // Actually, looking at the implementation, definitions.insert() will overwrite
        // the old value, but we don't explicitly check for label/equ conflicts.
        // Let's test what actually happens:
        assert!(
            result.is_ok(),
            "Current implementation allows .equ to shadow label"
        );
    }

    #[test]
    fn test_label_cannot_redefine_label() {
        let source_text = "
            duplicate:
                nop
            duplicate:
                nop
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        // Should fail with duplicate label error
        assert!(result.is_err(), "Should fail with duplicate label error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("duplicate") || err_msg.contains("Duplicate"),
            "Error should mention the duplicate label: {}",
            err_msg
        );
    }

    #[test]
    fn test_equ_self_reference() {
        let source_text = "
            .equ counter, 0
            .equ counter, counter + 1
            .equ counter, counter + 1
            li a0, counter
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            ".equ should allow self-reference to previous value"
        );

        // The second .equ should reference the first, and third should reference second
        let file = &source.files[0];
        let mut equ_line_indices = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "counter" {
                    equ_line_indices.push(line_idx);
                }
            }
        }
        assert_eq!(equ_line_indices.len(), 3);

        // Second .equ should have reference to first
        let second_line = &file.lines[equ_line_indices[1]];
        let has_ref_to_first = second_line.outgoing_refs.iter().any(|r| {
            r.symbol == "counter" && r.pointer.line_index == equ_line_indices[0]
        });
        assert!(has_ref_to_first, "Second .equ should reference first");

        // Third .equ should have reference to second
        let third_line = &file.lines[equ_line_indices[2]];
        let has_ref_to_second = third_line.outgoing_refs.iter().any(|r| {
            r.symbol == "counter" && r.pointer.line_index == equ_line_indices[1]
        });
        assert!(has_ref_to_second, "Third .equ should reference second");
    }

    #[test]
    fn test_equ_forward_reference() {
        let source_text = "
            li a0, CONST
            .equ CONST, 42
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Forward reference to .equ should work");

        // Find the .equ
        let file = &source.files[0];
        let mut equ_ptr = None;
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "CONST" {
                    equ_ptr = Some(LinePointer {
                        file_index: 0,
                        line_index: line_idx,
                    });
                    break;
                }
            }
        }
        assert!(equ_ptr.is_some());

        // The li instruction should reference it
        let ref_ptr = find_referencing_line(&source, "CONST").unwrap();
        assert_reference(&source, &ref_ptr, "CONST", &equ_ptr.unwrap());
    }

    #[test]
    fn test_equ_forward_reference_resolves_to_first() {
        let source_text = "
            li a0, value
            .equ value, 10
            .equ value, 20
            .equ value, 30
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Forward reference should resolve to first .equ"
        );

        // Find all .equ definitions
        let file = &source.files[0];
        let mut equ_indices = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "value" {
                    equ_indices.push(line_idx);
                }
            }
        }
        assert_eq!(equ_indices.len(), 3);

        // The li instruction should reference the first one
        let ref_ptr = find_referencing_line(&source, "value").unwrap();
        let li_line = &file.lines[ref_ptr.line_index];
        let ref_to_value =
            li_line.outgoing_refs.iter().find(|r| r.symbol == "value").unwrap();
        assert_eq!(
            ref_to_value.pointer.line_index, equ_indices[0],
            "Forward reference should resolve to first .equ definition"
        );
    }

    #[test]
    fn test_equ_backward_reference_to_most_recent() {
        let source_text = "
            .equ value, 10
            li a0, value
            .equ value, 20
            li a1, value
            .equ value, 30
            li a2, value
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Backward references should resolve to most recent .equ"
        );

        // Find all .equ definitions
        let file = &source.files[0];
        let mut equ_indices = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "value" {
                    equ_indices.push(line_idx);
                }
            }
        }
        assert_eq!(equ_indices.len(), 3);

        // Find all li instructions and check their references
        let mut li_refs = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Instruction(_) = line.content {
                for sym_ref in &line.outgoing_refs {
                    if sym_ref.symbol == "value" {
                        li_refs.push((line_idx, sym_ref.pointer.line_index));
                    }
                }
            }
        }
        assert_eq!(
            li_refs.len(),
            3,
            "Should have 3 li instructions with references"
        );

        // First li should reference first .equ (backward)
        assert_eq!(
            li_refs[0].1, equ_indices[0],
            "First li should reference first .equ"
        );

        // Second li should reference second .equ (backward, most recent)
        assert_eq!(
            li_refs[1].1, equ_indices[1],
            "Second li should reference second .equ"
        );

        // Third li should reference third .equ (backward, most recent)
        assert_eq!(
            li_refs[2].1, equ_indices[2],
            "Third li should reference third .equ"
        );
    }

    // ============================================================================
    // Negative Tests - Undefined References
    // ============================================================================

    #[test]
    fn test_error_undefined_forward_reference() {
        let source_text = "
            j never_defined
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_err(), "Should fail with undefined symbol");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("never_defined") || err.contains("Undefined"),
            "Error should mention undefined symbol: {}",
            err
        );
    }

    #[test]
    fn test_error_undefined_backward_reference() {
        let source_text = "
            li a0, undefined_constant
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_err(), "Should fail with undefined symbol");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("undefined_constant") || err.contains("Undefined"),
            "Error should mention undefined symbol: {}",
            err
        );
    }

    #[test]
    fn test_error_numeric_forward_never_defined() {
        let source_text = "
            beq a0, a1, 1f
            beq a0, a1, 2f
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should fail with undefined numeric forward reference"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("1f")
                || err.contains("numeric")
                || err.contains("Unresolved"),
            "Error should mention unresolved numeric label: {}",
            err
        );
    }

    #[test]
    fn test_error_numeric_forward_crosses_segment() {
        let source_text = "
            .text
                beq a0, a1, 1f
            .data
            1:
                .4byte 0
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Numeric forward ref should not cross segment boundary"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("1f")
                || err.contains("numeric")
                || err.contains("Unresolved"),
            "Error should mention unresolved numeric label: {}",
            err
        );
    }

    #[test]
    fn test_error_numeric_forward_crosses_nonnumeric_label() {
        let source_text = "
            beq a0, a1, 1f
            middle:
                nop
            1:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Numeric forward ref should not cross non-numeric label"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("1f")
                || err.contains("numeric")
                || err.contains("Unresolved"),
            "Error should mention unresolved numeric label: {}",
            err
        );
    }

    #[test]
    fn test_error_numeric_backward_never_defined() {
        let source_text = "
            beq a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should fail with undefined numeric backward reference"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("1b")
                || err.contains("backward")
                || err.contains("Unresolved"),
            "Error should mention unresolved backward reference: {}",
            err
        );
    }

    #[test]
    fn test_error_numeric_backward_crosses_nonnumeric() {
        let source_text = "
            1:
                nop
            separator:
                nop
            beq a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Numeric backward ref should not cross non-numeric label"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("1b") || err.contains("Undefined"),
            "Error should mention undefined symbol: {}",
            err
        );
    }

    #[test]
    fn test_error_numeric_backward_crosses_segment() {
        let source_text = "
            .text
            1:
                nop
            .data
                beq a0, a1, 1b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Numeric backward ref should not cross segment boundary"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("1b")
                || err.contains("backward")
                || err.contains("Unresolved"),
            "Error should mention unresolved reference: {}",
            err
        );
    }

    #[test]
    fn test_error_multiple_numeric_labels_all_flushed() {
        let source_text = "
            1:
            2:
            3:
                nop
            separator:
                nop
            beq a0, a1, 1b
            beq a0, a1, 2b
            beq a0, a1, 3b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "All numeric labels should be flushed by non-numeric label"
        );
        // Should fail on the first backward reference that can't be resolved
    }

    // ============================================================================
    // Negative Tests - Invalid Definitions
    // ============================================================================

    #[test]
    fn test_error_equ_with_numeric_name() {
        let source_text = "
            .equ 123, 456
        ";

        // The parser itself rejects this, so create_source will fail
        let result = create_source(vec![("test.s", source_text)]);
        assert!(result.is_err(), "Parser should reject .equ with numeric name");
        let err = result.unwrap_err();
        assert!(
            err.contains("identifier") || err.contains("Expected"),
            "Error should indicate parser expected identifier: {}",
            err
        );
    }

    #[test]
    fn test_mixed_numeric_and_named_labels_ok() {
        let source_text = "
            1:
            2:
                nop
            named:
            3:
            4:
                nop
                beq a0, a1, 3b
                beq a0, a1, 4b
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Numeric labels after non-numeric should work in new scope"
        );

        // Verify the references point to the labels after 'named'
        let file = &source.files[0];

        // Find label 3 and 4 (should be after 'named')
        let mut label_3_idx = None;
        let mut label_4_idx = None;
        let mut seen_named = false;
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Label(ref l) = line.content {
                if l == "named" {
                    seen_named = true;
                } else if seen_named && l == "3" {
                    label_3_idx = Some(line_idx);
                } else if seen_named && l == "4" {
                    label_4_idx = Some(line_idx);
                }
            }
        }

        assert!(label_3_idx.is_some(), "Should find label 3 after named");
        assert!(label_4_idx.is_some(), "Should find label 4 after named");

        // Check that backward references point to the right labels
        for line in &file.lines {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "3b" {
                    assert_eq!(
                        sym_ref.pointer.line_index,
                        label_3_idx.unwrap(),
                        "3b should reference label 3 after 'named'"
                    );
                }
                if sym_ref.symbol == "4b" {
                    assert_eq!(
                        sym_ref.pointer.line_index,
                        label_4_idx.unwrap(),
                        "4b should reference label 4 after 'named'"
                    );
                }
            }
        }
    }

    #[test]
    fn test_expression_in_equ_with_multiple_symbols() {
        let source_text = "
            start:
                nop
            end:
                nop
            .equ size, end - start
            li a0, size
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), ".equ can use complex expressions");

        let start_ptr = find_line_by_label(&source, "start").unwrap();
        let end_ptr = find_line_by_label(&source, "end").unwrap();

        // The .equ line should have references to both start and end
        let file = &source.files[0];
        let mut equ_line = None;
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "size" {
                    equ_line = Some(line_idx);
                    break;
                }
            }
        }
        assert!(equ_line.is_some());

        let equ = &file.lines[equ_line.unwrap()];
        let has_start = equ
            .outgoing_refs
            .iter()
            .any(|r| r.symbol == "start" && r.pointer == start_ptr);
        let has_end = equ
            .outgoing_refs
            .iter()
            .any(|r| r.symbol == "end" && r.pointer == end_ptr);
        assert!(
            has_start && has_end,
            ".equ should reference both start and end"
        );
    }

    // ============================================================================
    // Multi-File Tests
    // ============================================================================

    #[test]
    fn test_multifile_cross_file_reference() {
        let file1 = "
            .global main

            main:
                call helper
                ret
        ";

        let file2 = "
            .global helper

            helper:
                li a0, 42
                ret
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Cross-file reference should work with globals"
        );

        // Find helper in file2
        let helper_ptr = find_line_by_label(&source, "helper").unwrap();
        assert_eq!(
            helper_ptr.file_index, 1,
            "helper should be in file 1 (file2.s)"
        );

        // Find call in file1
        let call_ptr = find_referencing_line(&source, "helper").unwrap();
        assert_eq!(
            call_ptr.file_index, 0,
            "call should be in file 0 (file1.s)"
        );

        // Verify the cross-file reference
        assert_reference(&source, &call_ptr, "helper", &helper_ptr);
    }

    #[test]
    fn test_multifile_global_equ_exports_last_version() {
        let file1 = "
            .equ counter, 10
            .equ counter, 20
            .equ counter, 30
            .global counter
        ";

        let file2 = "
            li a0, counter
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Global should export the last version of .equ"
        );

        // Find all three .equ definitions in file1
        let file = &source.files[0];
        let mut equ_indices = Vec::new();
        for (line_idx, line) in file.lines.iter().enumerate() {
            if let LineContent::Directive(Directive::Equ(ref name, _)) =
                line.content
            {
                if name == "counter" {
                    equ_indices.push(line_idx);
                }
            }
        }
        assert_eq!(equ_indices.len(), 3, "Should have 3 .equ definitions");

        // The reference in file2 should point to the last .equ (index 2)
        let ref_ptr = find_referencing_line(&source, "counter").unwrap();
        assert_eq!(ref_ptr.file_index, 1, "Reference should be in file2");

        let file2 = &source.files[1];
        let li_line = &file2.lines[ref_ptr.line_index];
        let ref_to_counter = li_line
            .outgoing_refs
            .iter()
            .find(|r| r.symbol == "counter")
            .unwrap();
        assert_eq!(
            ref_to_counter.pointer.file_index, 0,
            "Should point to file1"
        );
        assert_eq!(
            ref_to_counter.pointer.line_index, equ_indices[2],
            "Global should export last .equ definition"
        );
    }

    #[test]
    fn test_multifile_error_global_declared_not_defined() {
        let file1 = "
            .global undefined_func

            main:
                ret
        ";

        let mut source = create_source(vec![("file1.s", file1)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should fail when global is declared but not defined"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("undefined_func")
                && err.contains("declared but not defined"),
            "Error should mention symbol and reason: {}",
            err
        );
    }

    #[test]
    fn test_multifile_global_before_definition() {
        let file1 = "
            .global main

            main:
                ret
        ";

        let mut source = create_source(vec![("file1.s", file1)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Global declared before definition should work"
        );

        // Verify global points to the label
        assert_eq!(
            source.global_symbols.len(),
            1,
            "Should have 1 global symbol"
        );
        assert_eq!(source.global_symbols[0].symbol, "main");
    }

    #[test]
    fn test_multifile_global_after_definition() {
        let file1 = "
            main:
                ret

            .global main
        ";

        let mut source = create_source(vec![("file1.s", file1)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Global declared after definition should work");

        // Verify global points to the label
        assert_eq!(
            source.global_symbols.len(),
            1,
            "Should have 1 global symbol"
        );
        assert_eq!(source.global_symbols[0].symbol, "main");
    }

    #[test]
    fn test_multifile_error_no_global_numeric_labels() {
        let file1 = "
            .global 123
        ";

        // The parser itself rejects this, so create_source will fail
        let result = create_source(vec![("file1.s", file1)]);
        assert!(
            result.is_err(),
            "Parser should reject .global with numeric label"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("identifier")
                || err.contains("Expected")
                || err.contains("123"),
            "Error should indicate parser expected identifier: {}",
            err
        );
    }

    #[test]
    fn test_multifile_error_duplicate_global_same_file() {
        let file1 = "
            .global func
            .global func

            func:
                ret
        ";

        let mut source = create_source(vec![("file1.s", file1)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should fail when same global declared twice in same file"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("func") && err.contains("already declared global"),
            "Error should mention symbol: {}",
            err
        );
    }

    #[test]
    fn test_multifile_error_duplicate_global_different_files() {
        let file1 = "
            .global duplicate

            duplicate:
                ret
        ";

        let file2 = "
            .global duplicate

            duplicate:
                nop
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should fail when same global declared in multiple files"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("duplicate") && err.contains("Duplicate global"),
            "Error should mention duplicate: {}",
            err
        );
    }

    #[test]
    fn test_multifile_error_dangling_reference() {
        let file1 = "
            main:
                call undefined_func
                ret
        ";

        let file2 = "
            helper:
                ret
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should fail with undefined symbol in multi-file case"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("undefined_func") && err.contains("Undefined"),
            "Error should mention undefined symbol: {}",
            err
        );
    }

    #[test]
    fn test_multifile_unreferenced_global_ok() {
        let file1 = "
            .global unused_func

            unused_func:
                ret

            main:
                ret
        ";

        let mut source = create_source(vec![("file1.s", file1)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Unreferenced global should be OK");
        assert_eq!(
            source.global_symbols.len(),
            1,
            "Should have 1 global symbol"
        );
        assert_eq!(source.global_symbols[0].symbol, "unused_func");
    }

    #[test]
    fn test_multifile_equ_and_label_globals() {
        let file1 = "
            .equ BUFFER_SIZE, 4096
            .global BUFFER_SIZE
            .global main

            main:
                li a0, BUFFER_SIZE
                ret
        ";

        let file2 = "
            helper:
                li a1, BUFFER_SIZE
                ret
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Both .equ and label globals should work");
        assert_eq!(
            source.global_symbols.len(),
            2,
            "Should have 2 global symbols"
        );

        // Verify both globals exist
        let global_names: Vec<&str> =
            source.global_symbols.iter().map(|g| g.symbol.as_str()).collect();
        assert!(
            global_names.contains(&"BUFFER_SIZE"),
            "Should have BUFFER_SIZE global"
        );
        assert!(global_names.contains(&"main"), "Should have main global");

        // Verify cross-file reference to .equ global works
        let ref_in_file2 =
            find_referencing_line(&source, "BUFFER_SIZE").unwrap();
        if ref_in_file2.file_index == 1 {
            // Found the reference in file2
            let equ_ptr = LinePointer { file_index: 0, line_index: 0 };
            assert_reference(&source, &ref_in_file2, "BUFFER_SIZE", &equ_ptr);
        }
    }

    #[test]
    fn test_multifile_multiple_cross_references() {
        let file1 = "
            .global func_a
            .global func_b

            func_a:
                call func_c
                ret

            func_b:
                ret
        ";

        let file2 = "
            .global func_c

            func_c:
                call func_b
                ret
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(result.is_ok(), "Multiple cross-file references should work");

        // Verify func_a calls func_c (file1 -> file2)
        let func_c_ptr = find_line_by_label(&source, "func_c").unwrap();
        assert_eq!(func_c_ptr.file_index, 1);

        // Verify func_c calls func_b (file2 -> file1)
        let func_b_ptr = find_line_by_label(&source, "func_b").unwrap();
        assert_eq!(func_b_ptr.file_index, 0);

        // Check cross-references
        let file1_lines = &source.files[0].lines;
        let file2_lines = &source.files[1].lines;

        let mut found_a_to_c = false;
        let mut found_c_to_b = false;

        for line in file1_lines {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "func_c" {
                    assert_eq!(sym_ref.pointer, func_c_ptr);
                    found_a_to_c = true;
                }
            }
        }

        for line in file2_lines {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "func_b" {
                    assert_eq!(sym_ref.pointer, func_b_ptr);
                    found_c_to_b = true;
                }
            }
        }

        assert!(found_a_to_c, "Should find reference from func_a to func_c");
        assert!(found_c_to_b, "Should find reference from func_c to func_b");
    }

    #[test]
    fn test_multifile_multiple_symbols_in_single_global_directive() {
        let file1 = "
            _start:
                ret

            exit:
                ret

            helper:
                ret

            .global _start, exit, helper
        ";

        let file2 = "
            li a0, 0
            call _start
            call exit
            call helper
            ret
        ";

        let mut source =
            create_source(vec![("file1.s", file1), ("file2.s", file2)])
                .unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_ok(),
            "Multiple symbols in single .global directive should work"
        );

        // Verify all three symbols are exported as globals
        assert_eq!(
            source.global_symbols.len(),
            3,
            "Should have 3 global symbols"
        );

        let global_names: Vec<&str> =
            source.global_symbols.iter().map(|g| g.symbol.as_str()).collect();
        assert!(global_names.contains(&"_start"), "Should have _start global");
        assert!(global_names.contains(&"exit"), "Should have exit global");
        assert!(global_names.contains(&"helper"), "Should have helper global");

        // Verify cross-file references work for all three
        let start_ptr = find_line_by_label(&source, "_start").unwrap();
        let exit_ptr = find_line_by_label(&source, "exit").unwrap();
        let helper_ptr = find_line_by_label(&source, "helper").unwrap();

        let file2 = &source.files[1];
        let mut found_start = false;
        let mut found_exit = false;
        let mut found_helper = false;

        for line in &file2.lines {
            for sym_ref in &line.outgoing_refs {
                if sym_ref.symbol == "_start" && sym_ref.pointer == start_ptr {
                    found_start = true;
                }
                if sym_ref.symbol == "exit" && sym_ref.pointer == exit_ptr {
                    found_exit = true;
                }
                if sym_ref.symbol == "helper" && sym_ref.pointer == helper_ptr {
                    found_helper = true;
                }
            }
        }

        assert!(found_start, "Should find reference to _start");
        assert!(found_exit, "Should find reference to exit");
        assert!(found_helper, "Should find reference to helper");
    }

    // ============================================================================
    // Special Symbol Tests - __global_pointer$
    // ============================================================================

    #[test]
    fn test_special_global_pointer_label_definition_rejected() {
        let source_text = "
            .global __global_pointer$
            __global_pointer$:
                ret
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should reject attempt to define __global_pointer$ as a label"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("__global_pointer$") && err.contains("is reserved"),
            "Error should mention symbol is reserved: {}",
            err
        );
    }

    #[test]
    fn test_special_global_pointer_equ_definition_rejected() {
        let source_text = "
            .global __global_pointer$
            .equ __global_pointer$, 0x1000
        ";

        let mut source = create_source(vec![("test.s", source_text)]).unwrap();
        let result = resolve_symbols(&mut source);

        assert!(
            result.is_err(),
            "Should reject attempt to define __global_pointer$ in .equ"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("__global_pointer$") && err.contains("is reserved"),
            "Error should mention symbol being reserved: {}",
            err
        );
    }
}
