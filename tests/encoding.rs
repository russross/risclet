use std::env::temp_dir;
use std::fs::{remove_file, write};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;

struct SourceFile(PathBuf);

impl Drop for SourceFile {
    fn drop(&mut self) {
        if let Err(error) = remove_file(&self.0) {
            assert_eq!(
                error.kind(),
                ErrorKind::NotFound,
                "remove temporary assembly source: {error}"
            );
        }
    }
}

#[test]
fn listings_preserve_encodings_and_pseudo_sequences() {
    let source = SourceFile(
        temp_dir().join(format!("risclet-encoding-{}.s", std::process::id())),
    );
    // Raw halfwords exercise both zero filling and sign extension on loading.
    write(
        &source.0,
        ".text\n.global _start\n_start:\n\
         .2byte 0x0001\nla a0, target\nli a7, 93\nli a0, 0\necall\n\
         target:\n.2byte 0x9002\n",
    )
    .expect("write assembly source");

    for mode in ["disassemble", "trace"] {
        for hex in ["--hex", "--no-hex"] {
            let run = |extra: &[&str]| {
                let output = Command::new(env!("CARGO_BIN_EXE_risclet"))
                    .arg(mode)
                    .arg(&source.0)
                    .args(["--no-show-addresses", hex])
                    .args(extra)
                    .output()
                    .expect("run risclet");
                assert!(output.status.success(), "{:?}", output.stderr);
                String::from_utf8(output.stdout).expect("UTF-8 listing")
            };

            // Defaults differ by mode, while explicit encoding display works in both.
            let default_listing = run(&[]);
            assert_eq!(
                default_listing.contains("00000517"),
                mode == "disassemble"
            );
            assert_eq!(
                default_listing.contains("01450513"),
                mode == "disassemble"
            );
            assert!(default_listing.contains("la      a0, target"));

            // The pseudo-operation has two encodings, with effects on its first row.
            let listing = run(&["--show-encoding"]);
            let lines: Vec<_> = listing.lines().collect();
            assert!(lines[0].starts_with("    0001 "));
            assert!(lines[1].starts_with("00000517 "));
            assert!(lines[1].contains("la      a0, target"));
            assert_eq!(lines[2], "01450513");
            assert!(listing.contains("00000073 "));
            if mode == "disassemble" {
                assert!(listing.contains("    9002 "));
                assert!(lines.iter().all(|line| *line == line.trim_end()));
            } else {
                assert!(lines[1].contains("a0 <- "));
                assert!(listing.contains("exit(0)"));
            }

            // Strict mode gives each encoding its own decoded instruction.
            let strict = run(&["--verbose-instructions", "--show-encoding"]);
            assert!(strict.lines().any(|line| {
                line.starts_with("01450513 ") && line.contains("addi")
            }));
            let hidden = run(&["--no-show-encoding"]);
            assert!(!hidden.contains("00000517"));
            assert!(!hidden.contains("01450513"));
            assert!(hidden.contains("la      a0, target"));
        }
    }
}

// Strict listings must reconstruct every supported compressed operand form exactly.
#[test]
fn strict_disassembly_round_trips_encodings() {
    let original = SourceFile(
        temp_dir()
            .join(format!("risclet-strict-original-{}.s", std::process::id())),
    );
    let rebuilt = SourceFile(
        temp_dir()
            .join(format!("risclet-strict-rebuilt-{}.s", std::process::id())),
    );
    write(
        &original.0,
        ".text\n.global _start\n_start:\n\
        lui a0, 0xfffff\nauipc a1, 0x80000\naddi a1, a1, -1\n\
        addi a2, gp, -16\njalr zero, ra, -2\n\
        c.nop\nc.li a0, -32\nc.lui a1, 0xfffff\nc.addi a0, -1\n\
        c.addi16sp sp, -512\nc.addi4spn a0, sp, 1020\n\
        c.slli a0, 31\nc.srli a0, 31\nc.srai a0, 31\nc.andi a0, -32\n\
        c.sub a0, a1\nc.xor a0, a1\nc.or a0, a1\nc.and a0, a1\n\
        c.mv a0, a1\nc.add a0, a1\nc.jr ra\nc.jalr t0\n\
        c.lw a0, 124(a1)\nc.sw a0, 124(a1)\n\
        c.lwsp a0, 252(sp)\nc.swsp a0, 252(sp)\n\
        c.beqz a0, . - 2\nc.bnez a0, . + 2\nc.j . - 2\nc.jal . + 2\nc.ebreak\n\
        .2byte 0x0000\n.2byte 0x1002\n",
    )
    .expect("write compressed source");

    // Decimal and hexadecimal operands must both survive parsing and encoding.
    for hex in ["--hex", "--no-hex"] {
        let disassemble = |path: &PathBuf, encoding: bool| {
            let output = Command::new(env!("CARGO_BIN_EXE_risclet"))
                .arg("disassemble")
                .arg(path)
                .args([
                    "--verbose-instructions",
                    "--no-show-addresses",
                    "--no-relax",
                    hex,
                ])
                .arg(if encoding {
                    "--show-encoding"
                } else {
                    "--no-show-encoding"
                })
                .output()
                .expect("disassemble round-trip source");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).expect("UTF-8 listing")
        };
        let listing = disassemble(&original.0, false);
        write(&rebuilt.0, format!(".text\n.global _start\n{listing}"))
            .expect("write reconstructed assembly");
        let encodings = |listing: String| {
            listing
                .lines()
                .map(|line| {
                    line.split_whitespace()
                        .next()
                        .expect("encoding")
                        .to_string()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            encodings(disassemble(&original.0, true)),
            encodings(disassemble(&rebuilt.0, true)),
            "{listing}"
        );
    }
}

// Canonical pseudo sequences remain assemblable with their complete address operands.
#[test]
fn pseudo_disassembly_round_trips_sequences() {
    let original = SourceFile(
        temp_dir()
            .join(format!("risclet-pseudo-original-{}.s", std::process::id())),
    );
    let rebuilt = SourceFile(
        temp_dir()
            .join(format!("risclet-pseudo-rebuilt-{}.s", std::process::id())),
    );
    write(&original.0, ".text\n.global _start\n_start:\n\
        li a0, 0x12345678\nli a1, -2147483648\nli a2, 2147483647\n\
        la a3, target\nlb a4, target\nlh a4, target\nlw a4, target\nlbu a4, target\nlhu a4, target\n\
        sb a0, target, t0\nsh a0, target, t1\nsw a0, target, t2\n\
        call target\ntail target\naddi a0, gp, 16\n\
        auipc a0, 0x80000\naddi a0, a0, -1\n\
        auipc zero, 1\naddi zero, zero, 4\ntarget:\nebreak\n")
        .expect("write pseudo source");
    for hex in ["--hex", "--no-hex"] {
        let listing = |path: &PathBuf, strict: bool| {
            let output = Command::new(env!("CARGO_BIN_EXE_risclet"))
                .arg("disassemble")
                .arg(path)
                .args(["--no-show-addresses", "--no-relax", hex])
                .arg(if strict {
                    "--verbose-instructions"
                } else {
                    "--no-verbose-instructions"
                })
                .arg(if strict {
                    "--show-encoding"
                } else {
                    "--no-show-encoding"
                })
                .output()
                .expect("disassemble pseudo source");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).expect("UTF-8 listing")
        };
        let pseudo = listing(&original.0, false);
        assert!(pseudo.contains("sw      a0, target, t2"));
        assert!(
            pseudo.contains("addi    a0, gp, 16")
                || pseudo.contains("addi    a0, gp, 0x10")
        );
        write(&rebuilt.0, format!(".text\n.global _start\n{pseudo}"))
            .expect("write reconstructed pseudos");
        let encodings = |listing: String| {
            listing
                .lines()
                .map(|line| {
                    line.split_whitespace()
                        .next()
                        .expect("encoding")
                        .to_string()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            encodings(listing(&original.0, true)),
            encodings(listing(&rebuilt.0, true)),
            "{pseudo}"
        );
    }
}

// Symbol-backed GP aliases retain the same encoding, while updates to GP stay explicit.
#[test]
fn gp_symbol_disassembly_round_trips() {
    let original = SourceFile(
        temp_dir()
            .join(format!("risclet-gp-original-{}.s", std::process::id())),
    );
    let rebuilt = SourceFile(
        temp_dir().join(format!("risclet-gp-rebuilt-{}.s", std::process::id())),
    );
    let data = ".data\ndata: .space 2048\nfar: .word 0\n";
    write(&original.0, format!(".text\n.global _start\n_start:\n\
        la gp, __global_pointer$\nla a0, data\naddi a1, gp, 0\n\
        addi gp, gp, -2048\naddi zero, gp, -2048\n\
        unusually_long_target_label:\nebreak\nj unusually_long_target_label\n{data}"))
        .expect("write GP source");
    let listing = |path: &PathBuf, strict: bool| {
        let output = Command::new(env!("CARGO_BIN_EXE_risclet"))
            .arg("disassemble")
            .arg(path)
            .args([
                "--no-show-addresses",
                "--no-hex",
                "--relax-gp",
                "--no-relax-compressed",
                "--no-relax-pseudo",
            ])
            .arg(if strict {
                "--verbose-instructions"
            } else {
                "--no-verbose-instructions"
            })
            .arg(if strict { "--show-encoding" } else { "--no-show-encoding" })
            .output()
            .expect("disassemble GP source");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 listing")
    };
    let pseudo = listing(&original.0, false);
    assert!(pseudo.contains("la      a0, data"));
    assert!(pseudo.contains("la      a1, __global_pointer$"));
    assert!(pseudo.contains("addi    gp, gp, -2048"));
    assert!(pseudo.contains("unusually_long_target_label:"));
    write(&rebuilt.0, format!(".text\n.global _start\n{pseudo}{data}"))
        .expect("write reconstructed GP source");
    let encodings = |listing: String| {
        listing
            .lines()
            .map(|line| {
                line.split_whitespace().next().expect("encoding").to_string()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        encodings(listing(&original.0, true)),
        encodings(listing(&rebuilt.0, true)),
        "{pseudo}"
    );
}
