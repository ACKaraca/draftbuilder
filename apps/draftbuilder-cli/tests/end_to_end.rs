//! End to end through the built binary, with the shared runner only: a package is created
//! (`--create`), a DXF drawing is opened into it, moved, the move undone with `history.undo`
//! and the drawing saved as DXF over the protocol; the saved file carries the handles of the
//! source. Then the shared subcommands run on the package.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_draftbuilder-cli");
const FIXTURE: &str = include_str!("../../../crates/draftbuilder-commands/tests/data/m0.dxf");

/// A directory removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("draftbuilder-cli-{name}-{}", std::process::id()));
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

/// A protocol session with the binary.
struct Session {
    child: Child,
    from: BufReader<ChildStdout>,
    next: u64,
}

impl Session {
    fn start(dir: &TempDir, package: &str) -> Self {
        let root = dir.0.to_str().unwrap();
        let mut child = Command::new(BIN)
            .args([
                "serve",
                "--stdio",
                "--create",
                package,
                "--save",
                "--allow-edit",
            ])
            .args(["--read-root", root, "--write-root", root])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let from = BufReader::new(child.stdout.take().unwrap());
        let mut session = Self {
            child,
            from,
            next: 0,
        };
        let r = session.call(
            "arc.initialize",
            json!({"clientName": "e2e", "protocolVersions": {"min": "1.0", "max": "1.0"},
                   "permissions": ["read", "edit", "file_read", "file_write"]}),
        );
        assert!(r.get("permissions").is_some(), "{r}");
        session
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let request =
            json!({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params});
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{request}").unwrap();
        stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(self.from.read_line(&mut line).unwrap() > 0, "session ended");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == json!(self.next) {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
        }
    }

    fn run(&mut self, command: &str, params: Value) -> Value {
        let r = self.call(
            "command.run",
            json!({"document": "work", "command": command, "params": params}),
        );
        r["value"].clone()
    }

    fn finish(mut self) {
        self.call("arc.shutdown", json!({}));
        drop(self.child.stdin.take());
        let status = self.child.wait().unwrap();
        let mut stderr = String::new();
        std::io::Read::read_to_string(self.child.stderr.as_mut().unwrap(), &mut stderr).unwrap();
        assert!(status.success(), "{status}: {stderr}");
    }
}

/// The handles of the entities of a DXF file's ENTITIES section, in file order.
fn entity_handles(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut in_entities = false;
    let mut out = Vec::new();
    let mut want = false;
    for pair in lines.chunks(2) {
        let [code, value] = pair else { break };
        match (*code, *value) {
            ("2", "ENTITIES") => in_entities = true,
            ("0", "ENDSEC") => in_entities = false,
            ("0", "VERTEX" | "SEQEND") => want = false,
            ("0", _) if in_entities => want = true,
            ("5", v) if in_entities && want => {
                out.push(v.to_owned());
                want = false;
            }
            _ => {}
        }
    }
    out
}

#[test]
fn open_move_undo_save_through_the_shared_runner() {
    let dir = TempDir::new("e2e");
    std::fs::write(dir.0.join("m0.dxf"), FIXTURE).unwrap();
    let package = dir.path("work.draftb");
    let mut session = Session::start(&dir, &package);

    let opened = session.run(
        "draftbuilder.drawing.open_dxf",
        json!({"path": dir.path("m0.dxf")}),
    );
    assert_eq!(opened["handles"], 15, "{opened}");
    let queried = session.call(
        "query.objects",
        json!({"document": "work", "types": ["core.draft.line", "core.draft.arc", "core.draft.polyline"],
               "fields": ["geometry"]}),
    );
    let before = queried["objects"].clone();
    let ids: Vec<Value> = before
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].clone())
        .collect();
    assert!(ids.len() >= 5, "{queried}");

    session.run(
        "core.draft.entity.transform",
        json!({"objects": ids, "transform": {"translation": [25.0, -10.0]}}),
    );
    let moved = session.call(
        "query.objects",
        json!({"document": "work", "types": ["core.draft.line", "core.draft.arc", "core.draft.polyline"],
               "fields": ["geometry"]}),
    );
    assert_ne!(moved["objects"], before, "the move changed the geometry");
    let undone = session.call("history.undo", json!({"document": "work"}));
    assert_eq!(undone["command"], "core.draft.entity.transform", "{undone}");
    let restored = session.call(
        "query.objects",
        json!({"document": "work", "types": ["core.draft.line", "core.draft.arc", "core.draft.polyline"],
               "fields": ["geometry"]}),
    );
    assert_eq!(restored["objects"], before, "undo restores the geometry");

    let saved = session.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": dir.path("saved.dxf")}),
    );
    assert_eq!(saved["handles_kept"], 15, "{saved}");
    session.finish();

    let saved = std::fs::read_to_string(dir.0.join("saved.dxf")).unwrap();
    assert_eq!(entity_handles(&saved), entity_handles(FIXTURE));

    // The shared subcommands see the product's commands, and the package they saved is valid.
    let commands = Command::new(BIN).arg("commands").output().unwrap();
    let text = String::from_utf8(commands.stdout).unwrap();
    for id in [
        "draftbuilder.drawing.open_dxf",
        "draftbuilder.drawing.save_dxf",
        "core.draft.entity.transform",
    ] {
        assert!(text.contains(id), "{id}");
    }
    let validate = Command::new(BIN)
        .args(["validate", &package])
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );
}

#[test]
fn the_runner_installs_the_drafting_validators_and_reports_failures_by_category() {
    let dir = TempDir::new("fail");
    let root = dir.0.to_str().unwrap();
    let output = Command::new(BIN)
        .args([
            "run",
            &dir.path("work.draftb"),
            "--create",
            "--save",
            "--allow-edit",
        ])
        .args([
            "--command",
            "core.draft.layer.create",
            "--params",
            r#"{"name": "A"}"#,
        ])
        .args(["--read-root", root, "--write-root", root])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A second layer of the same name is refused by the drafting names validator, which only
    // the product's document setup installs; the failure is a conflict (exit 1).
    let output = Command::new(BIN)
        .args([
            "run",
            &dir.path("work.draftb"),
            "--save",
            "--allow-edit",
            "--seed",
            "2",
        ])
        .args([
            "--command",
            "core.draft.layer.create",
            "--params",
            r#"{"name": "A"}"#,
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("draft.name.taken"));
}

#[test]
fn typed_names_and_aliases_run_through_the_protocol() {
    let dir = TempDir::new("protocol-names");
    let package = dir.path("work.draftb");
    let mut session = Session::start(&dir, &package);
    // The conventional typed name of the new-drawing command runs it; the identifier is what
    // the result names.
    let created = session.call(
        "command.run",
        json!({"document": "work", "command": "NEW", "params": {}}),
    );
    assert_eq!(created["command"], "draftbuilder.drawing.new", "{created}");
    assert_eq!(created["value"]["unit"], "millimeter", "{created}");
    session.finish();
}
