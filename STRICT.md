Strict validation
=================

Use `risclet run --strict`, `risclet trace --strict`, or `risclet debug --strict` to check register use, function calls, stack lifetime, and memory access patterns. Strict mode is disabled by default; `--no-strict` disables it explicitly.

These checks enforce calling conventions and other rules that make programs easier to reason about. Some are stricter than the rules enforced by a physical processor. A program can produce the expected output and still violate one of these rules.

Checks follow the instructions actually executed, not every possible path through the source. They normally run after an instruction executes. A failure does not undo its register changes, memory changes, input, or output. When a machine operation fails, strict mode can still identify an unavailable register that caused the operation to fail. An exit syscall terminates execution directly; it does not run the ordinary post-instruction strict checks.


Register availability
---------------------

A function can change temporary and argument registers, so a caller cannot rely on their old contents after a call. Likewise, a function needs a clear way to receive its inputs rather than depending on whatever happens to be in a register.

Initialize a register before reading it. Pass arguments in `a0`–`a7`, and keep values needed across calls in saved registers or on the stack. Risclet supports one return value, in `a0`.

For example, this loop fails after the first return from `update`, because `t0` is no longer available:

    li t0, 0
    1: addi t0, t0, 1
       call update
       j 1b

A function that uses saved registers must preserve their incoming values as described below.

Details:

-   At program entry, only `zero` and `sp` are available. Other registers must be initialized before reading, even if their physical bits happen to be zero.
-   Writing an ordinary result makes its destination available. A move copies the source value's identity and restrictions; moving an unavailable value is an error. Writes to `zero` do not change it, but the instruction's other actions still occur.
-   A callee cannot use incoming `t0`–`t6` as inputs. Initialize temporaries inside the callee before reading them.
-   After a function returns, `t0`–`t6` and `a1`–`a7` are unavailable to its caller, even if the callee left their bits unchanged. This includes values initialized before a loop containing a call.
-   The availability and restrictions of `a0` come from the callee. A function may return without a result, but the caller must initialize `a0` before reading it in that case.


Function calls and arguments
----------------------------

A call needs a return address so the callee can return to its caller. An agreed argument count also helps distinguish the values a function receives from unrelated register contents.

Call named functions using `ra` as the link register, and return from called functions with `ret`. Risclet can also check a function's argument count when the program declares it.

To declare that `update` takes two arguments, define an integer symbol:

    .global update_args
    .equ update_args, 2

With this declaration, `a0` and `a1` must contain available input values at the call, and `a2`–`a7` are unavailable inside `update`.
The `.global` directive exports the symbol; Risclet can also use a retained local symbol. Avoid defining the same metadata name more than once across files.

Details:

-   Every `jal` or `jalr` that writes a nonzero link register is treated as a function call. It must write the return address to `ra`, and its actual destination must have a symbol in the loaded program. An ordinary jump without a link does not create a new function context.
-   A `<function>_args` symbol declares a count from 0 through 8. An argument cannot contain a value restricted to saving and restoring.
-   Without argument metadata, available argument registers remain available on entry, and unavailable argument registers remain unavailable. Save-only argument values are rejected. Missing metadata does not initialize arguments or make incoming temporary registers available.
-   The conventional `ret` (`jalr zero, ra, 0`), including its compressed equivalent, must have a matching call. `_start` was not called and should terminate with an exit syscall rather than `ret`. Tail jumps do not create an additional call context; they continue under the current context's preservation requirements.


Preserved registers and value identity
--------------------------------------

A caller may need its saved registers after a function returns, and a function making a nested call needs its own return address afterward. Preserving these values lets functions cooperate without knowing one another's internal register use.

A called function must preserve `ra`, `gp`, `tp`, and `s0`–`s11`, and restore `sp` to its entry value before returning. Save a register's incoming value before using it for another purpose, and restore that value afterward. Risclet requires the original value to be restored, not just equal bits recomputed later.

Details:

-   On entry, the incoming values in `s0`–`s11` are save-only. They may be copied with an ordinary move or written with a full-word store, but cannot be used as arithmetic operands, addresses, or function arguments until the function initializes a working value in their place. Copies and full-word reloads retain the restriction.
-   A full-word save and matching reload preserve value identity. Recomputing equal bits does not. Partial stores, arithmetic, and other ordinary computations create new values. Preserve the incoming `ra` before a nested call and restore it afterward.
-   `ra`, `gp`, `tp`, and saved registers must have their incoming value identities at return; `sp` must have its entry numeric value. Changing `gp` or `tp` inside a called function is rejected immediately. Startup code can initialize them before making calls.
-   On return, the caller's saved-register availability is restored. A register unavailable to the caller does not become usable merely because the callee preserved it correctly.


Stack allocation and initialization
-----------------------------------

Stack frames give functions space for saved registers and local data. Allocating space before using it prevents a function from treating storage outside its current frame as its own, and alignment lets functions agree on how that storage is arranged.

Keep `sp` aligned to 16 bytes, allocate stack space before accessing it, and initialize stack data before reading it. Restore `sp` before returning.

Details:

-   Every write to `sp` must leave it divisible by 16. Allocate and release frames in multiples of 16 bytes.
-   An access cannot touch stack bytes below the current `sp`. Every byte read must have been initialized during its current allocation. These rules also apply to accesses through aliases of `sp` and to syscall buffers; initialized stack arguments belonging to the caller are permitted.
-   If an instruction both accesses memory and changes `sp`, validation uses the old frame boundary for the access. Raising `sp` releases storage and clears its logical initialization. Lowering `sp` again does not initialize the old bytes, even though their physical contents remain inspectable.


Memory alignment and stored values
----------------------------------

Alignment makes it possible to access multi-byte values at addresses suited to their size. Keeping loads consistent with stores also helps catch accidental overlap and mismatched data sizes.

Use naturally aligned addresses for scalar loads and stores. In Risclet's strict mode, load a value at the same width at which it was stored.

| Access   | Bytes | Required address |
|----------|-------|------------------|
| Byte     |     1 | Any address      |
| Halfword |     2 | Divisible by 2   |
| Word     |     4 | Divisible by 4   |

A 16-byte-aligned `sp` does not make every offset from it word-aligned: `sw s2, 22(sp)` is still unaligned. Data directives do not align themselves automatically. Put `.balign 4` before the label of a word or word array, particularly after strings or byte data. See [the source syntax reference](SYNTAX.md) for data directives.

Details:

-   Alignment checks include compressed scalar loads and stores. RISC-V execution environments can permit unaligned user-mode data accesses; Risclet's strict mode rejects them. Non-strict scalar accesses permit them when the complete access fits within one allocated memory region.
-   A scalar load must read one consistently initialized value with the same width as its tracked store. It cannot combine unrelated writes, read part of a wider stored value, or read a value that was only partly overwritten.
-   Full-word stores and loads preserve the source value's identity and save-only restriction; byte and halfword stores create ordinary values of their own widths.
-   Untouched program text/data can acquire its initial tracked identity and width on its first scalar read. This supports initialized program data and zero-filled BSS. The exception does not apply to stack or heap storage. Newly acquired heap bytes require explicit initialization, despite their physical zero fill. Shrinking the heap discards initialization; growing it again does not restore it. Once established, the width must be respected by subsequent loads.


Syscall buffers
---------------

Read and write syscalls operate on sequences of bytes. Mixing those operations with memory tracked as larger scalar values can obscure which values are being read or changed.

Use byte-oriented storage for syscall buffers. Allocate stack buffers before use, and initialize the bytes a write syscall will send.

Details:

-   Read and write buffers must not read or overwrite memory already tracked as halfword or word values. Untouched program data and byte values are allowed; stack buffers must also follow the stack allocation and initialization rules.
-   A read syscall initializes only the bytes actually received. A short read does not initialize the rest of the requested buffer, and EOF initializes no bytes. A write syscall requires all bytes read from a stack or heap buffer to be initialized.


Checks that also apply without strict mode
------------------------------------------

Some failures prevent the simulator from carrying out an operation at all. These checks apply whether or not strict mode is enabled.

Every memory access must fit entirely within a single allocated region. Stores require writable memory, and instruction fetches require executable memory containing a complete instruction. Execution must reach a decoded instruction boundary; jumping into the middle of an instruction is an error.

Unsupported instructions and syscalls, invalid syscall descriptors or negative buffer sizes, invalid executables, and an exhausted execution limit also stop execution.

Strict checking uses actual register reads and memory ranges for the supported instructions. Full scalar memory value/width tracking is implemented for ordinary loads/stores and their compressed equivalents, and byte-oriented tracking for syscall buffers. Do not assume those identity, width, and scalar alignment checks also model every atomic operation. Strict mode is not a proof of algorithm correctness or a complete architectural verifier.


Reading diagnostics
-------------------

The first line states what failed. The second gives the PC, execution step, source location, and operand values as available. Source or disassembly context marks the failing instruction with `>>>`. Related listings identify earlier calls, writes, or stack releases when that information is available. Help explains the applicable rule and links to this document.

Addresses and values follow the selected decimal or hexadecimal display mode. Source listings preserve the original text. Debug mode prints the full report before opening its terminal interface and retains it in the console after quitting; the debugger status bar shows only the first line.
