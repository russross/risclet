# Test: Logical Operations (AND, OR, XOR)
# Validates bitwise logical operations
#
# Test sequence:
# 1. ADDI x1, x0, 0xAA   ; x1 = 0xAA (1010_1010)
# 2. ADDI x2, x0, 0x55   ; x2 = 0x55 (0101_0101)
# 3. AND x3, x1, x2      ; x3 = 0x00 (no common bits)
# 4. OR x4, x1, x2       ; x4 = 0xFF (all bits set)
# 5. XOR x5, x1, x2      ; x5 = 0xFF (all different bits)
# 6. ANDI x6, x1, 0x0F   ; x6 = 0x0A (mask lower nibble)
# 7. ORI x7, x1, 0x05    ; x7 = 0xAF (OR with immediate)
# 8. XORI x8, x1, 0xFF   ; x8 = 0x55 (flip all bits in range)
# 9. ECALL               ; exit
#
# Expected final values:
#   x1 = 0xAA
#   x2 = 0x55
#   x3 = 0x00
#   x4 = 0xFF
#   x5 = 0xFF
#   x6 = 0x0A
#   x7 = 0xAF
#   x8 = 0x55

.section .text
.globl _start

_start:
    addi x1, x0, 0xAA      # x1 = 0xAA
    addi x2, x0, 0x55      # x2 = 0x55
    and x3, x1, x2         # x3 = 0xAA & 0x55 = 0x00
    or x4, x1, x2          # x4 = 0xAA | 0x55 = 0xFF
    xor x5, x1, x2         # x5 = 0xAA ^ 0x55 = 0xFF
    andi x6, x1, 0x0F      # x6 = 0xAA & 0x0F = 0x0A
    ori x7, x1, 0x05       # x7 = 0xAA | 0x05 = 0xAF
    xori x8, x1, 0xFF      # x8 = 0xAA ^ 0xFF = 0x55
    ecall                  # exit

.section .data
