risclet
=======

This is a lightweight RISC-V assembler, disassembler, simulator, debugger, and linter for students learning assembly language. By design it has few controls and limited functionality, with simplicity and approachability as overriding goals. It is designed to be the only tool a student needs to install in a course covering assembly language basics.

Try it out: [live demo](https://russross.github.io/risclet/)

Running risclet
---------------

The simplest usage is just:

    risclet

This assembles (and links) `*.s` files in the current directory in memory, or loads `a.out` if no assembly files are present, then runs the program and exits. Optional arguments select specific assembly files or an executable.

In addition:

*   `risclet run`: same as plain `risclet`
*   `risclet trace`: run the program, displaying each disassembled instruction and its side effects as it goes
*   `risclet assemble`: assemble source and write to an object file (`a.out` by default)
*   `risclet disassemble`: disassemble and dump the program
*   `risclet debug`: run the program to completion, then enter a TUI to step back and forth through execution and examine its effects

There are various other options and controls as well. Notably `--check-abi` for `run`, `trace`, and `debug` will check for common ABI errors and common mistakes beginning programmers are likely to make and treats them as errors.

Each command defaults to reading all `*.s` files in the current directory, assembling and linking them, and then proceeding. Or if there are no `*.s` files it will look for `a.out` instead. Obvious exceptions apply, but this does mean that `disassemble` will assemble and link the entire program and then disassemble it by default.


The debugger
------------

Use `risclet debug` to launch the interactive debugger, which does the following:

*   Disassembles the program
*   Simulates the complete execution of the program including all input/output
*   Traces and records the effects of each instruction
*   Launches a TUI (80×24 or larger terminal recommended) that allows simple stepping and jumping forward and backward through the program, while displaying:
    *   The disassembled source
    *   The register file
    *   Any program output/input (stdout and stdin only)
    *   The stack segment
    *   The data segment
    *   The text segment
*   The TUI also:
    *   Shows the net effect the next instruction to run will have
    *   For taken branches, draws a line to the branch target
    *   Uses subtle colors to give structure to memory displays:
        *   Each frame in the stack segment
        *   Each labeled chunk in the data segment
        *   Each function in the text segment
    *   Uses highlights to identify current/recent access:
        *   Most recent memory access in stack/data segment
        *   Bytes of the current instruction in the text segment

The controls are minimal and can be displayed by hitting `?`:

*   Scroll the cursor through the source using Up, Down, PgUp, and PgDown
*   Step forward/backward using Right, Left
*   Jump to beginning/end of current function using Home, End
*   Jump forward/backward to current cursor position using Enter, Backspace
*   Various toggles to control what is displayed

risclet is intended for students learning the basics of assembly language, and is especially for anyone who has been intimidated by the complexity of tools like `gdb`.


Features
--------

*   Support for the full rv32imac instruction set
*   Checks for proper register use according to the ABI, and lints to enforce simple function structure and stack usage
*   Minimal controls, no breakpoints or watch expressions
*   Lightweight navigation that makes it quick and easy to move to different execution points in the program
*   Emulates a tiny set of system calls:
    *   write to stdout
    *   read from stdin
    *   exit
*   Runs the entire program first, then launches the TUI, so lightly-interactive programs are easy to work with
*   Portable with only a single crate dependency (crossterm for the TUI)
*   Releases on github with single-file, statically linked, self-contained binaries for common systems


Editor syntax highlighting
--------------------------

Syntax definitions follow the language implemented by the assembler:

*   [CodeMirror 6](syntaxhighlighting/codemirror/README.md)
*   [Vim](syntaxhighlighting/vim/README.md)
*   [VS Code](syntaxhighlighting/vscode/README.md)
*   [Micro](syntaxhighlighting/micro/README.md)


Contributors
------------

This tool is made for students learning the basics of assembly language and is designed for small programs written by hand. Minimal features and especially simple controls are explicit goals. With that in mind, I will be reluctant to accept pull requests and feature requests if they work against those goals. Bug reports and fixes are welcome.

*   Russ Ross (github.com/russross)
