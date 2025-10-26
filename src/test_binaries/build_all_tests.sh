#!/bin/bash
set -e

# Determine script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTDIR="$SCRIPT_DIR"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

AS="riscv64-unknown-elf-as"
LD="riscv64-unknown-elf-ld"

cd "$PROJECT_ROOT"
mkdir -p "$TESTDIR"

echo "Generating all tests..."

for suite in rv32ui rv32um rv32uc; do
    if [ ! -d "$suite" ]; then
        continue
    fi
    
    ARCH="rv32im_zifencei"
    if [[ "$suite" == "rv32uc" ]]; then
        ARCH="rv32imc_zifencei"
    fi
    
    for test_file in "$suite"/*.S; do
        if [ ! -f "$test_file" ]; then
            continue
        fi
        
        basename=$(basename "$test_file" .S)
        
        # Skip tests not applicable for this project
        if [[ "$basename" == "fence_i" ]]; then
            continue
        fi
        
        output_s="$TESTDIR/${basename}.s"
        output_o="$TESTDIR/${basename}.o"
        output_bin="$TESTDIR/${basename}"
        
        echo "  Processing $test_file -> $output_bin"
        
        cpp -nostdinc -I"$TESTDIR" -D__riscv_xlen=32 -P "$test_file" "$output_s"
        
        $AS -march=$ARCH -mabi=ilp32 -o "$output_o" "$output_s"
        
        $LD -melf32lriscv --no-relax -o "$output_bin" "$output_o"
        
        rm -f "$output_o"
    done
done

echo "All tests generated successfully!"
