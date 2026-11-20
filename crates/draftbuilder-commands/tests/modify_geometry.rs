//! The geometry modify commands, headless: trim and extend (with edges, quick mode and extended
//! edges), offset (side, through point, polyline corners), fillet and chamfer (pairs, keep mode,
//! sharp corners, polylines), break, join, lengthen and stretch, each with its exact geometry, the
//! identifiers kept or replaced, undo, and the refusals.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "test code"
)]

mod common;

use arc_base::{Limits, ObjectId, Unit};
use arc_doc::Value;
use arc_draft::types::ENTITIES;
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use common::Env;
use serde_json::{Value as Json, json};

fn setup(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
}

/// Every entity of the drawing: identifier, type key and curve (`None` when it has no curve).
fn entities(env: &Env) -> Vec<(ObjectId, &'static str, Option<Curve2>)> {
    let snapshot = env.document.snapshot();
    let mut out = Vec::new();
    for key in ENTITIES {
        let Ok(iter) = snapshot.iter_type(key) else {
            continue;
        };
        for (_, object) in iter {
            let reader = Reader::new(snapshot.registry(), key, &object.values);
            let curve = match reader.get("geometry") {
                Some(Value::Geometry(bytes)) => {
                    Some(Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap())
                }
                _ => None,
            };
            out.push((object.id, *key, curve));
        }
    }
    out
}

fn curve_of(env: &Env, id: &str) -> Option<Curve2> {
    let wanted: ObjectId = id.parse().unwrap();
    entities(env)
        .into_iter()
        .find(|(i, _, _)| *i == wanted)
        .and_then(|(_, _, c)| c)
}

fn count(env: &Env, key: &str) -> usize {
    entities(env).iter().filter(|(_, k, _)| *k == key).count()
}

fn segment_ends(curve: &Curve2) -> ([f64; 2], [f64; 2]) {
    match curve {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("not a segment: {other:?}"),
    }
}

fn ids_of(result: &Json) -> Vec<String> {
    result["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    let result = env.run("draftbuilder.draw.line", json!({"points": [a, b]}));
    result["ids"][0].as_str().unwrap().to_owned()
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

// ----- trim and extend --------------------------------------------------------------------------

#[test]
fn trim_between_two_edges_keeps_the_identifier_of_the_first_part_and_creates_the_second() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let e1 = line(&mut env, [3.0, -1.0], [3.0, 1.0]);
    let e2 = line(&mut env, [7.0, -1.0], [7.0, 1.0]);
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({"edges": [e1, e2], "targets": [{"object": target.clone(), "pick": [5.0, 0.0]}]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 2, "{result}");
    assert_eq!(ids[0], target, "the first part keeps the identifier");
    assert_eq!(
        segment_ends(&curve_of(&env, &ids[0]).unwrap()),
        ([0.0, 0.0], [3.0, 0.0])
    );
    assert_eq!(
        segment_ends(&curve_of(&env, &ids[1]).unwrap()),
        ([7.0, 0.0], [10.0, 0.0])
    );
    undo(&mut env);
    assert_eq!(count(&env, "core.draft.line"), 3);
}

#[test]
fn trim_at_the_end_removes_the_stretch_up_to_the_edge() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [3.0, -1.0], [3.0, 1.0]);
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({"edges": [edge], "targets": [{"object": target.clone(), "pick": [1.0, 0.0]}]}),
    );
    assert_eq!(ids_of(&result), vec![target.clone()]);
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([3.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn trim_quick_mode_uses_every_other_entity_as_an_edge() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    line(&mut env, [4.0, -1.0], [4.0, 1.0]);
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({"targets": [{"object": target.clone(), "pick": [8.0, 0.0]}]}),
    );
    assert_eq!(ids_of(&result), vec![target.clone()]);
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [4.0, 0.0])
    );
}

#[test]
fn trim_without_an_intersection_is_refused_by_the_foundation_and_changes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [3.0, 1.0], [3.0, 2.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.trim",
            json!({"edges": [edge], "targets": [{"object": target.clone(), "pick": [5.0, 0.0]}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "geom.edit.no_intersection");
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn trim_with_extended_edges_reaches_an_edge_that_does_not_touch_the_target() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [3.0, 1.0], [3.0, 2.0]);
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({"edges": [edge], "extend_edges": true,
               "targets": [{"object": target.clone(), "pick": [8.0, 0.0]}]}),
    );
    assert_eq!(ids_of(&result), vec![target.clone()]);
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [3.0, 0.0])
    );
}

#[test]
fn trim_of_a_locked_entity_is_refused_with_the_foundation_code() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [3.0, -1.0], [3.0, 1.0]);
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [target.clone()]}),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.trim",
            json!({"edges": [edge], "targets": [{"object": target, "pick": [8.0, 0.0]}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "draft.layer.locked");
}

#[test]
fn extend_lengthens_the_end_nearest_the_pick_to_the_edge() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [5.0, 0.0]);
    let edge = line(&mut env, [8.0, -1.0], [8.0, 1.0]);
    let result = env.run(
        "draftbuilder.modify.extend",
        json!({"edges": [edge.clone()], "targets": [{"object": target.clone(), "pick": [4.0, 0.0]}]}),
    );
    assert_eq!(ids_of(&result), vec![target.clone()]);
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [8.0, 0.0])
    );
    // The other end: an edge at x = -3 extends the start.
    let target2 = line(&mut env, [0.0, 0.0], [5.0, 0.0]);
    let left = line(&mut env, [-3.0, -1.0], [-3.0, 1.0]);
    env.run(
        "draftbuilder.modify.extend",
        json!({"edges": [left], "targets": [{"object": target2.clone(), "pick": [0.5, 0.0]}]}),
    );
    assert_eq!(
        segment_ends(&curve_of(&env, &target2).unwrap()),
        ([-3.0, 0.0], [5.0, 0.0])
    );
}

#[test]
fn extend_prompt_form_takes_edges_then_targets_and_picks() {
    use arc_command::{CommandLine, Submitted, Target};
    use std::collections::BTreeMap;
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [5.0, 0.0]);
    let edge = line(&mut env, [8.0, -1.0], [8.0, 1.0]);
    let mut prompt = CommandLine::new();
    let mut seed = 900;
    let mut submit = |env: &Env, text: &str| -> Submitted {
        seed += 1;
        let localized = BTreeMap::new();
        let t = Target {
            registry: &env.commands,
            document: &env.document,
            session: &env.session,
            selection: &[],
            localized: &localized,
            seed,
        };
        prompt.submit(&t, text).unwrap()
    };
    assert!(matches!(submit(&env, "extend"), Submitted::Prompt(_)));
    submit(&env, &edge);
    submit(&env, &target);
    submit(&env, "4,0");
    let Submitted::Ran(outcome) = submit(&env, "end") else {
        panic!("the command did not run");
    };
    assert_eq!(ids_of(&outcome.value), vec![target.clone()]);
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [8.0, 0.0])
    );
}

// ----- offset -----------------------------------------------------------------------------------

#[test]
fn offset_line_to_the_side_of_a_point_keeps_the_source_or_erases_it() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source.clone()], "distance": 2.0, "side": [5.0, 1.0]}),
    );
    assert_eq!(ids_of(&result).len(), 1);
    let copy = ids_of(&result)[0].clone();
    assert_eq!(
        segment_ends(&curve_of(&env, &copy).unwrap()),
        ([0.0, 2.0], [10.0, 2.0])
    );
    assert!(curve_of(&env, &source).is_some());
    // The other side: a point below gives the copy below the line.
    let below = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source.clone()], "distance": 2.0, "side": [5.0, -1.0]}),
    );
    let below = ids_of(&below)[0].clone();
    assert_eq!(
        segment_ends(&curve_of(&env, &below).unwrap()),
        ([0.0, -2.0], [10.0, -2.0])
    );
    // Erasing the source leaves only the copy.
    env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source.clone()], "distance": 1.0, "side": [5.0, 1.0], "erase_source": true}),
    );
    assert!(curve_of(&env, &source).is_none());
    undo(&mut env);
    assert!(curve_of(&env, &source).is_some());
}

#[test]
fn offset_through_a_point_takes_the_point_distance_and_side() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source], "through": [5.0, 4.0]}),
    );
    let copy = ids_of(&result)[0].clone();
    assert_eq!(
        segment_ends(&curve_of(&env, &copy).unwrap()),
        ([0.0, 4.0], [10.0, 4.0])
    );
}

#[test]
fn offset_of_a_circle_inward_and_outward() {
    let mut env = Env::new();
    setup(&mut env);
    let circle = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 5.0}}),
    );
    let circle = circle["ids"][0].as_str().unwrap().to_owned();
    let inner = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [circle.clone()], "distance": 2.0, "side": [0.0, 0.0]}),
    );
    let inner = ids_of(&inner)[0].clone();
    match curve_of(&env, &inner).unwrap() {
        Curve2::Circle(c) => assert!((c.radius() - 3.0).abs() < 1e-9, "{}", c.radius()),
        other => panic!("{other:?}"),
    }
    let outer = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [circle], "distance": 2.0, "side": [10.0, 0.0]}),
    );
    let outer = ids_of(&outer)[0].clone();
    match curve_of(&env, &outer).unwrap() {
        Curve2::Circle(c) => assert!((c.radius() - 7.0).abs() < 1e-9, "{}", c.radius()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn offset_of_a_polyline_closes_its_corner_by_extension() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    );
    let source = source["ids"][0].as_str().unwrap().to_owned();
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source], "distance": 1.0, "side": [5.0, 1.0]}),
    );
    let copy = ids_of(&result)[0].clone();
    let Curve2::Polyline(p) = curve_of(&env, &copy).unwrap() else {
        panic!("not a polyline");
    };
    let points: Vec<[f64; 2]> = p
        .vertices()
        .iter()
        .map(|v| [v.point.x, v.point.y])
        .collect();
    assert_eq!(points, vec![[0.0, 1.0], [9.0, 1.0], [9.0, 10.0]]);
}

#[test]
fn offset_refuses_a_distance_with_no_side_and_a_zero_distance() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.offset",
            json!({"objects": [source.clone()], "distance": 2.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
    let error = env
        .invoke(
            "draftbuilder.modify.offset",
            json!({"objects": [source], "distance": 0.0, "side": [5.0, 1.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid");
}

#[test]
fn trim_quick_mode_leaves_out_objects_on_a_frozen_layer() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let hidden = line(&mut env, [4.0, -1.0], [4.0, 1.0]);
    // The current layer cannot be frozen, so the hidden edge gets a layer of its own.
    env.run("draftbuilder.layer.create", json!({"name": "Hidden"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [hidden.clone()], "layer": "Hidden"}),
    );
    env.run("draftbuilder.layer.freeze", json!({"objects": [hidden]}));
    let error = env
        .invoke(
            "draftbuilder.modify.trim",
            json!({"targets": [{"object": target.clone(), "pick": [8.0, 0.0]}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "geom.edit.no_intersection");
    assert_eq!(
        segment_ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
}
