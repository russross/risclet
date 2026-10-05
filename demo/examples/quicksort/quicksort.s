                .global quicksort
                .equ    quicksort_args, 3
                .text
                # quicksort(bytes, low, high): sort the half-open range [low, high).
quicksort:
                # Keep the range and pivot alive across partition and recursion.
                addi    sp, sp, -32
                sw      s0, 0(sp)
                sw      s1, 4(sp)
                sw      s2, 8(sp)
                sw      s3, 12(sp)
                sw      ra, 28(sp)
                mv      s0, a0
                mv      s1, a1
                mv      s2, a2

                # Empty and singleton ranges already satisfy the sorted order.
                sub     t0, s2, s1
                li      t1, 2
                blt     t0, t1, 1f
                jal     partition
                mv      s3, a0

                # Sort the left region, then the right region excluding the pivot.
                mv      a0, s0
                mv      a1, s1
                mv      a2, s3
                jal     quicksort
                mv      a0, s0
                addi    a1, s3, 1
                mv      a2, s2
                jal     quicksort

                # Both children are sorted when execution reaches this postlude.
1:
                lw      s0, 0(sp)
                lw      s1, 4(sp)
                lw      s2, 8(sp)
                lw      s3, 12(sp)
                lw      ra, 28(sp)
                addi    sp, sp, 32
                ret
