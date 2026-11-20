//! Handles across saves: the layer 0 of a new drawing keeps its handle when the file is saved again,
//! the handles of deleted or merged-away objects are not given to later objects, and each saved
//! file opens again.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_draft::types::LAYER;
use common::{Env, line_handle_at, named_handle};
use serde_json::json;

/// Saves the document to `name` in the environment's directory and returns the file's text.
fn save(env: &mut Env, name: &str) -> String {
    let path = env.path(name);
    env.run("draftbuilder.drawing.save_dxf", json!({"path": path}));
    std::fs::read_to_string(env.dir.join(name)).unwrap()
}

/// Opens a saved file in a fresh environment: the file must be valid.
fn reopens(file: &str, text: &str) {
    let mut other = Env::new();
    std::fs::write(other.dir.join(file), text).unwrap();
    let path = other.path(file);
    other.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
}

/// The identifier of the layer named `name`.
fn layer_id(env: &Env, name: &str) -> String {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .find(|(_, o)| {
            arc_draft_dxf::fields::name_of(&snapshot, Some(o.id)).as_deref() == Some(name)
        })
        .map(|(_, o)| o.id.to_string())
        .unwrap_or_else(|| panic!("no layer {name}"))
}

/// Creates a line from `(x, 0)` to `(x + 1, 0)` and returns its identifier.
fn line(env: &mut Env, x: f64) -> String {
    let space = env.space().to_string();
    env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [x, 0.0], "end": [x + 1.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn layer_zero_of_a_new_drawing_keeps_its_handle_across_saves() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let first = save(&mut env, "first.dxf");
    let handle =
        named_handle(&first, "LAYER", "0").expect("layer 0 has a handle in the first save");
    reopens("first.dxf", &first);

    let mut again = Env::new();
    std::fs::copy(env.dir.join("first.dxf"), again.dir.join("first.dxf")).unwrap();
    let path = again.path("first.dxf");
    again.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    let second = save(&mut again, "second.dxf");
    assert_eq!(
        named_handle(&second, "LAYER", "0"),
        Some(handle),
        "{second}"
    );
}

#[test]
fn the_handle_of_a_deleted_entity_is_not_given_to_a_later_entity() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let gone = line(&mut env, 1.0);
    let first = save(&mut env, "first.dxf");
    let deleted = line_handle_at(&first, 1.0).expect("the first line has a handle");
    env.run("core.draft.entity.delete", json!({"objects": [gone]}));
    line(&mut env, 7.0);
    let second = save(&mut env, "second.dxf");
    let later = line_handle_at(&second, 7.0).expect("the later line has a handle");
    assert_ne!(later, deleted, "{second}");
    assert_eq!(
        line_handle_at(&second, 1.0),
        None,
        "the deleted line is not written"
    );
    reopens("second.dxf", &second);
}

#[test]
fn the_handle_of_a_merged_away_layer_is_not_given_to_a_later_layer() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    env.run("draftbuilder.layer.create", json!({"name": "Src"}));
    env.run("draftbuilder.layer.create", json!({"name": "Dst"}));
    let first = save(&mut env, "first.dxf");
    let merged_away = named_handle(&first, "LAYER", "Src").expect("Src has a handle");
    let (src, dst) = (layer_id(&env, "Src"), layer_id(&env, "Dst"));
    env.run(
        "draftbuilder.layer.merge",
        json!({"sources": [src], "target": dst}),
    );
    env.run("draftbuilder.layer.create", json!({"name": "Later"}));
    let second = save(&mut env, "second.dxf");
    let later = named_handle(&second, "LAYER", "Later").expect("Later has a handle");
    assert_ne!(later, merged_away, "{second}");
    assert_eq!(named_handle(&second, "LAYER", "Src"), None, "{second}");
    reopens("second.dxf", &second);
}

#[test]
fn a_save_after_a_delete_with_contents_is_a_valid_file() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let kept = line(&mut env, 1.0);
    env.run("draftbuilder.layer.create", json!({"name": "Temp"}));
    let doomed = line(&mut env, 5.0);
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [doomed.clone()], "layer": "Temp"}),
    );
    let temp = layer_id(&env, "Temp");
    env.run(
        "draftbuilder.layer.delete_with_contents",
        json!({"layers": [temp]}),
    );
    let text = save(&mut env, "after.dxf");
    assert!(named_handle(&text, "LAYER", "Temp").is_none(), "{text}");
    assert!(line_handle_at(&text, 1.0).is_some(), "{text}");
    assert!(
        env.document
            .snapshot()
            .resolve(kept.parse().unwrap())
            .is_ok()
    );
    reopens("after.dxf", &text);
}
