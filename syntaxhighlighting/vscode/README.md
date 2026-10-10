RISC-V highlighting in VS Code
=============================

This extension adds a **RISC-V** language mode for assembly files. Its settings identifier is `riscv`. It provides highlighting, `#` comment toggling, and bracket/quote pairing; you do not need the Risclet executable to use it. For the assembly language reference, see [SYNTAX.md](../../SYNTAX.md).


Install
-------

These terminal commands are for the standard desktop VS Code installation on Linux or macOS.

1. Open a terminal in your downloaded or cloned `risclet` repository. Run `pwd` to see your location and `ls syntaxhighlighting/vscode/package.json` to check that you are in the right directory.
2. Link the extension into VS Code's extensions directory:

    ```sh
    mkdir -p ~/.vscode/extensions
    ln -s "$PWD/syntaxhighlighting/vscode" ~/.vscode/extensions/russross.riscv-0.1.0
    ```

    `~` means your home directory. `mkdir -p` creates missing directories. `ln -s` creates a link to this checkout, so keep the checkout in its current location. No `sudo` is needed. If the destination exists, inspect it with `ls -ld ~/.vscode/extensions/russross.riscv-0.1.0` before changing it.
3. Restart VS Code, or open the Command Palette (**Ctrl+Shift+P** on Linux, **Cmd+Shift+P** on macOS) and run **Developer: Reload Window**.
4. Open a `.s` file. Click the language name at the bottom right of the window, or run **Change Language Mode** from the Command Palette, and select **RISC-V**.

On Windows, copy this entire extension directory into `%USERPROFILE%\.vscode\extensions\russross.riscv-0.1.0` using File Explorer. `package.json` must be directly inside that directory, with `syntaxes/` beside it. Reload VS Code afterward.

VS Code Insiders, profiles, a custom `--extensions-dir`, and remote windows (SSH, WSL, or containers) can use different extension locations. Install in the location used by the window where you edit your files; a local desktop install may not appear in a remote window. The browser demo already includes highlighting and needs no VS Code extension.


Make RISC-V the default for `.s` files
------------------------------------

Several assembly extensions claim `.s`, often for x86. An explicit file association avoids relying on which extension wins detection.

Open the folder containing your RISC-V work, then run **Preferences: Open Workspace Settings (JSON)** from the Command Palette. Add:

```json
{
    "files.associations": {
        "*.s": "riscv"
    }
}
```

If settings already exist, add the entry to the existing `files.associations` object, or add that object inside the outer braces. Preserve other settings and put commas between entries. VS Code stores folder settings in `.vscode/settings.json`. Use **Preferences: Open User Settings (JSON)** instead if you want this association for all projects. Folder settings can override user settings. See VS Code's [language association guide](https://code.visualstudio.com/docs/languages/overview).


Common problems
---------------

*   **RISC-V is missing from Change Language Mode.** Check that the link points to an existing directory with `package.json` directly inside it. Open Extensions and check that **RISC-V Assembly** by `russross` is enabled in this window/profile. Reload after installation. If it remains missing, check **Developer: Show Logs… → Extension Host** for loading errors.
*   **The status bar says Assembly or another language.** Select **RISC-V** and add the workspace association above. Check both user and workspace settings for competing `files.associations` entries.
*   **The status bar says RISC-V but highlighting still looks wrong.** Another RISC-V extension can supply the same language identifier. Disable competing assembly extensions for this workspace, reload, and try again. **Developer: Inspect Editor Tokens and Scopes** should show `source.riscv` for this grammar; check the enabled extensions as well, since another grammar can use the same scope name.
*   **You moved the checkout.** Inspect the link with `ls -l ~/.vscode/extensions/russross.riscv-0.1.0`. If it is the broken link you created, remove it and repeat installation from the new location.
*   **Colors differ from examples.** Your VS Code color theme chooses colors. Highlighting does not run the assembler or validate the program.

To uninstall, remove the extension link/directory you installed and any associations to `riscv`, then reload VS Code. Removing a symbolic link does not remove the checkout.
