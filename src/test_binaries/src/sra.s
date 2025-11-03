# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# sra.S
#-----------------------------------------------------------------------------
# Test sra instruction.
#-----------------------------------------------------------------------
# Helper macros
#-----------------------------------------------------------------------
# We use a macro hack to simpify code generation for various numbers
# of bubble cycles.
#-----------------------------------------------------------------------
# RV64UI MACROS
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Tests for instructions with immediate operand
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Tests for an instruction with register operands
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Tests for an instruction with register-register operands
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Test memory instructions
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Test jump instructions
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# RV64UF MACROS
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Tests floating-point instructions
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Pass and fail code (assumes test num is in x28)
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Test data section
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Additional macros for load/store pointer operations
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Test load-store bypass macros
#-----------------------------------------------------------------------
#-----------------------------------------------------------------------
# Misaligned access test macros
#-----------------------------------------------------------------------
.globl _start
.text; _start:
  #-------------------------------------------------------------
  # Arithmetic tests
  #-------------------------------------------------------------
  test_2: li x28, 2; li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((0) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffff80000000) & 0xffffffff); bne x14, x7, fail;;
  test_3: li x28, 3; li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((1) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffc0000000) & 0xffffffff); bne x14, x7, fail;;
  test_4: li x28, 4; li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((7) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffff000000) & 0xffffffff); bne x14, x7, fail;;
  test_5: li x28, 5; li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((14) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x14, x7, fail;;
  test_6: li x28, 6; li x1, ((0xffffffff80000001) & 0xffffffff); li x2, ((31) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  test_7: li x28, 7; li x1, ((0x000000007fffffff) & 0xffffffff); li x2, ((0) & 0xffffffff); sra x14, x1, x2;; li x7, ((0x000000007fffffff) & 0xffffffff); bne x14, x7, fail;;
  test_8: li x28, 8; li x1, ((0x000000007fffffff) & 0xffffffff); li x2, ((1) & 0xffffffff); sra x14, x1, x2;; li x7, ((0x000000003fffffff) & 0xffffffff); bne x14, x7, fail;;
  test_9: li x28, 9; li x1, ((0x000000007fffffff) & 0xffffffff); li x2, ((7) & 0xffffffff); sra x14, x1, x2;; li x7, ((0x0000000000ffffff) & 0xffffffff); bne x14, x7, fail;;
  test_10: li x28, 10; li x1, ((0x000000007fffffff) & 0xffffffff); li x2, ((14) & 0xffffffff); sra x14, x1, x2;; li x7, ((0x000000000001ffff) & 0xffffffff); bne x14, x7, fail;;
  test_11: li x28, 11; li x1, ((0x000000007fffffff) & 0xffffffff); li x2, ((31) & 0xffffffff); sra x14, x1, x2;; li x7, ((0x0000000000000000) & 0xffffffff); bne x14, x7, fail;;
  test_12: li x28, 12; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((0) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffff81818181) & 0xffffffff); bne x14, x7, fail;;
  test_13: li x28, 13; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((1) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffc0c0c0c0) & 0xffffffff); bne x14, x7, fail;;
  test_14: li x28, 14; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((7) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffff030303) & 0xffffffff); bne x14, x7, fail;;
  test_15: li x28, 15; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((14) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xfffffffffffe0606) & 0xffffffff); bne x14, x7, fail;;
  test_16: li x28, 16; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((31) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  # Verify that shifts only use bottom six(rv64) or five(rv32) bits
  test_17: li x28, 17; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((0xffffffffffffffc0) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffff81818181) & 0xffffffff); bne x14, x7, fail;;
  test_18: li x28, 18; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((0xffffffffffffffc1) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffc0c0c0c0) & 0xffffffff); bne x14, x7, fail;;
  test_19: li x28, 19; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((0xffffffffffffffc7) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffff030303) & 0xffffffff); bne x14, x7, fail;;
  test_20: li x28, 20; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((0xffffffffffffffce) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xfffffffffffe0606) & 0xffffffff); bne x14, x7, fail;;
  test_21: li x28, 21; li x1, ((0xffffffff81818181) & 0xffffffff); li x2, ((0xffffffffffffffff) & 0xffffffff); sra x14, x1, x2;; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  #-------------------------------------------------------------
  # Source/Destination tests
  #-------------------------------------------------------------
  test_22: li x28, 22; li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((7) & 0xffffffff); sra x1, x1, x2;; li x7, ((0xffffffffff000000) & 0xffffffff); bne x1, x7, fail;;
  test_23: li x28, 23; li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((14) & 0xffffffff); sra x2, x1, x2;; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x2, x7, fail;;
  test_24: li x28, 24; li x1, ((7) & 0xffffffff); sra x1, x1, x1;; li x7, ((0) & 0xffffffff); bne x1, x7, fail;;
  #-------------------------------------------------------------
  # Bypassing tests
  #-------------------------------------------------------------
  test_25: li x28, 25; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((7) & 0xffffffff); sra x14, x1, x2; addi x6, x14, 0; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffff000000) & 0xffffffff); bne x6, x7, fail;;
  test_26: li x28, 26; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((14) & 0xffffffff); sra x14, x1, x2; nop; addi x6, x14, 0; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x6, x7, fail;;
  test_27: li x28, 27; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((31) & 0xffffffff); sra x14, x1, x2; nop; nop; addi x6, x14, 0; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x6, x7, fail;;
  test_28: li x28, 28; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((7) & 0xffffffff); sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffff000000) & 0xffffffff); bne x14, x7, fail;;
  test_29: li x28, 29; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((14) & 0xffffffff); nop; sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x14, x7, fail;;
  test_30: li x28, 30; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); li x2, ((31) & 0xffffffff); nop; nop; sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  test_31: li x28, 31; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); nop; li x2, ((7) & 0xffffffff); sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffff000000) & 0xffffffff); bne x14, x7, fail;;
  test_32: li x28, 32; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); nop; li x2, ((14) & 0xffffffff); nop; sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x14, x7, fail;;
  test_33: li x28, 33; li x4, 0; 1: li x1, ((0xffffffff80000000) & 0xffffffff); nop; nop; li x2, ((31) & 0xffffffff); sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  test_34: li x28, 34; li x4, 0; 1: li x2, ((7) & 0xffffffff); li x1, ((0xffffffff80000000) & 0xffffffff); sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffff000000) & 0xffffffff); bne x14, x7, fail;;
  test_35: li x28, 35; li x4, 0; 1: li x2, ((14) & 0xffffffff); li x1, ((0xffffffff80000000) & 0xffffffff); nop; sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x14, x7, fail;;
  test_36: li x28, 36; li x4, 0; 1: li x2, ((31) & 0xffffffff); li x1, ((0xffffffff80000000) & 0xffffffff); nop; nop; sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  test_37: li x28, 37; li x4, 0; 1: li x2, ((7) & 0xffffffff); nop; li x1, ((0xffffffff80000000) & 0xffffffff); sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffff000000) & 0xffffffff); bne x14, x7, fail;;
  test_38: li x28, 38; li x4, 0; 1: li x2, ((14) & 0xffffffff); nop; li x1, ((0xffffffff80000000) & 0xffffffff); nop; sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xfffffffffffe0000) & 0xffffffff); bne x14, x7, fail;;
  test_39: li x28, 39; li x4, 0; 1: li x2, ((31) & 0xffffffff); nop; nop; li x1, ((0xffffffff80000000) & 0xffffffff); sra x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((0xffffffffffffffff) & 0xffffffff); bne x14, x7, fail;;
  test_40: li x28, 40; li x1, ((15) & 0xffffffff); sra x2, x0, x1;; li x7, ((0) & 0xffffffff); bne x2, x7, fail;;
  test_41: li x28, 41; li x1, ((32) & 0xffffffff); sra x2, x1, x0;; li x7, ((32) & 0xffffffff); bne x2, x7, fail;;
  test_42: li x28, 42; sra x1, x0, x0;; li x7, ((0) & 0xffffffff); bne x1, x7, fail;;
  test_43: li x28, 43; li x1, ((1024) & 0xffffffff); li x2, ((2048) & 0xffffffff); sra x0, x1, x2;; li x7, ((0) & 0xffffffff); bne x0, x7, fail;;
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 

