# Test: Jump Instructions (JAL, JALR)
# Validates PC-relative (JAL) and register-based (JALR) jumps
#
# Test sequence:
# 1. ADDI x1, x0, 0       ; x1 = 0 (accumulator)
# 2. JAL x2, target1      ; Jump to target1, save return PC in x2
# 3. ADDI x1, x1, 100     ; (skipped by jump)
# target1:
# 4. ADDI x1, x1, 42      ; x1 = 42
# 5. ADDI x3, x0, 200     ; x3 = 200
# 6. JALR x4, x2, 0       ; Jump back via x2 (return from subroutine)
# 7. ADDI x1, x1, 1       ; (executed after return) x1 = 43
# 8. ECALL                ; exit
#
# Expected final values:
#   x1 = 43 (42 from target1, then +1)
#   x2 = return address from first JAL
#   x3 = 200
#   x4 = return address from JALR
#
# Notes:
# This tests that:
# - JAL correctly computes and stores return address
# - JALR can use that address to jump back
# - Code after the return point executes normally

.section .text
.globl _start

_start:
    addi x1, x0, 0         # x1 = 0
    jal x2, subroutine     # Jump to subroutine, x2 = return address
    addi x1, x1, 100       # (skipped by jal)

    # After return from subroutine
    addi x1, x1, 1         # x1 = 43
    ecall                  # exit

subroutine:
    addi x1, x1, 42        # x1 = 42
    addi x3, x0, 200       # x3 = 200
    jalr x0, x2, 0         # Return to caller (x2 contains return address)

.section .data
