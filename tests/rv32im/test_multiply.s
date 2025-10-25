# Test: Multiplication Instructions (M Extension)
# Validates RV32M multiply operations
#
# Test sequence:
# 1. ADDI x1, x0, 6       ; x1 = 6
# 2. ADDI x2, x0, 7       ; x2 = 7
# 3. MUL x3, x1, x2       ; x3 = 6 * 7 = 42
# 4. ADDI x4, x0, 0x1000  ; x4 = 4096
# 5. ADDI x5, x0, 0x2000  ; x5 = 8192
# 6. MUL x6, x4, x5       ; x6 = 4096 * 8192 = 0x8000000 (33554432)
#                         ; Only lower 32 bits kept: 0x00000000
# 7. MULH x7, x4, x5      ; x7 = upper 32 bits of (4096 * 8192) = 0x20
# 8. ADDI x8, x0, -3      ; x8 = -3
# 9. MULH x9, x8, x2      ; x9 = upper 32 bits of (-3 * 7)
# 10. ECALL               ; exit
#
# Expected final values:
#   x1 = 6
#   x2 = 7
#   x3 = 42
#   x4 = 4096
#   x5 = 8192
#   x6 = 0 (only lower 32 bits of product)
#   x7 = 2 (0x20 >> shift, upper bits)
#   x8 = -3
#   x9 = -1 (sign extension of upper bits of negative product)

.section .text
.globl _start

_start:
    addi x1, x0, 6         # x1 = 6
    addi x2, x0, 7         # x2 = 7
    mul x3, x1, x2         # x3 = 6 * 7 = 42
    lui x4, 0x1            # x4 = 0x1000 (load upper immediate)
    lui x5, 0x2            # x5 = 0x2000
    mul x6, x4, x5         # x6 = (4096 * 8192) & 0xFFFFFFFF
    mulh x7, x4, x5        # x7 = (4096 * 8192) >> 32
    addi x8, x0, -3        # x8 = -3
    mulh x9, x8, x2        # x9 = (-3 * 7) >> 32
    ecall                  # exit

.section .data
