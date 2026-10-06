                .global test_search
                .equ    test_search_args, 3
                .data
query:
                .ascii "target "
                .equ    query_len, (. - query)
result:
                .ascii " -> index "
                .equ    result_len, (. - result)
newline:
                .ascii "\n"
                .equ    newline_len, (. - newline)
                .text
                # test_search(bytes, count, target): display the query and returned index.
test_search:
                addi    sp, sp, -16
                sw      s0, 0(sp)
                sw      s1, 4(sp)
                sw      s2, 8(sp)
                sw      ra, 12(sp)
                mv      s0, a0
                mv      s1, a1
                mv      s2, a2

                # Print the target before restoring the three search arguments.
                li      a0, 1
                la      a1, query
                li      a2, query_len
                li      a7, 64
                ecall
                mv      a0, s2
                jal     print_int
                li      a0, 1
                la      a1, result
                li      a2, result_len
                li      a7, 64
                ecall
                mv      a0, s0
                mv      a1, s1
                mv      a2, s2
                jal     binary_search
                jal     print_int

                # Finish this output line, then restore the caller's frame.
                li      a0, 1
                la      a1, newline
                li      a2, newline_len
                li      a7, 64
                ecall
                lw      s0, 0(sp)
                lw      s1, 4(sp)
                lw      s2, 8(sp)
                lw      ra, 12(sp)
                addi    sp, sp, 16
                ret
