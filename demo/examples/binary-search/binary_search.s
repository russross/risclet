                .global binary_search
                .equ    binary_search_args, 3

                .text
                # binary_search(bytes, count, target) -> index, or -1 when absent.
                # t0 = low, t1 = high (exclusive), t2 = midpoint, t4 = value.
binary_search:
                li      t0, 0
                mv      t1, a1

                # halve the candidate interval without overflowing low + high.
1:              bge     t0, t1, 5f
                sub     t2, t1, t0
                srli    t2, t2, 1
                add     t2, t0, t2
                add     t3, a0, t2
                lb      t4, (t3)
                bne     t4, a2, 2f
                mv      a0, t2
                j       6f

                # exclude the midpoint on either side of an unsuccessful comparison.
2:              bge     t4, a2, 3f
                addi    t0, t2, 1
                j       4f
3:              mv      t1, t2
4:              j       1b
5:              li      a0, -1
6:              ret
