Syntax highlighting workflows
=============================

The editor-facing language identifier is `riscv`. These definitions color the assembly accepted by Risclet; they do not assemble, execute, or fully validate it. The assembler tokenizer and parser define the vocabulary and lexical behavior. Shared fixtures in `tests/highlighting.json` protect statement/operand distinctions, reserved registers, label references, literal escapes, and recovery at physical line boundaries.


Installation and selection
--------------------------

Vim loads this directory as a runtime package. Its `ftdetect/riscv.vim` handles lowercase `.s` files only when a previous detector has not selected a filetype. A user's later `BufRead,BufNewFile` rule with `setlocal filetype=riscv` overrides bundled assembly selection. Selecting that filetype loads comment/word settings from `ftplugin/riscv.vim` and highlighting from `syntax/riscv.vim`; another runtime entry with the same filename can take precedence. The syntax sets `b:current_syntax` to `riscv` and links its groups to the user's colorscheme.

Micro loads the copied `syntax/riscv.yaml` from its active configuration directory. Filename detection matches lowercase `.s`; an explicit buffer filetype or filename-specific setting selects `riscv` when another assembly definition matches. The installed file is a copy and must be copied again to update it. Another definition using the same filetype name can still conflict with explicit selection.

VS Code discovers the extension's `package.json`, which contributes the `riscv` language, `.s` association, language configuration, and `source.riscv` TextMate grammar. A user's file association selects the language identifier; it does not uniquely select a grammar if multiple enabled extensions contribute that identifier. Themes control the final colors. Installation links or copies the entire extension directory into the active extension location and requires a window reload.


Browser integration and verification
------------------------------------

CodeMirror consumers import `riscv()` and install its `LanguageSupport` alongside a highlighting style. The module also exports `riscvLanguage` and `riscvStreamParser`; the stream parser's name is `riscv`. Filename selection belongs to the consuming application. Switching languages must replace the active language extension rather than accumulate language support.

The demo's `ui/editor-text.ts` maps `.s` and `.S` to the shared CodeMirror factory. Editor sessions install the selected language when a file opens. Both browser highlighting fixtures and session tests import the shared module. `demo/Makefile` includes it in the hashed UI inventory, so a rename or edit invalidates the client bundle. Webpack resolves its CodeMirror dependency through the UI's node_modules; TypeScript resolves the same dependency through the UI configuration.

`make -C syntaxhighlighting test` checks asset formatting/parsing and exercises Vim, micro's actual highlighting engine, and VS Code's TextMate/Oniguruma engine. `make -C demo test-highlighting` type-checks the UI and exercises highlighting and editor sessions in Chrome with disposable profiles. Syntax checks create no persistent deployment outputs; their clean targets have nothing to remove. Demo `mostlyclean` removes browser fixture intermediates while retaining deployment assets.
