//! The viewport half of the M0 exit criterion, headless: the opened drawing is drawn to a PNG
//! through the drafting extractor of `arc-gpu`. The test skips cleanly when the machine has no
//! usable graphics adapter (on Linux, install `mesa-vulkan-drivers`).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests"
)]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::json;

const BIN: &str = env!("CARGO_BIN_EXE_draftbuilder-cli");
const FIXTURE: &str = include_str!("../../../crates/draftbuilder-commands/tests/data/m0.dxf");

#[test]
fn the_opened_drawing_renders_to_a_png() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("draftbuilder-render-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m0.dxf"), FIXTURE).unwrap();
    let package = dir.join("work.draftb");
    let root = dir.to_str().unwrap();

    let mut session = Command::new(BIN)
        .args([
            "serve",
            "--stdio",
            "--create",
            package.to_str().unwrap(),
            "--save",
            "--allow-edit",
        ])
        .args(["--read-root", root, "--write-root", root])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = session.stdin.take().unwrap();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "arc.initialize",
        "params": {"clientName": "t", "protocolVersions": {"min": "1.0", "max": "1.0"},
                   "permissions": ["read", "edit", "file_read"]}});
    let open = json!({"jsonrpc": "2.0", "id": 2, "method": "command.run",
        "params": {"document": "work", "command": "draftbuilder.drawing.open_dxf",
                   "params": {"path": dir.join("m0.dxf").to_str().unwrap()}}});
    writeln!(stdin, "{init}\n{open}").unwrap();
    // Wait for the open to finish before the session ends and saves.
    let mut from = BufReader::new(session.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(from.read_line(&mut line).unwrap() > 0, "session ended");
        if line.contains("\"id\":2") {
            assert!(line.contains("\"result\""), "{line}");
            break;
        }
    }
    drop(stdin);
    assert!(session.wait().unwrap().success());

    let png = dir.join("drawing.png");
    let output = Command::new(BIN)
        .args([
            "render",
            package.to_str().unwrap(),
            "--output",
            png.to_str().unwrap(),
        ])
        .args(["--width", "400", "--height", "300", "--write-root", root])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() && stderr.contains("gpu.adapter.not_found") {
        eprintln!("skipped: no usable graphics adapter");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    assert!(output.status.success(), "{stderr}");
    let image = image::open(&png).unwrap().to_rgba8();
    assert_eq!((image.width(), image.height()), (400, 300));
    let dark = image
        .pixels()
        .filter(|p| p.0[..3].iter().any(|c| *c < 200))
        .count();
    assert!(dark > 200, "the drawing is visible: {dark} drawn pixels");
    let _ = std::fs::remove_dir_all(&dir);
}
