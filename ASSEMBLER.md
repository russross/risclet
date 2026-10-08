How the assembler works
=======================

risclet turns a group of assembly source files into one executable. It does
the work of both an assembler, which translates instructions into bytes, and
a linker, which connects separately defined parts of a program.

The work proceeds in passes. Each pass answers a different question and builds
on the results of earlier passes:

| Pass                | Question                                        | Result                           |
|---------------------|-------------------------------------------------|----------------------------------|
| Parse               | What does each source line say?                 | Structured source                |
| Link symbols        | Which definition does each name refer to?       | References bound to definitions  |
| Lay out             | Where would each line go with these sizes?      | Addresses and section offsets    |
| Evaluate            | What do symbols and expressions mean there?     | Integer and address values       |
| Encode              | Which bytes represent each instruction?         | Bytes and revised sizes          |
| Build executable    | How should the program be loaded and started?   | ELF executable                   |

Layout, evaluation, and encoding repeat until the sizes stop changing. The
other passes run once.

The layers keep their results separate. Parsed source is read-only after
parsing. Symbol linking adds a separate set of connections rather than
replacing names in the source. Later passes read those two results and build
new address and value tables. Changing an instruction's size therefore changes
its neighbors' addresses without changing their source or symbol bindings.

See [SYNTAX.md](SYNTAX.md) for the language itself.


Reading the reports
-------------------

The `assemble` command can stop at a pass and print its results:

    risclet assemble main.s helper.s --dump-ast
    risclet assemble main.s helper.s --dump-symbols
    risclet assemble main.s helper.s --dump-layout
    risclet assemble main.s helper.s --dump-code
    risclet assemble main.s helper.s --dump-elf

Dump options produce reports instead of an executable. They do not overwrite
an existing output file. Combine options to see several layers; assembly stops
after the latest requested layer. An early dump can succeed even when a later
pass would reject the program.

Reports preserve source locations such as `[main.s:4]`. Printed instructions
and directives use normalized spellings, so they need not reproduce the
original text exactly.


1. Parse the source
-------------------

Tokenization divides each line into tokens: names, registers, numbers,
punctuation, and operators. Parsing checks how those tokens fit together and
builds an abstract syntax tree, or AST: a structured representation of the
line with a tree for each expression.

For example, `COUNT * 4 + 1` becomes an addition whose left operand is a
multiplication. Operator precedence is settled here; the value of `COUNT`
can wait. Comments and whitespace no longer matter, but file and line numbers
remain attached to the parsed records.

`--dump-ast` prints parenthesized expressions with the operation first:

    (+ (* (id "COUNT") (lit 4)) (lit 1))

Here `id` means a symbol name and `lit` means a literal value. A label and an
instruction on the same source line appear as separate records.

Simple pseudo-instructions are normalized now: `ret` becomes `jalr zero, ra, 0`.
Instructions such as `li`, `la`, and `call` remain pseudo-instructions because
choosing their encoding needs values or addresses that are not yet known.


2. Connect names to definitions
-------------------------------

A symbol is a name for an address or value. Symbol resolution connects each
use of a symbol to the source record that defines it. This pass establishes
identity, not a numeric address.

Each file is processed first. References bind to local definitions, including
forward references. Unresolved names are then matched against symbols exported
by other files. Missing definitions and conflicting exports are errors.

For a `call helper` in `main.s`, the symbol report might show:

    call helper  → helper@[helper.s:3]

The arrow identifies the definition used by that call. The global-symbol list
also shows where each name was defined and where it was exported. This is the
report to inspect when a reference resolves to an unexpected definition.

The assembler supplies `__global_pointer$` through a small built-in source
file. It may appear as `[<builtin>:3]` in the global-symbol list. Its address
will be the start of `.data` plus 2048.


### Differences from GNU tools

risclet treats source files as separate scopes. A symbol is local unless
exported, even though all files are assembled together. Put `.global helper`
in the file that defines `helper`; the caller needs no declaration. Unlike
[GNU `.global`](https://sourceware.org/binutils/docs/as/Global.html), risclet
requires an exported name to be defined in the declaring file and rejects
repeated export declarations.

Numeric references such as `1b` and `1f` stop at file boundaries and at every
`.text`, `.data`, or `.bss` directive. GNU numeric labels remain available
across section changes within a source file. See the
[GNU symbol rules](https://sourceware.org/binutils/docs/as/Symbol-Names.html).

For `.equ` and `.set`, each use binds to the most recent preceding definition,
or the next definition if there is no preceding one. Later redefinition does
not retarget an earlier use. This definition-by-definition rule is simpler
than GNU's target-dependent handling of redefined symbolic expressions;
see [GNU `.set`](https://sourceware.org/binutils/docs/as/Set.html).


3. Lay out, evaluate, and encode
--------------------------------

An instruction's size can depend on its operands. A small `li` needs only one
instruction; a larger one may need two. A nearby `call` can use a short jump.
But the distance to its target depends on the sizes of intervening instructions.

Relaxation is the process of choosing shorter encodings when they fit. risclet
handles this circular dependency by starting with estimated sizes and repeating
three passes.


### Lay out the program

Layout assigns each source record a section, an offset within that section,
and an absolute address. A section groups related content: `.text` holds code,
`.data` holds initialized data, and `.bss` reserves zero-initialized storage.

Content in each section follows input-file order. Without explicit file names,
the discovered `.s` files are sorted by name. Each file starts in `.text`,
regardless of the preceding file's final section. Offsets in `.text`, `.data`,
and `.bss` remain cumulative across files.

The layout is fixed by risclet: executable headers precede the instructions,
data begins at the next 4096-byte boundary after the code, and `.bss` follows
the data. `.balign` adds padding where requested. A label takes the address at
its position and occupies no bytes itself.

Initial estimates give ordinary instructions four bytes, explicit compressed
instructions two, and the remaining pseudo-instructions eight. Data lists and
strings have known lengths; `.space` and `.balign` start with zero-size estimates.

Use `--dump-layout` to see the addresses. They are hexadecimal, with `.t`, `.d`,
or `.b` identifying the section. Labels and non-emitting directives can share
an address. This report also annotates some literal operands, but it is not a
complete table of evaluated expressions.


### Evaluate symbols and expressions

With a layout available, labels have addresses. The evaluator follows the
already-established symbol connections to calculate `.equ` values, including
definitions that depend on other definitions. Circular definitions are errors.

Values retain their integer or address type. Subtracting two addresses gives
an integer distance; adding an integer to an address gives another address.
Instruction encoding checks that each operand has the required type. This is
stricter than GNU syntax: for example, risclet requires `la a0, label` rather
than treating a label as an ordinary `li` immediate.

These values are recomputed for each layout. A symbol keeps the same definition
throughout relaxation, but that definition's address or value may move.


### Encode and repeat

Encoding translates instructions and data into bytes using that iteration's
addresses and values. It also reports each line's actual size. `.bss` reports
reserved sizes without producing stored bytes.

If any size changed, the new sizes become the input to another layout pass.
When no size changes, the process has converged: another pass would use the
same addresses. risclet allows up to ten iterations. Failure to settle is an
error, not permission to emit an inconsistent program.

Data and fill range errors are retained until convergence, because an early
address difference may be too large while its final value fits. Other checks,
including operand types, still apply during encoding.

By default, nearby `call` and `tail` instructions can shrink. Automatic
compression requires `--relax-compressed`. `gp` relaxation is enabled when the
recognized `la gp, __global_pointer$` initialization is present, unless
overridden. Unlike GNU's usual source-level `.option` controls, these settings
apply to the whole risclet assembly. [SYNTAX.md](SYNTAX.md) describes the controls.


### Inspecting iterations

Without a selector, layout and code dumps show the final pass. To follow the
changing addresses, request all passes or a range:

    risclet assemble main.s helper.s '--dump-layout=*'
    risclet assemble main.s helper.s --dump-layout=1-3
    risclet assemble main.s helper.s '--dump-code=:main.s'
    risclet assemble main.s helper.s --dump-layout --dump-code

The selector is `PASSES:FILES`. Passes are numbered from 1; `2-` means pass 2
onward and `-2` means through pass 2. After the colon, give comma-separated
input file names. An empty pass selector means the final pass. Quote `*` so
the shell does not expand it. AST and symbol dumps also accept file selection,
but those stages run only once.

`--dump-code` places emitted bytes beside parsed instructions and directives.
It is a source-oriented listing, not a disassembly: a row may still say `call`
after that call has become a single `jal`. Use `risclet disassemble` to see
decoded machine instructions.

Intermediate code reports show each line's emitted bytes beside the tentative
address used to encode that pass. If an instruction shrinks, its next neighbor
can still have an address based on the old size; the next pass updates that
address. The report marked `FINAL` shows the settled addresses and bytes.
`--verbose` adds a compact summary of section sizes for each iteration.


4. Build the executable
-----------------------

The final pass packages the bytes into ELF, the Executable and Linkable Format.
It adds loading information, section descriptions, and a symbol table mapping
retained names to their final values. The exported `_start` symbol supplies
the entry point: the address where execution begins. Early dumps do not need
`_start`; building or dumping the executable does.

    risclet assemble main.s helper.s --dump-elf=headers,sections
    risclet assemble main.s helper.s --dump-elf=symbols
    risclet assemble main.s helper.s -o program

In the header report, `LOAD` entries describe regions loaded into memory.
`FileSiz` counts stored bytes; `MemSiz` includes reserved storage. In the section
report, `.bss` is `NOBITS`: it occupies memory without an initialization payload.
The symbol report distinguishes local and global names. Numeric labels and
unexported `.L` names are omitted from the executable's symbol table even
though they participated in assembly.


Why assembling and linking together helps
-----------------------------------------

The usual GNU workflow assembles each source into an object file, then links
the objects into an executable. An object file can contain relocations:
instructions to patch addresses once the linker knows where things belong.
A linker script determines the final placement of sections.

risclet has all source files and definitions available before it assigns final
addresses. It can evaluate expressions across files and reconsider instruction
sizes directly. It needs no intermediate object files, relocation records, or
user-written linker script. The same source representation remains available
for diagnostics throughout the process.

The trade-off is a smaller model: one program, three storage sections, fixed
placement rules, and no separate object-library or dynamic-linking stage.
The result follows RISC-V instruction encodings and uses ELF, but its source
semantics and output layout are not intended to reproduce every GNU choice.
