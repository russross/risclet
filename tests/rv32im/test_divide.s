# Test: Division Instructions (M Extension)
# Validates RV32M divide and remainder operations
#
# Test sequence:
# 1. ADDI x1, x0, 42      ; x1 = 42
# 2. ADDI x2, x0, 6       ; x2 = 6
# 3. DIV x3, x1, x2       ; x3 = 42 / 6 = 7
# 4. REM x4, x1, x2       ; x4 = 42 % 6 = 0
# 5. ADDI x5, x0, 43      ; x5 = 43
# 6. DIV x6, x5, x2       ; x6 = 43 / 6 = 7
# 7. REM x7, x5, x2       ; x7 = 43 % 6 = 1
# 8. ADDI x8, x0, -42     ; x8 = -42
# 9. DIV x9, x8, x2       ; x9 = -42 / 6 = -7
# 10. REM x10, x8, x2     ; x10 = -42 % 6 = 0
# 11. ADDI x11, x0, -43   ; x11 = -43
# 12. DIV x12, x11, x2    ; x12 = -43 / 6 = -7
# 13. REM x13, x11, x2    ; x13 = -43 % 6 = -1
# 14. ECALL               ; exit
#
# Expected final values:
#   x1 = 42
#   x2 = 6
#   x3 = 7
#   x4 = 0
#   x5 = 43
#   x6 = 7
#   x7 = 1
#   x8 = -42
#   x9 = -7
#   x10 = 0
#   x11 = -43
#   x12 = -7
#   x13 = -1

.section .text
.globl _start

_start:
    addi x1, x0, 42        # x1 = 42
    addi x2, x0, 6         # x2 = 6
    div x3, x1, x2         # x3 = 42 / 6 = 7
    rem x4, x1, x2         # x4 = 42 % 6 = 0
    addi x5, x0, 43        # x5 = 43
    div x6, x5, x2         # x6 = 43 / 6 = 7
    rem x7, x5, x2         # x7 = 43 % 6 = 1
    addi x8, x0, -42       # x8 = -42
    div x9, x8, x2         # x9 = -42 / 6 = -7
    rem x10, x8, x2        # x10 = -42 % 6 = 0
    addi x11, x0, -43      # x11 = -43
    div x12, x11, x2       # x12 = -43 / 6 = -7
    rem x13, x11, x2       # x13 = -43 % 6 = -1
    ecall                  # exit

.section .data
