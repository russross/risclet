# See LICENSE for license details.
#*****************************************************************************
# rvc.S - Modified for user-mode with proper .data section
#-----------------------------------------------------------------------------
# Test RVC corner cases.
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
  .align 2
  .option push
  .option norvc
  li x28, 2
  li a1, 666
  test_2: li x28, 2; addi a1, a1, 1; li x7, ((667) & 0xffffffff); bne a1, x7, fail;
  li sp, 0x1234
  test_3: li x28, 3; .option push; .option rvc; c.addi4spn a0, sp, 1020; .align 2; .option pop; li x7, ((0x1234 + 1020) & 0xffffffff); bne a0, x7, fail;
  test_4: li x28, 4; .option push; .option rvc; c.addi16sp sp, 496; .align 2; .option pop; li x7, ((0x1234 + 496) & 0xffffffff); bne sp, x7, fail;
  test_5: li x28, 5; .option push; .option rvc; c.addi16sp sp, -512; .align 2; .option pop; li x7, ((0x1234 + 496 - 512) & 0xffffffff); bne sp, x7, fail;
  la a1, data
  test_6: li x28, 6; .option push; .option rvc; c.lw a0, 4(a1); addi a0, a0, 1; c.sw a0, 4(a1); c.lw a2, 4(a1); .align 2; .option pop; li x7, ((0xfffffffffedcba99) & 0xffffffff); bne a2, x7, fail;
  test_8: li x28, 8; .option push; .option rvc; ori a0, x0, 1; c.addi a0, -16; .align 2; .option pop; li x7, ((-15) & 0xffffffff); bne a0, x7, fail;
  test_9: li x28, 9; .option push; .option rvc; ori a5, x0, 1; c.li a5, -16; .align 2; .option pop; li x7, ((-16) & 0xffffffff); bne a5, x7, fail;
  test_11: li x28, 11; .option push; .option rvc; c.lui s0, 0xfffe1; c.srai s0, 12; .align 2; .option pop; li x7, ((0xffffffffffffffe1) & 0xffffffff); bne s0, x7, fail;
  test_12: li x28, 12; .option push; .option rvc; c.lui s0, 0xfffe1; c.srli s0, 12; .align 2; .option pop; li x7, ((0x000fffe1) & 0xffffffff); bne s0, x7, fail;
  test_14: li x28, 14; .option push; .option rvc; c.li s0, -2; c.andi s0, ~0x10; .align 2; .option pop; li x7, ((~0x11) & 0xffffffff); bne s0, x7, fail;
  test_15: li x28, 15; .option push; .option rvc; li s1, 20; li a0, 6; c.sub s1, a0; .align 2; .option pop; li x7, ((14) & 0xffffffff); bne s1, x7, fail;
  test_16: li x28, 16; .option push; .option rvc; li s1, 20; li a0, 6; c.xor s1, a0; .align 2; .option pop; li x7, ((18) & 0xffffffff); bne s1, x7, fail;
  test_17: li x28, 17; .option push; .option rvc; li s1, 20; li a0, 6; c.or s1, a0; .align 2; .option pop; li x7, ((22) & 0xffffffff); bne s1, x7, fail;
  test_18: li x28, 18; .option push; .option rvc; li s1, 20; li a0, 6; c.and s1, a0; .align 2; .option pop; li x7, ((4) & 0xffffffff); bne s1, x7, fail;
  test_21: li x28, 21; .option push; .option rvc; li s0, 0x1234; c.slli s0, 4; .align 2; .option pop; li x7, ((0x12340) & 0xffffffff); bne s0, x7, fail;
  test_30: li x28, 30; .option push; .option rvc; li ra, 0; c.j 1f; c.j 2f; 1:c.j 1f; 2:j fail; 1:; .align 2; .option pop; li x7, ((0) & 0xffffffff); bne ra, x7, fail;
  test_31: li x28, 31; .option push; .option rvc; li a0, 0; c.beqz a0, 1f; c.j 2f; 1:c.j 1f; 2:j fail; 1:; .align 2; .option pop; li x7, ((0) & 0xffffffff); bne x0, x7, fail;
  test_32: li x28, 32; .option push; .option rvc; li a0, 1; c.bnez a0, 1f; c.j 2f; 1:c.j 1f; 2:j fail; 1:; .align 2; .option pop; li x7, ((0) & 0xffffffff); bne x0, x7, fail;
  test_33: li x28, 33; .option push; .option rvc; li a0, 1; c.beqz a0, 1f; c.j 2f; 1:c.j fail; 2:; .align 2; .option pop; li x7, ((0) & 0xffffffff); bne x0, x7, fail;
  test_34: li x28, 34; .option push; .option rvc; li a0, 0; c.bnez a0, 1f; c.j 2f; 1:c.j fail; 2:; .align 2; .option pop; li x7, ((0) & 0xffffffff); bne x0, x7, fail;
  test_35: li x28, 35; .option push; .option rvc; la t0, 1f; li ra, 0; c.jr t0; c.j 2f; 1:c.j 1f; 2:j fail; 1:; .align 2; .option pop; li x7, ((0) & 0xffffffff); bne ra, x7, fail;
  test_36: li x28, 36; .option push; .option rvc; la t0, 1f; li ra, 0; c.jalr t0; c.j 2f; 1:c.j 1f; 2:j fail; 1:sub ra, ra, t0; .align 2; .option pop; li x7, ((-2) & 0xffffffff); bne ra, x7, fail;
  test_37: li x28, 37; .option push; .option rvc; la t0, 1f; li ra, 0; c.jal 1f; c.j 2f; 1:c.j 1f; 2:j fail; 1:sub ra, ra, t0; .align 2; .option pop; li x7, ((-2) & 0xffffffff); bne ra, x7, fail;
  la sp, data
  test_40: li x28, 40; .option push; .option rvc; c.lwsp a0, 12(sp); addi a0, a0, 1; c.swsp a0, 12(sp); c.lwsp a2, 12(sp); .align 2; .option pop; li x7, ((0xfffffffffedcba99) & 0xffffffff); bne a2, x7, fail;
  test_42: li x28, 42; .option push; .option rvc; li a0, 0x123; c.mv t0, a0; c.add t0, a0; .align 2; .option pop; li x7, ((0x246) & 0xffffffff); bne t0, x7, fail;
  .option pop
  bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

  .align 3
data:
  .dword 0xfedcba9876543210
  .dword 0xfedcba9876543210

