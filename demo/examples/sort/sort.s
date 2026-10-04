                .global sort
                .text

# sort(begin_address, end_address)
sort:
                # t0: i (current element address)
                # t1: key value to insert
                # t2: j (scan address to the left)
                # t3: value at j

                # start with the second element
                addi    t0, a0, 4
1:              bge     t0, a1, 4f
                lw      t1, (t0)
                addi    t2, t0, -4

                # shift larger elements to the right
2:              blt     t2, a0, 3f
                lw      t3, (t2)
                ble     t3, t1, 3f
                sw      t3, 4(t2)
                addi    t2, t2, -4
                j       2b

                # insert the key after the scan position
3:              sw      t1, 4(t2)
                addi    t0, t0, 4
                j       1b
4:
                ret
