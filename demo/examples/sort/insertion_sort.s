                .global insertion_sort
                .equ    insertion_sort_args, 2
                .text
                # insertion_sort(bytes, count): grow a sorted prefix of signed bytes.
                # t0 = next index, t1 = key, t2 = hole index, t3 = array address.
insertion_sort:
                li      t0, 1
1:
                bge     t0, a1, 5f
                add     t3, a0, t0
                lb      t1, (t3)
                mv      t2, t0

                # Move larger predecessors one position right until the key fits.
2:
                blez    t2, 4f
                add     t3, a0, t2
                lb      t4, -1(t3)
                ble     t4, t1, 4f
                sb      t4, (t3)
                addi    t2, t2, -1
3:
                j       2b

                # Fill the hole; the prefix through t0 is now sorted.
4:
                add     t3, a0, t2
                sb      t1, (t3)
                addi    t0, t0, 1
                j       1b
5:
                ret
