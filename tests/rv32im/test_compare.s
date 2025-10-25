# Test: Comparison Operations (SLT, SLTU, SLTI)
# Validates less-than comparisons
#
# Test sequence:
# 1. ADDI x1, x0, 10      ; x1 = 10
# 2. ADDI x2, x0, 20      ; x2 = 20
# 3. SLT x3, x1, x2       ; x3 = 1 (10 < 20 is true)
# 4. SLT x4, x2, x1       ; x4 = 0 (20 < 10 is false)
# 5. SLTI x5, x1, 15      ; x5 = 1 (10 < 15 is true)
# 6. SLTI x6, x1, 5       ; x6 = 0 (10 < 5 is false)
# 7. ADDI x7, x0, -5      ; x7 = -5 (0xFFFFFFFB)
# 8. SLTU x8, x7, x1      ; x8 = 0 (-5 as unsigned is very large, not < 10)
# 9. SLTIU x9, x7, -1     ; x9 = 1 (-5 < -1 as unsigned)
# 10. ECALL               ; exit
#
# Expected final values:
#   x1 = 10
#   x2 = 20
#   x3 = 1
#   x4 = 0
#   x5 = 1
#   x6 = 0
#   x7 = -5 (0xFFFFFFFB)
#   x8 = 0
#   x9 = 1

.section .text
.globl _start

_start:
    addi x1, x0, 10        # x1 = 10
    addi x2, x0, 20        # x2 = 20
    slt x3, x1, x2         # x3 = 1 (10 < 20)
    slt x4, x2, x1         # x4 = 0 (20 < 10 is false)
    slti x5, x1, 15        # x5 = 1 (10 < 15)
    slti x6, x1, 5         # x6 = 0 (10 < 5 is false)
    addi x7, x0, -5        # x7 = -5
    sltu x8, x7, x1        # x8 = 0 (as unsigned, -5 > 10)
    sltiu x9, x7, -1       # x9 = 1 (as unsigned, -5 < -1)
    ecall                  # exit

.section .data
