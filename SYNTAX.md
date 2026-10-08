Assembly language reference
===========================

risclet is for students learning assembly language to understand the
hardware/software interface, not for professional assembly development. It
deliberately supports a subset of instructions, directives, and functionality.
Its opinionated rules help prevent common student mistakes.


Source syntax
-------------

Each line contains an optional label followed by at most one instruction or
directive. A label may also appear alone. Separate operands with commas. `#`
starts a comment that continues to the end of the line. Blank lines are ignored.
Names are case-sensitive; instruction, directive, and register names use
lowercase. Instructions are not allowed to span multiple lines, nor are
multiple instructions permitted on a single line. There is no macro system.

    loop:   addi t0, t0, -1       # Decrement the counter
            bnez t0, loop

Symbol names start with a letter, `_`, or `$`; subsequent characters may also
include digits and dots. Names beginning with `.L`, such as `.Lloop`, are also
supported. Register names cannot be used as symbol names.

Named labels are local to their source file unless exported with `.global`.
Forward references are allowed. Numeric labels can be reused: `1f` refers to
the next `1:`, and `1b` to the preceding `1:`. Numeric references cannot cross
a `.text`, `.data`, or `.bss` directive or a file boundary.


Disassembly output
------------------

`risclet disassemble` shows pseudo-instructions by default. It recovers `li`
from `lui` and matching `lui`/`addi` pairs, and `mv` from `addi` with a zero
immediate or `add` with a zero first source. Matching `auipc` pairs can become
`la`, direct loads and stores, `call`, or `tail`. A direct store includes its
temporary register, as in `sw a0, count, t0`.

Pairs must be adjacent and use a writable address or constant register. They
are not combined when the second instruction has a symbol or a discovered
control-flow target. `addi rd, gp, imm` becomes `la` only when the computed
address has a symbol and `rd` is neither `gp` nor `zero`; otherwise it stays
an arithmetic instruction or another applicable single-instruction alias.

Use `--verbose-instructions` to display each encoded instruction separately.
Compressed instructions retain their original `c.*` mnemonic and operand form,
including `c.li`, `c.mv`, and `c.addi16sp sp, imm`. Compressed encodings without
an accepted instruction spelling are emitted as `.2byte` directives. Upper
immediates use their unshifted 20-bit fields, as accepted by the assembler.

Address operands without a displayed symbol use current-address expressions,
such as `. + 16` or `. - 0x10`. Decoded address calculations wrap at the RV32 boundary;
the displayed expression names the resulting address within the 32-bit range.
Negative integer and memory operands retain their sign in hexadecimal mode.
Label names are preserved in full; numeric labels may be renumbered.

For instruction text suitable for reassembly, hide listing addresses and
encodings:

    risclet disassemble program.s --verbose-instructions --no-show-addresses --no-show-encoding

The listing contains the code, not a complete replacement for the source or
ELF file. Restore section directives, exports such as `.global _start`, and
data declarations when reconstructing a program. To preserve strict instruction
encodings, reassemble with `--no-relax` and preserve the original layout.
Pseudo-instructions can choose different encodings under relaxation.


Registers
---------

All 32 integer registers are available by number or ABI name:

| Number      | ABI name        |
|-------------|-----------------|
| `x0`        | `zero`          |
| `x1`        | `ra`            |
| `x2`        | `sp`            |
| `x3`        | `gp`            |
| `x4`        | `tp`            |
| `x5–x7`     | `t0–t2`         |
| `x8`        | `s0` or `fp`    |
| `x9`        | `s1`            |
| `x10–x17`   | `a0–a7`         |
| `x18–x27`   | `s2–s11`        |
| `x28–x31`   | `t3–t6`         |


Instruction sets
----------------

risclet supports RV32I integer instructions, the M extension for multiplication
and division, the word-sized A extension for atomics, and the integer RV32C
compressed instructions. It also accepts `fence.i` from Zifencei and
`fence.tso`. These do not imply support for other extensions.

RV64 instructions, floating-point instructions and registers, vector
instructions, CSR instructions (Zicsr), counter-reading instructions, and
privileged instructions are not supported.

In the forms below, `rd` is a destination register, `rs1` and `rs2` are source
registers, `imm` is an integer expression, and `target` is an address expression.


### RV32I: register arithmetic and logic

All use `op rd, rs1, rs2`:

    add a0, a1, a2
    sub t0, t1, t2
    sll t0, t0, a0

Instructions: `add`, `sub`, `sll`, `slt`, `sltu`, `xor`, `srl`, `sra`, `or`, `and`.


### RV32I: immediate arithmetic and logic

All use `op rd, rs1, imm`:

    addi sp, sp, -16
    andi t0, a0, 255
    slli t1, a1, 2

Instructions: `addi`, `slti`, `sltiu`, `xori`, `ori`, `andi`, `slli`, `srli`,
`srai`.

Shift amounts must be 0–31. Other immediates must be −2048–2047, including those
for unsigned comparisons and bitwise operations. For example, use `-1`, not
`0xfff`, for a 12-bit immediate with every bit set.


### RV32I: upper immediates

    lui t0, 0x12345
    auipc t1, 1

Forms: `lui rd, imm`, `auipc rd, imm`.

The immediate is the unshifted 20-bit field, in the range 0–1048575. The
instruction shifts it left by 12 bits. Address relocation operators such as
`%hi` and `%pcrel_hi` are not supported; use `la` to load an address.


### RV32I: loads and stores

Loads use `op rd, offset(rs1)`. Stores use `op rs2, offset(rs1)`.
The offset is an integer expression in the range −2048–2047 and may be omitted
when it is zero.

    lw a0, 12(sp)
    sw a0, 0(t0)
    sw a0, (t0)                  # Same as sw a0, 0(t0)
    lb a1, (t1)                  # Same as lb a1, 0(t1)

Loads: `lb`, `lh`, `lw`, `lbu`, `lhu`. Stores: `sb`, `sh`, `sw`.

The forms that access a label directly are pseudo-instructions, listed below.


### RV32I: branches and jumps

Branches use `op rs1, rs2, target`:

    beq a0, a1, equal
    bltu t0, t1, loop
    bne a0, zero, . + 8

Instructions: `beq`, `bne`, `blt`, `bge`, `bltu`, `bgeu`.

Targets must be addresses, not integer displacements. The resulting displacement
must be even and within −4096–4094 bytes of the branch instruction. Out-of-range
branches are errors; they are not expanded into longer sequences.

Direct jumps use `jal rd, target`. Omitting `rd` selects `ra`:

    jal ra, function
    jal function                 # Same as jal ra, function
    jal zero, loop               # Jump without saving a return address

The displacement must be even and within −1048576–1048574 bytes of the jump.
Use `call` or `tail` when the target may be farther away.

Indirect jumps accept these equivalent zero-offset forms:

    jalr ra, 0(t0)
    jalr ra, (t0)
    jalr ra, t0, 0
    jalr ra, t0
    jalr 0(t0)
    jalr (t0)
    jalr t0, 0
    jalr t0

Omitting the destination register selects `ra`. For a nonzero offset, use
`jalr rd, offset(rs1)` or `jalr rd, rs1, offset`, or omit the destination with
`jalr offset(rs1)` or `jalr rs1, offset`. The offset must be an integer in
−2048–2047. Two register operands always mean `rd, rs1`, with a zero offset.


### RV32I: environment instructions

`ecall` and `ebreak` take no operands. Fence instructions are listed with the
compatibility instructions below.


### M extension: multiplication and division

All use `op rd, rs1, rs2`, just like the register arithmetic instructions:

    mul a0, a1, a2
    div t0, t1, t2
    remu t3, t1, t2

Instructions: `mul`, `mulh`, `mulhsu`, `mulhu`, `div`, `divu`, `rem`, `remu`.


Pseudo-instructions
-------------------

These are the supported pseudo-instructions in addition to the abbreviated
`jal`, `jalr`, and memory forms above. A pseudo-instruction may produce more
than one machine instruction.


### Constants, addresses, and memory

| Form                     | Meaning                                               |
|--------------------------|-------------------------------------------------------|
| `li rd, imm`             | Load a 32-bit integer.                                |
| `la rd, address`         | Load an address.                                      |
| `lb rd, address`         | Load a signed byte at an address.                     |
| `lh rd, address`         | Load a signed halfword at an address.                 |
| `lw rd, address`         | Load a word at an address.                            |
| `lbu rd, address`        | Load an unsigned byte at an address.                  |
| `lhu rd, address`        | Load an unsigned halfword at an address.              |
| `sb rs, address, temp`   | Store a byte, using `temp` to hold the address.       |
| `sh rs, address, temp`   | Store a halfword, using `temp` to hold the address.   |
| `sw rs, address, temp`   | Store a word, using `temp` to hold the address.       |

    li a0, 0x12345678
    la t0, array + 4
    lw a1, count
    sw a1, count, t1

Direct loads use their destination register to form the address. Direct stores
overwrite the explicit temporary register; choose a writable register different
from the value being stored. These forms work with local as well as global labels.


### Register operations

| Form            | Equivalent instruction   |
|-----------------|--------------------------|
| `mv rd, rs`     | `addi rd, rs, 0`         |
| `not rd, rs`    | `xori rd, rs, -1`        |
| `neg rd, rs`    | `sub rd, zero, rs`       |
| `seqz rd, rs`   | `sltiu rd, rs, 1`        |
| `snez rd, rs`   | `sltu rd, zero, rs`      |
| `sltz rd, rs`   | `slt rd, rs, zero`       |
| `sgtz rd, rs`   | `slt rd, zero, rs`       |
| `nop`           | `addi zero, zero, 0`     |


### Control flow

| Form                      | Meaning or equivalent instruction               |
|---------------------------|-------------------------------------------------|
| `beqz rs, target`         | `beq rs, zero, target`                          |
| `bnez rs, target`         | `bne rs, zero, target`                          |
| `blez rs, target`         | `bge zero, rs, target`                          |
| `bgez rs, target`         | `bge rs, zero, target`                          |
| `bltz rs, target`         | `blt rs, zero, target`                          |
| `bgtz rs, target`         | `blt zero, rs, target`                          |
| `bgt rs1, rs2, target`    | `blt rs2, rs1, target`                          |
| `ble rs1, rs2, target`    | `bge rs2, rs1, target`                          |
| `bgtu rs1, rs2, target`   | `bltu rs2, rs1, target`                         |
| `bleu rs1, rs2, target`   | `bgeu rs2, rs1, target`                         |
| `j target`                | `jal zero, target`                              |
| `jr rs`                   | `jalr zero, rs, 0`                              |
| `ret`                     | `jalr zero, ra, 0`                              |
| `call target`             | Call an address, saving the return in `ra`.     |
| `tail target`             | Jump to an address without changing `ra`.       |

`call` can use `ra` as an address temporary. `tail` can overwrite `t1`.
Both can reach targets beyond the range of `jal`; by default they become a
single jump when the target is close enough.

`jr` also accepts `jr offset(rs)` and `jr rs, offset`. `jr (rs)` omits a zero
offset. These forms use `zero` as the destination of `jalr`.


### Omitted pseudo-instructions

Notable omissions include `lla`, `lga`, `sext.b`, `sext.h`, `zext.b`, `zext.h`,
`sext.w`, `zext.w`, and `negw`. CSR and counter aliases such as `csrr`, `csrw`,
`rdcycle`, `rdtime`, and `rdinstret` are absent, as are floating-point aliases.
`call` accepts only a target, not an explicit destination register.


Compressed, atomic, and fence instructions
------------------------------------------

These instructions expand compatibility with existing binaries. Students are
not expected to write them directly. See the
[RISC-V Unprivileged ISA specification](https://docs.riscv.org/reference/isa/unpriv/unpriv-index.html)
for instruction semantics, encodings, and architectural restrictions. The
accepted source spellings are listed here.


### C extension: RV32 integer compressed instructions

| Operand form                    | Instructions                                                      |
|---------------------------------|-------------------------------------------------------------------|
| `op rd, rs2`                    | `c.add`, `c.mv`, `c.sub`, `c.xor`, `c.or`, `c.and`                |
| `op rd, imm`                    | `c.li`, `c.lui`, `c.addi`, `c.slli`, `c.srli`, `c.srai`, `c.andi` |
| `c.addi16sp sp, imm`            | `c.addi16sp`                                                      |
| `c.addi4spn rd, sp, imm`        | `c.addi4spn`                                                      |
| `op rd, offset(base)`           | `c.lw`, `c.lwsp`                                                  |
| `op rs2, offset(base)`          | `c.sw`, `c.swsp`                                                  |
| `op rs1, target`                | `c.beqz`, `c.bnez`                                                |
| `op target`                     | `c.j`, `c.jal`                                                    |
| `op rs1`                        | `c.jr`, `c.jalr`                                                  |
| No operands                     | `c.nop`, `c.ebreak`                                               |

The compact register subset is `x8–x15` (`s0`, `s1`, `a0–a5`). It is required
for `c.sub`, `c.xor`, `c.or`, `c.and`, `c.srli`, `c.srai`, `c.andi`, `c.beqz`,
`c.bnez`, `c.addi4spn`'s destination, and both registers of `c.lw` and `c.sw`.
`c.lwsp` and `c.swsp` require `sp` as their base. Compressed memory forms allow
an omitted zero offset, such as `c.lw a0, (a1)` or `c.swsp a0, (sp)`.

`c.lui` uses an unsigned 20-bit immediate field, like `lui`. The accepted values
are 1–31 and `0xfffe0`–`0xfffff`; zero is reserved.

Explicit compressed branches and jumps accept either an address target or an
integer byte displacement. Other compressed immediate operands accept the
numeric value of either expression type, subject to their encoding limits.
This is less strict than the ordinary instruction forms.

Writing `c.*` explicitly always requests a compressed instruction. Automatic
compression of ordinary instructions is off by default and can be enabled with
`--relax-compressed`.


### A extension: word atomics

Instructions: `lr.w`, `sc.w`, `amoswap.w`, `amoadd.w`, `amoxor.w`, `amoand.w`,
`amoor.w`, `amomin.w`, `amomax.w`, `amominu.w`, `amomaxu.w`.

    lr.w a0, (a1)
    sc.w a0, a2, (a1)
    amoadd.w.aqrl a0, a2, (a1)

`lr.w` takes `rd, (rs1)`. All others take `rd, rs2, (rs1)`. Offsets are not
accepted. Every instruction accepts no ordering suffix, `.aq`, `.rl`, or
`.aqrl`. Doubleword atomics are not supported.


### Fences

Accepted forms are `fence`, `fence pred, succ`, `fence.i`, and `fence.tso`.
`fence` alone means `fence iorw, iorw`. Each set is a nonempty combination of
`i`, `o`, `r`, and `w`, such as `fence rw, rw` or `fence r, w`.
Numeric fence masks are not accepted.

`fence` is part of the base integer ISA. `fence.i` is the Zifencei instruction;
`fence.tso` is the TSO fence form. Neither takes operands here.


Expressions
-----------

Expressions are evaluated during assembly. Operands can be integer literals,
character literals, symbols, numeric label references, or `.` for the current
address. In numeric data lists, `.` advances for each value. Parentheses group
expressions.


### Literals and operators

Integer literals accept decimal (`123`), hexadecimal (`0x7b`), binary
(`0b1111011`), and octal (`0o173` or `0173`). A leading zero selects octal.
The radix letters may be uppercase. Literals represent 32-bit values;
`0xffffffff` is the integer `-1`.

The minimum signed integer can be written as `-2147483648` or `-0x80000000`.
The reference `0b` means the preceding `0:` label; a binary literal needs at
least one digit after its prefix, as in `0b0` or `0b1010`.

Character literals such as `'A'` are integer Unicode code points. Strings use
double quotes and are available only in string directives. Both accept
`\n`, `\t`, `\r`, `\\`, `\'`, `\"`, and `\0`. Hexadecimal and octal escape
sequences are not supported.

Operators, from highest to lowest precedence:

| Operators      | Meaning                                        |
|----------------|------------------------------------------------|
| `-`, `~`       | Unary negation and bitwise complement          |
| `*`, `/`, `%`  | Multiplication, division, remainder            |
| `+`, `-`       | Addition and subtraction                       |
| `<<`, `>>`     | Left shift and arithmetic right shift          |
| `&`            | Bitwise AND                                    |
| `^`            | Bitwise XOR                                    |
| `\|`           | Bitwise OR                                     |

Binary operators at the same precedence associate left to right. Shift counts
must be 0–31; left shifts discard bits shifted out. Integer arithmetic is signed
32-bit; arithmetic overflow and division by zero are errors. Division truncates
toward zero. Unary `+`, comparisons, logical operators, and relocation operators
such as `%hi`, `%lo`, `%pcrel_hi`, and `%pcrel_lo` are not supported.

    .equ COUNT, 12
    .equ BYTES, COUNT * 4
    li t0, (1 << 5) | 3
    addi a0, a0, 'a' - 'A'
    la t1, array + BYTES


### Integers and addresses

Labels, numeric label references, and `.` have address type. Literals have
integer type. A symbol defined by `.equ` or `.set` keeps the type of its
expression.

| Expression types                | Result                                |
|---------------------------------|---------------------------------------|
| Integer + integer               | Integer                               |
| Integer − integer               | Integer                               |
| Address + integer               | Address                               |
| Integer + address               | Address                               |
| Address − integer               | Address                               |
| Address − address               | Integer byte distance                 |
| Address + address               | Error                                 |
| Integer − address               | Error                                 |
| Other operators on addresses    | Error                                 |

For example, `array + 4` is an address, while `end - array` is an integer size.
Address arithmetic must stay within the unsigned 32-bit address range.

Ordinary instruction immediates, memory offsets, `li`, allocation sizes, and
alignment values require integers. `la`, direct label loads and stores, and
ordinary branch and jump targets require addresses. These rules catch mistakes
such as:

| Rejected source       | Reason and intended form                                          |
|-----------------------|-------------------------------------------------------------------|
| `li a0, array`        | An address is not an integer; use `la a0, array`.                 |
| `la a0, 42`           | An integer is not an address; use `li a0, 42`.                    |
| `lw a0, array(t0)`    | A register offset must be an integer, not a full address.         |
| `j 8`                 | A target must be an address; use `j . + 8` for a displacement.    |
| `.space end`          | An address is not a size; use a difference such as `end - start`. |

This is assembly-time expression checking, not tracking the types of values
held in registers. Numeric data directives accept both types so that, for
example, `.word array` can store a pointer. Explicit compressed operands have
the exceptions described above.


Directives
----------


### Sections and symbols

| Directive                  | Meaning                                                   |
|----------------------------|-----------------------------------------------------------|
| `.text`                    | Select instruction storage; each file starts here.        |
| `.data`                    | Select initialized data storage.                          |
| `.bss`                     | Select zero-initialized storage.                          |
| `.global name, ...`        | Export symbols defined in this file.                      |
| `.globl name, ...`         | Alias for `.global`.                                      |
| `.equ name, expression`    | Define or redefine a symbol's value.                      |
| `.set name, expression`    | Alias for `.equ`.                                         |

An exported symbol must be defined in the exporting file and must be unique
across files. A reference to another file's exported symbol needs no declaration
in the importing file. `.L` names obey the same scope rules as other named labels.

`.equ` and `.set` may redefine a symbol. A reference uses the most recent
preceding definition, or the next definition if none precedes it. Redefinition
does not change earlier references. Circular definitions are errors.


### Data and alignment

| Directive                    | Meaning                                           |
|------------------------------|---------------------------------------------------|
| `.byte expr, ...`            | Emit one byte per value.                          |
| `.half expr, ...`            | Emit two bytes per value; alias: `.2byte`.        |
| `.word expr, ...`            | Emit four bytes per value; alias: `.4byte`.       |
| `.ascii "text", ...`         | Emit UTF-8 bytes without a terminator.            |
| `.asciz "text", ...`         | Emit each string followed by a zero byte.         |
| `.string "text", ...`        | Alias for `.asciz`.                               |
| `.space size[, fill]`        | Reserve `size` bytes; default fill is zero.       |
| `.zero size[, fill]`         | Alias for `.space`, including optional fill.      |
| `.balign alignment`          | Pad with zero bytes to a byte-address multiple.   |

Numeric data is little-endian. `.byte` accepts integers from −128 to 255;
`.half` and `.2byte` accept integers from −32768 to 65535. Negative values use
two's-complement representation. Addresses must fit as unsigned values in the
chosen width: 0–255 for bytes and 0–65535 for halfwords. Out-of-range values
are errors. `.word` and `.4byte` store any 32-bit integer or address.

Data and fill range checks use the final addresses after relaxation.

These directives do not align their data automatically. Put `.balign` before
the label whose address needs alignment:

    .data
    message: .asciz "Hello\n"
    .balign 4
    array:   .word 10, 20, 30
    pointer: .word array
    .equ ARRAY_BYTES, pointer - array

`size` must be a nonnegative integer. `fill` must be an integer from −128 to
255, even when `size` is zero; negative fills use two's-complement representation.
`alignment` must be a positive integer, not a power-of-two exponent.
`.balign 4` aligns to four bytes. Fill and maximum-skip arguments to `.balign`
are not supported. Within a list of numeric data values, `.` is the address of
the current item, matching GNU `as`. For example,
`start: .word . - start, . - start, . - start` emits `0, 4, 8`.

In `.bss`, reserve storage with `.space` or `.zero` and a zero fill byte.
`.balign` reserves zero-initialized padding without adding bytes to the
executable. Labels and symbol directives are allowed. Instructions and
initialized data directives are not allowed there.


### Omitted directives and source features

The directives above are the complete supported set. Common omissions include:

*   General sections and alignment: `.section`, `.rodata`, `.pushsection`,
    `.popsection`, `.align`, `.p2align`, and `.org`.
*   Other data forms: `.dword`, `.8byte`, `.quad`, `.float`, `.double`, `.comm`,
    `.lcomm`, and `.fill`.
*   Metadata and instruction controls: `.type`, `.size`, `.attribute`, `.option`,
    `.file`, `.loc`, `.cfi_*`, and `.insn`.
*   Source expansion: `.include`, `.incbin`, `.macro`, `.rept`, conditional
    assembly (`.if`, `.ifdef`), and C preprocessor directives.

There is no `name = expression` assignment syntax; use `.equ` or `.set`.


Global pointer and relaxation
-----------------------------

The built-in address symbol `__global_pointer$` is the start of `.data` plus
2048 bytes. It is available without a source definition or `.global`
declaration. Use it to initialize `gp`:

    la gp, __global_pointer$

By default, this exact form anywhere in the input enables `gp` relaxation for
the program; `x3` is also accepted in place of `gp`. An alias, parenthesized
expression, or added `+ 0` does not trigger detection. Without the recognized
initialization, `gp` relaxation is off by default.

With `gp` relaxation enabled, `la rd, address` can become `addi rd, gp, offset`
when the address is within −2048–2047 bytes of `__global_pointer$`. The program
must execute the initialization before using such instructions and preserve
`gp` afterward. Detection does not check execution order.

An `la` whose destination is `gp` is never rewritten to depend on `gp` itself.
The initialization therefore needs no `.option norelax` wrapper; `.option` is
not supported. Direct label loads and stores use PC-relative addressing and
are not subject to this `gp` optimization.

`--relax-gp` explicitly enables this optimization and `--no-relax-gp` disables
it. `--relax` enables all relaxation options, including `gp` relaxation;
`--no-relax` disables them. Explicit enabling requires the program to establish
the expected `gp` value even if the initialization was not recognized.
