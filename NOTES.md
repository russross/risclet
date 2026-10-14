Core semantics
==============

This is the behavioral and design contract for Risclet. It describes what a student can do, what each operation means, and which relationships must survive changes to the implementation. Internal modules, storage representations, exact diagnostic wording, and equivalent drawing techniques may change. The observable distinctions, ordering, defaults, and debugger interactions below are part of the design.

Risclet serves small, hand-written assembly programs used to learn the hardware/software interface. One command handles assembly, linking, execution, disassembly, strict checking, and recorded debugging. The student should be able to inspect how an instruction changes registers, bytes, control flow, and I/O without managing a toolchain, linker script, debugger setup, or breakpoints. A correct instruction simulator with a generic debugger does not by itself preserve this design.

[SYNTAX.md](SYNTAX.md) is part of this contract: it supplies the complete accepted source forms, operand restrictions, directives, and expression rules. [ASSEMBLER.md](ASSEMBLER.md) gives worked explanations of the assembly passes. The browser application has its own [semantics](demo/SEMANTICS.md), as does [syntax highlighting](syntaxhighlighting/SEMANTICS.md). References to current source and tests are audit aids, not requirements to reproduce their structure.

Unless explicitly identified as a current limitation, the behavior below is to be preserved. A limitation documents the boundary of the existing guarantee; it is not a reason to perpetuate a defect. Deliberately changing a contract requires updating the corresponding examples and regression expectations, rather than silently weakening them.


Command and input resolution
----------------------------

`src/main.rs` obtains a `CliAction` from `src/config.rs` before reading program input. Help and version requests return without assembling or loading a program. Only the first argument can select a command; the default command is `run`.

1. Explicit source arguments preserve their supplied order. For simulation commands, every source path must end in `.s`; one non-`.s` positional argument selects an executable. Mixing the two kinds or supplying multiple executables is an error.
2. `-e`/`--executable` selects an executable directly and cannot accompany positional files. Without explicit input, discovery sorts the current directory's `*.s` files by name. Those files take precedence over `a.out`.
3. `assemble` accepts source paths regardless of extension, discovers `*.s` when none are specified, and fails if there are none. It never falls back to loading an executable.
4. Source input for `run`, `trace`, `debug`, and `disassemble` produces ELF bytes in memory. File input and those bytes converge at the same ELF loader. These commands do not write `a.out` as an intermediate result.

`Config` carries command, input, output, relaxation, execution-limit, and display settings through the pipeline. Option order determines the last effective setting. Defaults include a text base of `0x10000`, an execution limit of 100,000,000 machine instructions, pseudo-instruction relaxation enabled, automatic compressed encoding disabled, and GP relaxation selected by source inspection. Display preferences do not change executable bytes or instruction execution.

| Operation     | Default result                                                                |
|---------------|-------------------------------------------------------------------------------|
| `run`         | Assemble/load, perform program I/O, and exit with the recorded program status |
| `trace`       | Run while showing instructions and effects, with addresses and no encodings   |
| `assemble`    | Write executable `a.out`; remain silent on success unless inspection is asked |
| `disassemble` | Show decoded code with addresses and encodings; perform no execution          |
| `debug`       | Run and record, then open replay with pseudo-instructions and no addresses    |

Decimal values and non-strict execution are defaults. Display, strictness, relaxation, and step-limit controls are explicit overrides. Invalid options, incompatible input combinations, missing input, assembly failures, and loading failures produce a readable diagnostic and unsuccessful process status. They must not appear successful because an earlier phase completed. Dump options belong to `assemble`, not to execution commands.


Teaching-language contract
--------------------------

Source is line-oriented, case-sensitive, and intentionally restricted. A line has an optional label and at most one instruction or directive; `#` introduces a comment. Register numbers and ABI names denote the same registers, including `fp` as `s0`. There is no implicit macro expansion, include processing, preprocessor, general section system, or GNU-assembler compatibility mode. Unsupported forms fail rather than being silently ignored.

Names are file-local unless exported. Ordinary forward references work. Numeric labels can be repeated, but `1b`/`1f` searches cannot cross a file boundary or even a repeated section-selection directive. `.equ` and `.set` uses bind to a particular definition before values are evaluated; later redefinition must not retroactively retarget a previous use. Exporting a name in a caller does not declare an external reference: it requires a definition in that file.

Assembly-time integers and addresses are different kinds of values. A label, numeric label reference, and `.` are addresses; literals are integers; a constant definition retains its expression's kind. Address plus integer is an address, address minus address is an integer distance, and two addresses cannot be added. Ordinary `li` and memory offsets require integers; `la` and ordinary branches require addresses. Thus `li a0, array` and `j 8` fail, while `la a0, array` and `j . + 8` express the intended operations. Explicit compressed forms have the documented numeric-operand exceptions.

Assembly arithmetic uses checked signed 32-bit integer arithmetic and bounded unsigned 32-bit address arithmetic, not the wrapping arithmetic of executed instructions. Full-width literal bit patterns such as `0xffffffff` denote signed integer values. Division by zero, overflow, invalid shifts, and cyclic definitions fail. Operator precedence, left associativity, radix conventions, supported character/string escapes, and the distinction between `0b` as a backward label reference and a binary literal follow the language reference.

Data directives emit little-endian values without automatic alignment. `.byte` and halfword directives accept their documented signed integers or unsigned bit patterns, but an address must fit unsigned in that width. `.word` accepts any 32-bit integer or address. In a numeric list, `.` advances for each item: `start: .word . - start, . - start, . - start` emits 0, 4, 8. String directives encode strings and add terminators only for the zero-terminated forms. `.balign` takes a positive byte multiple, not an exponent; `.space` takes a nonnegative integer and checks its fill even for zero size. BSS permits zero-filled reservation and alignment but rejects instructions and initialized data.

Instruction validation must catch out-of-range immediates and branch displacements before emission. Ordinary 12-bit immediates use the signed range even for bitwise and unsigned-comparison instructions. Upper immediates are unshifted 20-bit fields. An out-of-range branch is an error, not an automatically synthesized far branch. `call` and `tail` can use far sequences, and direct stores explicitly name their writable temporary register. These restrictions are teaching aids, not incidental parser choices.


Source to executable
--------------------

`assemble_files` in `src/assembler.rs` reads every source as text before calling `assemble`. The pipeline retains separate representations: `Source` contains parsed files and lines, `SymbolLinks` binds references to definitions, `Layout` assigns section offsets and addresses, and `SymbolValues` contains evaluated integers and addresses. Source locations remain available for diagnostics. Later passes do not rewrite the parsed source to store their results.

1. Tokenize and parse each nonempty source line, retaining its filename and original line number. A label and instruction can become separate records. Simple pseudo-instructions normalize during parsing; size-dependent pseudo-instructions remain available to encoding. Append the built-in source defining `__global_pointer$`.
2. Bind names before assigning addresses. File-local definitions take precedence over exported definitions from other files. An export must have a definition in its declaring file; conflicting exports and unresolved references fail. Numeric-label searches stop at file and section-directive boundaries. Redefinable constants bind each use to its preceding definition, or its next definition when none precedes it.
3. Estimate line sizes, then repeatedly compute layout, evaluate symbols and expressions, and encode bytes with revised line sizes. Each file begins in `.text`; offsets within each of `.text`, `.data`, and `.bss` accumulate across files in input order. Headers precede code, data begins at the next page boundary after text, and BSS follows data.
4. Accept encoding only when all line sizes stabilize. Addresses and values are recomputed on each pass while symbol identities stay fixed. Integer and address values remain distinct; expression cycles and invalid operand types fail. Data and fill range errors deferred during relaxation must be checked at convergence. Failure to converge within ten passes is an error.
5. Build ELF from the stable layout and bytes. A globally exported `_start` supplies the entry address. BSS reserves memory without emitting its contents into the file. Retained symbols receive their final values; numeric labels and unexported `.L` symbols are omitted from the executable symbol table.

GP relaxation defaults to enabled only when the parsed source contains the recognized `la gp, __global_pointer$` initialization. This is syntactic detection, not proof that execution reaches that instruction. Explicit flags override detection. The built-in GP value is the data start plus 2048; relaxation must not eliminate the initialization needed to establish that value at runtime.

Dump requests are successful inspection operations. The latest requested phase determines how far assembly proceeds; earlier requested phases also print their reports. Early dumps need not satisfy later requirements such as `_start`. `AssemblyOutput::Dumped` prevents both execution and output-file creation. A dump must never truncate an existing executable.

Only `assemble_and_save` writes the completed ELF, then sets mode `0755`. Assembly failure occurs before opening the destination. Once saving starts, the implementation uses direct create/write operations rather than atomic replacement; a filesystem write failure can leave a partial output file.


Executable to machine
---------------------

`src/elf.rs` parses the shared ELF representation; `src/elf_loader.rs` applies executable policy and constructs `Machine`. Input must be a supported ELF32 little-endian System V RISC-V executable with program headers and a symbol table. The loader rejects unsupported relocation and dynamic-linking section types. Section names are not the authority for finding the symbol table or its linked string table.

Allocated, nonempty `PROGBITS` and `NOBITS` sections define simulator memory regions. Their section flags determine write and execute permissions. Initialization bytes come from load-segment data at the section address; the remaining region is zero-filled. Regions cannot begin at zero or have an overflowing end address. This section-based loading model requires section metadata even though a native OS loader may rely primarily on program headers.

`Machine` owns CPU state, memory, symbol maps, instruction effects, I/O recording, and atomic reservation state. Reset restores each region from its initial bytes, zero-fills reserved space, clears registers, sets `sp` to the top of an 8192-byte stack, and selects the ELF entry PC. Register `x0` always reads zero and ignores writes. The loaded global-pointer symbol is metadata for interpretation and display; reset does not initialize the `gp` register from it.

Memory accesses must fit within a single region; stores additionally require write permission. Instruction fetch requires executable memory and reads either two or four bytes according to the encoding. An incomplete instruction at a region boundary fails. Integer memory encoding is little-endian.

The normal small-program stack occupies the 8192 bytes ending at `0x100000` and grows toward lower addresses. Stack allocation is separate from program data; initial stack bytes are physically zero, but strict mode treats them as uninitialized. There is no process argument/environment stack, heap allocator, filesystem syscall interface, or operating-system emulation in the core. The Linux environment in the browser demo surrounds Risclet; it is not the environment implemented by the RV32 simulator.


Instruction semantics and execution limits
------------------------------------------

The supported execution domain is RV32 integer instructions with M, word-sized A, and integer C extensions, plus the accepted fence forms. RV64, floating-point, vector, CSR/counter, and privileged instructions are outside that domain. Compressed operations retain two-byte instruction length even when their behavior is expressed as an ordinary integer operation.

| Operation family | Observable requirements                                                                                                       |
|------------------|-------------------------------------------------------------------------------------------------------------------------------|
| Arithmetic       | Register values are 32-bit patterns; add/subtract/multiply wrap. Signed and unsigned comparisons remain distinct.             |
| Shifts           | Register shift amounts use the low five bits. Arithmetic right shift sign-extends; logical right shift inserts zeros.         |
| Loads/stores     | Byte and halfword loads sign- or zero-extend as requested; stores use the low bytes. Effective addresses wrap to 32 bits.     |
| Control flow     | PC-relative transfers use the instruction's address; links use its address plus actual length. `jalr` clears target bit zero. |
| M extension      | High products use the specified signedness. Division by zero yields all ones; remainder by zero returns the dividend.         |

Signed division truncates toward zero. Dividing the minimum signed value by −1 returns that minimum value; its remainder is zero. A source register shared with a jump's destination is read before writing the link. Writes to `zero` are discarded, but the rest of the instruction still occurs: a load to `zero` still accesses memory and can fail.

Fences have no observable effect in this single-machine execution model. `ebreak` terminates with an execution error; it does not open a live breakpoint session. Unsupported decoded instructions fail when executed. Atomic word operations return the old word and perform the specified replacement; load-reserved/store-conditional use an address reservation and return zero for a successful conditional store and one for failure. Ordering suffixes do not add concurrent execution to the simulator.

The current atomic model is limited: a matching store-conditional consumes the reservation, but a failed attempt at a different address leaves it intact; ordinary stores do not invalidate it or model interference from another hart. These are current compatibility limitations, not a complete architectural reservation model. Ordinary non-strict data accesses permit unaligned bytes within a valid region; strict scalar accesses impose natural alignment. Do not claim system-level concurrency, exception, or memory-model conformance from support for these instruction encodings.


Decode, execute, and stop
-------------------------

`run_simulator` in `src/simulator.rs` decodes the text address interval before dispatching the selected mode. It creates an address-to-instruction map, adds local target labels, and groups recognizable machine sequences for pseudo-instruction display. `disassemble` prints this representation and returns without executing instructions or performing guest I/O.

`trace` in `src/execution.rs` drives all executing modes. The PC selects a decoded instruction; a missing next instruction terminates the recording with an error when an effect is available to carry it. `Op::execute` implements RV32 integer, multiply/divide, atomic, and compressed operations. Each executed instruction produces `Effects` containing old/new PC, register and memory changes, memory reads, and optional syscall, frame, or terminal information. Sequential PC advancement uses the decoded instruction length; control transfers supply their own PC change.

The simulated syscall interface uses `a7` for the operation and `a0`–`a2` for arguments. It supports `read` (63) from fd 0, `write` (64) to fd 1, and `exit` (93). Read/write return the byte count in `a0`; negative counts, unsupported descriptors, and unknown calls fail. Exit retains the low eight bits of `a0`. These calls interact with the host streams during initial execution, not during debugger navigation.

Execution ends on an exit, instruction/memory/syscall error, strict-check failure, or step limit. The limit counts machine instructions, including the individual instructions behind a displayed pseudo-instruction. Failures are recorded effects, not rollback transactions: the instruction may already have changed state or performed I/O before a later check rejects it.

`run` and `trace` propagate a recorded guest exit status to the host process and report recorded execution failures with status 1. `run` also treats ending without a terminal result as an error. `trace` prints syscall signatures before executing them, so a pending input operation is visible. Pseudo-instruction reporting combines effects for display without changing execution. Non-debug modes release consumed syscall bytes and execution history rather than retaining a full replay.

Reads can return fewer bytes than requested, including zero at EOF; only returned bytes are written to guest memory. Writes preserve the requested byte sequence. Debug's initial run displays program output as it happens and also echoes redirected stdin so the interaction remains visible before the TUI takes over. Interactive terminal input relies on the terminal's normal echo. Replay later uses the recorded bytes, including read results, rather than asking the student to repeat input.


Readable disassembly and tracing
--------------------------------

Disassembly is reconstructed from executable bytes and symbols, not from retained source text. Programs loaded from ELF and programs just assembled must receive the same presentation. The listing uses ABI register names, labels, aligned instruction/operand columns, and optional addresses and encodings. It is a code listing rather than a recovered source file: data declarations, section directives, and exports are not reconstructed.

Default presentation favors recognizable pseudo-instructions. It recovers constants from suitable `lui`/`addi` sequences, moves, zero comparisons, conventional returns, and suitable PC-relative address, load/store, call, and tail sequences. Pair recognition must require adjacency and valid register dependencies. A symbol or discovered control-flow target on the second instruction prevents combining the pair, because that instruction is independently reachable. Static targets, including recognizable `auipc`/`jalr` targets, are identified before grouping. Generated numeric labels distinguish otherwise unnamed destinations and reset their numbering at existing symbols.

GP-relative arithmetic becomes `la` only when its computed address has a symbol and the destination is neither `gp` nor `zero`. Merely mentioning `gp` must not make ordinary arithmetic look like an address load. Recovered direct stores include the temporary register, so their clobber is visible.

Verbose instruction display exposes individual encoded instructions. Compressed instructions retain accepted `c.*` spellings and operand forms; unsupported compressed spellings can be represented as `.2byte`. Unshifted upper-immediate fields, signed negative hexadecimal operands, full symbol names, and current-address expressions for unnamed targets must remain suitable for reassembly within the documented limits. Address calculations wrap at the RV32 boundary. Turning off listing addresses and encodings must not leave decorative text mixed into instruction syntax.

Trace executes at machine-instruction granularity but may combine adjacent pseudo-sequence effects into a readable report. It flushes any preceding combined report before printing an upcoming syscall, so a blocked read is identifiable. Reports expose register assignments, nonsequential PC changes, syscall arguments/results and transferred text, and terminal errors. Display settings must never cause extra execution, skip checks, or consume input. No full-history retention is required outside debug mode.


Strict checking
---------------

Strict mode catches beginner mistakes that a physically correct machine may execute without an architectural exception. It runs after each otherwise nonterminal instruction and reasons from actual register reads and actual accessed bytes, including aliased addresses and syscall buffers. It does not undo the instruction or external I/O on failure. Equal numeric bits do not prove that a preserved value was restored correctly.

### Register availability and value preservation

At program entry only `zero` and `sp` are logically available. Other registers are unavailable even though their physical bits are zero. Writing a result establishes a new usable value. A plain move preserves the source's identity and restrictions. Incoming saved registers in a called function are save-only: the callee may copy them with an ordinary move or save them with a full-word store, but cannot use them as arithmetic input, an address, or a function argument until it gives the register its own value.

Each link-writing `jal` or `jalr` is checked as a call. It must write `ra`, and its actual destination must have a displayed symbol. The conventional `ret` form is checked as a return; returning without a matching call is an error. Tail jumps without a link are not treated as a new call context.

A symbol named `<callee>_args`, if present as a non-address symbol, declares an argument count from 0 through 8. The corresponding `a0` onward must be initialized usable values at the call, and argument registers beyond that count become unavailable inside the callee. Without metadata, available argument registers remain available, unavailable ones remain unavailable, and save-only argument values are rejected. Temporaries become unavailable on entry in either case.

The callee must restore the identities of `ra`, `gp`, `tp`, and every saved register, as well as the numeric entry stack pointer, before returning. Modifying `gp` or `tp` inside a called function is rejected immediately. Caller availability is restored on return, except that only `a0` carries the callee's supported result; other argument registers and temporaries become unavailable. Computing a coincidentally equal saved value is not a substitute for preserving it.

### Memory initialization and stack lifetime

Every write to `sp` must leave it 16-byte aligned. Stack accesses cannot touch bytes below the applicable `sp`; reads additionally require that every accessed stack byte be initialized. An instruction that changes `sp` while accessing memory uses the old `sp` for that access. When `sp` increases, released bytes lose their logical initialization even though the physical bytes remain visible. Reallocating the same addresses does not revive their validity.

Scalar byte, halfword, and word loads/stores require natural alignment. Full-word saves/loads preserve a value's identity and save-only status. Partial stores create ordinary values. A later load must cover consistently initialized bytes from a compatible value and width: loading a word assembled from unrelated byte writes, reading half of a tracked word as a halfword, or reading a partially overwritten value fails. Untouched program text/data can acquire its initial tracked identity on its first scalar read; stack contents cannot use this exemption.

Syscall buffers are byte-oriented. Input establishes byte values only for bytes actually read. Input/output buffers cannot overwrite/read memory already tracked as halfword or word data; untouched program data and byte data are allowed, subject to stack initialization checks. These are pedagogical constraints stricter than raw byte-addressable memory.

Call/return checks produce frame-entry/exit information used by debugger stack colors and viewport planning. In the current implementation those frame events are available with strict checking; non-strict debugging still shows stack bytes and `sp` but does not promise the same validated call-frame partitioning. The checker is not a complete abstract interpreter for every compatibility instruction: scalar save/load provenance rules should not be assumed for all atomic operations.


Recorded execution to debugger
------------------------------

Debugging is an examination of one completed execution. The program runs first, including its interactive input and output, until exit, error, or the configured limit. Then the debugger returns the visible machine to its initial state and opens the recorded path at its first instruction. The initial source cursor follows the ELF entry point, which need not be the lowest text address. All pane visibility preferences initially permit display; addresses are hidden and pseudo-instructions are shown unless overridden.

Let the recording contain N instruction occurrences, indexed 0 through N−1. At replay position k, registers and memory describe the state before occurrence k, the display identifies that occurrence as the next instruction, and the status previews its recorded effects. The step counter is k+1 out of N. Repeated loop visits to the same address are separate occurrences. The final terminal occurrence is visible but is not stepped past. This allows the student to inspect the inputs to a failed instruction or exit call without losing the relevant line.

Forward movement applies the recorded transition at k, then advances to k+1; backward movement first selects k−1, then undoes that transition. Register values, memory bytes, PC, frame boundaries, and visible I/O history must move together. Undo followed by redo restores the same observable state without executing guest code, rereading stdin, or writing stdout again. The immutable I/O record survives resets of replay state. The initial run's external effects cannot be undone; rewinding changes the debugger's presentation only.

The source cursor is an independent browsing position. Scrolling it must not change registers, memory, output, the upcoming instruction, or the Text memory highlight. Execution navigation brings it back to the new replay position. There are no breakpoints, watch expressions, editable register values, live reruns, or debugger commands to learn before basic use. The complete control set remains small enough to display in one help overlay.


Debugger controls and navigation
--------------------------------

| Key           | Required interaction                                                                                              |
|---------------|-------------------------------------------------------------------------------------------------------------------|
| Up / Down     | Move the source cursor one displayed instruction, clamping at either end; do not move replay                      |
| PgUp / PgDown | Move the cursor by the source pane's current content height, in displayed instructions                            |
| Left / Right  | Undo/apply exactly one recorded machine instruction, then follow execution with the source cursor                 |
| Enter         | Seek the next recorded occurrence of the cursor's instruction address, strictly after the current occurrence      |
| Backspace     | Seek the previous recorded occurrence of the cursor's instruction address, strictly before the current occurrence |

In pseudo mode, cursor movement goes between pseudo groups, but Left/Right still move one machine instruction. A two-instruction `la` can therefore keep the same highlighted source row while its intermediate register value, step number, effect preview, and Text bytes change. Verbose mode exposes the two rows. Changing display mode must not merge or skip recorded transitions.

Enter/Backspace stop before executing the selected occurrence. Selecting the current address and pressing Enter is useful for advancing one loop iteration; Backspace visits its prior iteration. If no occurrence exists in the requested direction, neither machine state nor cursor changes. These operations replay/undo all intervening effects, including nested calls and I/O events; they are not address-only PC assignments.

| Key                        | Required interaction                                                                                                      |
|----------------------------|---------------------------------------------------------------------------------------------------------------------------|
| Home                       | Rewind toward the entry of the current function region, starting from execution rather than an independently moved cursor |
| End                        | Advance to a conventional return in the current function region, or to the recording's end                                |
| `?`                        | Show the control summary over the existing display                                                                        |
| Any key while help is open | Dismiss help and consume the key; do not also execute its normal action                                                   |
| `q`                        | Leave the debugger and restore the terminal                                                                               |

Current Home/End navigation is a label-based heuristic, not a general call-stack unwinder. Its region extends from the nearest preceding nonnumeric code label to the next nonnumeric code label, with text bounds as fallbacks. Home looks backward for entry into that region from an outside link-writing call; End looks forward for `jalr zero, ra, 0` within it. Nested execution is traversed through recorded effects. Internal named labels, recursion, unusual returns, and tail calls can limit this heuristic; function-color inference below is a separate rule and must not be mistaken for a stronger Home/End guarantee.

| Toggle        | Meaning                                                        |
|---------------|----------------------------------------------------------------|
| `r`, `o`      | Permit/hide Registers and Output                               |
| `s`, `d`, `t` | Permit/hide Stack, Data, and Text memory panes                 |
| `v`           | Switch between pseudo and individual encoded instruction views |
| `a`           | Show/hide listing addresses                                    |
| `x`           | Switch decimal/hexadecimal numeric presentation                |

Toggles change presentation only. A pane automatically hidden for lack of room retains its visibility preference and returns when space permits. Help lists all controls and these toggles; a persistent help/quit hint appears in the bottom border when it fits beside the effect report. Unknown keys are ignored. Help is a centered bordered overlay, clipped to a smaller screen, rather than a separate mode that loses the replay view.


Debugger source and effect presentation
---------------------------------------

The source pane displays disassembly, with labels in a dedicated left field and optional address prefixes. It shows the selected occurrence's step number, total recording length, and PC in its top border. The current execution row has a green background with dark text across the row. An independently selected cursor row has a distinct gray background; when cursor and execution coincide, execution color wins. Normal source rows remain light text on black. Long content clips at the pane edge rather than wrapping into neighboring instruction rows.

The source viewport follows the cursor with context on both sides. Center it when possible, shifting near either end to display more real instructions. When the whole listing is shorter than the pane, blank rows are allowed around it instead of pinning every short program to the top. Source scrolling is independent of memory viewport stability.

The bottom-border effect report belongs to the upcoming machine occurrence, not the independently selected source row and not the previously executed instruction. It previews register assignments and nonsequential PC changes, or syscall operation/arguments and return count, or terminal diagnostics. Keep it compact enough to coexist with the main view; the current presentation takes the first two report lines. Current syscall reports take precedence over an additional generic error message, a limitation of their diagnostic presentation. Memory values remain the pre-instruction values even when the highlighted bytes identify a pending store.

Draw a connecting line in the label field for the upcoming taken conditional branch or non-linking jump to another decoded instruction. It joins the source and target addresses through intervening listing rows in either direction. Do not draw it for fallthrough, a self-target, a destination outside decoded code, a link-writing call, or conventional `ret`. Other non-linking indirect jumps qualify, including an `ra`-based jump with nonzero offset. Both pseudo and verbose listings retain this local-control-flow indication. It represents the recorded outcome, not a guess based on the mnemonic.

Registers occupy four ordered rows: `ra sp gp tp`; `a0`–`a7`; `t0`–`t6`; `s0`–`s11`. Omit the immutable zero register. Values are those before the upcoming instruction. Decimal mode shows signed values; hexadecimal mode exposes register bit patterns, retaining simple one-digit values for 0–9. Memory addresses and bytes remain hexadecimal independently of that toggle. Instruction operands retain their signed-operand conventions rather than displaying every negative immediate as an unsigned register pattern.


Debugger screen allocation
--------------------------

The screen is one bordered composition with shared separators and connected corners. Source occupies the left upper area, Registers and Output can appear below it, and the right column contains Stack, Data, and Text in that order. Memory rows need a fixed 39-character content width: address, eight hex bytes, and eight character cells. Give additional terminal width to the left side. At fewer than 80 columns, hide the entire memory column and reclaim its space for the source/register/output side.

Use terminal height H including outer borders. Each interior separator costs one row. A missing data segment makes Data ineligible before allocation, just as its toggle does. Space pressure must not accidentally re-enable a disabled pane.

| Eligible memory panes | Allocation requirement                                                                                                 |
|-----------------------|------------------------------------------------------------------------------------------------------------------------|
| Three, H ≥ 24         | At H=24 give Stack/Data/Text 7/7/6 content rows; at H=25 give 7/7/7                                                    |
| Three, H < 24         | Drop Text first; then apply the two-pane rule                                                                          |
| Two, H ≥ 17           | Give each at least seven rows; split surplus with twice the growth for Data, or evenly when neither pane is Data       |
| Two, H < 17           | Keep Stack or Data according to upcoming/most-recent memory activity; if one candidate is Text, keep the non-Text pane |
| One                   | Use H−2 content rows; if none remain eligible, remove the memory column                                                |

For three panes above H=25, let E=H−25. Stack gets `7 + floor((E+2)/4)` rows, Text gets `7 + floor(E/4)`, and Data gets the remaining content rows. Thus H=29 gives 8/9/8 and H=49 gives 13/19/13. For two panes let E=H−17: when Data is first it receives `7 + E − floor(E/3)`; when Data is second the first pane receives `7 + floor(E/3)`; without Data the first receives `7 + floor(E/2)`. The second gets the remaining content rows. These rounding rules avoid unused rows and preserve predictable growth.

On the left, protect twelve source content rows before allocating auxiliary panes. Registers needs exactly four content rows. Output is eligible only when replay has visible content and its toggle permits it; an empty Output pane must not consume space.

| Terminal situation                        | Left-side behavior                                                                                        |
|-------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| H < 19                                    | Source alone                                                                                              |
| 19 ≤ H < 24, Registers enabled            | Four register rows plus separator; no Output                                                              |
| H ≥ 24, Registers enabled, output exists  | At least 12 source rows, four register rows, four output rows, and two separators                         |
| H ≥ 19, Registers disabled, output exists | Output can appear; its growth first uses the five rows otherwise reserved for Registers and its separator |
| Output empty or disabled                  | Reclaim its space for Source; other eligible panes retain their rules                                     |

With Registers and Output both present, Output's natural content height is `4 + ceil((H−24)/3)`, capped by the number of visible output lines but never below four. Source takes the rest; sufficiently long Output gets the first extra row above H=24, then roughly one row for every two added source rows. With Registers disabled, start Output at four rows at H=19, grow by one row per terminal row through H=24, then use the same one-third surplus rule. Short output remains content-capped with a four-row minimum. At 80×24, one output line gives 12 Source / 4 Registers / 4 Output rows; without output, Source gets 17 and Registers 4. With no auxiliary panes Source gets all H−2 rows.

Resizing recomputes layout and memory placement, even if a particular pane's height happens to remain unchanged. Hidden-by-size panes return automatically when eligible again. All combinations of toggles must fit without overwriting borders, underflowing dimensions, or leaving surplus rows unassigned. Below a usable minimum (currently five columns by three rows), drawing can be deferred. A small terminal must not corrupt replay state or require restarting the debugger to recover its layout.


Memory bytes, focus, and colors
-------------------------------

All memory panes show eight bytes per row, grouped from the segment's start. Each row shows its hexadecimal address, bytes in increasing address order left to right, and printable ASCII equivalents; other bytes use a visible placeholder. Missing bytes in a partial final row are blank, not fabricated zeroes. Memory rows run upward as addresses increase: the low-address row is drawn at the bottom of a pane. Keep hex and character columns aligned and color each representation of a byte identically.

Text highlights exactly the upcoming instruction's two or four bytes, even if they cross a row boundary. It follows execution order through branches and repeated addresses. It does not follow the source cursor, pseudo-group start, or the preceding instruction. Toggling verbose mode must leave these exact bytes unchanged.

Stack and Data highlight the upcoming memory access when it applies to their segment; otherwise each retains the most recent applicable access from the recorded prefix. Prefer a read to a write when an occurrence has both for highlight selection. The complete accessed byte range is highlighted, including overlaps and row crossings, rather than merely the changed bytes or a whole word/row. A new focus completely replaces the old focus. Rewinding recomputes focus from the earlier occurrence; it must not keep highlights from the future. The same upcoming/recent activity decides which memory pane wins on a short screen.

Highlight by swapping the foreground/background of each byte's underlying region color. Do not paint a uniform rectangle that erases region boundaries inside an access. Separating spaces may share color only where adjacent byte colors agree; color must not leak into blank cells, the address field, or unrelated bytes.

### Persistent region identity

Data colors distinguish address-symbol regions, with boundaries at symbol addresses and colors independent of scrolling. Stack colors distinguish active frames from oldest caller to newest, using a repeating pastel palette so a call does not recolor its callers. Include the current `sp` as a boundary unless already present. Stack space below `sp` is dim gray. Returning or releasing space changes its base color to inactive gray without erasing a retained recent-access highlight; that highlight becomes inverted gray. Duplicate frame boundaries must not arbitrarily change the older frame's color.

Text colors distinguish inferred function regions, not every assembly label. Include the ELF entry, static link-writing direct-call targets, recognizable adjacent `auipc`/link-writing `jalr` targets, and observed indirect-call destinations that actually wrote a link. Only decoded instruction addresses can start regions. Tail jumps and invalid destinations do not add call entries.

Also recognize an entirely unvisited interval beginning at a nonnumeric executable symbol as an unused function candidate. Its interval ends at the next such name or its executable-segment boundary. Visiting any instruction in that interval prevents this unused-region rule from splitting it. Generated numeric labels, symbols inside an instruction, and non-code symbols do not create function colors. This preserves a function's color across visited internal labels while making unused routines distinguishable. Text preceding the first inferred entry stays neutral. Region colors are derived from the whole recording once, so stepping and scrolling do not recolor functions.

The current palette uses similarly bright, moderately saturated colors in an order that separates adjacent regions; it cycles after six regions. Exact color codes are incidental, but the visual roles are not: light-on-black normal text, green execution selection, gray browsing selection, distinct pastel memory regions, dim inactive stack, and inverted-byte focus. Color identities must not depend on the visible window's start address.


Memory viewport stability and planning
--------------------------------------

Memory positioning is automatic and uses the completed recording to show useful nearby values before they are accessed. It must avoid making a student chase an array or stack value around the screen on every step. Keeping the focused bytes visible takes precedence over showing surrounding regions; keeping an already useful viewport stationary takes precedence over repeated recentering.

### When a window moves

If a segment fits completely, keep a fixed placement regardless of access: Stack high addresses at the top, Data centered, and Text low addresses at the bottom. For Data an odd spare row belongs above the segment. Blank padding stays blank. For a longer segment, retain the previous viewport while its height is unchanged and the entire focused range remains visible. With no nonempty focus, retain an existing placement rather than scanning again on every redraw.

Replan when the focus no longer fits, the pane height changes, or terminal dimensions change. If a focused range spans at least the whole pane, anchor at its first row; show the beginning consistently rather than oscillating between its ends. Row span includes byte misalignment: ten bytes starting at the last byte of a row span three rows, not two.

### Choosing a new window

The following priorities define placement; different algorithms must preserve their outcomes.

1. Include the current-or-most-recent highlighted access for the pane. For a retained past stack access, use the stack/frame boundaries that existed at that occurrence, not the current frame. If there has been no access, future accesses may establish the first placement.
2. Gather nearby complete values in recording order: current occurrence, two future instructions, one past instruction, then repeat the two-forward/one-backward pattern. Bound the search to 1000 future and 500 past instructions. Count instructions, including those with no relevant access. For Text, an instruction's exact bytes are its access; for data panes, consider reads before writes and consider both when present.
3. Each included access constrains the window to contain all of its rows. When another access cannot fit, stop only that search direction. The other direction can still contribute useful values. If constraints determine exactly one window, use it. Simulated lookahead/rewind for frame boundaries must not modify the actual displayed machine.
4. Use remaining freedom first to maximize overlap with the accesses that stopped each direction, then with surrounding regions in encounter order. Preserve all earlier choices when resolving later ties. An unreachable region contributes no preference. Value visibility outranks region visibility, and an earlier region outranks a later region when both cannot fit.
5. Among remaining placements, Stack chooses the highest feasible row start, putting its low-address working values toward the bottom of the screen. Data and Text center the gathered extent, including fully containable surrounding regions; an odd spare row goes above it. Clamp long-segment windows to segment bounds. With no useful accesses, use the high end for Stack and low end for Data/Text.

Surrounding regions are frame/`sp` intervals for Stack, address-symbol intervals for Data, and inferred-function intervals for Text. Region context belongs to the state at the access: a future callee's frame can influence placement before its call executes, and a past callee's frame can still explain the focus after return. This use of lookahead is a display operation only.

For example, with seven visible rows and successive accesses at rows 100 through 109, the first window can cover 100–106. It remains stationary through the first seven accesses. On access 107 it shifts to cover 103–109, and a single step backward keeps that second window because the earlier access still fits. Replacing this with center-on-every-step behavior is a UX regression even though every accessed byte remains visible.


Recorded output presentation
----------------------------

Output is the interaction up to the replay position, not the initial run's complete output pasted into a pane. Append input and output events in execution order as their instructions are applied; remove them on rewind. A prompt, the student's answer, and the next response remain in that order. Input characters use a distinct blue/cyan foreground, while program output uses the normal foreground. Do not lose that distinction when they share a line.

Decode the visible concatenated byte stream across syscall/event boundaries so a multibyte UTF-8 character split across writes displays correctly. Invalid byte sequences use replacement characters associated with the bytes' origin. Newlines create logical rows; suppress a single empty final row caused by a terminating newline. Show the most recent rows that fit, with partial final lines retained. Long rows clip horizontally rather than wrapping and changing the vertical allocation. The debugger's output pane is a recorded text view, not a second active terminal interpreter.

Rewind across a read removes its displayed answer as well as undoing the buffer and result-register changes. Replaying that read restores the same answer without host input. Output becoming empty removes its pane and returns space to Source; stepping forward until content reappears restores the pane according to the same allocation rules.


Terminal ownership and recovery
-------------------------------

The initial program interaction uses the normal terminal. Only the interactive debugger takes raw input, hides the terminal cursor, and uses an alternate screen. On normal exit or UI failure, restore ordinary input mode, the original screen, and cursor visibility. Redraws and help overlays must not leak guest output or control sequences into the surrounding terminal. Debug requires an interactive stdout terminal; inability to open the TUI is reported after the initial run, not disguised as successful debugging.

Current edge cases outside the normal teaching workflow include an empty recording, an invalid initial entry address, unusual disjoint executable layouts, and Home/End behavior for unconventional function structure. This document does not elevate crashes or inconsistent handling in such cases into required behavior. It does require preserving the valid-program interactions above, and reporting execution failures in the normal recorded workflow.


Audit boundaries
----------------

These scenarios are acceptance criteria, whether checked automatically or by inspecting the debugger. Matching instruction results alone is insufficient.

### Program preparation and strict checking

| Scenario                                                          | Required observation                                                                                       |
|-------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------|
| Several source files, local duplicate names, exported helper      | File-local bindings and explicit global calls coexist; explicit input order determines placement           |
| A pseudo-instruction shrinks before an address-dependent value    | Final data, symbols, disassembly targets, and ELF entry agree with the converged layout                    |
| Early dump of a source without `_start`, existing `a.out` present | The requested early report succeeds and the output file is untouched                                       |
| Save a callee-saved register, clobber it, restore it, return      | Correct save/restore succeeds; recomputing equal bits or corrupting part of the save fails strict checking |
| Release and reallocate stack storage                              | Bytes remain inspectable, but strict reads fail until the reallocated bytes are initialized again          |

### Replay and source interactions

| Scenario                                                              | Required observation                                                                                              |
|-----------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|
| ELF entry follows other text                                          | Initial execution selection and source cursor start at entry                                                      |
| Step through a two-instruction pseudo and a compressed instruction    | Each Right applies one machine occurrence; Text highlights exactly the corresponding four/two bytes               |
| Browse far from the PC, then step                                     | Browsing changes only the source cursor; stepping restores source follow and updates the coherent replay state    |
| Repeated loop visits and an unvisited selected line                   | Enter/Backspace visit the nearest occurrence in that direction; a missing occurrence leaves state untouched       |
| Call, return, taken branch, untaken branch, non-linking indirect jump | Only qualifying local transfers draw connectors; stack frames and pre-instruction effect previews remain coherent |

### Memory presentation

| Scenario                                                  | Required observation                                                                                             |
|-----------------------------------------------------------|------------------------------------------------------------------------------------------------------------------|
| Byte range crosses an eight-byte row or a region boundary | Exact bytes invert in both hex/ASCII; other bytes and each region's identity remain intact                       |
| Source cursor moves while replay stays fixed              | Text focus and memory values remain unchanged                                                                    |
| Long array scan forward, then backward                    | Windows stay stationary while focus fits, shift only when necessary, and do not immediately recenter on reversal |
| Upcoming store overlaps a previous load                   | Focus switches to the entire upcoming store before applying it; rewind restores the prior preview                |
| Return releases recently accessed stack                   | Retained focus becomes dim/inverted inactive gray; context planning uses the historical frame correctly          |

### Space, output, and controls

| Scenario                                                        | Required observation                                                                                        |
|-----------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------|
| Resize 80×24 → 80×23 → 80×25 and 80 columns → 79 → 80           | Text and the memory column disappear/return by policy, without changing toggles or replay state             |
| Disable Data with Stack/Text enabled on a short screen          | Hidden Data never wins allocation; all available rows and border intersections remain correct               |
| Prompt/write, input/read, split UTF-8 writes, rewind then redo  | Correct event order, input color, text decoding, and identical reconstruction without new I/O               |
| No output, one output line, many lines; toggle Registers/Output | Empty pane omitted, four-line minimum when present, capped growth, and reclaimed Source space follow policy |
| Open help, press a toggle key, then quit                        | First key dismisses only; later commands work normally; exiting restores the original terminal              |

Existing coverage in `src/ui_tests.rs` exercises actual rendered cells, viewport decisions, colors, and allocation thresholds; `src/checkabi_tests.rs`, `src/*tests.rs`, `tests/cli.rs`, and `tests/encoding.rs` cover the other behavioral boundaries. These tests are evidence for the contract, not a substitute for it. A refactor must audit uncovered navigation and terminal-lifecycle behavior as well. Build and release behavior is separate; the demo consumes published binaries rather than compiling this working tree into its guest.
