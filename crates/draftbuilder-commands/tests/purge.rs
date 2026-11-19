//! The purge command, headless: categories, previews that match a run, repeats through nested
//! references, protected names, preserved records, and undo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "test code"
)]

mod common;

use arc_draft::style::STYLES;
use arc_draft::types::{BLOCK, LAYER};
use common::{Env, FIXTURE, name_of};
use serde_json::{Value, json};

fn removed_names(result: &Value) -> Vec<String> {
    result["removed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap_or("").to_owned())
        .collect()
}

fn names_of(env: &Env, key: &str) -> Vec<String> {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(key)
        .unwrap()
        .filter_map(|(_, o)| name_of(&snapshot, o.id))
        .collect()
}

/// Makes block `MID` contain a reference to block `INNER`, which contains a line. Neither is used.
fn nested_blocks(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({}));
    let space = env.space().to_string();
    let line = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [5.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "core.draft.block.define",
        json!({"name": "INNER", "objects": [line]}),
    );
    let inner_id = env
        .document
        .snapshot()
        .iter_type(BLOCK)
        .unwrap()
        .find(|(_, o)| name_of(&env.document.snapshot(), o.id).as_deref() == Some("INNER"))
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    let reference = env.run(
        "core.draft.block.insert",
        json!({"owner": space, "block": inner_id, "position": [10.0, 0.0]}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "core.draft.block.define",
        json!({"name": "MID", "objects": [reference]}),
    );
}

#[test]
fn a_preview_with_repeat_lists_everything_a_run_with_repeat_removes() {
    let mut env = Env::new();
    nested_blocks(&mut env);
    let before = env.document.snapshot().revision();
    let preview = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["blocks"], "repeat": true, "preview": true}),
    );
    assert_eq!(
        env.document.snapshot().revision(),
        before,
        "a preview changes nothing"
    );
    let listed = removed_names(&preview);
    assert!(
        listed.contains(&"MID".to_owned()) && listed.contains(&"INNER".to_owned()),
        "{preview}"
    );

    let run = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["blocks"], "repeat": true}),
    );
    assert_eq!(
        removed_names(&run),
        listed,
        "the run removes what the preview listed"
    );
    assert!(run["passes"].as_u64().unwrap() >= 2, "{run}");
    assert!(
        !names_of(&env, BLOCK)
            .iter()
            .any(|n| n == "MID" || n == "INNER")
    );
}

#[test]
fn without_repeat_a_nested_block_is_purged_one_level_at_a_time_and_undo_restores_it() {
    let mut env = Env::new();
    nested_blocks(&mut env);
    let run = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["blocks"]}),
    );
    assert_eq!(removed_names(&run), ["MID"], "{run}");
    assert!(names_of(&env, BLOCK).iter().any(|n| n == "INNER"));
    assert!(env.document.undo().wait().unwrap().value);
    assert!(names_of(&env, BLOCK).iter().any(|n| n == "MID"));
}

#[test]
fn a_dimension_style_that_uses_a_text_style_frees_it_on_the_next_pass() {
    let mut env = Env::new();
    env.run(
        "core.draft.style.create",
        json!({"kind": "text", "name": "TSPARE"}),
    );
    let text_style = env
        .document
        .snapshot()
        .iter_type(STYLES[0])
        .unwrap()
        .find(|(_, o)| name_of(&env.document.snapshot(), o.id).as_deref() == Some("TSPARE"))
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    env.run(
        "core.draft.style.create",
        json!({"kind": "dimension", "name": "DSPARE"}),
    );
    let dim = env
        .document
        .snapshot()
        .iter_type(STYLES[1])
        .unwrap()
        .find(|(_, o)| name_of(&env.document.snapshot(), o.id).as_deref() == Some("DSPARE"))
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    env.run(
        "core.draft.style.modify",
        json!({"objects": [dim], "field": "text_style", "value": text_style}),
    );
    let preview = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["styles"], "repeat": true, "preview": true}),
    );
    let listed = removed_names(&preview);
    assert!(
        listed.contains(&"DSPARE".to_owned()) && listed.contains(&"TSPARE".to_owned()),
        "{preview}"
    );
    let run = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["styles"], "repeat": true}),
    );
    assert!(removed_names(&run).contains(&"TSPARE".to_owned()), "{run}");
}

#[test]
fn a_layer_that_uses_a_linetype_frees_it_on_the_next_pass() {
    // DASHED is named only by the fixture's table: once the layer that uses it is gone, the
    // linetype is unused, so a second pass removes it.
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let dashed = env
        .document
        .snapshot()
        .iter_type(arc_draft::types::LINETYPE)
        .unwrap()
        .find(|(_, o)| name_of(&env.document.snapshot(), o.id).as_deref() == Some("DASHED"))
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    env.run(
        "core.draft.layer.create",
        json!({"name": "LSPARE", "linetype": dashed}),
    );
    let once = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["layers", "linetypes"]}),
    );
    assert_eq!(removed_names(&once), ["LSPARE"], "{once}");
    assert!(env.document.undo().wait().unwrap().value);
    let repeated = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["layers", "linetypes"], "repeat": true}),
    );
    let removed = removed_names(&repeated);
    assert!(
        removed.contains(&"LSPARE".to_owned()) && removed.contains(&"DASHED".to_owned()),
        "{repeated}"
    );
}

#[test]
fn the_protected_names_are_kept_per_category() {
    let mut env = Env::new();
    env.run("core.draft.layer.create", json!({"name": "Defpoints"}));
    let run = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["layers"], "repeat": true}),
    );
    let removed = removed_names(&run);
    assert!(
        !removed.contains(&"0".to_owned()) && !removed.contains(&"Defpoints".to_owned()),
        "{run}"
    );
    assert!(names_of(&env, LAYER).contains(&"Defpoints".to_owned()));
}

#[test]
fn a_layer_named_only_in_a_preserved_record_is_kept() {
    // The layer SPARE is named by an entity type the reader does not know; only that record uses
    // the layer, which the reader keeps as preserved data.
    let text = "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n  5\n10\n  0\nLAYER\n  5\n11\n  2\nSPARE\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n  0\nENDTAB\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nZZCUSTOM\n  5\n1A\n  8\nSPARE\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  0\nENDSEC\n  0\nEOF\n";
    let mut env = Env::new();
    let path = env.write("preserved.dxf", text.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let run = env.run(
        "draftbuilder.drawing.purge",
        json!({"categories": ["layers"], "repeat": true}),
    );
    assert!(!removed_names(&run).contains(&"SPARE".to_owned()), "{run}");
    assert!(names_of(&env, LAYER).contains(&"SPARE".to_owned()));
}

#[test]
fn purge_removes_only_what_is_unused_in_the_fixture() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let run = env.run("draftbuilder.drawing.purge", json!({"repeat": true}));
    // DASHED is named only by the table: unused, so it is the one object the default categories
    // remove. The entities and the other tables keep their objects.
    assert_eq!(removed_names(&run), ["DASHED"], "{run}");
    assert_eq!(run["capped"], false);
}

#[test]
fn purge_refuses_unknown_categories_at_the_parameter_schema() {
    let mut env = Env::new();
    let error = env
        .invoke(
            "draftbuilder.drawing.purge",
            json!({"categories": ["empty_text"]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "cmd.params.invalid");
}
