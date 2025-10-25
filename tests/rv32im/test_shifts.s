# Test: Shift Operations (SLL, SRL, SRA)
# Validates logical and arithmetic shift instructions
#
# Test sequence:
# 1. ADDI x1, x0, 16     ; x1 = 16 (0x10)
# 2. SLLI x2, x1, 2      ; x2 = 64 (0x40) - left shift by 2
# 3. SRLI x3, x2, 1      ; x3 = 32 (0x20) - logical right shift by 1
# 4. ADDI x4, x0, -8     ; x4 = -8 (0xFFFFFFF8 in 32-bit)
# 5. SRAI x5, x4, 2      ; x5 = -2 (0xFFFFFFFE) - arithmetic right shift by 2
# 6. ECALL               ; exit
#
# Expected final values:
#   x1 = 16  (0x0000_0010)
#   x2 = 64  (0x0000_0040)
#   x3 = 32  (0x0000_0020)
#   x4 = -8  (0xFFFF_FFF8)
#   x5 = -2  (0xFFFF_FFFE)

.section .text
.globl _start

_start:
    addi x1, x0, 16        # x1 = 16
    slli x2, x1, 2         # x2 = 16 << 2 = 64
    srli x3, x2, 1         # x3 = 64 >> 1 = 32 (logical)
    addi x4, x0, -8        # x4 = -8
    srai x5, x4, 2         # x5 = -8 >> 2 = -2 (arithmetic)
    ecall                  # exit

.section .data
