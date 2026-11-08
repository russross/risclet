Writing and assessment
======================

Plan from contracts and storage
-------------------------------

Describe parameters, result, mutation, element widths/signedness, range conventions, and preconditions before selecting instructions. Choose either pointer endpoints or a pointer/count interface and keep its units consistent. For words, scale an index by four; bytes need no scaling. Distinguish the address of a pointer slot from the pointer loaded out of it. Use signed comparisons for signed values and appropriate unsigned comparisons for addresses/unsigned values.

Map each live value to a register or explicit stack slot. a- and t-registers are equally valid scratch registers; a-registers are not reserved for arguments throughout a function. Leaf functions should normally use only a-/t-registers and leave `sp` and `ra` alone. For a non-leaf, identify precisely which values survive each call; keep only those in saved registers or stack storage. Do not save unused s-registers or preserve dead scratch values.

Use `s0` as an ordinary saved register, without a frame pointer. Allocate a frame rounded up to a multiple of 16, save exactly the used s-registers and `ra` (at the highest word slot), then move persistent arguments into their assigned registers. Funnel exits through one postlude. Establish the result in `a0` before restoring registers and releasing the frame. Prefer a matching save/load order and explicit `0(sp)` for stack slot symmetry.

This non-leaf example computes `adjust(x) = twice(x) + x`. Both functions take one argument; `twice` must be defined/exported in its own source if separate. The frame has unused alignment padding, not speculative saves:

    .global adjust
    .equ    adjust_args, 1
    .text
adjust:
    # Preserve x across the call and save the caller's return address.
    addi    sp, sp, -16
    sw      s0, 8(sp)
    sw      ra, 12(sp)
    mv      s0, a0

    # result = twice(x) + x
    jal     twice
    add     a0, a0, s0

    # Restore the caller's saved values and stack pointer.
    lw      s0, 8(sp)
    lw      ra, 12(sp)
    addi    sp, sp, 16
    ret

`_start` is an entry block, so it can use s-registers for its own long-lived test state without saving an incoming caller's values. It must initialize arguments again after calls that invalidate them, and exit explicitly.


Control flow and source style
-----------------------------

Preserve the lexical order of the source pseudocode. For an if/else, branch on the inverse condition past the then block, jump over else, then continue. For a while/for loop, place the condition first, branch out on its inverse, emit the body and update, then jump back. Avoid detached blocks that jump away and return to the middle of a construct, or converting a while into a do/while merely to reduce branches.

Use named labels for functions and data, numeric labels for local control flow, and `f`/`b` directions on every numeric reference. Favor common pseudoinstructions, `jal function`, and `ret`. Local numeric labels also avoid confusing the debugger's function-region heuristics with internal names. Never infer authorship or misconduct from label style alone.

Put labels at the left margin and function labels on their own lines. Prefer `.global` over `.globl`. In generated teaching source, indent instructions/directives 16 spaces and place operands at column 25 (after an 8-character mnemonic field). Numeric labels can share an instruction line. Use standalone `#` comments at natural boundaries: a short C/Python-like operation followed immediately by its assembly, with blank lines between conceptual groups. Comments should explain the algorithm, data layout, or register roles rather than translate every mnemonic into English.


Narrated test harnesses
-----------------------

Use `_start` to drive a sequence of readable demonstrations. For each case, print what is being tested, the input, then the result or changed data. State a non-obvious purpose in the output, before calling the function, so even an early failure leaves useful context. Print actual results rather than only pass/fail labels. For mutation, show before and after. Keep output deterministic and compact enough to locate the first difference.

Define `.equ student_function_args, N` in the supplied harness when its interface must remain fixed; define metadata alongside provided helper functions otherwise. Do not assume file protection exists: preserve whatever boundary the surrounding exercise supplies. Export function labels in their defining files; callers need no external declaration. Include every required helper source in validation commands. There is no built-in `print_int` or `print_string` service: inspect or supply assembly implementations and their contracts.

Derive counts from labels and element widths. Cover contract-relevant empty/singleton inputs, boundaries, signedness, duplicates, existing order, reversed order, and multiple calls. For searches, consider first/last/interior hits and absent values between and outside elements. For recursion, include base cases and nested calls. Test only defined inputs unless error handling is itself specified. Use modest sizes appropriate for stepping and the bounded stack.

Record reference output from a working solution, but independently check representative expected results. Capture stdout separately from diagnostics and inspect process status. Compare student output to reference output when the contract selects a unique result. If multiple results are valid (such as any matching duplicate index), either make the required choice explicit, choose unambiguous cases, or validate the semantic property instead of rejecting another correct result. Do not silently change the contract to make output comparison convenient.

Exact-output tests should keep narration and formatting in supplied code where practical. Reinitialize each mutable case or deliberately describe dependencies. A later case should not accidentally depend on leftover caller-saved registers. On failure, the output prefix should identify the relevant input and operation without needing hidden harness knowledge.


Review and tutoring
-------------------

Read the contract and supplied harness before judging a submission. Separate output errors, memory errors, strict violations, and style differences. Reproduce the first failure using the actual source set and input. Inspect calls, live values, element widths, alignment, and endpoint conditions on that path. Report the smallest concrete counterexample and explain how the relevant machine state violates the contract.

A run that produces the right number while reading an invalid caller-saved register still has an ABI defect. A style difference that is accepted and correct is not an execution defect. Strict mode is dynamic and cannot prove unexecuted branches, termination, or the intended algorithm. Avoid lowering standards by disabling strict mode; if a legitimate exercise intentionally uses a pattern outside its teaching model, explain that mismatch and respect the exercise's requested mode.

When tutoring, ask the learner to predict the next relevant register or memory change, then use the debugger to compare prediction and observation. Prefer a localized explanation or correction over replacing the entire solution unless requested. Preserve assignment constraints and avoid exposing a reference solution when the task is diagnostic feedback.
