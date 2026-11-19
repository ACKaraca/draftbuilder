//! The name forms of a command through the CLI: every form of a typed name runs the command it
//! names, and an unknown name fails with `cmd.unknown`. Each run creates its own package, so the
//! same command can run in every form.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_draftbuilder-cli");

/// A directory removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("draftbuilder-names-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The arguments that create a package in `dir` and run `--command` `name` on it.
fn args_for(dir: &TempDir, package: &str) -> Vec<String> {
    let root = dir.0.to_str().unwrap().to_owned();
    vec![
        "run".to_owned(),
        dir.path(package),
        "--create".to_owned(),
        "--save".to_owned(),
        "--allow-edit".to_owned(),
        "--read-root".to_owned(),
        root.clone(),
        "--write-root".to_owned(),
        root,
        "--params".to_owned(),
        "{}".to_owned(),
    ]
}

/// Runs the command `name` (in any form) on a new package named `package` in `dir`.
fn run_form(dir: &TempDir, package: &str, name: &str) -> Output {
    Command::new(BIN)
        .args(args_for(dir, package))
        .args(["--command", name])
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn every_name_form_of_a_typed_command_runs_it() {
    let dir = TempDir::new("forms");
    // The typed name NEW creates an empty drawing; every form of it runs the same command.
    for (n, form) in ["NEW", "new", "_NEW", "-NEW", ".NEW"].iter().enumerate() {
        let output = run_form(&dir, &format!("p{n}.draftb"), form);
        assert!(
            output.status.success(),
            "{form}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("draftbuilder.drawing.new"), "{form}: {text}");
    }
    // The same form written as `--command=NAME`.
    let output = Command::new(BIN)
        .args(args_for(&dir, "equals.draftb"))
        .arg("--command=-NEW")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn an_identifier_in_a_name_form_runs_the_command() {
    let dir = TempDir::new("ids");
    let output = run_form(&dir, "ids.draftb", "_draftbuilder.drawing.new");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn an_unknown_name_in_any_form_fails_with_cmd_unknown() {
    let dir = TempDir::new("unknown");
    for (n, form) in ["FROBNICATE", "_FROBNICATE", "-FROBNICATE", ".FROBNICATE"]
        .iter()
        .enumerate()
    {
        let output = run_form(&dir, &format!("u{n}.draftb"), form);
        assert!(!output.status.success(), "{form}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("cmd.unknown"),
            "{form}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
