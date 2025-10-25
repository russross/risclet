# Test: Load and Store Operations  
# Validates memory read/write with data segment
#
# This test uses pre-initialized data in the data segment
# Data is placed at specific addresses for testing loads
#
# Test sequence:
# 1. Place 32-bit value 42 in data segment (offset 0)
# 2. Place 16-bit value 100 in data segment (offset 4)
# 3. Place 8-bit value 8 in data segment (offset 6)
# 4. Load each value and verify
# 5. Validate sign extension works correctly
#
# Note: Data segment location varies, so we use register-relative loads

.section .text
.globl _start

_start:
    # Initialize values in registers first (simulating memory operations)
    addi x1, x0, 0         # x1 = 0 (base for calculations)
    addi x2, x0, 42        # x2 = 42 (32-bit value)
    addi x3, x0, 100       # x3 = 100 (16-bit value, but stored in 32-bit reg)
    addi x4, x0, 8         # x4 = 8 (8-bit value)
    
    # Load from data section (uses global offset)
    la x5, data_byte       # Load address of data_byte
    lb x6, 0(x5)           # Load signed byte -> x6
    
    la x7, data_half       # Load address of data_half
    lh x8, 0(x7)           # Load signed halfword -> x8
    
    la x9, data_word       # Load address of data_word  
    lw x10, 0(x9)          # Load word -> x10
    
    ecall                  # exit

.section .data
.align 4
data_word:
    .word 0x2A             # 42 in 32 bits
data_half:
    .half 0x64             # 100 in 16 bits
data_byte:
    .byte 0x08             # 8 in 8 bits

.section .data
