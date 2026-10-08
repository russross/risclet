Vim syntax highlighting
=======================

Install the runtime directory from the repository root:

```sh
mkdir -p ~/.vim/pack/local/start
ln -s "$PWD/syntaxhighlighting/vim" ~/.vim/pack/local/start/risclet
```

Enable `filetype plugin on` and `syntax on` in your vimrc. Assembly files with
the `.s` extension use the `risclet` filetype when no earlier detector
has selected another filetype. Use `:setfiletype risclet` to select it manually,
or `:setlocal syntax=risclet` to change highlighting in an existing buffer.

The syntax follows `src/tokenizer.rs` and `src/parser.rs`, with the same closed
instruction and directive sets as [CodeMirror](../codemirror/README.md).
Mnemonics are special at statement positions; they can also name labels and
operand symbols. Registers are case-sensitive and reserved. Comments start
with `#`; literals and syntax context end on each physical line. Numeric label
references may contain whitespace before `f` or `b`.

This is highlighting, not validation. It does not check integer overflow,
operand counts, expression validity, register restrictions, or symbol
resolution. Unsupported statement names and malformed escapes are highlighted
as errors. No GAS directives, relocation functions, or C-style comments are
added to the language.

Run the shared behavioral fixtures from the repository root:

```sh
vim -Nu NONE -n -es -S syntaxhighlighting/vim/test.vim
```
