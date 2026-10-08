Micro syntax highlighting
=========================

Install the definition from the repository root:

```sh
mkdir -p ~/.config/micro/syntax
cp syntaxhighlighting/micro/risclet.yaml ~/.config/micro/syntax/risclet.yaml
```

Reopen the source file. The definition detects `.s` filenames. If
another assembly definition wins detection, press Ctrl-e and enter
`set filetype risclet`. For a custom micro configuration directory, put the file
in its `syntax/` subdirectory instead.

Dialect
-------

The source of truth is `src/tokenizer.rs` and `src/parser.rs`, reached through
the assembler's per-line parsing. The definition recognizes the same closed
instruction and directive sets as [CodeMirror](../codemirror/README.md):

*   Implemented base instructions, pseudo instructions, compressed instructions,
    and word-width atomics with `.aq`, `.rl`, or `.aqrl` ordering.
*   Case-sensitive registers, named and integer labels, numeric label references,
    symbols, current address `.`, and expression operators.
*   Decimal, hexadecimal, binary, explicit octal and leading-zero octal numbers;
    character and string literals with the assembler's supported escapes.
*   `#` comments and literal regions that end on the current physical line.

Mnemonics are highlighted at statement positions and remain ordinary symbols in
operands. Unsupported statement names and malformed escapes receive error groups.
No GAS directives, relocation functions, or C-style comments are added.

This is a regex highlighter, not an assembler validator. It does not check integer
overflow, operand counts, expression validity, compressed-register restrictions,
or symbol resolution. An unterminated string retains string coloring until the
end of its line. Micro's region engine also restarts anchored pattern matching
after each literal, so malformed source or expressions containing adjacent
literals and mnemonic-shaped symbols may receive approximate context coloring.

Verification
------------

Run from any directory with Go installed:

```sh
syntaxhighlighting/micro/test.sh
```

The script builds a check against micro 2.0.14's actual YAML parser and highlighting
engine in a temporary directory. It downloads test dependencies without adding
runtime dependencies or Go module files to this project. Tests cover the shared
editor fixtures and micro-specific cases, including compact register lists,
Unicode symbols, literal comments, character labels, and recovery after an
unterminated string. The definition was also loaded in micro 2.0.14 to check
filetype detection and displayed highlighting.
