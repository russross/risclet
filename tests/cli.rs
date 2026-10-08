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
    // Inspection does not require the executable's entry point.
    write(workspace.0.join("program.s"), ".text\nlocal:\nli a0, 0\n")
        .expect("write inspection source");
    write(workspace.0.join("saved"), b"preserve me")
        .expect("write existing output");
    for dumps in [
        vec!["--dump-ast"],
        vec!["--dump-symbols"],
        vec!["--dump-values"],
        vec!["--dump-code"],
        vec!["--dump-elf"],
        vec![
            "--dump-ast",
            "--dump-symbols",
            "--dump-values",
            "--dump-code",
            "--dump-elf",
        ],
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
                "--dump-values" => "SYMBOL VALUES DUMP",
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
