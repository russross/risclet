VS Code syntax highlighting
===========================

Install the extension directory from the repository root:

```sh
mkdir -p ~/.vscode/extensions
ln -s "$PWD/syntaxhighlighting/vscode" ~/.vscode/extensions/russross.risclet-0.1.0
```

Restart VS Code or run `Developer: Reload Window`. Select **Risclet** with
**Change Language Mode**. The extension associates `.s` files with
Risclet; if another extension uses this suffix, select Risclet explicitly
or configure `files.associations` for your project.

The extension contains a declarative TextMate grammar and language configuration;
it has no runtime code or dependencies. The configuration enables `#` comments,
parenthesis matching, and quote pairing. The grammar uses VS Code's standard
scopes so installed themes control the colors.

The dialect follows `src/tokenizer.rs` and `src/parser.rs`, matching the closed
instruction and directive sets documented for [CodeMirror](../codemirror/README.md).
Statement mnemonics, opcode-shaped labels and symbols, case-sensitive registers,
numeric label references, literal escapes, and per-line recovery receive distinct
scopes. Unsupported opcodes and directives are highlighted as errors.

This grammar does not validate integer overflow, operand counts, expression
validity, register restrictions, or symbol resolution. It does not add GAS
syntax or C-style comments. Syntax colors are not an assembler diagnostic.

Run the shared fixtures using VS Code's TextMate and Oniguruma libraries. These
are test tools installed in a temporary directory, not extension dependencies:

```sh
check_dir=$(mktemp -d)
npm install --prefix "$check_dir" --no-audit --no-fund vscode-textmate vscode-oniguruma
NODE_PATH="$check_dir/node_modules" node syntaxhighlighting/vscode/test.cjs
rm -r "$check_dir"
```
