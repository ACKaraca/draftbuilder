//! The drawing commands through the protocol: a rectangle, a circle and a polyline with an arc
//! segment are drawn with `command.run`, saved as DXF, and the saved file holds those entities.

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

#[test]
fn rectangle_circle_and_arc_polyline_draw_and_save_through_the_protocol() {
    let dir = TempDir::new("draw");
    let package = dir.path("work.draftb");
    let mut session = Session::start(&dir, &package);
    session.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    session.run(
        "draftbuilder.draw.rectangle",
        json!({"shape": {"mode": "corners", "first": [0.0, 0.0], "other": [4.0, 2.0]}}),
    );
    session.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [2.0, 1.0], "radius": 0.5}}),
    );
    session.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [
            {"point": [6.0, 3.0], "bulge": 1.0},
            {"point": [8.0, 3.0]},
            {"point": [10.0, 3.0]}
        ]}),
    );
    session.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": dir.path("drawn.dxf")}),
    );
    session.finish();
    let text = std::fs::read_to_string(dir.path("drawn.dxf")).unwrap();
    let kinds: Vec<&str> = text.lines().map(str::trim).collect();
    let count = |name: &str| kinds.iter().filter(|line| **line == name).count();
    assert_eq!(count("LWPOLYLINE"), 2, "rectangle and polyline");
    assert_eq!(count("CIRCLE"), 1);
}
