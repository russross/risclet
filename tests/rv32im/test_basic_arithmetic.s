# Test: Basic Arithmetic Operations (ADD, SUB, ADDI)
# This validates fundamental arithmetic instruction execution in RV32
#
# Test sequence:
# 1. ADDI x1, x0, 100    ; x1 = 100
# 2. ADDI x2, x0, 42     ; x2 = 42
# 3. ADD x3, x1, x2      ; x3 = 142
# 4. SUB x4, x1, x2      ; x4 = 58
# 5. ECALL               ; exit
#
# Expected final values:
#   x1 = 100 (0x64)
#   x2 = 42  (0x2a)
#   x3 = 142 (0x8e)
#   x4 = 58  (0x3a)

.section .text
.globl _start

_start:
    addi x1, x0, 100       # x1 = 100
    addi x2, x0, 42        # x2 = 42
    add x3, x1, x2         # x3 = 100 + 42 = 142
    sub x4, x1, x2         # x4 = 100 - 42 = 58
    ecall                  # exit

.section .data
