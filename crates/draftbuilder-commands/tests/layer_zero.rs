//! Layer 0 of a drawing: a new drawing holds it, and opening a file that has none creates it and
//! reports the creation; a file that has one is opened unchanged.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_draft::types::LAYER;
use common::{Env, FIXTURE, name_of, reasons};
use serde_json::json;

fn has_layer(env: &Env, name: &str) -> bool {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .any(|(_, o)| name_of(&snapshot, o.id).as_deref() == Some(name))
}

#[test]
fn opening_a_file_with_layer_zero_reports_nothing_about_it() {
    let mut env = Env::new();
    let path = env.write("with-zero.dxf", FIXTURE.as_bytes());
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(!reasons(&opened).contains(&"db.io.layer_zero_created".to_owned()));
    assert!(has_layer(&env, "0"));
}

#[test]
fn opening_a_file_without_layer_zero_creates_it_and_reports_it() {
    let mut env = Env::new();
    // The first layer record of the fixture is layer 0; renamed, the file has no layer 0.
    let text = FIXTURE.replacen("  2\n0\n 70\n0\n", "  2\nRenamed\n 70\n0\n", 1);
    assert_ne!(text, FIXTURE);
    let path = env.write("no-zero.dxf", text.as_bytes());
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(has_layer(&env, "Renamed"));
    assert!(has_layer(&env, "0"), "{opened}");
    assert!(
        reasons(&opened).contains(&"db.io.layer_zero_created".to_owned()),
        "{opened}"
    );
}

#[test]
fn layers_of_a_file_that_differ_only_by_case_are_kept_and_reported() {
    let mut env = Env::new();
    // NOTES renamed to walls: WALLS and walls are the same name under the product's fold.
    let text = FIXTURE.replacen("  2\nNOTES\n", "  2\nwalls\n", 1);
    assert_ne!(text, FIXTURE);
    let path = env.write("case-duplicates.dxf", text.as_bytes());
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    // The product reads the two names as one layer: WALLS stays, and no layer is named walls.
    assert!(
        has_layer(&env, "WALLS") && !has_layer(&env, "walls"),
        "{opened}"
    );
    assert!(
        reasons(&opened).contains(&"db.io.layer_name_case_duplicate".to_owned()),
        "{opened}"
    );
    // Clean fixture: nothing reported.
    let mut clean = Env::new();
    let path = clean.write("clean.dxf", FIXTURE.as_bytes());
    let opened = clean.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(!reasons(&opened).contains(&"db.io.layer_name_case_duplicate".to_owned()));
}
