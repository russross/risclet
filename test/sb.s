# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# sb.S
#-----------------------------------------------------------------------------
# Test sb instruction.
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
  test_2: li x28, 2; la x1, tdat; li x2, 0xffffffffffffffaa; la x15, 7f; sb x2, 0(x1); lb x14, 0(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffffaa) & 0xffffffff); bne x14, x7, fail;;
  test_3: li x28, 3; la x1, tdat; li x2, 0x0000000000000000; la x15, 7f; sb x2, 1(x1); lb x14, 1(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x0000000000000000) & 0xffffffff); bne x14, x7, fail;;
  test_4: li x28, 4; la x1, tdat; li x2, 0xffffffffffffefa0; la x15, 7f; sb x2, 2(x1); lh x14, 2(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffefa0) & 0xffffffff); bne x14, x7, fail;;
  test_5: li x28, 5; la x1, tdat; li x2, 0x000000000000000a; la x15, 7f; sb x2, 3(x1); lb x14, 3(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x000000000000000a) & 0xffffffff); bne x14, x7, fail;;
  # Test with negative offset
  test_6: li x28, 6; la x1, tdat8; li x2, 0xffffffffffffffaa; la x15, 7f; sb x2, -3(x1); lb x14, -3(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffffaa) & 0xffffffff); bne x14, x7, fail;;
  test_7: li x28, 7; la x1, tdat8; li x2, 0x0000000000000000; la x15, 7f; sb x2, -2(x1); lb x14, -2(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x0000000000000000) & 0xffffffff); bne x14, x7, fail;;
  test_8: li x28, 8; la x1, tdat8; li x2, 0xffffffffffffffa0; la x15, 7f; sb x2, -1(x1); lb x14, -1(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffffffffa0) & 0xffffffff); bne x14, x7, fail;;
  test_9: li x28, 9; la x1, tdat8; li x2, 0x000000000000000a; la x15, 7f; sb x2, 0(x1); lb x14, 0(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x000000000000000a) & 0xffffffff); bne x14, x7, fail;;
  # Test with a negative base
  test_10: li x28, 10; la x1, tdat9; li x2, 0x12345678; addi x4, x1, -32; sb x2, 32(x4); lb x5, 0(x1);; li x7, ((0x78) & 0xffffffff); bne x5, x7, fail;
  # Test with unaligned base
  test_11: li x28, 11; la x1, tdat9; li x2, 0x00003098; addi x1, x1, -6; sb x2, 7(x1); la x4, tdat10; lb x5, 0(x4);; li x7, ((0xffffffffffffff98) & 0xffffffff); bne x5, x7, fail;
  #-------------------------------------------------------------
  # Bypassing tests
  #-------------------------------------------------------------
  test_12: li x28, 12; li x4, 0; 1: li x1, 0xffffffffffffffdd; la x2, tdat; sb x1, 0(x2); lb x14, 0(x2); li x7, 0xffffffffffffffdd; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_13: li x28, 13; li x4, 0; 1: li x1, 0xffffffffffffffcd; la x2, tdat; nop; sb x1, 1(x2); lb x14, 1(x2); li x7, 0xffffffffffffffcd; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_14: li x28, 14; li x4, 0; 1: li x1, 0xffffffffffffffcc; la x2, tdat; nop; nop; sb x1, 2(x2); lb x14, 2(x2); li x7, 0xffffffffffffffcc; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_15: li x28, 15; li x4, 0; 1: li x1, 0xffffffffffffffbc; nop; la x2, tdat; sb x1, 3(x2); lb x14, 3(x2); li x7, 0xffffffffffffffbc; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_16: li x28, 16; li x4, 0; 1: li x1, 0xffffffffffffffbb; nop; la x2, tdat; nop; sb x1, 4(x2); lb x14, 4(x2); li x7, 0xffffffffffffffbb; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_17: li x28, 17; li x4, 0; 1: li x1, 0xffffffffffffffab; nop; nop; la x2, tdat; sb x1, 5(x2); lb x14, 5(x2); li x7, 0xffffffffffffffab; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_18: li x28, 18; li x4, 0; 1: la x2, tdat; li x1, 0x33; sb x1, 0(x2); lb x14, 0(x2); li x7, 0x33; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_19: li x28, 19; li x4, 0; 1: la x2, tdat; li x1, 0x23; nop; sb x1, 1(x2); lb x14, 1(x2); li x7, 0x23; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_20: li x28, 20; li x4, 0; 1: la x2, tdat; li x1, 0x22; nop; nop; sb x1, 2(x2); lb x14, 2(x2); li x7, 0x22; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_21: li x28, 21; li x4, 0; 1: la x2, tdat; nop; li x1, 0x12; sb x1, 3(x2); lb x14, 3(x2); li x7, 0x12; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_22: li x28, 22; li x4, 0; 1: la x2, tdat; nop; li x1, 0x11; nop; sb x1, 4(x2); lb x14, 4(x2); li x7, 0x11; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_23: li x28, 23; li x4, 0; 1: la x2, tdat; nop; nop; li x1, 0x01; sb x1, 5(x2); lb x14, 5(x2); li x7, 0x01; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  li a0, 0xef
  la a1, tdat
  sb a0, 3(a1)
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 
tdat:
tdat1: .byte 0xef
tdat2: .byte 0xef
tdat3: .byte 0xef
tdat4: .byte 0xef
tdat5: .byte 0xef
tdat6: .byte 0xef
tdat7: .byte 0xef
tdat8: .byte 0xef
tdat9: .byte 0xef
tdat10: .byte 0xef

