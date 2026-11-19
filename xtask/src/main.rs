//! Repository tasks of DraftBuilder, run as `cargo xtask <task>`:
//!
//! - `layers` checks the workspace against `xtask/layers.toml`.
//! - `catalogs` checks that each crate's message identifiers and its localization catalog in
//!   `locales/en-US` are the same set.
//! - `ci` runs every gate in order; it is the single command CI runs: format, clippy, layers,
//!   tests (nextest and documentation tests), `cargo deny`, `cargo audit` and the catalog check.

// A command-line tool reports on the terminal.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod catalog;
mod layers;
mod workspace;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use workspace::Failure;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "layers" => run_layers(),
        "catalogs" => run_catalogs(),
        "ci" => run_ci(),
        _ => Err("usage: cargo xtask <layers|catalogs|ci>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("xtask {task}: {failure}");
            ExitCode::FAILURE
        }
    }
}

fn run_layers() -> Result<(), Failure> {
    let table = workspace::load_table()?;
    let facts = workspace::load_facts()?;
    let violations = layers::check(&table, &facts);
    for violation in &violations {
        eprintln!("{violation}");
    }
    if violations.is_empty() {
        println!(
            "layers: {} crates respect the layer table",
            facts.workspace.len()
        );
        Ok(())
    } else {
        Err(format!("{} layering violation(s)", violations.len()))
    }
}

fn run_ci() -> Result<(), Failure> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ])?;
    run_layers()?;
    cargo(&["nextest", "run", "--workspace", "--all-features"])?;
    // nextest does not run documentation tests.
    cargo(&["test", "--workspace", "--doc"])?;
    cargo(&["deny", "check"])?;
    cargo(&["audit"])?;
    run_catalogs()?;
    println!("ci: all gates passed");
    Ok(())
}

fn run_catalogs() -> Result<(), Failure> {
    let members = workspace::load_members()?;
    let dir = workspace::root().join("locales").join("en-US");
    let mut problems = Vec::new();
    let mut checked = 0;
    for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if path.extension().is_some_and(|e| e == "ftl") && !members.iter().any(|m| m.name == stem) {
            problems.push(format!("{}: catalog without a crate", path.display()));
        }
    }
    for member in &members {
        let path = dir.join(format!("{}.ftl", member.name));
        let text = fs::read_to_string(&path).ok();
        let mut ids = Vec::new();
        for file in rust_files(&member.dir.join("src"))? {
            let source =
                fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            ids.extend(catalog::referenced(&source));
        }
        checked += usize::from(text.is_some());
        problems.extend(catalog::check(&member.name, text.as_deref(), &ids));
    }
    report(
        "catalogs",
        checked,
        "catalog(s) match their crates",
        problems,
    )
}

fn report(task: &str, checked: usize, ok: &str, problems: Vec<String>) -> Result<(), Failure> {
    for problem in &problems {
        eprintln!("{problem}");
    }
    if problems.is_empty() {
        println!("{task}: {checked} {ok}");
        Ok(())
    } else {
        Err(format!("{} problem(s) in {task}", problems.len()))
    }
}

/// Every `.rs` file below `dir`; a missing directory has none.
fn rust_files(dir: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(out);
    };
    for entry in entries {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.is_dir() {
            out.extend(rust_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(out)
}

fn cargo(args: &[&str]) -> Result<(), Failure> {
    let line = format!("cargo {}", args.join(" "));
    println!("==> {line}");
    let status = Command::new(workspace::cargo())
        .args(args)
        .current_dir(workspace::root())
        .status()
        .map_err(|e| format!("{line}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{line}` failed ({status})"))
    }
}
