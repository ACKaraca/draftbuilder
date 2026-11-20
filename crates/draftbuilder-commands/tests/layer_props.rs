//! Color, pattern and property summary commands, headless: true colors on layers and objects,
//! the continuous dash pattern of polylines (`CHPROP`), and the property summary (`PROPERTIES`).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_doc::{ColorSource, Value};
use arc_draft::types::{entity, layer};
use common::Env;
use serde_json::{Value as Json, json};

fn drawing(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({}));
}

fn line_on(env: &mut Env, name: &str) -> String {
    let space = env.space().to_string();
    let id = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [5.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [id.clone()], "layer": name}),
    );
    id
}

fn polyline(env: &mut Env) -> String {
    env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

fn value_of(env: &Env, id: &str, index: usize) -> Option<Value> {
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id.parse().unwrap()).unwrap();
    snapshot.get(handle).unwrap().values[index].clone()
}

fn code_of(result: Result<Json, arc_base::Error>) -> &'static str {
    result.unwrap_err().code()
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

#[test]
fn a_true_color_is_stored_as_rgb_without_a_palette_source() {
    let mut env = Env::new();
    drawing(&mut env);
    let line = line_on(&mut env, "0");
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [line.clone()], "color": [255, 0, 0]}),
    );
    match value_of(&env, &line, entity::COLOR) {
        Some(Value::Color(color)) => {
            assert!((color.rgba.r - 1.0).abs() < 1e-6);
            assert!(color.rgba.g.abs() < 1e-6 && color.rgba.b.abs() < 1e-6);
            assert_eq!(color.source, None);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        value_of(&env, &line, entity::COLOR_BY),
        Some(Value::Enum(2))
    );
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [line.clone()], "color": 1}),
    );
    match value_of(&env, &line, entity::COLOR) {
        Some(Value::Color(color)) => assert_eq!(color.source, Some(ColorSource::Indexed(1))),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_layer_takes_a_true_color_and_refuses_index_zero() {
    let mut env = Env::new();
    drawing(&mut env);
    let created = env.run(
        "draftbuilder.layer.create",
        json!({"name": "Blue", "color": [0, 0, 255]}),
    );
    let id = created["layers"][0].as_str().unwrap().to_owned();
    match value_of(&env, &id, layer::COLOR) {
        Some(Value::Color(color)) => {
            assert!((color.rgba.b - 1.0).abs() < 1e-6);
            assert_eq!(color.source, None);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.create",
            json!({"name": "Zero", "color": 0})
        )),
        "db.cmd.value_invalid"
    );
}

#[test]
fn continuous_pattern_sets_polylines_and_leaves_other_entities_alone() {
    let mut env = Env::new();
    drawing(&mut env);
    let poly = polyline(&mut env);
    let line = line_on(&mut env, "0");
    assert_eq!(
        value_of(&env, &poly, 17),
        Some(Value::Bool(false)),
        "the default restarts at each vertex"
    );
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [poly.clone(), line.clone()], "continuous_pattern": true}),
    );
    assert_eq!(
        value_of(&env, &poly, 17),
        Some(Value::Bool(true)),
        "polyline flag set"
    );
    undo(&mut env);
    assert_eq!(value_of(&env, &poly, 17), Some(Value::Bool(false)));
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.change",
            json!({"objects": [line], "continuous_pattern": true}),
        )),
        "draft.object.wrong_type"
    );
}

#[test]
fn summary_lists_shared_properties_and_the_properties_that_differ() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let a = line_on(&mut env, "A");
    let b = line_on(&mut env, "B");
    let summary = env.run(
        "draftbuilder.property.summary",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert_eq!(summary["objects"].as_array().unwrap().len(), 2);
    assert_eq!(summary["shared"]["general.kind"], json!("core.draft.line"));
    assert_eq!(summary["shared"]["appearance.color"], json!("by_layer"));
    assert!(
        summary["mixed"]
            .as_array()
            .unwrap()
            .contains(&json!("general.layer"))
    );
    assert!(summary["shared"].get("general.layer").is_none());
}

#[test]
fn summary_filters_to_one_object_type() {
    let mut env = Env::new();
    drawing(&mut env);
    let line = line_on(&mut env, "0");
    let poly = polyline(&mut env);
    let filtered = env.run(
        "draftbuilder.property.summary",
        json!({"objects": [line.clone(), poly.clone()], "kind": "line"}),
    );
    assert_eq!(filtered["objects"], json!([line]));
    assert_eq!(filtered["mixed"], json!([]));
    assert_eq!(
        env.run(
            "draftbuilder.property.summary",
            json!({"objects": [line, poly], "kind": "Polyline"})
        )["objects"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
