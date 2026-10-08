Binary search
=============

```sh
risclet
risclet disassemble
risclet trace --strict
risclet debug --strict
```

Run in this directory; Risclet assembles all `.s` files directly. `start.s`
drives queries and exits. `test_search.s` prints targets and returned indices;
`binary_search(bytes, count, target)` returns an index or -1. The sorted input
uses signed bytes. Tests exercise both endpoints, an interior hit, absent values
inside and outside the range, duplicates, singleton and empty ranges. With
duplicates, any matching index is a valid result; these displays make no verdicts.

Watch the bounds contract
-------------------------

1. In the debugger, hide stack and text with `s` and `t`. Keep registers visible;
    press `x` if you prefer decimal. Scroll to `binary_search`'s `li t0, 0` and
    press Enter to skip query printing and reach the first search, for 23.
2. Move the cursor to `bge t0, t1, ...`, the loop condition, and press Enter.
    `t0` and `t1` delimit the half-open candidate range `[low, high)`.
    `a2` is the target. The first range is `[0, 15)`.
3. Leave the cursor there and press Enter twice. Each jump includes one full
    unsuccessful comparison and bounds update: the ranges become `[8, 15)`
    and `[8, 11)`. Press Backspace to undo a full iteration and widen the range.
4. From `[8, 11)`, use Right to inspect the midpoint calculation and byte load:
    `t2` becomes 9 and `t4` becomes 23. The equality path returns index 9.
    Pressing Enter at the loop condition instead skips this successful return
    and reaches the next query's loop; notice that `a2` changes to -12.
5. To compare entire searches, select the first instruction of `test_search`,
    `addi sp, sp, -16`. Repeated Enter/Backspace steps between test invocations,
    adding/removing complete query/result lines in the output panel. For one
    search only, select `jal print_int` immediately after `jal binary_search`
    and press Enter to land at the returned index before it is printed.

Source scrolling does not execute anything. Enter/Backspace seek the next/previous
visit to the selected instruction; Right/Left step one instruction. Numeric
labels may be renumbered in disassembly. Press `?` for help and `q` to quit.

Read an undeclared argument
---------------------------

`binary_search_args` is 3, so only a0, a1, and a2 arrive as initialized
arguments. Insert `mv t5, a3` immediately after `mv t1, a1` in
`binary_search.s`. Run `risclet --strict`: it reports the read of an
uninitialized a3. This is distinct from using a3 as a temporary after writing
it, which is allowed. Remove the inserted instruction afterward.
