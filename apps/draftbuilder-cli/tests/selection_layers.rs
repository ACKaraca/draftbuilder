//! Selection, layers and properties through the automation protocol of the built binary: typed
//! names run their commands, a selection becomes the session's selection (`selection.get`), a
//! later command uses it as the previous set, and undo steps back a property change.

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

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

#[test]
fn selection_layers_and_properties_run_through_the_protocol() {
    let dir = TempDir::new("protocol-selection");
    let package = dir.path("work.draftb");
    let mut session = Session::start(&dir, &package);

    // The typed name of the new-drawing command runs it; its result names the model space.
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
    let crossing = line(&mut session, [15.0, 15.0], [30.0, 15.0]);
    let outside = line(&mut session, [40.0, 40.0], [50.0, 50.0]);
    let ids = |value: &Value| -> Vec<String> {
        sorted(
            value["objects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect(),
        )
    };

    // A window selection returns the set; selecting changes nothing in the document.
    let window = session.run(
        "draftbuilder.select.window",
        json!({"first": [-1.0, -1.0], "second": [20.0, 20.0]}),
    );
    assert_eq!(window["picked"], json!(1), "{window}");
    assert_eq!(ids(&window), sorted(vec![inside.clone()]));

    // Add mode combines the crossing set with the session's current selection (the window's set).
    let added = session.run(
        "draftbuilder.select.crossing",
        json!({"first": [14.0, 14.0], "second": [16.0, 16.0], "mode": "add"}),
    );
    assert_eq!(added["picked"], json!(1), "{added}");
    assert_eq!(ids(&added), sorted(vec![inside.clone(), crossing.clone()]));

    // The previous set is the session's set before the last selection: the window's set.
    let previous = session.run("draftbuilder.select.previous", json!({}));
    assert_eq!(previous["objects"], json!([inside.clone()]), "{previous}");

    // The typed FILTER name selects by type over the whole drawing.
    let lines = session.run(
        "FILTER",
        json!({"filter": {"test": {"property": "type", "op": "equal", "value": "line"}}}),
    );
    assert_eq!(
        ids(&lines),
        sorted(vec![inside.clone(), crossing.clone(), outside.clone()]),
        "{lines}"
    );

    // The layer of the outside line is turned off through the typed LAYER and LAYOFF names: the
    // all selection then leaves it out.
    session.run("LAYER", json!({"name": "Hidden"}));
    session.run(
        "draftbuilder.layer.change",
        json!({"objects": [outside.clone()], "layer": "Hidden"}),
    );
    session.run("LAYOFF", json!({"objects": [outside.clone()]}));
    let all = session.run("draftbuilder.select.all", json!({}));
    assert_eq!(
        ids(&all),
        sorted(vec![inside.clone(), crossing.clone()]),
        "{all}"
    );

    // CHPROP sets a color by index; a color filter finds it.
    session.run("CHPROP", json!({"objects": [inside.clone()], "color": 2}));
    let yellow = session.run(
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "color", "op": "equal", "value": "#ffff00"}}}),
    );
    assert_eq!(ids(&yellow), sorted(vec![inside.clone()]), "{yellow}");

    // Selections are not undo steps: ONE undo reverts the property change, and the color is gone.
    let undone = session.call("history.undo", json!({"document": "work"}));
    assert_eq!(
        undone["command"],
        json!("draftbuilder.property.change"),
        "{undone}"
    );
    let none = session.run(
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "color", "op": "equal", "value": "#ffff00"}}}),
    );
    assert_eq!(none["objects"], json!([]), "{none}");

    session.finish();
}
