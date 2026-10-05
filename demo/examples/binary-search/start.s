                .global _start
                .data
values:
                .byte   -12, -7, -3, 0, 2, 5, 9, 14, 18, 23, 31, 42, 56, 71, 88
                .equ    values_len, (. - values)
duplicates:
                .byte   1, 2, 2, 2, 9
                .equ    duplicates_len, (. - duplicates)
                .text
_start:
                la      gp, __global_pointer$

                # Search for 23 among values_len entries.
                la      a0, values
                li      a1, values_len
                li      a2, 23
                jal     test_search

                # Search for -12 among values_len entries.
                la      a0, values
                li      a1, values_len
                li      a2, -12
                jal     test_search

                # Search for 88 among values_len entries.
                la      a0, values
                li      a1, values_len
                li      a2, 88
                jal     test_search

                # Search for 6 among values_len entries.
                la      a0, values
                li      a1, values_len
                li      a2, 6
                jal     test_search

                # Search for -20 among values_len entries.
                la      a0, values
                li      a1, values_len
                li      a2, -20
                jal     test_search

                # Search for 100 among values_len entries.
                la      a0, values
                li      a1, values_len
                li      a2, 100
                jal     test_search

                # Search for 2 among duplicates_len entries.
                la      a0, duplicates
                li      a1, duplicates_len
                li      a2, 2
                jal     test_search

                # Search for -12 among 1 entries.
                la      a0, values
                li      a1, 1
                li      a2, -12
                jal     test_search

                # Search for 0 among 1 entries.
                la      a0, values
                li      a1, 1
                li      a2, 0
                jal     test_search

                # Search for 23 among 0 entries.
                la      a0, values
                li      a1, 0
                li      a2, 23
                jal     test_search

                li      a0, 0
                li      a7, 93
                ecall
