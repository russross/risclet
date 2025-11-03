#!/bin/bash
# Test reference encodings for compressed instructions

RISCV_PREFIX=riscv64-unknown-elf
AS="${RISCV_PREFIX}-as"
OBJDUMP="${RISCV_PREFIX}-objdump"
OUR_ASM="./target/debug/assembler"

if ! command -v "$AS" &> /dev/null; then
    echo "Error: $AS not found"
    exit 1
fi

echo "=== Compressed Instruction Encoding Tests ==="
echo

# Test 1: c.nop
echo "Test: c.nop"
cat > /tmp/test_cnop.s << 'EOF'
.text
.global _start
_start:
c.nop
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cnop.o /tmp/test_cnop.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cnop.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cnop.out /tmp/test_cnop.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cnop.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 2: c.add rd, rs2
echo "Test: c.add a0, a1"
cat > /tmp/test_cadd.s << 'EOF'
.text
.global _start
_start:
c.add a0, a1
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cadd.o /tmp/test_cadd.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cadd.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cadd.out /tmp/test_cadd.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cadd.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 3: c.jr rs1
echo "Test: c.jr ra"
cat > /tmp/test_cjr.s << 'EOF'
.text
.global _start
_start:
c.jr ra
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cjr.o /tmp/test_cjr.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cjr.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cjr.out /tmp/test_cjr.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cjr.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 4: c.ebreak
echo "Test: c.ebreak"
cat > /tmp/test_cebreak.s << 'EOF'
.text
.global _start
_start:
c.ebreak
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cebreak.o /tmp/test_cebreak.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cebreak.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cebreak.out /tmp/test_cebreak.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cebreak.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 5: c.mv rd, rs2
echo "Test: c.mv a0, a1"
cat > /tmp/test_cmv.s << 'EOF'
.text
.global _start
_start:
c.mv a0, a1
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cmv.o /tmp/test_cmv.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cmv.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cmv.out /tmp/test_cmv.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cmv.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 6: c.jalr rs1
echo "Test: c.jalr ra"
cat > /tmp/test_cjalr.s << 'EOF'
.text
.global _start
_start:
c.jalr ra
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cjalr.o /tmp/test_cjalr.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cjalr.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cjalr.out /tmp/test_cjalr.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cjalr.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 7: c.and rd', rs2'
echo "Test: c.and s0, s1"
cat > /tmp/test_cand.s << 'EOF'
.text
.global _start
_start:
c.and s0, s1
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cand.o /tmp/test_cand.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cand.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cand.out /tmp/test_cand.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cand.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 8: c.or rd', rs2'
echo "Test: c.or s0, s1"
cat > /tmp/test_cor.s << 'EOF'
.text
.global _start
_start:
c.or s0, s1
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cor.o /tmp/test_cor.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cor.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cor.out /tmp/test_cor.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cor.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 9: c.xor rd', rs2'
echo "Test: c.xor s0, s1"
cat > /tmp/test_cxor.s << 'EOF'
.text
.global _start
_start:
c.xor s0, s1
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cxor.o /tmp/test_cxor.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cxor.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cxor.out /tmp/test_cxor.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cxor.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 10: c.sub rd', rs2'
echo "Test: c.sub s0, s1"
cat > /tmp/test_csub.s << 'EOF'
.text
.global _start
_start:
c.sub s0, s1
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_csub.o /tmp/test_csub.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_csub.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_csub.out /tmp/test_csub.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_csub.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 11: c.addi rd, imm
echo "Test: c.addi a0, 5"
cat > /tmp/test_caddi.s << 'EOF'
.text
.global _start
_start:
c.addi a0, 5
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_caddi.o /tmp/test_caddi.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_caddi.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_caddi.out /tmp/test_caddi.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_caddi.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 12: c.li rd, imm
echo "Test: c.li a0, 5"
cat > /tmp/test_cli.s << 'EOF'
.text
.global _start
_start:
c.li a0, 5
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cli.o /tmp/test_cli.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cli.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cli.out /tmp/test_cli.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cli.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 13: c.slli rd, shamt
echo "Test: c.slli a0, 2"
cat > /tmp/test_cslli.s << 'EOF'
.text
.global _start
_start:
c.slli a0, 2
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cslli.o /tmp/test_cslli.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cslli.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cslli.out /tmp/test_cslli.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cslli.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 14: c.lwsp rd, offset(sp)
echo "Test: c.lwsp a0, 4(sp)"
cat > /tmp/test_clwsp.s << 'EOF'
.text
.global _start
_start:
c.lwsp a0, 4
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_clwsp.o /tmp/test_clwsp.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_clwsp.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_clwsp.out /tmp/test_clwsp.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_clwsp.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

# Test 15: c.swsp rs2, offset(sp)
echo "Test: c.swsp a0, 4(sp)"
cat > /tmp/test_cswsp.s << 'EOF'
.text
.global _start
_start:
c.swsp a0, 4
EOF

$AS -march=rv32imac -mabi=ilp32 -o /tmp/gnu_cswsp.o /tmp/test_cswsp.s 2>/dev/null
GNU_ENCODING=$($OBJDUMP -d /tmp/gnu_cswsp.o 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  GNU encoding: 0x$GNU_ENCODING"

$OUR_ASM -o /tmp/our_cswsp.out /tmp/test_cswsp.s 2>/dev/null
OUR_ENCODING=$($OBJDUMP -d /tmp/our_cswsp.out 2>/dev/null | grep -A1 "_start" | tail -1 | awk '{print $2}')
echo "  Our encoding: 0x$OUR_ENCODING"

if [ "$GNU_ENCODING" = "$OUR_ENCODING" ]; then
    echo "  ✓ PASS"
else
    echo "  ✗ FAIL"
fi
echo

echo "Done!"
