                .global _start
                .equ    sys_exit, 93
                .equ    reduction_steps_args, 1

                .data
heading:        .asciz  "reduction_steps(value) -> steps\n"
msg_open:       .asciz  "  ("
msg_close:      .asciz  ") -> "
newline:        .asciz  "\n"
                .balign 4
inputs:         .4byte  0, 1, 2, 3, 6, 7, 8, 9
                .4byte  15, 16, 31, 32, 42, 255, 256, 1023, 1024
inputs_end:

                .text
_start:
                la      gp, __global_pointer$
                la      a0, heading
                jal     print_string
                la      s0, inputs
                la      s1, inputs_end
1:              bge     s0, s1, 2f
                lw      s2, (s0)
                la      a0, msg_open
                jal     print_string
                mv      a0, s2
                jal     print_int
                la      a0, msg_close
                jal     print_string
                mv      a0, s2
                jal     reduction_steps
                jal     print_int
                la      a0, newline
                jal     print_string
                addi    s0, s0, 4
                j       1b
2:              li      a0, 0
                li      a7, sys_exit
                ecall
