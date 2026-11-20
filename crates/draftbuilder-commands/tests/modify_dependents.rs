//! Dependents and pieces, headless: an entity another entity refers to (a path array's path) is never
//! erased silently when a command changes its kind, and a split keeps the identifier of the piece that has
//! the source's kind, creating the others beside the source in draw order.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_base::{Limits, ObjectId, Unit};
use arc_doc::Value;
use arc_draft::types::entity;
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use common::Env;
use serde_json::json;

fn setup(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
}

fn exists(env: &Env, id: &str) -> bool {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    snapshot.resolve(wanted).is_ok()
}

fn curve_of(env: &Env, id: &str) -> Curve2 {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    let object = snapshot.get(handle).unwrap();
    let key = snapshot.type_key(handle).unwrap().as_str().to_owned();
    match Reader::new(snapshot.registry(), &key, &object.values).get("geometry") {
        Some(Value::Geometry(bytes)) => Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap(),
        _ => panic!("no geometry"),
    }
}

fn draw_order(env: &Env, id: &str) -> i64 {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    match snapshot.get(handle).unwrap().values.get(entity::DRAW_ORDER) {
        Some(Some(Value::I64(order))) => *order,
        other => panic!("no draw order: {other:?}"),
    }
}

#[test]
fn breaking_a_circle_that_a_path_array_uses_as_its_path_is_refused_and_nothing_changes() {
    let mut env = Env::new();
    setup(&mut env);
    let circle = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 5.0}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let source = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[20.0, 0.0], [21.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.modify.array_path",
        json!({"objects": [source], "path": circle.clone(), "count": 3}),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.break",
            json!({"object": circle.clone(), "first": [5.0, 0.0], "second": [0.0, 5.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.erase_dependent", "{error:?}");
    assert!(
        exists(&env, &circle),
        "the circle the array uses is still there"
    );
    assert!(matches!(curve_of(&env, &circle), Curve2::Circle(_)));
}

#[test]
fn a_split_keeps_the_identifier_of_the_piece_of_the_sources_kind_and_places_the_rest_beside_it() {
    let mut env = Env::new();
    setup(&mut env);
    let ray = env.run(
        "draftbuilder.draw.ray",
        json!({"start": [0.0, 0.0], "through": [[1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let later = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[30.0, 0.0], [31.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[2.0, -1.0], [2.0, 1.0]]}),
    );
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[5.0, -1.0], [5.0, 1.0]]}),
    );
    // Two edges cut the ray at x = 2 and x = 5; the pick at x = 3.5 removes the stretch between them.
    let edges: Vec<String> = env
        .document
        .snapshot()
        .iter_type(arc_draft::types::LINE)
        .unwrap()
        .map(|(_, o)| o.id.to_string())
        .filter(|id| *id != later)
        .collect();
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({"edges": edges, "targets": [{"object": ray.clone(), "pick": [3.5, 0.0]}]}),
    );
    let ids: Vec<String> = result["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 2, "{result}");
    assert!(
        ids.contains(&ray),
        "the ray keeps its identifier as the piece of its kind"
    );
    let other = ids.iter().find(|id| **id != ray).unwrap();
    assert!(matches!(curve_of(&env, other), Curve2::Segment(_)));
    assert!(matches!(curve_of(&env, &ray), Curve2::Ray(_)));
    // The created segment is placed beside the ray, not at the top of the drawing.
    assert!(draw_order(&env, other) < draw_order(&env, &later));
    assert!(draw_order(&env, other) > draw_order(&env, &ray));
}

fn array_count(env: &Env) -> usize {
    env.document
        .snapshot()
        .iter_type(arc_draft::array::ARRAY)
        .map(Iterator::count)
        .unwrap_or(0)
}

#[test]
fn an_array_is_refused_when_another_entity_refers_to_its_source() {
    let mut env = Env::new();
    setup(&mut env);
    let path = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 5.0], [10.0, 5.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let source = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    // A path array refers to `path`: arraying `path` would move it out from under that reference.
    env.run(
        "draftbuilder.modify.array_path",
        json!({"objects": [source], "path": path.clone(), "count": 2}),
    );
    let before = array_count(&env);
    let error = env
        .invoke(
            "draftbuilder.modify.array_rect",
            json!({"objects": [path.clone()], "rows": 1, "columns": 2, "column_spacing": 3.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.array_dependent", "{error:?}");
    assert_eq!(array_count(&env), before);
    assert_eq!(curve_of_line(&env, &path), ([0.0, 5.0], [10.0, 5.0]));
}

fn curve_of_line(env: &Env, id: &str) -> ([f64; 2], [f64; 2]) {
    match curve_of(env, id) {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_array_of_sources_in_two_owners_is_refused_and_nothing_changes() {
    let mut env = Env::new();
    setup(&mut env);
    let inside = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let outside = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[5.0, 5.0], [6.0, 5.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "core.draft.block.define",
        json!({"name": "HOLDER", "objects": [inside.clone()]}),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.array_rect",
            json!({"objects": [inside, outside.clone()], "rows": 1, "columns": 2, "column_spacing": 3.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.array_owner_mixed", "{error:?}");
    assert_eq!(array_count(&env), 0);
    assert_eq!(curve_of_line(&env, &outside), ([5.0, 5.0], [6.0, 5.0]));
}

#[test]
fn an_array_of_sources_in_one_block_is_created_in_that_block() {
    let mut env = Env::new();
    setup(&mut env);
    let a = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let b = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 1.0], [1.0, 1.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let defined = env.run(
        "core.draft.block.define",
        json!({"name": "PARTS", "objects": [a.clone(), b.clone()]}),
    );
    let block = defined["id"].as_str().unwrap().to_owned();
    let result = env.run(
        "draftbuilder.modify.array_rect",
        json!({"objects": [a, b], "rows": 1, "columns": 2, "column_spacing": 3.0}),
    );
    let array: ObjectId = result["objects"][0].as_str().unwrap().parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(array).unwrap();
    assert_eq!(
        snapshot.get(handle).unwrap().header.owner,
        Some(block.parse().unwrap()),
        "the array is created in the block that owns its sources"
    );
}

#[test]
fn an_array_whose_count_exceeds_the_item_limit_is_refused_before_anything_changes() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.array_rect",
            json!({"objects": [source], "rows": 10000, "columns": 2,
                   "row_spacing": 1.0, "column_spacing": 1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid", "{error:?}");
    assert_eq!(array_count(&env), 0);
}

#[test]
fn a_split_polyline_piece_keeps_the_kind_fields_of_its_source() {
    let mut env = Env::new();
    setup(&mut env);
    let polyline = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [polyline.clone()], "field": "continuous_pattern", "value": true}),
    );
    let result = env.run(
        "draftbuilder.modify.break",
        json!({"object": polyline.clone(), "first": [5.0, 0.0], "second": [10.0, 5.0]}),
    );
    let ids: Vec<String> = result["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 2, "{result}");
    let created = ids.iter().find(|id| **id != polyline).unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(created.parse().unwrap()).unwrap();
    let object = snapshot.get(handle).unwrap();
    let reader = Reader::new(
        snapshot.registry(),
        arc_draft::types::POLYLINE,
        &object.values,
    );
    assert_eq!(reader.get("continuous_pattern"), Some(&Value::Bool(true)));
}
