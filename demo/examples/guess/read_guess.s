                .global read_guess
                .equ    read_guess_args, 0
                .data
input_byte:
                .space  1
                .text
                # read_guess() -> digit 0..9, -1 for an invalid line, -2 at EOF.
                # Read through newline so piped input behaves like terminal input.
read_guess:
                li      t0, -1
                li      t1, 0
1:
                li      a0, 0
                la      a1, input_byte
                li      a2, 1
                li      a7, 63
                ecall
                blez    a0, 5f
                lbu     t2, (a1)
                li      t3, 10
                beq     t2, t3, 5f

                # Accept a digit only as the first character of the line.
                bnez    t1, 4f
                li      t3, '0'
                blt     t2, t3, 2f
                li      t3, '9'
                bgt     t2, t3, 2f
                addi    t0, t2, -48
                j       4f
2:
                li      t0, -1
4:
                addi    t1, t1, 1
                j       1b

                # An empty EOF ends play; a final digit needs no newline.
5:
                bnez    a0, 6f
                bnez    t1, 6f
                li      t0, -2
                j       7f
6:
                li      t3, 1
                beq     t1, t3, 7f
                li      t0, -1
7:
                mv      a0, t0
                ret
