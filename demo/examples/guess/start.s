                .global _start
                .data
intro:
                .ascii "Find the secret digit. Hints follow each guess.\n"
                .equ    intro_len, (. - intro)
                .text
_start:
                # This fixed secret makes captured games repeatable for debugger exercises.
                la      gp, __global_pointer$
                li      a0, 1
                la      a1, intro
                li      a2, intro_len
                li      a7, 64
                ecall
                li      a0, 6
                jal     play

                # Exit once the game has won or consumed all available input.
                li      a0, 0
                li      a7, 93
                ecall
