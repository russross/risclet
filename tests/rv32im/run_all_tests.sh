#!/bin/bash
# Test runner script for RV32IM test suite
# Runs all tests and reports results

RISCLET="${1:-../../target/debug/risclet}"
TESTS=(
    "test_basic_arithmetic"
    "test_shifts"
    "test_logical"
    "test_compare"
    "test_memory"
    "test_multiply"
    "test_divide"
    "test_branches"
    "test_jumps"
)

if [ ! -f "$RISCLET" ]; then
    echo "Error: risclet binary not found at $RISCLET"
    echo "Build with: cargo build"
    exit 1
fi

echo "RV32IM Test Suite Runner"
echo "========================================"
echo "Using risclet: $RISCLET"
echo ""

PASSED=0
FAILED=0

for test in "${TESTS[@]}"; do
    ELF="$test.elf"
    
    if [ ! -f "$ELF" ]; then
        echo "❌ $test - Binary not found"
        FAILED=$((FAILED + 1))
        continue
    fi
    
    echo -n "Running $test... "
    
    # Run test with 100k step limit
    output=$("$RISCLET" -e "$ELF" -l false -m run -s 100000 2>&1)
    exit_code=$?
    
    # Check for successful completion (ecall with exit code 0)
    if echo "$output" | grep -q "unsupported syscall 0"; then
        echo "✅ PASSED"
        PASSED=$((PASSED + 1))
    else
        echo "❌ FAILED"
        echo "   Output: $output" | head -1
        FAILED=$((FAILED + 1))
    fi
done

echo ""
echo "========================================"
echo "Results: $PASSED passed, $FAILED failed"

if [ $FAILED -eq 0 ]; then
    echo "All tests passed! ✅"
    exit 0
else
    echo "Some tests failed. ❌"
    exit 1
fi
