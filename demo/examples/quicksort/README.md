Quicksort
=========

```sh
risclet
risclet disassemble
risclet debug --check-abi
```

Run from this demo's directory; no Makefile or executable is needed. `start.s`
prints before/after arrays without judging results. Cases include mixed signed
bytes, sorted and reversed arrays, duplicates, a singleton, and an empty range.

`quicksort(bytes, low, high)` sorts a half-open range: low is included, high is
excluded. `partition.s` places the last element at its final position and returns
that pivot index. The two recursive calls sort the regions on either side.
This deliberately simple pivot rule can produce uneven recursion on sorted or
equal input. The small tests keep that behavior visible.

Watch recursive regions become sorted
-------------------------------------

1. Press `s` and `t` to hide stack and text memory, giving the byte arrays more
    space. Keep the data and register panels visible. Memory always displays
    hexadecimal bytes: -6 appears as `fa`. `x` toggles the register display base.
2. Scroll the source cursor to the first instruction of `quicksort`,
    `addi sp, sp, -32`, and press Enter. This skips the first before-array print
    and stops at the initial sort call. Right a few times to inspect the saved
    range: `s1` is low and `s2` is high.
3. Find the **second** `jal quicksort` in `quicksort.s`, then place the cursor
    on the very next instruction, `lw s0, (sp)` (written `lw s0, 0(sp)` in the
    source). Press Enter. This shared postlude is reached after both recursive
    children return, and is also reached by empty and singleton base cases.
4. Keep the cursor on that restore and press Enter repeatedly. At each stop,
    before any saved registers are restored, `[s1, s2)` is sorted. Base cases
    cause stops without changes; other stops complete smaller regions and then
    larger parent regions. Watch the `values` bytes accumulate sorted regions
    until the outer call's range `[0, 12)` is complete. Once that call returns,
    later Enter presses reach the other test arrays.
5. Press Backspace repeatedly at the same instruction to revisit these
    completion points in reverse. To examine one partition instead, select its
    first instruction and press Backspace or Enter to a nearby invocation,
    then use Right/Left for individual swaps. Home/End navigate to function
    entry/return; with recursion, the shared return instruction can belong to
    a child, so the selected postlude and range registers are the useful guide.

The debugger replays a completed trace. Enter searches for the next execution
of the selected instruction, not simply the next source line; it does nothing
when there is no later visit. Numeric labels can differ in disassembly. `?`
shows help and `q` quits.

Break a frame restore
---------------------

Run `risclet --check-abi` on the original. In the quicksort postlude, swap the
**offsets** of the first two restores: use `lw s0, 4(sp)` and `lw s1, 0(sp)`.
Keep the saves unchanged. Run again with ABI checking: restoring a register
from another register's slot violates preservation, even when the two visible
values happen to be equal. Swapping only the order of otherwise correct loads
would not create this violation. Restore the offsets afterward.
