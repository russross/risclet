RISC-V highlighting in micro
===========================

This definition adds RISC-V assembly highlighting to micro. Its filetype name is `riscv`. You do not need the Risclet executable to use it. For the assembly language reference, see [SYNTAX.md](../../SYNTAX.md).


Install
-------

The commands below are for a Linux or macOS terminal, outside micro.

1. Open a terminal in your downloaded or cloned `risclet` repository. Run `pwd` to see your location and `ls syntaxhighlighting/micro/riscv.yaml` to check that you are in the right directory.
2. Copy the definition into your micro configuration:

    ```sh
    mkdir -p ~/.config/micro/syntax
    cp syntaxhighlighting/micro/riscv.yaml ~/.config/micro/syntax/riscv.yaml
    ```

    `~` means your home directory. `mkdir -p` creates missing directories; `cp` copies the file. No `sudo` is needed. Repeating the copy updates the installed definition, replacing that destination file.
3. Quit and restart micro, then open an assembly file: `micro program.s` (replace `program.s` with your filename). The definition recognizes lowercase `.s` filenames.
4. If micro selects another assembly language, press Ctrl-e, type `set filetype riscv`, and press Enter. The status line should show `riscv`.

The default configuration directory is `~/.config/micro`. If you set `MICRO_CONFIG_HOME`, `XDG_CONFIG_HOME`, or start micro with `-config-dir`, copy the definition into the `syntax/` directory under the configuration directory actually in use. See micro's [configuration documentation](https://github.com/micro-editor/micro/blob/master/runtime/help/options.md).


Keep `.s` files associated with RISC-V
-------------------------------------

Other assembly definitions can also match `.s`. To choose RISC-V consistently, edit `~/.config/micro/settings.json` (or the equivalent file in your custom configuration directory) and add this filename-specific setting:

```json
{
    "*.s": {
        "filetype": "riscv"
    }
}
```

If the file already contains settings, add the `"*.s"` entry inside the existing outer braces; separate entries with commas. Do not replace your other settings. Save and restart micro. This applies to every lowercase `.s` file; if you also write x86 assembly, use a narrower filename pattern for your RISC-V files or select the filetype manually instead. Micro documents filename-specific settings in its [options guide](https://github.com/micro-editor/micro/blob/master/runtime/help/options.md).


Common problems
---------------

*   **`set filetype riscv` gives no highlighting.** Check that the file is named `riscv.yaml`, not `riscv.yaml.txt`, and is in the active configuration directory's `syntax/` subdirectory. Restart micro after copying it. Press Ctrl-e and enter `set syntax true` if syntax highlighting is disabled.
*   **Another definition keeps winning.** Check the `"*.s"` entry and other filename-specific settings in `settings.json`. Look for duplicate RISC-V definitions in your configuration's `syntax/` and plugin directories. A definition with the same filetype name can conflict even when you select `riscv` explicitly. Disable the conflicting user-installed definition rather than changing micro's bundled files.
*   **Colors are faint or different on another computer.** Micro's colorscheme and your terminal determine the colors. Press Ctrl-e and enter `help colors` for micro's color settings. Syntax colors do not check whether a program assembles correctly.

To update, repeat the copy command and restart micro. To uninstall, remove the installed `riscv.yaml` and any settings that select `riscv`, then restart micro.
