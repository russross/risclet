# See LICENSE for license details.
# See LICENSE for license details.
#*****************************************************************************
# lrsr.S
#-----------------------------------------------------------------------------
# Test LR/SC instructions.
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
 # get a unique core id
la a0, coreid
li a1, 1
amoadd.w a2, a1, (a0)
# for now, only run this on core 0
1:li a3, 1
bgeu a2, a3, 1b
1: lw a1, (a0)
bltu a1, a3, 1b
# make sure that sc without a reservation fails.
test_2: li x28, 2; la a0, foo; li a5, 0xdeadbeef; sc.w a4, a5, (a0);; li x7, ((1) & 0xffffffff); bne a4, x7, fail;
 # make sure the failing sc did not commit into memory
test_3: li x28, 3; lw a4, foo;; li x7, ((0) & 0xffffffff); bne a4, x7, fail;
 # Disable test case 4 for now. It assumes a <1K reservation granule, when
# in reality any size granule is valid. After discussion in issue #315,
# decided to simply disable the test for now.
# (See https:
## make sure that sc with the wrong reservation fails.
## TODO is this actually mandatory behavior?
#test_4: li x28, 4; # la a0, foo; # la a1, fooTest3; # lr.w a1, (a1); # sc.w a4, a1, (a0); #; li x7, ((1) & 0xffffffff); bne a4, x7, fail;
 # have each core add its coreid+1 to foo 1024 times
la a0, foo
li a1, 1<<10
addi a2, a2, 1
1: lr.w a4, (a0)
add a4, a4, a2
sc.w a4, a4, (a0)
bnez a4, 1b
addi a1, a1, -1
bnez a1, 1b
# wait for all cores to finish
la a0, barrier
li a1, 1
amoadd.w x0, a1, (a0)
1: lw a1, (a0)
blt a1, a3, 1b
fence
# expected result is 512*ncores*(ncores+1)
test_5: li x28, 5; lw a0, foo; slli a1, a3, 10 -1; 1:sub a0, a0, a1; addi a3, a3, -1; bgez a3, 1b; li x7, ((0) & 0xffffffff); bne a0, x7, fail;
 # make sure that sc-after-successful-sc fails.
test_6: li x28, 6; la a0, foo; 1:lr.w a1, (a0); sc.w a1, x0, (a0); bnez a1, 1b; sc.w a1, x0, (a0); sc.w a2, x0, (a0); add a1, a1, a2; li x7, ((2) & 0xffffffff); bne a1, x7, fail;
bne x0, x28, pass; fail: mv a0, x28; ebreak; pass: li a0, 0; li a7, 93; ecall

  .data

 
coreid: .word 0
barrier: .word 0
foo: .word 0
.skip 1024
fooTest3: .word 0

