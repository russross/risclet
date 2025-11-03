# See LICENSE for license details.
#*****************************************************************************
# mul.S
#-----------------------------------------------------------------------------
# Test mul instruction.
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
  test_32: li x28, 32; li x1, ((0x00007e00) & 0xffffffff); li x2, ((0xb6db6db7) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00001200) & 0xffffffff); bne x14, x7, fail;;
  test_33: li x28, 33; li x1, ((0x00007fc0) & 0xffffffff); li x2, ((0xb6db6db7) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00001240) & 0xffffffff); bne x14, x7, fail;;
  test_2: li x28, 2; li x1, ((0x00000000) & 0xffffffff); li x2, ((0x00000000) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000000) & 0xffffffff); bne x14, x7, fail;;
  test_3: li x28, 3; li x1, ((0x00000001) & 0xffffffff); li x2, ((0x00000001) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000001) & 0xffffffff); bne x14, x7, fail;;
  test_4: li x28, 4; li x1, ((0x00000003) & 0xffffffff); li x2, ((0x00000007) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000015) & 0xffffffff); bne x14, x7, fail;;
  test_5: li x28, 5; li x1, ((0x00000000) & 0xffffffff); li x2, ((0xffff8000) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000000) & 0xffffffff); bne x14, x7, fail;;
  test_6: li x28, 6; li x1, ((0x80000000) & 0xffffffff); li x2, ((0x00000000) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000000) & 0xffffffff); bne x14, x7, fail;;
  test_7: li x28, 7; li x1, ((0x80000000) & 0xffffffff); li x2, ((0xffff8000) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000000) & 0xffffffff); bne x14, x7, fail;;
  test_30: li x28, 30; li x1, ((0xaaaaaaab) & 0xffffffff); li x2, ((0x0002fe7d) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x0000ff7f) & 0xffffffff); bne x14, x7, fail;;
  test_31: li x28, 31; li x1, ((0x0002fe7d) & 0xffffffff); li x2, ((0xaaaaaaab) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x0000ff7f) & 0xffffffff); bne x14, x7, fail;;
  test_34: li x28, 34; li x1, ((0xff000000) & 0xffffffff); li x2, ((0xff000000) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000000) & 0xffffffff); bne x14, x7, fail;;
  test_35: li x28, 35; li x1, ((0xffffffff) & 0xffffffff); li x2, ((0xffffffff) & 0xffffffff); mul x14, x1, x2;; li x7, ((0x00000001) & 0xffffffff); bne x14, x7, fail;;
  test_36: li x28, 36; li x1, ((0xffffffff) & 0xffffffff); li x2, ((0x00000001) & 0xffffffff); mul x14, x1, x2;; li x7, ((0xffffffff) & 0xffffffff); bne x14, x7, fail;;
  test_37: li x28, 37; li x1, ((0x00000001) & 0xffffffff); li x2, ((0xffffffff) & 0xffffffff); mul x14, x1, x2;; li x7, ((0xffffffff) & 0xffffffff); bne x14, x7, fail;;
  #-------------------------------------------------------------
  # Source/Destination tests
  #-------------------------------------------------------------
  test_8: li x28, 8; li x1, ((13) & 0xffffffff); li x2, ((11) & 0xffffffff); mul x1, x1, x2;; li x7, ((143) & 0xffffffff); bne x1, x7, fail;;
  test_9: li x28, 9; li x1, ((14) & 0xffffffff); li x2, ((11) & 0xffffffff); mul x2, x1, x2;; li x7, ((154) & 0xffffffff); bne x2, x7, fail;;
  test_10: li x28, 10; li x1, ((13) & 0xffffffff); mul x1, x1, x1;; li x7, ((169) & 0xffffffff); bne x1, x7, fail;;
  #-------------------------------------------------------------
  # Bypassing tests
  #-------------------------------------------------------------
  test_11: li x28, 11; li x4, 0; 1: li x1, ((13) & 0xffffffff); li x2, ((11) & 0xffffffff); mul x14, x1, x2; addi x6, x14, 0; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((143) & 0xffffffff); bne x6, x7, fail;;
  test_12: li x28, 12; li x4, 0; 1: li x1, ((14) & 0xffffffff); li x2, ((11) & 0xffffffff); mul x14, x1, x2; nop; addi x6, x14, 0; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((154) & 0xffffffff); bne x6, x7, fail;;
  test_13: li x28, 13; li x4, 0; 1: li x1, ((15) & 0xffffffff); li x2, ((11) & 0xffffffff); mul x14, x1, x2; nop; nop; addi x6, x14, 0; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((165) & 0xffffffff); bne x6, x7, fail;;
  test_14: li x28, 14; li x4, 0; 1: li x1, ((13) & 0xffffffff); li x2, ((11) & 0xffffffff); mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((143) & 0xffffffff); bne x14, x7, fail;;
  test_15: li x28, 15; li x4, 0; 1: li x1, ((14) & 0xffffffff); li x2, ((11) & 0xffffffff); nop; mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((154) & 0xffffffff); bne x14, x7, fail;;
  test_16: li x28, 16; li x4, 0; 1: li x1, ((15) & 0xffffffff); li x2, ((11) & 0xffffffff); nop; nop; mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((165) & 0xffffffff); bne x14, x7, fail;;
  test_17: li x28, 17; li x4, 0; 1: li x1, ((13) & 0xffffffff); nop; li x2, ((11) & 0xffffffff); mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((143) & 0xffffffff); bne x14, x7, fail;;
  test_18: li x28, 18; li x4, 0; 1: li x1, ((14) & 0xffffffff); nop; li x2, ((11) & 0xffffffff); nop; mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((154) & 0xffffffff); bne x14, x7, fail;;
  test_19: li x28, 19; li x4, 0; 1: li x1, ((15) & 0xffffffff); nop; nop; li x2, ((11) & 0xffffffff); mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((165) & 0xffffffff); bne x14, x7, fail;;
  test_20: li x28, 20; li x4, 0; 1: li x2, ((11) & 0xffffffff); li x1, ((13) & 0xffffffff); mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((143) & 0xffffffff); bne x14, x7, fail;;
  test_21: li x28, 21; li x4, 0; 1: li x2, ((11) & 0xffffffff); li x1, ((14) & 0xffffffff); nop; mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((154) & 0xffffffff); bne x14, x7, fail;;
  test_22: li x28, 22; li x4, 0; 1: li x2, ((11) & 0xffffffff); li x1, ((15) & 0xffffffff); nop; nop; mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((165) & 0xffffffff); bne x14, x7, fail;;
  test_23: li x28, 23; li x4, 0; 1: li x2, ((11) & 0xffffffff); nop; li x1, ((13) & 0xffffffff); mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((143) & 0xffffffff); bne x14, x7, fail;;
  test_24: li x28, 24; li x4, 0; 1: li x2, ((11) & 0xffffffff); nop; li x1, ((14) & 0xffffffff); nop; mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((154) & 0xffffffff); bne x14, x7, fail;;
  test_25: li x28, 25; li x4, 0; 1: li x2, ((11) & 0xffffffff); nop; nop; li x1, ((15) & 0xffffffff); mul x14, x1, x2; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b; li x7, ((165) & 0xffffffff); bne x14, x7, fail;;
  test_26: li x28, 26; li x1, ((31) & 0xffffffff); mul x2, x0, x1;; li x7, ((0) & 0xffffffff); bne x2, x7, fail;;
  test_27: li x28, 27; li x1, ((32) & 0xffffffff); mul x2, x1, x0;; li x7, ((0) & 0xffffffff); bne x2, x7, fail;;
  test_28: li x28, 28; mul x1, x0, x0;; li x7, ((0) & 0xffffffff); bne x1, x7, fail;;
  test_29: li x28, 29; li x1, ((33) & 0xffffffff); li x2, ((34) & 0xffffffff); mul x0, x1, x2;; li x7, ((0) & 0xffffffff); bne x0, x7, fail;;
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 

