# Test: Branch Instructions
# Validates conditional branching (BEQ, BNE, BLT, BGE, BLTU, BGEU)
#
# Test sequence:
# Uses a simple branching approach to set values based on conditions
# 1. ADDI x1, x0, 1       ; x1 = 1 (base value)
# 2. ADDI x2, x0, 10      ; x2 = 10
# 3. ADDI x3, x0, 10      ; x3 = 10
# 4. BEQ x2, x3, skip1    ; If 10 == 10 (true), skip increment
#    ADDI x1, x1, 1       ; (skipped)
# skip1:
# 5. ADDI x4, x0, 20      ; x4 = 20
# 6. BNE x2, x4, skip2    ; If 10 != 20 (true), skip increment
#    ADDI x1, x1, 1       ; (skipped)
# skip2:
# 7. ADDI x5, x0, 5       ; x5 = 5
# 8. BLT x5, x2, skip3    ; If 5 < 10 (true), skip increment
#    ADDI x1, x1, 1       ; (skipped)
# skip3:
# 9. BGE x2, x5, skip4    ; If 10 >= 5 (true), skip increment
#    ADDI x1, x1, 1       ; (skipped)
# skip4:
# 10. ADDI x6, x0, -5     ; x6 = -5
# 11. BLTU x6, x2, skip5  ; If -5 < 10 as unsigned (false, -5 > 10 unsigned)
#     ADDI x1, x1, 1      ; (executed)
# skip5:
# 12. BGEU x2, x6, skip6  ; If 10 >= -5 as unsigned (true)
#     ADDI x1, x1, 1      ; (skipped)
# skip6:
# 13. ECALL               ; exit
#
# Expected final values:
#   x1 = 2 (starts at 1, incremented once by the BLTU branch)
#   x2 = 10
#   x3 = 10
#   x4 = 20
#   x5 = 5
#   x6 = -5

.section .text
.globl _start

_start:
    addi x1, x0, 1         # x1 = 1
    addi x2, x0, 10        # x2 = 10
    addi x3, x0, 10        # x3 = 10
    beq x2, x3, skip1      # If equal, skip increment
    addi x1, x1, 1         # (skipped if equal)
skip1:
    addi x4, x0, 20        # x4 = 20
    bne x2, x4, skip2      # If not equal, skip increment
    addi x1, x1, 1         # (skipped if not equal)
skip2:
    addi x5, x0, 5         # x5 = 5
    blt x5, x2, skip3      # If 5 < 10, skip increment
    addi x1, x1, 1         # (skipped if true)
skip3:
    bge x2, x5, skip4      # If 10 >= 5, skip increment
    addi x1, x1, 1         # (skipped if true)
skip4:
    addi x6, x0, -5        # x6 = -5 (0xFFFFFFFB)
    bltu x6, x2, skip5     # If -5 < 10 unsigned (false)
    addi x1, x1, 1         # (executed because condition is false)
skip5:
    bgeu x2, x6, skip6     # If 10 >= -5 unsigned (true)
    addi x1, x1, 1         # (skipped if true)
skip6:
    ecall                  # exit

.section .data
