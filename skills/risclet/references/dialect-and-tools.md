Dialect and tools
=================

Commands and execution environment
----------------------------------

    risclet run --strict start.s function.s print.s
    risclet trace --strict start.s function.s print.s
    risclet debug --strict start.s function.s print.s
    risclet disassemble start.s function.s print.s
    risclet assemble start.s function.s print.s -o program

Bare `risclet` means `run`. Without explicit inputs, commands assemble current-directory `*.s` in sorted order; executing/disassembling commands fall back to `a.out` only if no sources exist. Explicit source order controls layout. A single non-`.s` positional input to simulation commands selects an executable; `-e`/`--executable` also selects one. Do not mix source and executable inputs. `assemble` requires sources and accepts explicit paths regardless of extension. Run, trace, debug, and disassemble assemble in memory without writing an intermediate executable. Assemble writes `a.out` unless `-o` changes it.

The simulator supports RV32I, M, word A, and integer C, with accepted fences. Default teaching code should use RV32IM; RV64, floating point, vectors, CSR/counter instructions, and privileged instructions are unavailable. Arithmetic in registers wraps at 32 bits. Signed and unsigned comparisons and loads differ; `lb`/`lh` sign-extend, `lbu`/`lhu` zero-extend. The stack is 8192 bytes; a growable heap is available through `brk`, without a malloc/free allocator or general OS emulation. The default execution limit is 100,000,000 machine instructions; inspect command help for overrides.

`ecall` uses `a7` for the operation:

| Call         | a7  | Arguments                        | Result                   |
|--------------|-----|----------------------------------|--------------------------|
| read stdin   | 63  | a0=0, a1=buffer, a2=byte count    | a0=bytes read            |
| write stdout | 64  | a0=1, a1=buffer, a2=byte count    | a0=bytes written         |
| exit         | 93  | a0=status                        | Low 8 bits become status |
| brk          | 214 | a0=requested break, 0 to query   | a0=resulting break       |

Read can return fewer bytes than requested or zero at EOF. Only bytes actually read become initialized. Printing integers or zero-terminated strings requires assembly helper functions, not RARS-style print syscalls. Discover the actual helper contract rather than assuming a standard library. `ebreak` is an execution error, not a debugger breakpoint.

`brk` uses the raw Linux-style syscall return convention: success returns the requested break; failure returns the unchanged break, not -1 or an errno. Query with zero, retain the old break as the start of newly acquired storage, then request an absolute end address rather than a byte increment. Compare the returned address with the requested address before using the storage. The heap starts empty on a 4096-byte boundary at or after the highest loaded section end. Its exclusive break is byte-exact and cannot move below the heap start or above the fixed stack start. The alignment gap stays unmapped. Growth supplies physical zeroes; shrinking discards bytes, and regrowth supplies fresh zeroes. Heap memory is writable and nonexecutable. Reset restores the empty heap.


Syntax and expressions
----------------------

Each case-sensitive line has an optional label and at most one instruction/directive. Use commas between operands and `#` comments. ABI register names and `x0`–`x31` are accepted; `fp` aliases `s0`. Labels and constants cannot use register names.

Named symbols are file-local unless exported. Put `.global function` in its defining file, never in a caller merely to reference it. Exported names must be defined there and unique. Numeric labels may repeat; `1f` and `1b` find the next/preceding `1:` but cannot cross a file boundary or any `.text`, `.data`, or `.bss` directive. `.L` labels are accepted but are not the preferred teaching style.

Labels and `.` are addresses. Literals are integers. `.equ` retains its expression's kind. Address plus integer yields an address; address minus address yields an integer distance. Adding two addresses, subtracting an address from an integer, and bitwise operations on addresses fail. Use `la a0, array`, `li a1, count`, and `j . + 8`; `li a0, array`, `la a0, 42`, and `j 8` fail. A memory offset must be an integer: load the base with `la`, then use `lw a0, 4(t0)` rather than `lw a0, array(t0)`.

Decimal, `0x`, `0b`, `0o`, and leading-zero octal literals are accepted; avoid accidental octal. `0xffffffff` is integer -1; `0b` alone refers backward to label `0:`. Character literals give Unicode code points; strings emit UTF-8. Escapes are `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, and `\0`, without hex/octal escapes.

Precedence, highest first: unary `- ~`; `* / %`; `+ -`; `<< >>`; `&`; `^`; `|`. Binary operators associate left. Assembly arithmetic checks signed 32-bit overflow and division by zero; shift counts are 0–31. Addresses stay within unsigned 32 bits. There are no expression comparisons, logical operators, unary plus, or `%hi`/`%lo` relocations.

Ordinary 12-bit immediates and memory offsets are -2048 through 2047, even for bitwise and unsigned operations. Use `-1`, not `0xfff`, for all twelve bits set. Shift immediates are 0–31; `lui`/`auipc` take an unshifted unsigned 20-bit field. Conditional branches compare registers, not an immediate constant; load nonzero constants first. Ordinary branches require address targets and even displacements -4096 through 4094. `jal` reaches -1048576 through 1048574. Out-of-range branches are errors, not automatic far branches.

Prefer `li`, `la`, `mv`, `neg`, `not`, zero-test and reversed-comparison branch pseudos, `j`, `jal function`, and `ret`. `jal function` implies destination `ra`; `j` does not write a link. `call` and `tail` support distant targets; `tail` can clobber `t1`. Direct loads such as `lw a0, count` form the address in the destination. Direct stores require an explicit writable temporary distinct from the stored value: `sw a0, count, t0`. Zero offsets may be omitted as `(t0)`. Pseudos may expand into multiple machine instructions.

Explicit `c.*` forms request compressed encodings; ordinary instruction compression is disabled by default (`--relax-compressed` enables it). Explicit compressed operands have documented type/encoding exceptions: consult the installed syntax reference before writing them. Do not introduce compressed or atomic forms into beginner examples merely because the tool supports them.


Directives and layout
---------------------

| Forms                           | Behavior                                                |
|---------------------------------|---------------------------------------------------------|
| .text / .data / .bss             | Code / initialized data / zero-initialized reservation   |
| .global names / .globl names     | Export definitions from this file                       |
| .equ name, expr / .set name, expr| Define or redefine a typed assembly-time value          |
| .byte / .half / .word            | Emit 1 / 2 / 4 bytes per expression, little-endian       |
| .2byte / .4byte                  | Aliases for .half / .word                               |
| .ascii strings                  | Emit UTF-8 without terminators                          |
| .asciz strings / .string strings| Append a zero byte to each string                       |
| .space size[, fill] / .zero ...  | Reserve bytes, fill defaults to zero                    |
| .balign multiple                | Pad with zeros to a byte multiple, not a power exponent |

There is no automatic data alignment. Put `.balign 4` BEFORE a word-array label, especially after strings/bytes. `.byte` integers may be -128 through 255; halfwords -32768 through 65535. Addresses must fit unsigned in those widths. Words accept any 32-bit integer/address. `.space` sizes are nonnegative integers and fill is -128 through 255. BSS allows only zero-filled reservations/alignment and symbol directives, not initialized data directives or instructions.

Use `.equ count, (. - array) / 4` immediately after a word array; omit division for bytes. In numeric lists `.` advances per item. `.equ`/`.set` references bind to the preceding definition, or the next if none precedes; later redefinition does not retarget earlier uses.

Unsupported GNU features include `.section`, `.rodata`, `.align`, `.p2align`, `.type`, `.size`, `.option`, `.include`, `.macro`, conditional assembly, and preprocessor directives. Supply multiple source files to the command instead of including them. Use `.data`, `.balign`, and `.equ`, not unsupported boilerplate.

Export `_start` as the entry point. `_start` is not called and must exit with `ecall`, not `ret`. If using GP relaxation, execute exactly `la gp, __global_pointer$` before relaxed address loads. This recognized source form enables GP relaxation automatically, but detection does not verify execution order. The built-in pointer is data-start + 2048. Preserve `gp`; do not add an unsupported `.option norelax` wrapper. `--no-relax-gp` disables this optimization; `--no-relax` disables relaxation generally. Explicit `--relax` enables GP relaxation too, so requires initialization.


Strict checks
-------------

Use strict mode as the normal teaching validation. Initially only `zero` and `sp` are logically usable; other physical zero bits are not initialized values. At calls, temporaries become unavailable. Define `.equ function_args, N` with integer N from 0 to 8 for each function. It requires initialized `a0` through `a(N-1)` at entry and makes later argument registers unavailable. Without this metadata, strict mode cannot enforce the full argument contract. The checker reads retained ELF constant names, so metadata can live in a separate harness without being exported. Avoid competing definitions of the same metadata name across files.

After a return, only `a0` carries the supported result; other a- and t-registers are unavailable regardless of whether the callee physically overwrote them. Preserve values needed across calls in properly saved s-registers or stack slots. Calls must write `ra` and target a symbol. Conventional `ret` must match a call. Non-linking tail jumps do not create a new call context.

Incoming s-registers are save-only until assigned a local value. Preserve and restore the original identities of used s-registers and `ra`, not merely equal numeric bits. `gp` and `tp` must not be modified inside a callee. Restore the entry `sp` at return. Every write to `sp` must leave it 16-byte aligned; scalar loads/stores require natural alignment.

Stack reads require initialized allocated bytes; no accesses below `sp`. Releasing stack bytes invalidates them logically even though their bits remain visible. Full-word saves/loads preserve identity. Partial overwrites, mixed-width reinterpretation, and assembling a word from unrelated byte writes can fail strict mode. Syscall byte buffers cannot overlap tracked halfword/word data. Explain these as deliberate pedagogical constraints, not universal RISC-V hardware rules. Checks follow actual executed paths; passing tests leave other paths unproven.

New heap bytes require explicit initialization in strict mode despite their physical zero fill. This applies to scalar reads and buffers sent to `write`; a `read` initializes only received bytes. Shrinking discards tracked identities and widths, and regrowth does not restore them. Heap scalar accesses follow the usual alignment and matching-width rules; untouched static data/BSS retains its separate lazy-initialization behavior.


Assembler and linker teaching
-----------------------------

Use reports to answer a concrete question:

    risclet assemble main.s helper.s --dump-ast
    risclet assemble main.s helper.s --dump-symbols
    risclet assemble main.s helper.s '--dump-layout=*'
    risclet assemble main.s helper.s --dump-code
    risclet assemble main.s helper.s --dump-elf=headers,sections,symbols

Parsing produces structured source and normalizes simple pseudos. Symbol linking binds references to definitions before assigning addresses. Layout, expression evaluation, and encoding repeat until instruction sizes stabilize, with a ten-iteration limit. Symbol identities stay fixed while values move. Each file starts in text; section contents accumulate in input order. Data starts on the next page after text and BSS follows data. ELF records the final bytes, entry point, loading metadata, and retained symbols; BSS occupies memory without stored initialization bytes. There are no intermediate object files, external linker script, or dynamic linking steps.

Dump options stop after the latest requested stage and do not write an executable. Early reports can succeed without a valid entry point or complete executable. Layout/code selectors use `PASSES:FILES`, e.g. `--dump-layout=1-3` or `'--dump-code=:main.s'`; quote wildcard selectors. Intermediate addresses can still reflect previous size estimates; explain the settled `FINAL` pass before drawing conclusions. `--verbose` adds iteration size summaries. A code dump pairs bytes with parsed source; it is not disassembly of the emitted instructions.

`risclet disassemble --verbose-instructions` exposes machine instructions rather than recovered pseudos. `--no-show-addresses --no-show-encoding` removes listing columns. Disassembly is not complete source: data, section directives, and exports still need reconstruction. Numeric labels can be renumbered and pseudo recovery does not preserve original spelling.
