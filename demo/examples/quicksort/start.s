                .global _start
                .data
before:
                .string "before: "
                .equ    before_len, (. - before)
after:
                .string "after:  "
                .equ    after_len, (. - after)
values:
                .byte   9, -3, 7, 1, 8, 2, -6, 5, 0, 4, 7, -1
                .equ    values_len, (. - values)
ordered:
                .byte   -4, -1, 0, 3, 8
                .equ    ordered_len, (. - ordered)
reversed:
                .byte   8, 3, 0, -1, -4
                .equ    reversed_len, (. - reversed)
equal:
                .byte   2, 2, 2, 2
                .equ    equal_len, (. - equal)
single:
                .byte   -8
                .equ    single_len, (. - single)
                .text
_start:
                la      gp, __global_pointer$

                # Print the values case before and after sorting.
                li      a0, 1
                la      a1, before
                li      a2, before_len
                li      a7, 64
                ecall
                la      a0, values
                li      a1, values_len
                jal     print_array
                la      a0, values
                li      a1, 0
                li      a2, values_len
                jal     quicksort
                li      a0, 1
                la      a1, after
                li      a2, after_len
                li      a7, 64
                ecall
                la      a0, values
                li      a1, values_len
                jal     print_array

                # Print the ordered case before and after sorting.
                li      a0, 1
                la      a1, before
                li      a2, before_len
                li      a7, 64
                ecall
                la      a0, ordered
                li      a1, ordered_len
                jal     print_array
                la      a0, ordered
                li      a1, 0
                li      a2, ordered_len
                jal     quicksort
                li      a0, 1
                la      a1, after
                li      a2, after_len
                li      a7, 64
                ecall
                la      a0, ordered
                li      a1, ordered_len
                jal     print_array

                # Print the reversed case before and after sorting.
                li      a0, 1
                la      a1, before
                li      a2, before_len
                li      a7, 64
                ecall
                la      a0, reversed
                li      a1, reversed_len
                jal     print_array
                la      a0, reversed
                li      a1, 0
                li      a2, reversed_len
                jal     quicksort
                li      a0, 1
                la      a1, after
                li      a2, after_len
                li      a7, 64
                ecall
                la      a0, reversed
                li      a1, reversed_len
                jal     print_array

                # Print the equal case before and after sorting.
                li      a0, 1
                la      a1, before
                li      a2, before_len
                li      a7, 64
                ecall
                la      a0, equal
                li      a1, equal_len
                jal     print_array
                la      a0, equal
                li      a1, 0
                li      a2, equal_len
                jal     quicksort
                li      a0, 1
                la      a1, after
                li      a2, after_len
                li      a7, 64
                ecall
                la      a0, equal
                li      a1, equal_len
                jal     print_array

                # Print the single case before and after sorting.
                li      a0, 1
                la      a1, before
                li      a2, before_len
                li      a7, 64
                ecall
                la      a0, single
                li      a1, single_len
                jal     print_array
                la      a0, single
                li      a1, 0
                li      a2, single_len
                jal     quicksort
                li      a0, 1
                la      a1, after
                li      a2, after_len
                li      a7, 64
                ecall
                la      a0, single
                li      a1, single_len
                jal     print_array

                # Print the empty case before and after sorting.
                li      a0, 1
                la      a1, before
                li      a2, before_len
                li      a7, 64
                ecall
                la      a0, single
                li      a1, 0
                jal     print_array
                la      a0, single
                li      a1, 0
                li      a2, 0
                jal     quicksort
                li      a0, 1
                la      a1, after
                li      a2, after_len
                li      a7, 64
                ecall
                la      a0, single
                li      a1, 0
                jal     print_array

                # Return success after the complete display sequence.
                li      a0, 0
                li      a7, 93
                ecall
