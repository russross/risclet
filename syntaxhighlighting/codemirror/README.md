CodeMirror 6 syntax highlighting
===============================

`risclet.ts` exports `risclet()`, a CodeMirror 6 `LanguageSupport` factory,
`riscletLanguage`, and `riscletStreamParser`. Copy the file into a TypeScript
application that already uses `@codemirror/language`.

The repository's browser demo imports this file directly. Its Make inventory,
TypeScript configuration, and Webpack resolver include the shared source;
changes here rebuild the demo without copying a second syntax definition.

```typescript
import { risclet } from "./risclet";

// Add this to the editor's extensions, alongside its syntax highlighting theme.
const extensions = [risclet()];
```

Dialect
-------

The source of truth is `src/tokenizer.rs` and `src/parser.rs`, reached through
the assembler's per-line source parsing. The highlighter recognizes:

*   Only the implemented instructions, pseudo instructions, compressed names,
    and word-width atomics with optional `.aq`, `.rl`, or `.aqrl` ordering.
*   The exact directives: `.global`, `.globl`, `.equ`, `.set`, `.text`, `.data`,
    `.bss`, `.space`, `.zero`, `.balign`, `.ascii`, `.string`, `.asciz`, `.byte`,
    `.half`, `.2byte`, `.word`, `.4byte`.
*   Case-sensitive `x0`–`x31` and ABI register aliases, including `fp`.
*   Named and integer labels, `f`/`b` numeric label references, current address
    `.`, expression operators, and parenthesized expressions or base registers.
*   Decimal, hexadecimal, binary, explicit octal and leading-zero octal numbers;
    character and string literals with only `\n`, `\t`, `\r`, `\\`, `\'`, `\"`,
    and `\0` escapes; and `#` comments.

Identifiers start with an ASCII letter, `_`, or `$`; subsequent characters can
also include Unicode alphanumeric characters and dots. Internal names beginning
with `.L` are also identifiers. Other leading-dot labels, `.section`, `.8byte`,
`%hi`/`%lo` relocation syntax, `.rel` ordering,
and C-style comments are not added as alternate syntax. An opcode spelling can
also name a label or symbol, while register spellings are reserved. The parser
accepts whitespace between an integer and its `f`/`b` reference suffix; the
highlighter preserves that behavior.

The backward reference `0b` refers to a preceding `0:` label; binary literals
require digits, such as `0b1010`. Disassembly forms such as
`c.addi16sp sp, -16`, `sw a0, count, t0`, and `la a0, . + 16` use the same
instruction, register, symbol, and expression tokens as handwritten source.

This is a stream highlighter, not an assembler validator. It marks unknown
opcodes/directives, unexpected characters, out-of-range integer tokens, and
malformed quoted literals as invalid. Operand counts, expression precedence,
register restrictions on compressed instructions, symbol resolution, and
instruction encoding remain the assembler's responsibility. Token boundaries
follow the tokenizer: for example, `08` splits into octal `0` and decimal `8`,
and `%hi` splits into the modulo operator and the symbol `hi`.

Tokens use standard CodeMirror/Lezer highlight tags: `keyword` for opcodes,
`meta` for directives, `variableName.standard` for registers, `labelName` for
label definitions and numeric references, and `variableName` for symbols.
Strings and parser context reset on every physical line.
