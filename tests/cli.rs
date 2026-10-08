use std::fs::{create_dir, read, remove_dir_all, write};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("risclet-cli-{}-{id}", std::process::id()));
        create_dir(&path).expect("create isolated CLI workspace");
        Self(path)
    }

    // Each subprocess gets its own working directory, without changing test state.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_risclet"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("run risclet")
    }

    fn source(&self, name: &str, code: i32) {
        write(
            self.0.join(name),
            format!(".text\n.global _start\n_start:\nli a0, {code}\nli a7, 93\necall\n"),
        )
        .expect("write assembly source");
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        remove_dir_all(&self.0).expect("remove CLI workspace");
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn explicit_executables_override_source_discovery_and_extension_inference() {
    let workspace = Workspace::new();
    workspace.source("program.s", 7);
    success(&workspace.run(&["assemble", "program.s"]));
    success(&workspace.run(&["assemble", "program.s", "-o", "binary.s"]));
    success(&workspace.run(&["assemble", "program.s", "-o", "run"]));
    workspace.source("program.s", 9);

    // Explicit selection must use the saved program even beside newer source.
    for args in [
        vec!["a.out"],
        vec!["run", "a.out"],
        vec!["-e", "a.out"],
        vec!["run", "--executable", "a.out"],
        vec!["run", "-e", "binary.s"],
        vec!["-e", "run"],
    ] {
        assert_eq!(workspace.run(&args).status.code(), Some(7), "{args:?}");
    }
    assert_eq!(workspace.run(&["program.s"]).status.code(), Some(9));
    let missing = workspace.run(&["-e", "missing"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing"));
}

#[test]
fn default_run_and_assemble_discover_sources_before_saved_executables() {
    let workspace = Workspace::new();
    workspace.source("program.s", 7);
    success(&workspace.run(&["assemble"]));
    workspace.source("program.s", 9);
    for args in [&[][..], &["run"][..], &["trace"][..]] {
        assert_eq!(workspace.run(args).status.code(), Some(9));
    }
    success(&workspace.run(&["disassemble"]));
    success(&workspace.run(&["assemble", "-o", "saved"]));
    assert_eq!(workspace.run(&["saved"]).status.code(), Some(9));

    // Discovery also links multiple files rather than choosing just one source.
    write(
        workspace.0.join("program.s"),
        ".text\n.global _start\n_start:\ncall finish\n",
    )
    .expect("write calling source");
    write(
        workspace.0.join("library.s"),
        ".text\n.global finish\nfinish:\nli a0, 11\nli a7, 93\necall\n",
    )
    .expect("write library source");
    assert_eq!(workspace.run(&[]).status.code(), Some(11));
    success(&workspace.run(&["assemble"]));
    assert_eq!(workspace.run(&["a.out"]).status.code(), Some(11));
}

// Section selection resets per file, while storage offsets continue across files.
#[test]
fn source_files_default_to_text_after_data_or_bss() {
    let workspace = Workspace::new();
    write(
        workspace.0.join("second.s"),
        ".global _start, second_data, second_bss\n_start:\n\
         call helper\nli a7, 93\necall\n\
         .data\nsecond_data: .word 7\n.bss\nsecond_bss: .space 4\n",
    )
    .expect("write source with implicit text section");

    // The helper reads initialized and zero-filled storage from both source files.
    for section in [".data", ".bss"] {
        write(
            workspace.0.join("first.s"),
            format!(
                ".global helper, first_data, first_bss\nhelper:\n\
                 lw a0, first_data\nlw a1, second_data\nadd a0, a0, a1\n\
                 lw a1, first_bss\nadd a0, a0, a1\n\
                 lw a1, second_bss\nadd a0, a0, a1\nret\n\
                 .data\nfirst_data: .word 5\n.bss\nfirst_bss: .space 4\n{section}\n"
            ),
        )
        .expect("write source ending in a storage section");
        success(&workspace.run(&["assemble", "first.s", "second.s"]));
        let output = workspace.run(&["run", "a.out"]);
        assert_eq!(output.status.code(), Some(12), "{section}: {output:?}");
        assert!(output.stderr.is_empty(), "{section}: {output:?}");
    }
}

#[test]
fn discovery_without_sources_only_falls_back_to_elf_for_execution() {
    let workspace = Workspace::new();
    assert_eq!(workspace.run(&[]).status.code(), Some(1));
    workspace.source("program.s", 7);
    success(&workspace.run(&["assemble"]));
    std::fs::remove_file(workspace.0.join("program.s")).expect("remove source");
    assert_eq!(workspace.run(&[]).status.code(), Some(7));
    let saved = read(workspace.0.join("a.out")).expect("read saved ELF");
    assert_eq!(workspace.run(&["assemble"]).status.code(), Some(1));
    assert_eq!(read(workspace.0.join("a.out")).unwrap(), saved);
}

#[test]
fn informational_commands_succeed_without_input_and_show_stable_defaults() {
    let workspace = Workspace::new();
    for args in [
        vec!["--help"],
        vec!["help"],
        vec!["--version"],
        vec!["assemble", "--help"],
        vec!["run", "--help"],
        vec!["debug", "--help"],
        vec!["disassemble", "--help"],
        vec!["trace", "--help"],
    ] {
        let output = workspace.run(&args);
        success(&output);
        assert!(!output.stdout.is_empty());
    }
    assert_eq!(
        workspace.run(&["trace", "--help"]).stdout,
        workspace
            .run(&["trace", "-t", "0x20000", "--no-relax", "--help"])
            .stdout,
    );
}

#[test]
fn dumps_succeed_without_creating_or_overwriting_output() {
    let workspace = Workspace::new();
    // Earlier assembly phases can be inspected without an executable entry.
    write(workspace.0.join("program.s"), ".text\nlocal:\nli a0, 0\n")
        .expect("write inspection source");
    write(workspace.0.join("saved"), b"preserve me")
        .expect("write existing output");
    for dumps in [
        vec!["--dump-ast"],
        vec!["--dump-symbols"],
        vec!["--dump-layout"],
        vec!["--dump-code"],
    ] {
        let mut args = vec!["assemble", "program.s", "-o", "saved"];
        args.extend(dumps.iter().copied());
        let output = workspace.run(&args);
        success(&output);
        let listing = String::from_utf8_lossy(&output.stdout);
        assert!(listing.contains("No output file generated"));
        for dump in dumps {
            let heading = match dump {
                "--dump-ast" => "AST Dump",
                "--dump-symbols" => "SYMBOL RESOLUTION DUMP",
                "--dump-layout" => "LAYOUT DUMP",
                "--dump-code" => "CODE GENERATION DUMP",
                "--dump-elf" => "ELF DUMP",
                _ => panic!("unknown inspection option"),
            };
            assert!(listing.contains(heading), "{listing}");
        }
        assert_eq!(read(workspace.0.join("saved")).unwrap(), b"preserve me");
        assert!(!workspace.0.join("a.out").exists());
    }
    success(&workspace.run(&["assemble", "--dump-code"]));
    assert!(!workspace.0.join("a.out").exists());

    // ELF inspection requires the same entry symbol as saved executables.
    let output =
        workspace.run(&["assemble", "program.s", "--dump-elf", "-o", "saved"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("_start symbol not defined")
    );
    assert_eq!(read(workspace.0.join("saved")).unwrap(), b"preserve me");
    workspace.source("program.s", 0);
    let output = workspace.run(&[
        "assemble",
        "program.s",
        "-o",
        "saved",
        "--dump-ast",
        "--dump-symbols",
        "--dump-layout",
        "--dump-code",
        "--dump-elf",
    ]);
    success(&output);
    let listing = String::from_utf8_lossy(&output.stdout);
    assert!(listing.contains("ELF DUMP"));
    assert!(listing.contains("No output file generated"));
    assert_eq!(read(workspace.0.join("saved")).unwrap(), b"preserve me");
    assert!(!workspace.0.join("a.out").exists());
}

#[test]
fn elf_dumps_describe_the_completed_executable() {
    let workspace = Workspace::new();
    workspace.source("program.s", 0);
    success(&workspace.run(&["assemble", "program.s", "-o", "saved"]));
    let bytes = read(workspace.0.join("saved")).unwrap();
    let output = workspace.run(&["assemble", "program.s", "--dump-elf"]);
    success(&output);
    let listing = String::from_utf8_lossy(&output.stdout);
    let field = |label: &str| {
        listing
            .lines()
            .find_map(|line| {
                line.trim_start().strip_prefix(label).map(str::trim)
            })
            .expect("ELF dump field")
    };
    let word = |offset: usize| {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    };
    let halfword = |offset: usize| {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    };

    // The inspection must reflect the actual header, not constructor defaults.
    assert_eq!(field("Class:"), "ELF32");
    assert_eq!(field("Entry point address:"), format!("0x{:x}", word(24)));
    assert_eq!(
        field("Start of program headers:"),
        format!("{} (bytes into file)", word(28))
    );
    assert_eq!(
        field("Start of section headers:"),
        format!("{} (bytes into file)", word(32))
    );
    assert_eq!(field("Number of program headers:"), halfword(44).to_string());
    assert_eq!(field("Number of section headers:"), halfword(48).to_string());
    assert_eq!(
        field("Section header string table index:"),
        halfword(50).to_string()
    );
    assert!(listing.contains("LOAD"));
    assert!(listing.contains(".text"));
    assert!(listing.contains(".symtab"));
    assert!(listing.contains("_start"));
    assert!(!workspace.0.join("a.out").exists());
    assert_eq!(read(workspace.0.join("saved")).unwrap(), bytes);
}

#[test]
fn intermediate_code_dumps_follow_emitted_sizes_and_file_filters() {
    let workspace = Workspace::new();
    write(
        workspace.0.join("first.s"),
        ".text\nli a0, 1\n.data\n.space 3, 170\n",
    )
    .unwrap();
    write(
        workspace.0.join("second.s"),
        ".text\nli a1, 2\necall\n.data\n.byte 187\n.bss\n.space 5\n",
    )
    .unwrap();

    // Instructions shrink and data grows on the first pass. Both byte streams
    // must advance through the hidden file before reporting the selected file.
    for selector in ["--dump-code=1", "--dump-code=1:second.s", "--dump-code"] {
        let output =
            workspace.run(&["assemble", "first.s", "second.s", selector]);
        success(&output);
        let listing = String::from_utf8_lossy(&output.stdout);
        let li = listing
            .lines()
            .find(|line| line.contains("li      a1, 2"))
            .unwrap();
        assert!(li.contains("93 05 20 00"), "{listing}");
        let ecall =
            listing.lines().find(|line| line.contains("ecall")).unwrap();
        assert!(ecall.contains("73 00 00 00"), "{listing}");
        assert!(
            listing.lines().any(|line| {
                line.contains("[second.s:5]")
                    && line.split_whitespace().last() == Some("bb")
            }),
            "{listing}"
        );
        if selector.ends_with(":second.s") {
            assert!(!listing.contains("File: first.s"), "{listing}");
        }
    }
}

#[test]
fn dumps_stop_at_requested_phase_and_preserve_real_errors() {
    let workspace = Workspace::new();
    write(workspace.0.join("program.s"), ".text\ncall missing\n")
        .expect("write unresolved source");
    success(&workspace.run(&["assemble", "--dump-ast", "program.s"]));
    for dumps in [vec!["--dump-symbols"], vec!["--dump-ast", "--dump-code"]] {
        let mut args = vec!["assemble", "program.s"];
        args.extend(dumps);
        let output = workspace.run(&args);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("missing"));
    }
    write(workspace.0.join("program.s"), "not_an_instruction\n")
        .expect("write invalid source");
    assert_eq!(
        workspace.run(&["assemble", "--dump-ast", "program.s"]).status.code(),
        Some(1)
    );
}

#[test]
fn conflicting_inputs_and_options_remain_errors() {
    let workspace = Workspace::new();
    for args in [
        vec!["run", "-e", "a.out", "program.s"],
        vec!["run", "program.s", "a.out"],
        vec!["run", "one", "two"],
        vec!["run", "--dump-ast", "program.s"],
        vec!["assemble", "--check-abi", "program.s"],
        vec!["run", "-o", "saved", "program.s"],
        vec!["-t", "0x10000", "assemble", "program.s"],
        vec!["run", "--steps", "invalid", "program.s"],
        vec!["assemble", "-o"],
        vec!["run", "-e"],
    ] {
        let output = workspace.run(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(!output.stderr.is_empty());
    }
    // The assemble command explicitly treats even extensionless paths as source.
    workspace.source("source", 7);
    success(&workspace.run(&["assemble", "source"]));
    assert_eq!(workspace.run(&["a.out"]).status.code(), Some(7));
}
