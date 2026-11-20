//! The modify commands and the session selection through the automation protocol of the built binary:
//! a window and a crossing selection become the session's current and previous sets, `previous` (and
//! the prefixed command names `_MOVE` and `.MOVE`, which the product's name resolver reads) move the
//! entities of the previous set, and the moved entity is the one the earlier selection named.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_draftbuilder-cli");

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

/// A protocol session with the binary, over a new package.
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
            json!({"clientName": "selection", "protocolVersions": {"min": "1.0", "max": "1.0"},
                   "permissions": ["read", "edit", "file_read", "file_write"]}),
        );
        assert!(r.get("permissions").is_some(), "{r}");
        session
    }

    /// One request and its result; an error answer fails the test.
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

    /// Runs a command, by identifier or typed name, and returns its value.
    fn run(&mut self, command: &str, params: Value) -> Value {
        self.call(
            "command.run",
            json!({"document": "work", "command": command, "params": params}),
        )["value"]
            .clone()
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

#[test]
fn move_of_the_previous_selection_moves_the_object_the_earlier_window_selected() {
    let dir = TempDir::new("protocol-modify");
    let package = dir.path("work.draftb");
    let mut session = Session::start(&dir, &package);
    let created = session.run("NEW", json!({}));
    let space = created["space"].as_str().unwrap().to_owned();
    let line = |session: &mut Session, start: [f64; 2], end: [f64; 2]| -> String {
        session.run(
            "core.draft.entity.create",
            json!({"owner": space, "geometry": {"kind": "segment", "start": start, "end": end}}),
        )["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let inside = line(&mut session, [1.0, 1.0], [4.0, 4.0]);
    let far = line(&mut session, [15.0, 15.0], [30.0, 15.0]);

    // The window makes `inside` the current set; the crossing then makes `far` current and
    // `inside` previous.
    session.run(
        "draftbuilder.select.window",
        json!({"first": [-1.0, -1.0], "second": [6.0, 6.0]}),
    );
    session.run(
        "draftbuilder.select.crossing",
        json!({"first": [14.0, 14.0], "second": [16.0, 16.0]}),
    );

    // `previous` names the window's set, and the move (by its typed name, `_MOVE` and `.MOVE`) moves it.
    let moved = session.run(
        "draftbuilder.modify.move",
        json!({"objects": ["previous"], "displacement": [0.0, 5.0]}),
    );
    assert_eq!(moved["objects"], json!([inside.clone()]), "{moved}");
    let again = session.run(
        "_MOVE",
        json!({"objects": ["previous"], "displacement": [0.0, 0.0]}),
    );
    assert_eq!(again["objects"], json!([far.clone()]), "{again}");
    let built_in = session.run(
        ".MOVE",
        json!({"objects": [far.clone()], "displacement": [0.0, 0.0]}),
    );
    assert_eq!(built_in["objects"], json!([far]), "{built_in}");
    session.finish();
}

#[test]
fn a_part_is_drawn_and_edited_through_the_protocol_and_undone_step_by_step() {
    let dir = TempDir::new("protocol-part");
    let package = dir.path("work.draftb");
    let mut session = Session::start(&dir, &package);
    session.run("NEW", json!({}));

    // Draw an outline, a hole and its array, then round the outline and offset it.
    let outline = session.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [20.0, 0.0]},
                            {"point": [20.0, 10.0]}, {"point": [0.0, 10.0]}], "close": true}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let hole = session.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [5.0, 5.0], "radius": 1.0}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let array = session.run(
        "draftbuilder.modify.array_rect",
        json!({"objects": [hole], "rows": 1, "columns": 2, "column_spacing": 10.0}),
    );
    assert_eq!(array["objects"].as_array().unwrap().len(), 1, "{array}");
    let rounded = session.run(
        "draftbuilder.modify.fillet",
        json!({"polyline": outline.clone(), "radius": 2.0}),
    );
    assert_eq!(rounded["objects"], json!([outline.clone()]), "{rounded}");
    let offset = session.run(
        "draftbuilder.modify.offset",
        json!({"objects": [outline], "distance": 1.0, "side": [10.0, 5.0]}),
    );
    assert_eq!(offset["objects"].as_array().unwrap().len(), 1, "{offset}");

    // The offset, then the fillet, are undone one step each; the outline is back to its corners.
    let undone = session.call("history.undo", json!({"document": "work"}));
    assert_eq!(undone["command"], "draftbuilder.modify.offset", "{undone}");
    let undone = session.call("history.undo", json!({"document": "work"}));
    assert_eq!(undone["command"], "draftbuilder.modify.fillet", "{undone}");
    session.finish();
}
