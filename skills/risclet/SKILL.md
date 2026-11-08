---
name: risclet
description: Write and review Risclet RISC-V assembly, design narrated exercises and tests, explain assembler reports, and teach recorded debugger navigation. Use for Risclet programs and tools rather than generic GNU, RARS, or Venus assembly.
---

Risclet
=======

Use Risclet to teach the hardware/software interface, CPU operations, concrete memory layout, and the translation of C-like control flow into machine instructions. Favor small, hand-written, entirely assembly programs with explicit function contracts. Keep explanations grounded in registers, bytes, addresses, and observed execution.

Read the relevant reference before working:

*   [Dialect and tools](references/dialect-and-tools.md): source syntax, directives, expression types, command usage, strict checks, and assembler/linker inspection.
*   [Writing and assessment](references/writing-and-assessment.md): register planning, function patterns, formatting, narrated tests, and evidence-based review.
*   [Debugger tutoring](references/debugger.md): navigating repeated instruction occurrences, mixing seeks with steps, and designing useful exercises.

Establish the program's entry point, complete source file set, function contracts, data representation, and expected output before changing code. Trace the affected path from `_start` through calls and memory changes to output and exit. Keep supplied interfaces and harness files intact unless the task includes changing them.

Use the installed `risclet --version` and command-specific `--help` to resolve version differences. These references reflect Risclet 0.5.5. When a checkout or packaged documentation is available, consult `SYNTAX.md` for accepted forms, `ASSEMBLER.md` for assembly passes, and `NOTES.md` for behavioral details; demo example READMEs provide executable debugger exercises. Do not require a source checkout or any particular host editor or grading platform. Run existing tools rather than fetching or installing tooling without a task need.

Keep these distinctions explicit:

*   Risclet's address/integer distinction applies to assembly-time expressions; registers still hold 32-bit patterns.
*   Strict checking enforces a teaching model beyond hardware behavior. Correct output alone does not establish ABI correctness; passing strict mode does not establish algorithm correctness.
*   Numeric local labels, ordered control flow, scratch-only leaf functions, and a single postlude are the preferred teaching style here, not the complete set of accepted assembly forms. Adapt to explicitly requested conventions.
*   Debugging replays one recorded execution. Help users find relevant invocations quickly, then inspect only the local instructions that explain the behavior.

Validate new examples with `risclet run --strict` using the intended source set and representative inputs. Compare narrated output against an independently reasoned result. For instructional failures, verify the original succeeds and the changed version fails for the intended reason. Use temporary copies for deliberate faults. Report what ran, observed results, and any limits on verification.
