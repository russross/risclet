Insertion sort
==============

Run these commands in this demo's directory (or the browser's VM tab):

```sh
risclet
risclet trace
risclet debug --check-abi
```

`start.s` prints each array before and after calling
`insertion_sort(bytes, count)`. The tests cover mixed values, sorted and reversed
inputs, duplicates, a singleton, and an empty range. Values are signed bytes;
`print.s` contains only integer and array printing. No build step is needed.

Watch one shift at a time
-------------------------

1. In the debugger, press `s` and `t` to hide stack and text memory. Keep data and
    registers visible. Registers start in decimal; `x` toggles their base.
    Memory bytes always appear in hexadecimal; -3 is `fd`.
2. Use Up/Down or PageUp/PageDown to put the cursor on `insertion_sort`'s first
    instruction, `li t0, 1`. Press Enter to skip the initial printing and reach
    the first sort call. Cursor movement alone does not execute instructions.
3. Move the cursor to `addi t2, t2, -1`, immediately after `sb t4, (t3)`.
    Press Enter. The first larger predecessor has moved right; `t1` holds the
    key and `t2` still identifies the hole just filled by the shift.
4. Leave the cursor there and press Enter repeatedly. Each stop has performed
    another shift. Backspace reverses to the previous visit to that instruction.
    Early in the trace the key changes between insertions; when the key is -6,
    several consecutive stops show the same key moving toward the front. The
    temporary duplicate values are expected: the key stays in a register until
    `sb t1, (t3)` fills the final hole.
5. To watch complete insertions, select `bge t0, a1, ...` at the outer loop's
    beginning. Each Enter reaches the next pass with a longer sorted prefix;
    Backspace undoes a pass. The disassembler may renumber numeric labels, so
    identify the instruction by its operands rather than the label number.

Watch complete printing calls
-----------------------------

Select `addi s0, s0, 1` in `print_array`, just after `jal print_int`.
Enter advances through a complete integer-printing call to the next visit;
Backspace reverses that operation. Observe digits appearing and disappearing in
the output panel. Select `print_array`'s first instruction instead to jump
between whole array-printing calls and the intervening sorts. Right/Left always
move by one instruction; `?` lists controls and `q` quits.

Create a caller-saved register violation
----------------------------------------

Run `risclet --check-abi` first. In `print_array`, replace **both** the initialization
`mv s0, a0` and the later cursor update `addi s0, s0, 1` with corresponding uses
of `t5`: `mv t5, a0` and `addi t5, t5, 1`. Also change `lb a0, (s0)` to
`lb a0, (t5)`. Run again with ABI checking. The update reads `t5` after
`jal print_int`, when caller-saved registers are invalid, even if this particular
callee happens to leave its hardware value untouched. Restore the three lines
before continuing.
