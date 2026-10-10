RISC-V highlighting in CodeMirror 6
==================================

This is for developers embedding a CodeMirror editor in a web application. If you only want to edit assembly in the [browser demo](https://russross.github.io/risclet/), highlighting is already installed. For the assembly language reference, see [SYNTAX.md](../../SYNTAX.md).


Add it to an application
-----------------------

Use a TypeScript application with CodeMirror 6 and `@codemirror/language` installed. This file does not work with CodeMirror 5 or install into a desktop editor.

1. Copy `riscv.ts` from this directory into your application's source directory. Keep its import of `@codemirror/language`; your application's package manager and bundler must resolve that package.
2. Import the language factory and include it in your editor's extensions. For an application using the `codemirror` package:

    ```typescript
    import { basicSetup } from "codemirror";
    import { EditorView } from "@codemirror/view";
    import { riscv } from "./riscv";

    const parent = document.querySelector<HTMLElement>("#editor");
    if (parent === null) throw new Error("Missing #editor element");

    const editor = new EditorView({
      parent,
      doc: "li a0, 42\n# RISC-V assembly\n",
      extensions: [basicSetup, riscv()],
    });
    ```

3. Rebuild your application and open it in the browser. Instructions, registers, and comments should receive highlighting. `basicSetup` includes a default highlighting style; if you use a custom setup, include `syntaxHighlighting(defaultHighlightStyle)` from `@codemirror/language`, or your own highlighting style.

The module exports `riscv()` (a `LanguageSupport` factory), `riscvLanguage` (a `StreamLanguage`), and `riscvStreamParser`. The language name is `riscv`. It uses standard highlight tags so your theme controls the colors. See the [CodeMirror documentation](https://codemirror.net/docs/) for editor configuration.


Select the language when opening files
-------------------------------------

CodeMirror does not choose this language just because a file ends in `.s`. Your application must select `riscv()` when it opens RISC-V source. If an existing file picker maps `.s` to an x86 assembly language, change that mapping.

For an editor that switches among languages, keep its language extension in a CodeMirror `Compartment` and reconfigure that compartment when the selected file changes. Install one language at a time in that compartment; do not leave a previous assembly language in the editor's extension list. Keep the theme and highlighting style outside the compartment so switching languages preserves them.

The repository's demo imports this module directly from `syntaxhighlighting/codemirror/riscv.ts`. `demo/ui/editor-text.ts` selects it for `.s` and `.S` files. The demo's Make input inventory tracks the shared file, so edits rebuild the bundle without maintaining a second copy.


Common problems
---------------

*   **No syntax colors.** Check both the language extension and the highlighting style. A CSS editor theme alone does not necessarily provide syntax highlighting. Confirm that your filename selection code reaches `riscv()`.
*   **Another assembly language is still active.** Inspect all extensions and language compartments, including any automatic language loader. Replace the previous language support when switching files.
*   **The bundler cannot resolve `@codemirror/language`.** Make sure it is installed in the application consuming this file. Importing a shared source file outside the application directory may require configuring your bundler's module search path and TypeScript's resolution paths, as the demo does.
*   **An “Unrecognized extension value” error appears.** Check for multiple installed copies of CodeMirror's core packages. Ensure the application and this module resolve compatible packages from the same dependency tree.

These rules highlight the RISC-V assembly supported by Risclet. Colors do not validate a program or guarantee support for every directive accepted by other RISC-V assemblers.
