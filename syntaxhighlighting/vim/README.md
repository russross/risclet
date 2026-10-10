RISC-V highlighting in Vim
=========================

These files add RISC-V assembly highlighting and `#` comment settings to Vim. The filetype name is `riscv`. You do not need to install the Risclet executable to use them. For the assembly language reference, see [SYNTAX.md](../../SYNTAX.md).


Install
-------

These instructions use Vim 8 or newer on Linux or macOS. Run shell commands in a terminal; run commands beginning with `:` inside Vim, after pressing Esc.

1. Open a terminal in your downloaded or cloned `risclet` repository. Run `pwd` to see the current directory and `ls syntaxhighlighting/vim` to check that you are in the right place.
2. Install the runtime files:

    ```sh
    mkdir -p ~/.vim/pack/local/start
    ln -s "$PWD/syntaxhighlighting/vim" ~/.vim/pack/local/start/riscv
    ```

    `~` means your home directory. `mkdir -p` creates missing directories. `ln -s` creates a link to this checkout, so keep it in its current location. No `sudo` is needed. If the destination already exists, inspect it with `ls -ld ~/.vim/pack/local/start/riscv` before changing it.
3. Open your configuration with `vim ~/.vimrc`. Add the following near the end of the file, after other filetype setup:

    ```vim
    filetype plugin on
    syntax on

    augroup riscv_source
      autocmd!
      autocmd BufRead,BufNewFile *.s setlocal filetype=riscv
    augroup END
    ```

    Press `i` to insert text, then Esc and `:wq` followed by Enter to save and quit. Preserve your existing configuration.
4. Restart Vim and open one of your assembly files, for example `vim program.s` (replace `program.s` with your filename). Run `:setlocal filetype? syntax?`; both values should be `riscv`.

The `.vimrc` rule deliberately selects RISC-V for every lowercase `.s` file. Vim's bundled rules, including those supplied by Debian, can otherwise choose generic or x86 assembly first. This package's default detector only selects `riscv` if no earlier detector has chosen a filetype. The explicit `setlocal filetype=riscv` overrides that choice; `:setfiletype riscv` does not reliably override an existing filetype. See Vim's [filetype documentation](https://vimhelp.org/filetype.txt.html).

If you also edit other assembly languages, replace `*.s` in the rule with a path pattern for your RISC-V work, such as `*/2810/*.s`, or select the filetype manually for each buffer with `:setlocal filetype=riscv`.


Common problems
---------------

*   **The file still uses assembly or x86 colors.** Run `:verbose setlocal filetype? syntax?` to see the current values and where they were last set. Check for later rules in your vimrc, plugins, or `after/` directories. Put the override after other filetype setup and restart Vim. For the current buffer, use `:setlocal filetype=riscv syntax=riscv`.
*   **The filetype is right but there are no colors.** Run `:syntax on`, then `:setlocal syntax=riscv`. Use `:scriptnames` to check that this package's `syntax/riscv.vim` loaded. `:echo globpath(&runtimepath, 'syntax/riscv.vim')` lists matching files on the runtime search path; another installed RISC-V definition may be loaded first. Inspect `~/.vim/syntax`, other packages, and system runtime directories. Remove or disable only a conflicting copy you recognize; do not edit Debian's system files.
*   **The link stopped working after moving the repository.** Run `ls -l ~/.vim/pack/local/start/riscv` in the terminal. If it is the link created above, remove that link with `rm ~/.vim/pack/local/start/riscv`, then repeat installation from the new repository location. Removing the link does not remove the checkout.
*   **Colors differ from a screenshot.** Your Vim colorscheme and terminal palette choose the colors. Highlighting does not assemble the program or prove that it is correct.

To uninstall, remove the `riscv` package link and the `riscv_source` block from your vimrc, then restart Vim.
