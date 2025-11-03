// See LICENSE for license details.

#ifndef _ENV_VIRTUAL_SINGLE_CORE_H
#define _ENV_VIRTUAL_SINGLE_CORE_H

//-----------------------------------------------------------------------
// Begin Macro
//-----------------------------------------------------------------------

#undef RVTEST_RV64U
#define RVTEST_RV64U RVTEST_RV32U

#undef RVTEST_RV32U
#define RVTEST_RV32U                                                    \
        .globl _start

#undef RVTEST_CODE_BEGIN
#define RVTEST_CODE_BEGIN                                               \
        .text;                                                          \
_start:

#undef RVTEST_CODE_END
#define RVTEST_CODE_END

#undef RVTEST_DATA_BEGIN
#define RVTEST_DATA_BEGIN

//-----------------------------------------------------------------------
// Pass/Fail Macro
//-----------------------------------------------------------------------

#undef RVTEST_PASS
#define RVTEST_PASS                                                     \
        li a0, 0;                                                       \
        li a7, 93;                                                      \
        ecall

#undef RVTEST_FAIL
#define RVTEST_FAIL                                                     \
        mv a0, TESTNUM;                                                 \
        ebreak

//-----------------------------------------------------------------------
// Data Section Macro
//-----------------------------------------------------------------------

#undef RVTEST_DATA_END
#define RVTEST_DATA_END

#endif
