//! The hidden `bench` subcommand on a tiny drawing: the report has the structure its module documents.
//! No time is asserted; the timings are measured on reference hardware, not in a test.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_draftbuilder-cli");

#[test]
fn bench_report_has_its_structure() {
    let dir = std::env::temp_dir().join(format!("draftbuilder-cli-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let output = Command::new(BIN)
        .args([
            "bench",
            "--entities",
            "100",
            "--reps",
            "2",
            "--format",
            "json",
            "--work",
        ])
        .arg(&dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["entities"], 100);
    assert_eq!(report["entities_in_opened_drawing"], 100);
    assert_eq!(report["reps"], 2);
    assert_eq!(report["build_reps"], 2);
    let measurements = report["measurements"].as_array().unwrap();
    let names: Vec<&str> = measurements
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    for name in [
        "generate",
        "save_dxf",
        "open_dxf",
        "move_selection.commit",
        "copy_selection.undo.settled",
        "erase_selection.redo.commit",
        "move_one_entity.settled",
    ] {
        assert!(names.contains(&name), "missing series {name}");
    }
    for m in measurements {
        assert_eq!(m["samples"].as_array().unwrap().len(), 2, "{}", m["name"]);
        assert!(m["min"].as_f64().unwrap() <= m["median"].as_f64().unwrap());
    }
    assert!(
        report["not_measured"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["target"] == "pan and zoom")
    );
}
