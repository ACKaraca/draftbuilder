//! End to end through the built binary: a package is created, a DXF drawing is opened into it,
//! moved, the move undone and the drawing saved as DXF; the saved file carries the handles of
//! the source. Then the shared subcommands run on the package.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

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

/// A running `session` subcommand.
struct Script {
    child: Child,
    to: ChildStdin,
    from: BufReader<ChildStdout>,
}

impl Script {
    fn start(dir: &TempDir, file: &str) -> Self {
        let root = dir.0.to_str().unwrap();
        let mut child = Command::new(BIN)
            .args([
                "session",
                file,
                "--save",
                "--allow-edit",
                "--read-root",
                root,
                "--write-root",
                root,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let to = child.stdin.take().unwrap();
        let from = BufReader::new(child.stdout.take().unwrap());
        Self { child, to, from }
    }

    fn send(&mut self, request: &Value) -> Value {
        writeln!(self.to, "{request}").unwrap();
        self.to.flush().unwrap();
        let mut line = String::new();
        self.from.read_line(&mut line).unwrap();
        let answer: Value =
            serde_json::from_str(&line).unwrap_or_else(|_| panic!("no answer: {line:?}"));
        answer["ok"].clone()
    }

    fn finish(self) -> Output {
        let Self { child, to, from } = self;
        drop(to);
        drop(from);
        child.wait_with_output().unwrap()
    }
}

/// The handles of the entities of a DXF file, in file order, by the group 5 that follows an
/// entity type in the ENTITIES section.
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
fn open_move_undo_save_through_the_cli() {
    let dir = TempDir::new("e2e");
    std::fs::write(dir.0.join("m0.dxf"), FIXTURE).unwrap();
    let package = dir.path("work.draftb");
    let mut script = Script::start(&dir, &package);

    let opened = script.send(&json!({"command": "draftbuilder.drawing.open_dxf",
                                     "params": {"path": dir.path("m0.dxf")}}));
    assert_eq!(opened["handles"], 6, "{opened}");
    let entities = script.send(&json!({"list": "entities"}));
    let list = entities.as_array().unwrap();
    assert_eq!(list.len(), 6);
    let handles: Vec<&str> = list.iter().map(|e| e["handle"].as_str().unwrap()).collect();
    assert_eq!(handles, ["100", "101", "102", "103", "104", "109"]);
    assert!(
        list.iter()
            .all(|e| e["stable_id"].as_str().is_some_and(|s| s.len() == 36))
    );

    let ids: Vec<&str> = list.iter().map(|e| e["id"].as_str().unwrap()).collect();
    script.send(&json!({"command": "draftbuilder.modify.move",
                        "params": {"objects": ids, "offset": [25.0, -10.0]}}));
    assert_eq!(script.send(&json!({"undo": true}))["changed"], true);
    assert_eq!(
        script.send(&json!({"undo": true}))["changed"],
        true,
        "the open is undoable too"
    );
    assert_eq!(script.send(&json!({"redo": true}))["changed"], true);
    assert_eq!(
        script
            .send(&json!({"list": "entities"}))
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let saved = script.send(&json!({"command": "draftbuilder.drawing.save_dxf",
                                    "params": {"path": dir.path("saved.dxf")}}));
    assert_eq!(saved["handles_carried"], 6, "{saved}");
    let output = script.finish();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let saved = std::fs::read_to_string(dir.0.join("saved.dxf")).unwrap();
    assert_eq!(entity_handles(&saved), entity_handles(FIXTURE));

    // The shared subcommands see the product's commands and the package.
    let commands = Command::new(BIN).arg("commands").output().unwrap();
    let text = String::from_utf8(commands.stdout).unwrap();
    for id in [
        "draftbuilder.drawing.open_dxf",
        "draftbuilder.drawing.save_dxf",
        "draftbuilder.modify.move",
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
fn a_failing_request_stops_the_session_with_its_error_code() {
    let dir = TempDir::new("fail");
    let mut script = Script::start(&dir, &dir.path("work.draftb"));
    writeln!(
        script.to,
        "{}",
        json!({"command": "draftbuilder.modify.move",
                                      "params": {"objects": [], "offset": [1.0, 1.0]}})
    )
    .unwrap();
    let output = script.finish();
    assert_eq!(output.status.code(), Some(3), "invalid parameters");
    assert!(String::from_utf8_lossy(&output.stderr).contains("cmd.params.invalid"));
}
