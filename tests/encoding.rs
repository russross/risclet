use std::env::temp_dir;
use std::fs::{remove_file, write};
use std::path::PathBuf;
use std::process::Command;

struct SourceFile(PathBuf);

impl Drop for SourceFile {
    fn drop(&mut self) {
        remove_file(&self.0).expect("remove temporary assembly source");
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
            assert_eq!(default_listing.contains("00000517"), mode == "disassemble");
            assert_eq!(default_listing.contains("01450513"), mode == "disassemble");
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
