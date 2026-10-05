Guess the digit
===============

```sh
risclet
risclet debug --check-abi
```

Run in this directory or the browser's VM tab. Guess a digit from 0 to 9 and
press Enter; each wrong guess produces a low/high hint. The secret is fixed at
6 so debugger exercises are repeatable. Try 2, 9, an invalid line such as `xx`,
and finally 6. Invalid lines do not count as attempts. EOF also ends the game.
No printing library is needed: every message has a calculated length and goes
directly to the write syscall.

`start.s` prints the introduction, calls `play(secret)`, and exits.
`play.s` owns the game loop and the `attempts` word. `read_guess.s` reads a full
line and returns a digit, -1 for invalid input, or -2 at EOF. It reads bytes
individually so multiple lines supplied through a pipe are consumed separately;
a final digit without a newline is accepted too.

Play first, replay afterward
----------------------------

1. Launch `risclet debug --check-abi` and play the complete game using the sequence
    above. The debugger opens **after** the win or EOF. During playback it uses
    captured input; it does not ask for new guesses or allow you to change them.
2. Hide stack and text memory with `s` and `t`; keep output, data, and registers
    visible. Scroll to the first instruction of `play`, `addi sp, sp, -16`,
    and press Enter to skip the introduction. The secret arrives in a0 and is
    preserved in s0. `?` lists controls; `q` quits playback.
3. Select the loop's prompt instruction, `li a0, 1` just after `mv s0, a0`.
    Press Enter to reach the first iteration, then repeatedly Enter to watch
    whole unsuccessful rounds. Backspace undoes a round. The output panel
    gains/loses the prompt, captured input, and hint; `attempts` increases for
    valid wrong guesses and stays unchanged for the invalid line. The count is
    a little-endian word: `02 00 00 00` means two attempts. Memory stays in hex;
    `x` toggles the register display base.
4. To focus on complete input calls, select `li t2, 0`, immediately after
    `jal read_guess`. Enter reaches the first call's return with a0 = 2;
    subsequent Enter presses reach a0 = 9, -1, and 6. Backspace reverses these
    stops, including the response and next prompt between input calls. At the
    final stop, the winning attempt has not yet been counted; Right steps
    through its count update and winning response.
5. For byte-level detail, select the `ecall` in `read_guess` and jump to a visit.
    Right/Left show one captured byte entering `input_byte`. Enter at that
    syscall skips to the next read; compare this detail with the round-level
    view. Enter does nothing at a selected instruction with no future visit,
    including the prompt after the final winning round.

A repeatable plain trace can also consume piped input:

```sh
printf '2\n9\nxx\n6\n' | risclet trace --check-abi
```

Break preservation of the secret
--------------------------------

Run `risclet --check-abi` first. In `play.s`, replace `mv s0, a0` with
`mv t6, a0` and change both comparisons using s0 to use t6. Run again with ABI
checking and enter a valid digit. `jal read_guess` invalidates caller-saved t6,
so the later comparison fails validation even though `read_guess` does not
physically overwrite t6. Restore the initialization and both comparisons.
