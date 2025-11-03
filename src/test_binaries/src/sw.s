# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# sw.S
#-----------------------------------------------------------------------------
# Test sw instruction.
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
  test_2: li x28, 2; la x1, tdat; li x2, 0x0000000000aa00aa; la x15, 7f; sw x2, 0(x1); lw x14, 0(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x0000000000aa00aa) & 0xffffffff); bne x14, x7, fail;;
  test_3: li x28, 3; la x1, tdat; li x2, 0xffffffffaa00aa00; la x15, 7f; sw x2, 4(x1); lw x14, 4(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffaa00aa00) & 0xffffffff); bne x14, x7, fail;;
  test_4: li x28, 4; la x1, tdat; li x2, 0x000000000aa00aa0; la x15, 7f; sw x2, 8(x1); lw x14, 8(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x000000000aa00aa0) & 0xffffffff); bne x14, x7, fail;;
  test_5: li x28, 5; la x1, tdat; li x2, 0xffffffffa00aa00a; la x15, 7f; sw x2, 12(x1); lw x14, 12(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffa00aa00a) & 0xffffffff); bne x14, x7, fail;;
  # Test with negative offset
  test_6: li x28, 6; la x1, tdat8; li x2, 0x0000000000aa00aa; la x15, 7f; sw x2, -12(x1); lw x14, -12(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x0000000000aa00aa) & 0xffffffff); bne x14, x7, fail;;
  test_7: li x28, 7; la x1, tdat8; li x2, 0xffffffffaa00aa00; la x15, 7f; sw x2, -8(x1); lw x14, -8(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffaa00aa00) & 0xffffffff); bne x14, x7, fail;;
  test_8: li x28, 8; la x1, tdat8; li x2, 0x000000000aa00aa0; la x15, 7f; sw x2, -4(x1); lw x14, -4(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0x000000000aa00aa0) & 0xffffffff); bne x14, x7, fail;;
  test_9: li x28, 9; la x1, tdat8; li x2, 0xffffffffa00aa00a; la x15, 7f; sw x2, 0(x1); lw x14, 0(x1); j 8f; 7: mv x14, x2; 8:; li x7, ((0xffffffffa00aa00a) & 0xffffffff); bne x14, x7, fail;;
  # Test with a negative base
  test_10: li x28, 10; la x1, tdat9; li x2, 0x12345678; addi x4, x1, -32; sw x2, 32(x4); lw x5, 0(x1);; li x7, ((0x12345678) & 0xffffffff); bne x5, x7, fail;
  # Test with unaligned base
  test_11: li x28, 11; la x1, tdat9; li x2, 0x58213098; addi x1, x1, -3; sw x2, 7(x1); la x4, tdat10; lw x5, 0(x4);; li x7, ((0x58213098) & 0xffffffff); bne x5, x7, fail;
  #-------------------------------------------------------------
  # Bypassing tests
  #-------------------------------------------------------------
  test_12: li x28, 12; li x4, 0; 1: li x1, 0xffffffffaabbccdd; la x2, tdat; sw x1, 0(x2); lw x14, 0(x2); li x7, 0xffffffffaabbccdd; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_13: li x28, 13; li x4, 0; 1: li x1, 0xffffffffdaabbccd; la x2, tdat; nop; sw x1, 4(x2); lw x14, 4(x2); li x7, 0xffffffffdaabbccd; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_14: li x28, 14; li x4, 0; 1: li x1, 0xffffffffddaabbcc; la x2, tdat; nop; nop; sw x1, 8(x2); lw x14, 8(x2); li x7, 0xffffffffddaabbcc; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_15: li x28, 15; li x4, 0; 1: li x1, 0xffffffffcddaabbc; nop; la x2, tdat; sw x1, 12(x2); lw x14, 12(x2); li x7, 0xffffffffcddaabbc; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_16: li x28, 16; li x4, 0; 1: li x1, 0xffffffffccddaabb; nop; la x2, tdat; nop; sw x1, 16(x2); lw x14, 16(x2); li x7, 0xffffffffccddaabb; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_17: li x28, 17; li x4, 0; 1: li x1, 0xffffffffbccddaab; nop; nop; la x2, tdat; sw x1, 20(x2); lw x14, 20(x2); li x7, 0xffffffffbccddaab; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_18: li x28, 18; li x4, 0; 1: la x2, tdat; li x1, 0x00112233; sw x1, 0(x2); lw x14, 0(x2); li x7, 0x00112233; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_19: li x28, 19; li x4, 0; 1: la x2, tdat; li x1, 0x30011223; nop; sw x1, 4(x2); lw x14, 4(x2); li x7, 0x30011223; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_20: li x28, 20; li x4, 0; 1: la x2, tdat; li x1, 0x33001122; nop; nop; sw x1, 8(x2); lw x14, 8(x2); li x7, 0x33001122; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_21: li x28, 21; li x4, 0; 1: la x2, tdat; nop; li x1, 0x23300112; sw x1, 12(x2); lw x14, 12(x2); li x7, 0x23300112; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_22: li x28, 22; li x4, 0; 1: la x2, tdat; nop; li x1, 0x22330011; nop; sw x1, 16(x2); lw x14, 16(x2); li x7, 0x22330011; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  test_23: li x28, 23; li x4, 0; 1: la x2, tdat; nop; nop; li x1, 0x12233001; sw x1, 20(x2); lw x14, 20(x2); li x7, 0x12233001; bne x14, x7, fail; addi x4, x4, 1; li x5, 2; bne x4, x5, 1b;
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 
tdat:
tdat1: .word 0xdeadbeef
tdat2: .word 0xdeadbeef
tdat3: .word 0xdeadbeef
tdat4: .word 0xdeadbeef
tdat5: .word 0xdeadbeef
tdat6: .word 0xdeadbeef
tdat7: .word 0xdeadbeef
tdat8: .word 0xdeadbeef
tdat9: .word 0xdeadbeef
tdat10: .word 0xdeadbeef

