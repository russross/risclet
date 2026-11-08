Debugger tutoring
==================

The central workflow
--------------------

Teach seeking to meaningful repeated events, then stepping locally. Avoid starting every investigation at `_start` and stepping through printing routines. Use actual instructions and register roles from the learner's program, not labels or register assignments guessed from a generic algorithm.

1. Run `risclet debug --strict` with the intended sources. Complete interactive input first: Risclet records the execution before opening replay at its beginning. Errors and execution limits also end recording.
2. Browse to an instruction representing the event of interest: function entry, loop condition, store, instruction after a call, or start of the common postlude. Enter seeks its next recorded occurrence; Backspace seeks its previous occurrence.
3. Repeat Enter/Backspace at that same instruction to compare whole iterations, invocations, or completed recursive regions. Inspect bounds, parameters, data, and output to identify which occurrence is selected.
4. Near the first disagreement, use Right/Left to apply/undo individual machine instructions. Check the inputs before the suspect instruction and the state after it, then return to event-level navigation as needed.

A replay position shows the state BEFORE the highlighted upcoming instruction. Its effect preview describes what that instruction will do. Enter/Backspace stop before the selected occurrence, searching strictly after/before the current one. If there is no matching occurrence, nothing moves. Enter at a loop header can cross a successful return and reach a different invocation's loop header; inspect parameters rather than assuming every seek is the same loop.

Browsing with Up/Down/PgUp/PgDown moves only the source cursor, not execution. Stepping or seeking brings the cursor back to execution. The displayed listing is disassembly, so numeric labels and pseudo spellings can differ from the source. Identify sites by function, instruction, and surrounding operations.


Controls and display
--------------------

| Keys            | Meaning                                                   |
|-----------------|-----------------------------------------------------------|
| Up/Down         | Browse displayed instructions                             |
| PgUp/PgDown     | Browse a source-pane page                                 |
| Right/Left      | Apply/undo one recorded machine instruction               |
| Enter/Backspace | Next/previous occurrence of the cursor's instruction      |
| Home/End        | Seek toward current function's entry/return               |
| ? / q           | Show controls / quit                                      |
| r / o           | Toggle registers / output                                 |
| s / d / t       | Toggle stack / data / text memory panes                   |
| v               | Toggle pseudo versus individual encoded instructions      |
| a / x           | Toggle addresses / decimal-versus-hex numeric presentation |

A key dismissing help does not also perform its usual action. An 80×24 or larger terminal is recommended; panes can disappear automatically when space is limited. For array work hide stack/text to expose data. For call-preservation work retain stack/registers. Memory remains hexadecimal bytes even when `x` changes register-number presentation. Decode little-endian words explicitly: `02 00 00 00` is 2; byte `fa` is -6 when interpreted as signed.

Right/Left always move ONE machine instruction, even when the listing combines two into an `la` or `li` pseudo. A repeated highlighted row can therefore represent progress. Toggle `v` to expose the expansion, then return to the simpler view when it no longer helps.

Home/End use a label-based function-region heuristic, not a general call-stack unwinder. Internal named labels, recursion, tail calls, and unusual returns can make them less useful. With recursion, choose the shared postlude and inspect range/depth values to understand completed subproblems; End can reach a child's return. Use explicit occurrence seeks when invocation identity matters.

There are no breakpoints, watch expressions, live state edits, or new input during replay. Rewinding and replaying restore the recorded registers, memory, frames, and visible input/output together. They neither reread host input nor reissue host output. To change input or code, leave replay and run again. The final exit/failing occurrence remains inspectable before its effects are stepped past.

The Data pane includes allocated heap bytes in a dedicated color, with the usual inverted-byte focus. The alignment gap after static data has address spacing but blank hex and ASCII bytes. Programs without static data gain a Data pane while their heap is nonempty. Stepping across `brk` changes the visible heap extent; undoing a shrink restores the discarded bytes, and redoing a growth reproduces its zero-filled storage.


Useful exercises
----------------

For insertion sort, select the outer-loop condition to compare successive sorted prefixes. Then select an inner shift/store to inspect individual moves. State which register is the key, hole, or array pointer in this implementation. Backspace should demonstrate the reversal of an entire insertion or shift before returning to local steps.

For binary search, seek the loop condition and compare the candidate interval with the target. For a half-open implementation, verify `[low, high)` contracts and remains consistent with empty termination. Step through one midpoint calculation and signed/unsigned element load. To compare complete calls, select the instruction immediately after the search call and inspect `a0` before the caller overwrites it.

For recursive sorting, seek the shared postlude before saved range registers are restored. At each occurrence the relevant subrange should be sorted, including unchanged base cases. Compare smaller completed children with larger completed parents. Do not promise that repeated entry seeks skip all recursive children.

For interactive code, finish a known input sequence, then seek a prompt to compare whole rounds or a post-call instruction to compare parsed input results. Step around the read syscall to inspect captured bytes. Rewinding should remove the corresponding visible input/output and restore buffer state. A seek to the next prompt after the final round correctly does nothing.

For ABI errors, start from a strict-clean example in a temporary copy. Change a live value from a saved register to a temporary across a call, or swap the OFFSETS of two saved-register restores while leaving saves unchanged. Verify that strict mode fails for the intended violation. Merely reordering correct restore instructions is not a preservation bug. Use this to distinguish value identity from coincidentally equal bits.

End an exercise with a concrete observation: which instruction first changed the wrong value, which invocation had the wrong parameters, or which preservation rule failed. A list of keys without an event to navigate and a state to inspect is not a debugger lesson.
