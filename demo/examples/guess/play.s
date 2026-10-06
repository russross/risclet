                .global play
                .equ    play_args, 1
                .data
prompt:
                .ascii "Guess a digit (0-9): "
                .equ    prompt_len, (. - prompt)
invalid:
                .ascii "Enter exactly one digit.\n"
                .equ    invalid_len, (. - invalid)
low:
                .ascii "Too low.\n"
                .equ    low_len, (. - low)
high:
                .ascii "Too high.\n"
                .equ    high_len, (. - high)
won:
                .ascii "Correct!\n"
                .equ    won_len, (. - won)
ended:
                .ascii "Input ended.\n"
                .equ    ended_len, (. - ended)
                .balign 4
attempts:
                .word  0
                .text
                # play(secret): repeat guesses until correct or input ends.
play:
                addi    sp, sp, -16
                sw      s0, 0(sp)
                sw      ra, 12(sp)
                mv      s0, a0
1:
                # Each iteration prompts, reads, and selects one response.
                li      a0, 1
                la      a1, prompt
                li      a2, prompt_len
                li      a7, 64
                ecall
                jal     read_guess
                li      t2, 0
                li      t0, -2
                bne     a0, t0, 2f
                la      a1, ended
                li      a2, ended_len
                li      t2, 1
                j       7f
2:
                bgez    a0, 3f
                la      a1, invalid
                li      a2, invalid_len
                j       7f
3:
                # Valid guesses increment the visible count in data memory.
                la      t0, attempts
                lw      t1, (t0)
                addi    t1, t1, 1
                sw      t1, (t0)
                bge     a0, s0, 4f
                la      a1, low
                li      a2, low_len
                j       7f
4:
                ble     a0, s0, 5f
                la      a1, high
                li      a2, high_len
                j       7f
5:
                la      a1, won
                li      a2, won_len
                li      t2, 1
7:
                # Write the selected response and continue unless play is done.
                li      a0, 1
                li      a7, 64
                ecall
                bnez    t2, 8f
                j       1b
8:
                lw      s0, 0(sp)
                lw      ra, 12(sp)
                addi    sp, sp, 16
                ret
