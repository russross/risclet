#!/bin/bash

# Determine script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTDIR="$SCRIPT_DIR"
QEMU="qemu-riscv32"

total=0
passed=0
failed=0

echo "Running all tests..."
echo "===================="

for test_bin in "$TESTDIR"/addi "$TESTDIR"/add "$TESTDIR"/sub "$TESTDIR"/and "$TESTDIR"/or "$TESTDIR"/xor \
                "$TESTDIR"/sll "$TESTDIR"/srl "$TESTDIR"/sra "$TESTDIR"/slt "$TESTDIR"/sltu \
                "$TESTDIR"/andi "$TESTDIR"/ori "$TESTDIR"/xori "$TESTDIR"/slli "$TESTDIR"/srli "$TESTDIR"/srai \
                "$TESTDIR"/slti "$TESTDIR"/sltiu \
                "$TESTDIR"/beq "$TESTDIR"/bne "$TESTDIR"/blt "$TESTDIR"/bge "$TESTDIR"/bltu "$TESTDIR"/bgeu \
                "$TESTDIR"/jal "$TESTDIR"/jalr \
                "$TESTDIR"/lui "$TESTDIR"/auipc \
                "$TESTDIR"/lb "$TESTDIR"/lh "$TESTDIR"/lw "$TESTDIR"/lbu "$TESTDIR"/lhu \
                "$TESTDIR"/sb "$TESTDIR"/sh "$TESTDIR"/sw \
                "$TESTDIR"/ld_st "$TESTDIR"/st_ld "$TESTDIR"/ma_data \
                "$TESTDIR"/simple \
                "$TESTDIR"/mul "$TESTDIR"/mulh "$TESTDIR"/mulhsu "$TESTDIR"/mulhu \
                "$TESTDIR"/div "$TESTDIR"/divu "$TESTDIR"/rem "$TESTDIR"/remu \
                "$TESTDIR"/rvc; do
    
    if [ ! -f "$test_bin" ]; then
        continue
    fi
    
    basename=$(basename "$test_bin")
    total=$((total + 1))
    
    if $QEMU "$test_bin" > /dev/null 2>&1; then
        exit_code=$?
        if [ $exit_code -eq 0 ]; then
            echo "✓ $basename"
            passed=$((passed + 1))
        else
            echo "✗ $basename (exit code: $exit_code)"
            failed=$((failed + 1))
        fi
    else
        exit_code=$?
        echo "✗ $basename (exit code: $exit_code)"
        failed=$((failed + 1))
    fi
done

echo "===================="
echo "Total: $total"
echo "Passed: $passed"
echo "Failed: $failed"

if [ $failed -eq 0 ]; then
    echo "All tests passed!"
    exit 0
else
    echo "Some tests failed."
    exit 1
fi
