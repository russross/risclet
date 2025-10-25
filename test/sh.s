# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# sh.S
#-----------------------------------------------------------------------------
# Test sh instruction.
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
  # Basic tests
  #-------------------------------------------------------------
  test_2: li x28, 2; la x1, tdat; li x2, 0x00000000000000aa; la x15, 7f; sh x2, 0(x1); lh x14, 0(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x00000000000000aa) & 0xffffffff); bne x14, x7, fail;;
  test_3: li x28, 3; la x1, tdat; li x2, 0xffffffffffffaa00; la x15, 7f; sh x2, 2(x1); lh x14, 2(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffaa00) & 0xffffffff); bne x14, x7, fail;;
  test_4: li x28, 4; la x1, tdat; li x2, 0xffffffffbeef0aa0; la x15, 7f; sh x2, 4(x1); lw x14, 4(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffbeef0aa0) & 0xffffffff); bne x14, x7, fail;;
  test_5: li x28, 5; la x1, tdat; li x2, 0xffffffffffffa00a; la x15, 7f; sh x2, 6(x1); lh x14, 6(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffa00a) & 0xffffffff); bne x14, x7, fail;;
  # Test with negative offset
  test_6: li x28, 6; la x1, tdat8; li x2, 0x00000000000000aa; la x15, 7f; sh x2, -6(x1); lh x14, -6(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x00000000000000aa) & 0xffffffff); bne x14, x7, fail;;
  test_7: li x28, 7; la x1, tdat8; li x2, 0xffffffffffffaa00; la x15, 7f; sh x2, -4(x1); lh x14, -4(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffaa00) & 0xffffffff); bne x14, x7, fail;;
  test_8: li x28, 8; la x1, tdat8; li x2, 0x0000000000000aa0; la x15, 7f; sh x2, -2(x1); lh x14, -2(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x0000000000000aa0) & 0xffffffff); bne x14, x7, fail;;
  test_9: li x28, 9; la x1, tdat8; li x2, 0xffffffffffffa00a; la x15, 7f; sh x2, 0(x1); lh x14, 0(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffa00a) & 0xffffffff); bne x14, x7, fail;;
  # Test with a negative base
  test_10: li x28, 10; la x1, tdat9; li x2, 0x12345678; addi x4, x1, -32; sh x2, 32(x4); lh x5, 0(x1);; li x7, ((0x5678) & 0xffffffff); bne x5, x7, fail;
  # Test with unaligned base
  test_11: li x28, 11; la x1, tdat9; li x2, 0x00003098; addi x1, x1, -5; sh x2, 7(x1); la x4, tdat10; lh x5, 0(x4);; li x7, ((0x3098) & 0xffffffff); bne x5, x7, fail;
  #-------------------------------------------------------------
  # Bypassing tests
  #-------------------------------------------------------------
  test_12: li x28, 12; li x4, 0; 1: li x1, 0xffffffffffffccdd; la x2, tdat; sh x1, 0(x2); lh x14, 0(x2); li x7, 0xffffffffffffccdd; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_13: li x28, 13; li x4, 0; 1: li x1, 0xffffffffffffbccd; la x2, tdat; nop; sh x1, 2(x2); lh x14, 2(x2); li x7, 0xffffffffffffbccd; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_14: li x28, 14; li x4, 0; 1: li x1, 0xffffffffffffbbcc; la x2, tdat; nop; nop; sh x1, 4(x2); lh x14, 4(x2); li x7, 0xffffffffffffbbcc; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_15: li x28, 15; li x4, 0; 1: li x1, 0xffffffffffffabbc; nop; la x2, tdat; sh x1, 6(x2); lh x14, 6(x2); li x7, 0xffffffffffffabbc; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_16: li x28, 16; li x4, 0; 1: li x1, 0xffffffffffffaabb; nop; la x2, tdat; nop; sh x1, 8(x2); lh x14, 8(x2); li x7, 0xffffffffffffaabb; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_17: li x28, 17; li x4, 0; 1: li x1, 0xffffffffffffdaab; nop; nop; la x2, tdat; sh x1, 10(x2); lh x14, 10(x2); li x7, 0xffffffffffffdaab; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_18: li x28, 18; li x4, 0; 1: la x2, tdat; li x1, 0x2233; sh x1, 0(x2); lh x14, 0(x2); li x7, 0x2233; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_19: li x28, 19; li x4, 0; 1: la x2, tdat; li x1, 0x1223; nop; sh x1, 2(x2); lh x14, 2(x2); li x7, 0x1223; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_20: li x28, 20; li x4, 0; 1: la x2, tdat; li x1, 0x1122; nop; nop; sh x1, 4(x2); lh x14, 4(x2); li x7, 0x1122; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_21: li x28, 21; li x4, 0; 1: la x2, tdat; nop; li x1, 0x0112; sh x1, 6(x2); lh x14, 6(x2); li x7, 0x0112; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_22: li x28, 22; li x4, 0; 1: la x2, tdat; nop; li x1, 0x0011; nop; sh x1, 8(x2); lh x14, 8(x2); li x7, 0x0011; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_23: li x28, 23; li x4, 0; 1: la x2, tdat; nop; nop; li x1, 0x3001; sh x1, 10(x2); lh x14, 10(x2); li x7, 0x3001; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  li a0, 0xbeef
  la a1, tdat
  sh a0, 6(a1)
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 
tdat:
tdat1: .half 0xbeef
tdat2: .half 0xbeef
tdat3: .half 0xbeef
tdat4: .half 0xbeef
tdat5: .half 0xbeef
tdat6: .half 0xbeef
tdat7: .half 0xbeef
tdat8: .half 0xbeef
tdat9: .half 0xbeef
tdat10: .half 0xbeef

