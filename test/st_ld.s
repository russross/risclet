# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# st_ld.S
#-----------------------------------------------------------------------------
# Test store and load instructions
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
  # Bypassing Tests
  #-------------------------------------------------------------
  # Test sb and lb (signed byte)
  test_2: li x28, 2; la x2, tdat; li x1, 0xffffffffffffffdd; sb x1, 0(x2); lb x14, 0(x2); li x7, 0xffffffffffffffdd; bne x14, x7, fail;;
  test_3: li x28, 3; la x2, tdat; li x1, 0xffffffffffffffcd; sb x1, 1(x2); lb x14, 1(x2); li x7, 0xffffffffffffffcd; bne x14, x7, fail;;
  test_4: li x28, 4; la x2, tdat; li x1, 0xffffffffffffffcc; sb x1, 2(x2); lb x14, 2(x2); li x7, 0xffffffffffffffcc; bne x14, x7, fail;;
  test_5: li x28, 5; la x2, tdat; li x1, 0xffffffffffffffbc; sb x1, 3(x2); lb x14, 3(x2); li x7, 0xffffffffffffffbc; bne x14, x7, fail;;
  test_6: li x28, 6; la x2, tdat; li x1, 0xffffffffffffffbb; sb x1, 4(x2); lb x14, 4(x2); li x7, 0xffffffffffffffbb; bne x14, x7, fail;;
  test_7: li x28, 7; la x2, tdat; li x1, 0xffffffffffffffab; sb x1, 5(x2); lb x14, 5(x2); li x7, 0xffffffffffffffab; bne x14, x7, fail;;
  test_8: li x28, 8; la x2, tdat; li x1, 0x33; sb x1, 0(x2); lb x14, 0(x2); li x7, 0x33; bne x14, x7, fail;;
  test_9: li x28, 9; la x2, tdat; li x1, 0x23; sb x1, 1(x2); lb x14, 1(x2); li x7, 0x23; bne x14, x7, fail;;
  test_10: li x28, 10; la x2, tdat; li x1, 0x22; sb x1, 2(x2); lb x14, 2(x2); li x7, 0x22; bne x14, x7, fail;;
  test_11: li x28, 11; la x2, tdat; li x1, 0x12; sb x1, 3(x2); lb x14, 3(x2); li x7, 0x12; bne x14, x7, fail;;
  test_12: li x28, 12; la x2, tdat; li x1, 0x11; sb x1, 4(x2); lb x14, 4(x2); li x7, 0x11; bne x14, x7, fail;;
  test_13: li x28, 13; la x2, tdat; li x1, 0x01; sb x1, 5(x2); lb x14, 5(x2); li x7, 0x01; bne x14, x7, fail;;
  # Test sb and lbu (unsigned byte)
  test_14: li x28, 14; la x2, tdat; li x1, 0x33; sb x1, 0(x2); lbu x14, 0(x2); li x7, 0x33; bne x14, x7, fail;;
  test_15: li x28, 15; la x2, tdat; li x1, 0x23; sb x1, 1(x2); lbu x14, 1(x2); li x7, 0x23; bne x14, x7, fail;;
  test_16: li x28, 16; la x2, tdat; li x1, 0x22; sb x1, 2(x2); lbu x14, 2(x2); li x7, 0x22; bne x14, x7, fail;;
  test_17: li x28, 17; la x2, tdat; li x1, 0x12; sb x1, 3(x2); lbu x14, 3(x2); li x7, 0x12; bne x14, x7, fail;;
  test_18: li x28, 18; la x2, tdat; li x1, 0x11; sb x1, 4(x2); lbu x14, 4(x2); li x7, 0x11; bne x14, x7, fail;;
  test_19: li x28, 19; la x2, tdat; li x1, 0x01; sb x1, 5(x2); lbu x14, 5(x2); li x7, 0x01; bne x14, x7, fail;;
  # Test sw and lw (signed word)
  test_20: li x28, 20; la x2, tdat; li x1, 0xffffffffaabbccdd; sw x1, 0(x2); lw x14, 0(x2); li x7, 0xffffffffaabbccdd; bne x14, x7, fail;;
  test_21: li x28, 21; la x2, tdat; li x1, 0xffffffffdaabbccd; sw x1, 4(x2); lw x14, 4(x2); li x7, 0xffffffffdaabbccd; bne x14, x7, fail;;
  test_22: li x28, 22; la x2, tdat; li x1, 0xffffffffddaabbcc; sw x1, 8(x2); lw x14, 8(x2); li x7, 0xffffffffddaabbcc; bne x14, x7, fail;;
  test_23: li x28, 23; la x2, tdat; li x1, 0xffffffffcddaabbc; sw x1, 12(x2); lw x14, 12(x2); li x7, 0xffffffffcddaabbc; bne x14, x7, fail;;
  test_24: li x28, 24; la x2, tdat; li x1, 0xffffffffccddaabb; sw x1, 16(x2); lw x14, 16(x2); li x7, 0xffffffffccddaabb; bne x14, x7, fail;;
  test_25: li x28, 25; la x2, tdat; li x1, 0xffffffffbccddaab; sw x1, 20(x2); lw x14, 20(x2); li x7, 0xffffffffbccddaab; bne x14, x7, fail;;
  test_26: li x28, 26; la x2, tdat; li x1, 0x00112233; sw x1, 0(x2); lw x14, 0(x2); li x7, 0x00112233; bne x14, x7, fail;;
  test_27: li x28, 27; la x2, tdat; li x1, 0x30011223; sw x1, 4(x2); lw x14, 4(x2); li x7, 0x30011223; bne x14, x7, fail;;
  test_28: li x28, 28; la x2, tdat; li x1, 0x33001122; sw x1, 8(x2); lw x14, 8(x2); li x7, 0x33001122; bne x14, x7, fail;;
  test_29: li x28, 29; la x2, tdat; li x1, 0x23300112; sw x1, 12(x2); lw x14, 12(x2); li x7, 0x23300112; bne x14, x7, fail;;
  test_30: li x28, 30; la x2, tdat; li x1, 0x22330011; sw x1, 16(x2); lw x14, 16(x2); li x7, 0x22330011; bne x14, x7, fail;;
  test_31: li x28, 31; la x2, tdat; li x1, 0x12233001; sw x1, 20(x2); lw x14, 20(x2); li x7, 0x12233001; bne x14, x7, fail;;
  # Test sh and lh (signed halfword)
  test_32: li x28, 32; la x2, tdat; li x1, 0xffffffffffffccdd; sh x1, 0(x2); lh x14, 0(x2); li x7, 0xffffffffffffccdd; bne x14, x7, fail;;
  test_33: li x28, 33; la x2, tdat; li x1, 0xffffffffffffbccd; sh x1, 2(x2); lh x14, 2(x2); li x7, 0xffffffffffffbccd; bne x14, x7, fail;;
  test_34: li x28, 34; la x2, tdat; li x1, 0xffffffffffffbbcc; sh x1, 4(x2); lh x14, 4(x2); li x7, 0xffffffffffffbbcc; bne x14, x7, fail;;
  test_35: li x28, 35; la x2, tdat; li x1, 0xffffffffffffabbc; sh x1, 6(x2); lh x14, 6(x2); li x7, 0xffffffffffffabbc; bne x14, x7, fail;;
  test_36: li x28, 36; la x2, tdat; li x1, 0xffffffffffffaabb; sh x1, 8(x2); lh x14, 8(x2); li x7, 0xffffffffffffaabb; bne x14, x7, fail;;
  test_37: li x28, 37; la x2, tdat; li x1, 0xffffffffffffdaab; sh x1, 10(x2); lh x14, 10(x2); li x7, 0xffffffffffffdaab; bne x14, x7, fail;;
  test_38: li x28, 38; la x2, tdat; li x1, 0x2233; sh x1, 0(x2); lh x14, 0(x2); li x7, 0x2233; bne x14, x7, fail;;
  test_39: li x28, 39; la x2, tdat; li x1, 0x1223; sh x1, 2(x2); lh x14, 2(x2); li x7, 0x1223; bne x14, x7, fail;;
  test_40: li x28, 40; la x2, tdat; li x1, 0x1122; sh x1, 4(x2); lh x14, 4(x2); li x7, 0x1122; bne x14, x7, fail;;
  test_41: li x28, 41; la x2, tdat; li x1, 0x0112; sh x1, 6(x2); lh x14, 6(x2); li x7, 0x0112; bne x14, x7, fail;;
  test_42: li x28, 42; la x2, tdat; li x1, 0x0011; sh x1, 8(x2); lh x14, 8(x2); li x7, 0x0011; bne x14, x7, fail;;
  test_43: li x28, 43; la x2, tdat; li x1, 0x3001; sh x1, 10(x2); lh x14, 10(x2); li x7, 0x3001; bne x14, x7, fail;;
  # Test sh and lhu (unsigned halfword)
  test_44: li x28, 44; la x2, tdat; li x1, 0x2233; sh x1, 0(x2); lhu x14, 0(x2); li x7, 0x2233; bne x14, x7, fail;;
  test_45: li x28, 45; la x2, tdat; li x1, 0x1223; sh x1, 2(x2); lhu x14, 2(x2); li x7, 0x1223; bne x14, x7, fail;;
  test_46: li x28, 46; la x2, tdat; li x1, 0x1122; sh x1, 4(x2); lhu x14, 4(x2); li x7, 0x1122; bne x14, x7, fail;;
  test_47: li x28, 47; la x2, tdat; li x1, 0x0112; sh x1, 6(x2); lhu x14, 6(x2); li x7, 0x0112; bne x14, x7, fail;;
  test_48: li x28, 48; la x2, tdat; li x1, 0x0011; sh x1, 8(x2); lhu x14, 8(x2); li x7, 0x0011; bne x14, x7, fail;;
  test_49: li x28, 49; la x2, tdat; li x1, 0x3001; sh x1, 10(x2); lhu x14, 10(x2); li x7, 0x3001; bne x14, x7, fail;;
  # RV64-specific tests for ld, sd, and lwu
  li a0, 0xef # Immediate load for manual store test
  la a1, tdat # Load address of tdat
  sb a0, 3(a1) # Store byte at offset 3 of tdat
  lb a2, 3(a1) # Load byte back for verification
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 
tdat:
    .rept 20
    .word 0xdeadbeef
    .endr

