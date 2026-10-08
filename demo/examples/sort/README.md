Insertion sort
==============

Each of these examples includes suggestions of things to try to explore
risclet's functionality and use cases. Select the VM tab for a live Linux
system with risclet and this demo loaded.

Run these commands in this demo's directory (or the browser's VM tab):

```sh
risclet
risclet trace
risclet debug --strict
```

`start.s` prints each array before and after calling
`insertion_sort(bytes, count)`. The tests cover mixed values, sorted and reversed
inputs, duplicates, a singleton, and an empty range. Values are signed bytes;
`print.s` contains only integer and array printing. No build step is needed.

Watch one shift at a time
-------------------------

The debugger runs the entire program first and gathers a trace. Then it lets
you easily navigate the program's run.

One way to use this is to observe a high-level operation without getting lost
in the details. The steps below will help you watch one value shift at a time
in the insersion sort, but skipping directly from the point where one array
element is shifted to the next (or going backwards to the previous shift) as a
single step.

1.  In the debugger, press `s` and `t` to hide stack and text memory. Keep data
    and registers visible. Registers start in decimal; `x` toggles their base.
    Memory bytes always appear in hexadecimal; -3 is `fd`.
2.  Use Up/Down or PageUp/PageDown to put the cursor on `insertion_sort`'s
    first instruction, `li t0, 1`. Press Enter (fast forward to this
    instructions) to skip the initial printing and reach the first sort call.
    Cursor movement alone does not execute instructions.
3.  Move the cursor to `addi t2, t2, -1`, immediately after `sb t4, (t3)`.
    Press Enter. The first larger predecessor has moved right; `t1` holds the
    key and `t2` still identifies the hole just filled by the shift.
4.  Leave the cursor there and press Enter repeatedly. Each time fast forwards
    to the next time that instruction is ready to execute. Each time it will
    have performed another shift. Backspace reverses to the previous visit to
    that instruction. Early in the trace the key changes between insertions;
    when the key is -6, several consecutive stops show the same key moving
    toward the front. The temporary duplicate values are expected: the key
    stays in a register until `sb t1, (t3)` fills the final hole.
5.  To watch complete insertions, select `bge t0, a1, ...` at the outer loop's
    beginning. Each Enter reaches the next pass with a longer sorted prefix;
    Backspace undoes a pass. The disassembler may renumber numeric labels, so
    identify the instruction by its operands rather than the label number.


Watch complete printing calls
-----------------------------

Select `addi s0, s0, 1` in `print_array`, just after `jal print_int`. Enter
advances through a complete integer-printing call to the next visit; Backspace
rewinds to the previous time the current instruction executed. Observe digits
appearing and disappearing in the output panel. Select `print_array`'s first
instruction instead to jump between whole array-printing calls and the
intervening sorts. Right/Left always move by one instruction; `?` lists
controls and `q` quits.


Create a caller-saved register violation
----------------------------------------

Risclet can watch for common mistakes and treat them as fatal errors.

Run `risclet --strict` first. In `print_array`, replace **both** the
initialization `mv s0, a0` and the later cursor update `addi s0, s0, 1` with
corresponding uses of `t5`: `mv t5, a0` and `addi t5, t5, 1`. Also change
`lb a0, (s0)` to `lb a0, (t5)`. Run again with ABI checking. The update reads
`t5` after `jal print_int`, when caller-saved registers are invalid, even if
this particular callee happens to leave its hardware value untouched. Restore
the three lines before continuing.
