risclet
=======

This is a lightweight RISC-V disassembler, simulator, and linter for
students learning assembly language. By design it has few controls
and limited functionality, with simplicity and approachability as
overriding goals.

In its default mode, risclet does the following:

*   Loads `a.out` and disassembles it as an rv64imc binary
*   Simulates the complete execution of the program
*   Performs some strict ABI checks, especially around register
    calling convensions. Any violation is flagged as a fatal error.
*   Traces and records the effects of each instruction
*   Launches a TUI (80×24 or larger terminal recommended) that
    allows simple stepping and jumping forward and backward through
    the program, while displaying:
    *   The disassembled source
    *   The register file
    *   Any program output/input (stdout and stdin only)
    *   The stack
    *   The data segment
*   The TUI also:
    *   Shows the net effect the next instruction to run will have
    *   For taken branches, draws a line to the branch target
    *   Highlights each stack frame
    *   Highlights the most recent memory access
    *   Highlights each labeled data segment chunk

The controls are minimal and can be displayed by hitting `?`:

*   Scroll the cursor through the source using Up, Down, PgUp, and PgDown
*   Step forward/backward using Right, Left
*   Jump to beginning/end of current function using Home, End
*   Jump forward/backward to current cursor position using Enter, Backspace
*   Various toggles to control what is displayed

risclet is intended for students learning the basics of assembly
language, and is especially for anyone who has been intimidated by
standard debugging tools.


Features
--------

*   Support for the full RV64imc instruction set
*   Checks for proper register use according to the ABI, and
    emphasizing simple function structure and stack usage
*   Minimal controls, no breakpoints or watch expressions
*   Lightweight navigation that makes it quick and easy to move to
    different execution points in the program
*   Emulates a tiny set of system calls:
    *   write to stdout
    *   read from stdin
    *   exit
*   Runs the entire program first, then launches the TUI, so
    lightly-interactive programs are easy to work with
*   Portable with only a single crate dependency (crossterm for the
    TUI)


Contributors
------------

This tool is made for students learning the basics of assembly
language and is designed for small programs written by hand. Minimal
features and especially simple controls are explicit goals. With
that in mind, I will be reluctant to accept pull requests and
feature requests if they work against those goals. Bug reports and
fixes are welcome.

*   Russ Ross (github.com/russross)


## RISC-V Test Suite

Complete RV32IMC instruction validation test suite: **50/50 tests passing (100%)**

All test infrastructure is self-contained in the `test/` directory.

**Quick start:**
```bash
cd test
./run_all_tests.sh
```

**Documentation:** See `test/INDEX.md` for complete documentation index, or `test/GETTING_STARTED.md` to get started.

**Test coverage:**
- 40 RV32I base instruction tests
- 8 RV32M multiply/divide tests  
- 1 RV32C compressed instruction test
- Complete validation of all applicable RISC-V instructions

