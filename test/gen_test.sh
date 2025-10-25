#!/bin/bash
set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 <input.S>"
    echo "Example: $0 rv32ui/addi.S"
    exit 1
fi

# Determine script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

INPUT="$1"
BASENAME=$(basename "$INPUT" .S)
OUTPUT="$SCRIPT_DIR/${BASENAME}.s"

TESTDIR="$SCRIPT_DIR"
ARCH="rv32im"

cd "$PROJECT_ROOT"

if [[ "$INPUT" == rv32uc/* ]] || [[ "$INPUT" == rv64uc/* ]]; then
    ARCH="rv32imc"
fi

echo "Processing $INPUT -> $OUTPUT (arch: $ARCH)"

cpp -nostdinc \
    -I"$TESTDIR" \
    -D__riscv_xlen=32 \
    -P \
    "$INPUT" \
    "$OUTPUT"

echo "Generated $OUTPUT"
