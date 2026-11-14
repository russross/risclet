#!/usr/bin/env python3
"""Regenerate fixed instruction/input/result records using GNU binutils and QEMU."""

import argparse
import random
import struct
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", default="riscv64-unknown-elf-")
    parser.add_argument("--qemu", default="qemu-riscv32")
    args = parser.parse_args()
    directory = Path(__file__).resolve().parent
    encodings = (directory.parent / "bitmanip_encodings").read_bytes()
    source = (directory / "bitmanip_encodings.s").read_text().splitlines()
    instructions = [line for line in source if line and not line.startswith(("#", ".", "_", "c."))]

    # Fixed extremes and deterministic patterns cover signedness and bit positions.
    values = [0, 1, 2, 31, 32, 0x7F, 0x80, 0x8000, 0x80000000, 0xFFFFFFFF, 0x12345678, 0xAAAAAAAA]
    random_source = random.Random(0)
    values.extend(random_source.getrandbits(32) for _ in range(4))
    records: list[tuple[int, int, int]] = []
    assembly = [".option norvc", ".option norelax", ".text", ".global _start", "_start:", "la s0, results"]
    for index, instruction in enumerate(instructions):
        word = struct.unpack_from("<I", encodings, index * 4)[0]
        operands = instruction.split(",")
        immediate = len(operands) == 3 and operands[-1].strip().isdigit()
        inputs = ((a, b) for a in values for b in (range(32) if immediate else values if len(operands) == 3 else [0]))
        for a, b in inputs:
            encoding = (word & ~(31 << 20)) | (b << 20) if immediate else word
            records.append((encoding, a, b))
            operation = f'{instruction.rsplit(",", 1)[0]}, {b}' if immediate else instruction
            assembly.extend([f"li a1, {a}", f"li a2, {b}", operation, "sw a0, 0(s0)", "addi s0, s0, 4"])

    # QEMU writes each result as a word; register setup never depends on prior cases.
    assembly.extend(["li a0, 1", "la a1, results", f"li a2, {len(records) * 4}", "li a7, 64", "ecall", "li a0, 0", "li a7, 93", "ecall", ".bss", ".balign 4", f"results: .space {len(records) * 4}"])
    with tempfile.TemporaryDirectory(prefix="risclet-bitmanip-") as temporary:
        work = Path(temporary)
        (work / "oracle.s").write_text("\n".join(assembly) + "\n")
        subprocess.run([args.prefix + "as", "-march=rv32imc_zba_zbb_zbs_zbkb_zcb", "-mabi=ilp32", "-o", str(work / "oracle.o"), str(work / "oracle.s")], check=True)
        subprocess.run([args.prefix + "ld", "-melf32lriscv", "--no-relax", "-o", str(work / "oracle"), str(work / "oracle.o")], check=True)
        result = subprocess.run([args.qemu, "-cpu", "max", str(work / "oracle")], check=True, capture_output=True)
    if len(result.stdout) != len(records) * 4:
        raise RuntimeError("QEMU returned an incomplete result stream")
    output = bytearray()
    for record, (expected,) in zip(records, struct.iter_unpack("<I", result.stdout), strict=True):
        output.extend(struct.pack("<IIII", *record, expected))
    (directory.parent / "bitmanip_results").write_bytes(output)
    print(f"Recorded {len(records)} GNU/QEMU instruction cases")


if __name__ == "__main__":
    try:
        main()
    except (OSError, subprocess.CalledProcessError, RuntimeError) as error:
        print(f"Could not generate instruction results: {error}", file=sys.stderr)
        sys.exit(1)
