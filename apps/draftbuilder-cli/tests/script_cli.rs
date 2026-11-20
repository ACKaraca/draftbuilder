//! The `script` subcommand of the CLI, run as the binary: the report on standard output (and in a
//! file with `--report`), the save of the package, the stop at the failing line with its error and
//! its exit code, the usage errors, and the roots and the edit permission the flags give.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{Harness, fill};
use draftbuilder_commands::script;
use serde_json::Value;

const W01: &str = include_str!("data/w01.scr");
const W01_NO_ARRAY: &str = include_str!("data/w01_no_array.scr");
const TEMPLATE: &[u8] = include_bytes!("data/team_metric_template.dxf");

/// The CLI with `args`.
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_draftbuilder-cli"))
        .args(args)
        .output()
        .unwrap()
}

/// The last line of standard error, parsed: the error object of a failed run.
fn last_error(output: &Output) -> Value {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let line = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap();
    serde_json::from_str(line).unwrap()
}

/// The arguments of `script` for the package, the script and the directory as the roots.
fn script_args<'a>(
    package: &'a str,
    script: &'a str,
    root: &'a str,
    extra: &[&'a str],
) -> Vec<&'a str> {
    let mut args = vec![
        "script",
        package,
        "--script",
        script,
        "--create",
        "--read-root",
        root,
        "--write-root",
        root,
    ];
    args.extend_from_slice(extra);
    args
}

#[test]
fn script_runs_the_workflow_writes_the_report_and_saves_the_package() {
    let dir = Harness::new();
    let template = dir.write("team_metric_template.dxf", TEMPLATE);
    let output = dir.path("out.dxf");
    let script_path = dir.write(
        "w01.scr",
        fill(
            W01_NO_ARRAY,
            &[("template", &template), ("output", &output)],
        )
        .as_bytes(),
    );
    let package = dir.path("part.draftb");
    let report_path = dir.path("report.json");
    let root = dir.dir.to_str().unwrap().to_owned();
    let (package, script_path, report_path) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
        report_path.to_str().unwrap().to_owned(),
    );
    let mut args = script_args(&package, &script_path, &root, &["--save", "--allow-edit"]);
    args.extend(["--report", &report_path]);
    let run = cli(&args);
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let stdout: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert!(stdout["failure"].is_null());
    assert_eq!(
        stdout["totals"]["commands"], 20,
        "the array-free workflow completes twenty commands, the save among them"
    );
    assert_eq!(stdout["reports"][0]["command"], "draftbuilder.drawing.save");
    assert_eq!(stdout["reports"][0]["report"]["header"]["format"], "dxf");
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
    assert_eq!(
        file, stdout,
        "the report file is the report on standard output"
    );
    assert!(output.exists(), "the drawing is written");
    assert!(Path::new(&package).exists(), "the package is saved");
}

#[test]
fn script_stops_at_the_failing_line_and_exits_with_the_error_category() {
    let dir = Harness::new();
    let template = dir.write("team_metric_template.dxf", TEMPLATE);
    let output = dir.path("out.dxf");
    let text = fill(W01, &[("template", &template), ("output", &output)]);
    let script_path = dir.write("w01.scr", text.as_bytes());
    let last = script::lines(&text).last().unwrap().number;
    let package = dir.path("part.draftb");
    let root = dir.dir.to_str().unwrap().to_owned();
    let (package, script_path) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
    );
    let run = cli(&script_args(
        &package,
        &script_path,
        &root,
        &["--save", "--allow-edit"],
    ));

    assert_eq!(
        run.status.code(),
        Some(4),
        "unsupported: the array cannot be saved"
    );
    let error = last_error(&run);
    assert_eq!(
        error["error"]["data"]["code"],
        "db.cmd.export_drops_objects"
    );
    assert_eq!(error["error"]["data"]["details"]["line"], last.to_string());
    let stdout: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(stdout["failure"]["line"], last);
    assert_eq!(stdout["failure"]["code"], "db.cmd.export_drops_objects");
    assert!(!output.exists(), "nothing is written");
    assert!(
        !Path::new(&package).exists(),
        "a failed run leaves no package behind"
    );
}

#[test]
fn script_without_its_flags_exits_with_a_usage_error() {
    let dir = Harness::new();
    let root = dir.dir.to_str().unwrap().to_owned();
    let package = dir.path("part.draftb");
    let package = package.to_str().unwrap();
    let no_script = cli(&["script", package, "--create", "--read-root", &root]);
    assert_eq!(no_script.status.code(), Some(2));
    assert_eq!(
        last_error(&no_script)["error"]["data"]["code"],
        "cmd.cli.usage"
    );

    let unknown = cli(&["script", package, "--script", package, "--bogus"]);
    assert_eq!(unknown.status.code(), Some(2));
}

#[test]
fn script_edits_need_the_edit_permission_of_the_flags() {
    let dir = Harness::new();
    let template = dir.write("team_metric_template.dxf", TEMPLATE);
    let output = dir.path("out.dxf");
    let script_path = dir.write(
        "w01.scr",
        fill(
            W01_NO_ARRAY,
            &[("template", &template), ("output", &output)],
        )
        .as_bytes(),
    );
    let package = dir.path("part.draftb");
    let root = dir.dir.to_str().unwrap().to_owned();
    let (package, script_path) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
    );
    let run = cli(&script_args(&package, &script_path, &root, &[]));
    assert_eq!(
        run.status.code(),
        Some(5),
        "denied: the ceiling has no edit"
    );
    let stdout: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(
        stdout["failure"]["line"], 7,
        "the first edit is the new drawing"
    );
    assert_eq!(stdout["failure"]["code"], "cmd.permission_denied");
    assert!(!output.exists());
}

#[test]
fn script_reads_its_file_only_through_a_read_root() {
    let dir = Harness::new();
    let outside = Harness::new();
    let script_path = outside.write("w01.scr", b"NEW unit=meter\n");
    let root = dir.dir.to_str().unwrap().to_owned();
    let package = dir.path("part.draftb");
    let (package, script_path) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
    );
    let run = cli(&script_args(
        &package,
        &script_path,
        &root,
        &["--allow-edit"],
    ));
    assert_eq!(run.status.code(), Some(5));
    assert_eq!(
        last_error(&run)["error"]["data"]["code"],
        "cmd.path.outside_roots"
    );
}

#[test]
fn the_report_path_is_checked_before_the_script_runs() {
    // The script would save a file; a report outside the write roots is refused before anything runs.
    let dir = Harness::new();
    let outside = Harness::new();
    let output = dir.path("out.dxf");
    let script_path = dir.write(
        "s.scr",
        format!(
            "NEW unit=meter\nLINE\n0,0\n1,0\n\nSAVE path={}\n",
            common::quoted(&output)
        )
        .as_bytes(),
    );
    let package = dir.path("part.draftb");
    let root = dir.dir.to_str().unwrap().to_owned();
    let report = outside.path("report.json");
    let (package, script_path, report) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
        report.to_str().unwrap().to_owned(),
    );
    let mut args = script_args(&package, &script_path, &root, &["--save", "--allow-edit"]);
    args.extend(["--report", &report]);
    let run = cli(&args);
    assert_eq!(
        run.status.code(),
        Some(5),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        last_error(&run)["error"]["data"]["code"],
        "cmd.path.outside_roots"
    );
    assert!(!output.exists(), "the script did not run");
    assert!(!Path::new(&package).exists());
}

#[test]
fn an_existing_package_is_checked_against_the_read_roots() {
    let dir = Harness::new();
    let outside = Harness::new();
    let package = outside.path("part.draftb");
    let root = dir.dir.to_str().unwrap().to_owned();
    let script_path = dir.write("s.scr", b"NEW unit=meter\n");
    let (package, script_path) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
    );
    let run = cli(&[
        "script",
        &package,
        "--script",
        &script_path,
        "--read-root",
        &root,
        "--write-root",
        &root,
        "--allow-edit",
    ]);
    assert_eq!(
        run.status.code(),
        Some(5),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        last_error(&run)["error"]["data"]["code"],
        "cmd.path.outside_roots"
    );
}

#[test]
fn a_script_file_over_the_size_cap_is_refused_before_it_runs() {
    let dir = Harness::new();
    let cap = draftbuilder_commands::script::MAX_SCRIPT_BYTES;
    let script_path = dir.path("big.scr");
    std::fs::write(&script_path, vec![b'\n'; cap + 1]).unwrap();
    let package = dir.path("part.draftb");
    let root = dir.dir.to_str().unwrap().to_owned();
    let (package, script_path) = (
        package.to_str().unwrap().to_owned(),
        script_path.to_str().unwrap().to_owned(),
    );
    let run = cli(&script_args(
        &package,
        &script_path,
        &root,
        &["--allow-edit"],
    ));
    assert_eq!(
        run.status.code(),
        Some(6),
        "limit exceeded: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        last_error(&run)["error"]["data"]["code"],
        "db.cmd.script_limit"
    );
    assert!(!Path::new(&package).exists());
}
