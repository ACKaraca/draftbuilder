//! Viewport layer overrides, headless: freezing layers in layout viewports (`LAYVPI`, `VPLAYER`)
//! and replacing or clearing a layer's appearance inside one viewport. Each command's exact
//! override records, its refusals, and undo.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_doc::Value;
use arc_draft::layout::{overrides_of, viewport};
use arc_draft::types::LAYER;
use common::{Env, name_of};
use serde_json::{Value as Json, json};

/// A drawing with a line on `Walls`, a layer `Dims`, and two viewports of one layout.
/// Returns the line and the viewports.
fn setup(env: &mut Env) -> (String, String, String) {
    env.run("draftbuilder.drawing.new", json!({}));
    let space = env.space().to_string();
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    env.run("draftbuilder.layer.create", json!({"name": "Dims"}));
    let line = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [5.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Walls"}),
    );
    let layout = env.run(
        "core.draft.layout.create",
        json!({"name": "sheet", "paper_width": 297.0, "paper_height": 210.0}),
    );
    let mut viewports = Vec::new();
    for center in [[100.0, 100.0], [200.0, 100.0]] {
        viewports.push(
            env.run(
                "core.draft.viewport.create",
                json!({"layout": layout["space"].clone(), "model": space,
                       "center": center, "width": 50.0, "height": 40.0}),
            )["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let second = viewports.pop().unwrap();
    let first = viewports.pop().unwrap();
    (line, first, second)
}

fn layer_id(env: &Env, name: &str) -> arc_base::ObjectId {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .map(|(_, o)| o.id)
        .find(|id| name_of(&snapshot, *id).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no layer {name}"))
}

/// The overrides of viewport `id`, read through the foundation's accessor.
fn overrides(env: &Env, id: &str) -> arc_draft::layout::Overrides {
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id.parse().unwrap()).unwrap();
    let values = snapshot.get(handle).unwrap().values.clone();
    overrides_of(&values)
}

fn frozen_in(env: &Env, id: &str, name: &str) -> bool {
    overrides(env, id).frozen.contains(&layer_id(env, name))
}

fn code_of(result: Result<Json, arc_base::Error>) -> &'static str {
    result.unwrap_err().code()
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

#[test]
fn viewport_isolate_freezes_the_objects_layers_in_every_other_viewport() {
    let mut env = Env::new();
    let (line, keep, other) = setup(&mut env);
    env.run(
        "draftbuilder.layer.viewport_isolate",
        json!({"objects": [line], "viewport": keep.clone()}),
    );
    assert!(!frozen_in(&env, &keep, "Walls"));
    assert!(frozen_in(&env, &other, "Walls"));
    assert!(!frozen_in(&env, &other, "Dims"));
    undo(&mut env);
    assert!(!frozen_in(&env, &other, "Walls"));
}

#[test]
fn viewport_freeze_freezes_thaws_and_resets_named_layers() {
    let mut env = Env::new();
    let (_, first, second) = setup(&mut env);
    env.run(
        "draftbuilder.layer.viewport_freeze",
        json!({"action": "freeze", "viewports": [first.clone(), second.clone()], "layers": ["Dims"]}),
    );
    assert!(frozen_in(&env, &first, "Dims"));
    assert!(frozen_in(&env, &second, "Dims"));

    env.run(
        "draftbuilder.layer.viewport_freeze",
        json!({"action": "thaw", "viewports": [first.clone()], "layers": ["Dims"]}),
    );
    assert!(!frozen_in(&env, &first, "Dims"));
    assert!(frozen_in(&env, &second, "Dims"));

    env.run(
        "draftbuilder.layer.viewport_freeze",
        json!({"action": "reset", "viewports": [second.clone()]}),
    );
    assert!(!frozen_in(&env, &second, "Dims"));
    assert!(overrides(&env, &second).layers.is_empty());
}

#[test]
fn viewport_freeze_refuses_the_current_layer_and_unknown_or_missing_names() {
    let mut env = Env::new();
    let (_, first, _) = setup(&mut env);
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_freeze",
            json!({"action": "freeze", "viewports": [first.clone()], "layers": ["0"]}),
        )),
        "db.cmd.current_layer"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_freeze",
            json!({"action": "freeze", "viewports": [first.clone()], "layers": ["Nope"]}),
        )),
        "db.cmd.layer_unknown"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_freeze",
            json!({"action": "freeze", "viewports": [first.clone()]}),
        )),
        "db.cmd.nothing_to_change"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_freeze",
            json!({"action": "reset", "viewports": [first], "layers": ["Dims"]}),
        )),
        "db.cmd.params_conflict"
    );
}

#[test]
fn viewport_override_replaces_color_and_clear_keeps_the_freeze() {
    let mut env = Env::new();
    let (_, first, _) = setup(&mut env);
    env.run(
        "draftbuilder.layer.viewport_override",
        json!({"viewports": [first.clone()], "layers": ["Walls"], "color": [255, 0, 0],
               "transparency": 0.5}),
    );
    let record = overrides(&env, &first);
    let walls = layer_id(&env, "Walls");
    let entry = record.layers.get(&walls).unwrap();
    let color = entry.color.as_ref().unwrap();
    assert!((color.rgba.r - 1.0).abs() < 1e-6 && color.rgba.g.abs() < 1e-6);
    assert_eq!(color.source, None);
    assert_eq!(entry.transparency, Some(0.5));

    env.run(
        "draftbuilder.layer.viewport_freeze",
        json!({"action": "freeze", "viewports": [first.clone()], "layers": ["Walls"]}),
    );
    env.run(
        "draftbuilder.layer.viewport_clear",
        json!({"viewports": [first.clone()], "layers": ["Walls"]}),
    );
    let after = overrides(&env, &first);
    assert!(after.frozen.contains(&walls), "the freeze stays");
    if let Some(entry) = after.layers.get(&walls) {
        assert!(entry.color.is_none() && entry.transparency.is_none());
    }
}

#[test]
fn viewport_clear_without_layers_removes_every_appearance_and_empty_records() {
    let mut env = Env::new();
    let (_, first, _) = setup(&mut env);
    env.run(
        "draftbuilder.layer.viewport_override",
        json!({"viewports": [first.clone()], "layers": ["Walls", "Dims"], "lineweight": 0.5}),
    );
    assert_eq!(overrides(&env, &first).layers.len(), 2);
    env.run(
        "draftbuilder.layer.viewport_clear",
        json!({"viewports": [first.clone()]}),
    );
    assert!(overrides(&env, &first).layers.is_empty());
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(first.parse().unwrap()).unwrap();
    match &snapshot.get(handle).unwrap().values[viewport::LAYER_OVERRIDES] {
        Some(Value::List(items)) => assert!(items.is_empty(), "{items:?}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn viewport_override_refuses_continuous_bad_values_and_wrong_objects() {
    let mut env = Env::new();
    let (line, first, _) = setup(&mut env);
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_override",
            json!({"viewports": [first.clone()], "layers": ["Walls"], "linetype": "Continuous"}),
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_override",
            json!({"viewports": [first.clone()], "layers": ["Walls"], "transparency": 1.5}),
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_override",
            json!({"viewports": [first.clone()], "layers": ["Walls"], "color": 0}),
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.viewport_override",
            json!({"viewports": [line], "layers": ["Walls"], "lineweight": 0.5}),
        )),
        "draft.object.wrong_type"
    );
    assert!(overrides(&env, &first).layers.is_empty());
}

#[test]
fn viewport_override_is_one_undoable_step() {
    let mut env = Env::new();
    let (_, first, _) = setup(&mut env);
    env.run(
        "draftbuilder.layer.viewport_override",
        json!({"viewports": [first.clone()], "layers": ["Walls"], "color": 2}),
    );
    assert_eq!(overrides(&env, &first).layers.len(), 1);
    undo(&mut env);
    assert!(overrides(&env, &first).layers.is_empty());
}
