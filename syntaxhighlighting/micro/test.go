package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/zyedidia/micro/v2/pkg/highlight"
)

// Fixtures use byte columns; convert them to rune columns for micro's matches.
type check struct {
	column int
	name   string
}
type fixture struct {
	Line   string              `json:"line"`
	Checks [][]json.RawMessage `json:"checks"`
}

func run() error {
	data, err := os.ReadFile("micro/risclet.yaml")
	if err != nil {
		return err
	}
	header, err := highlight.MakeHeaderYaml(data)
	if err != nil {
		return err
	}
	file, err := highlight.ParseFile(data)
	if err != nil {
		return err
	}
	definition, err := highlight.ParseDef(file, header)
	if err != nil {
		return err
	}
	if !header.MatchFileName("example.s") {
		return fmt.Errorf("filetype detection failed for example.s")
	}
	if header.MatchFileName("example.txt") {
		return fmt.Errorf("filetype detection matched example.txt")
	}

	// Test the shared behavior through micro's own YAML parser and highlighter.
	data, err = os.ReadFile("tests/highlighting.json")
	if err != nil {
		return err
	}
	var fixtures []fixture
	if err = json.Unmarshal(data, &fixtures); err != nil {
		return err
	}
	data, err = os.ReadFile("micro/highlighting.json")
	if err != nil {
		return err
	}
	var additional []fixture
	if err = json.Unmarshal(data, &additional); err != nil {
		return err
	}
	fixtures = append(fixtures, additional...)
	groups := map[string]string{
		"Label": "special.label", "Reference": "special.label", "Instruction": "statement",
		"Directive": "preproc", "Register": "special.register", "Symbol": "identifier.var",
		"Fence": "constant.fence", "String": "constant.string", "EscapeError": "error.escape",
		"Comment": "comment", "Number": "constant.number", "Character": "constant.number",
		"StatementError": "error.statement", "NumericError": "error.number", "CharacterError": "error.character",
		"Address": "constant", "Operator": "symbol.operator",
	}
	lines := make([]string, len(fixtures))
	for i, f := range fixtures {
		lines[i] = f.Line
	}
	matches := highlight.NewHighlighter(definition).HighlightString(strings.Join(lines, "\n"))
	for i, f := range fixtures {
		for _, raw := range f.Checks {
			var expected check
			if err = json.Unmarshal(raw[0], &expected.column); err != nil {
				return err
			}
			if err = json.Unmarshal(raw[1], &expected.name); err != nil {
				return err
			}
			column := len([]rune(f.Line[:expected.column]))
			var actual highlight.Group
			for pos := 0; pos <= column; pos++ {
				if group, ok := matches[i][pos]; ok {
					actual = group
				}
			}
			if actual.String() != groups[expected.name] {
				return fmt.Errorf("%s:%d: expected %s, got %s", f.Line, expected.column, groups[expected.name], actual.String())
			}
		}
	}
	fmt.Printf("Micro highlighting checks passed (%d fixtures).\n", len(fixtures))
	return nil
}

// Keep failures concise so this standalone check can be used in shell workflows.
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
