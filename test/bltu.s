# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# bltu.S
#-----------------------------------------------------------------------------
# Test bltu instruction.
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
  # Branch tests
  #-------------------------------------------------------------
  # Each test checks both forward and backward branches
  test_2: li x28, 2; li x1, 0x00000000; li x2, 0x00000001; bltu x1, x2, 2f; bne x0, x28, fail; 1: bne x0, x28, 3f; 2: bltu x1, x2, 1b; bne x0, x28, fail; 3:;
  test_3: li x28, 3; li x1, 0xfffffffe; li x2, 0xffffffff; bltu x1, x2, 2f; bne x0, x28, fail; 1: bne x0, x28, 3f; 2: bltu x1, x2, 1b; bne x0, x28, fail; 3:;
  test_4: li x28, 4; li x1, 0x00000000; li x2, 0xffffffff; bltu x1, x2, 2f; bne x0, x28, fail; 1: bne x0, x28, 3f; 2: bltu x1, x2, 1b; bne x0, x28, fail; 3:;
  test_5: li x28, 5; li x1, 0x00000001; li x2, 0x00000000; bltu x1, x2, 1f; bne x0, x28, 2f; 1: bne x0, x28, fail; 2: bltu x1, x2, 1b; 3:;
  test_6: li x28, 6; li x1, 0xffffffff; li x2, 0xfffffffe; bltu x1, x2, 1f; bne x0, x28, 2f; 1: bne x0, x28, fail; 2: bltu x1, x2, 1b; 3:;
  test_7: li x28, 7; li x1, 0xffffffff; li x2, 0x00000000; bltu x1, x2, 1f; bne x0, x28, 2f; 1: bne x0, x28, fail; 2: bltu x1, x2, 1b; 3:;
  test_8: li x28, 8; li x1, 0x80000000; li x2, 0x7fffffff; bltu x1, x2, 1f; bne x0, x28, 2f; 1: bne x0, x28, fail; 2: bltu x1, x2, 1b; 3:;
  #-------------------------------------------------------------
  # Bypassing tests
  #-------------------------------------------------------------
  test_9: li x28, 9; li x4, 0; 1: li x1, 0xf0000000; li x2, 0xefffffff; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_10: li x28, 10; li x4, 0; 1: li x1, 0xf0000000; li x2, 0xefffffff; nop; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_11: li x28, 11; li x4, 0; 1: li x1, 0xf0000000; li x2, 0xefffffff; nop; nop; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_12: li x28, 12; li x4, 0; 1: li x1, 0xf0000000; nop; li x2, 0xefffffff; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_13: li x28, 13; li x4, 0; 1: li x1, 0xf0000000; nop; li x2, 0xefffffff; nop; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_14: li x28, 14; li x4, 0; 1: li x1, 0xf0000000; nop; nop; li x2, 0xefffffff; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_15: li x28, 15; li x4, 0; 1: li x1, 0xf0000000; li x2, 0xefffffff; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_16: li x28, 16; li x4, 0; 1: li x1, 0xf0000000; li x2, 0xefffffff; nop; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_17: li x28, 17; li x4, 0; 1: li x1, 0xf0000000; li x2, 0xefffffff; nop; nop; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_18: li x28, 18; li x4, 0; 1: li x1, 0xf0000000; nop; li x2, 0xefffffff; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_19: li x28, 19; li x4, 0; 1: li x1, 0xf0000000; nop; li x2, 0xefffffff; nop; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_20: li x28, 20; li x4, 0; 1: li x1, 0xf0000000; nop; nop; li x2, 0xefffffff; bltu x1, x2, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  #-------------------------------------------------------------
  # Test delay slot instructions not executed nor bypassed
  #-------------------------------------------------------------
  test_21: li x28, 21; li x1, 1; bltu x0, x1, 1f; addi x1, x1, 1; addi x1, x1, 1; addi x1, x1, 1; addi x1, x1, 1; 1: addi x1, x1, 1; addi x1, x1, 1;; li x7, ((3) & 0xffffffff); bne x1, x7, fail;
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 

