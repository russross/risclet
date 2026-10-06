                .global partition
                .equ    partition_args, 3

                .text
                # partition(bytes, low, high) -> pivot index; high is exclusive.
                # t0 = pivot value, t1 = boundary, t2 = scan index.
partition:
                addi    t6, a2, -1
                add     t3, a0, t6
                lb      t0, (t3)
                mv      t1, a1
                mv      t2, a1

                # values smaller than the pivot accumulate before the boundary.
1:              bge     t2, t6, 3f
                add     t3, a0, t2
                lb      t4, (t3)
                bge     t4, t0, 2f
                add     t5, a0, t1
                lb      a3, (t5)
                sb      t4, (t5)
                sb      a3, (t3)
                addi    t1, t1, 1
2:              addi    t2, t2, 1
                j       1b

                # put the pivot between the two regions and return its index.
3:              add     t3, a0, t6
                add     t5, a0, t1
                lb      t4, (t5)
                sb      t0, (t5)
                sb      t4, (t3)
                mv      a0, t1
                ret
