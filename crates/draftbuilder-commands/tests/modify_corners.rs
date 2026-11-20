//! Corners, breaks, joins, lengths, stretches and explodes, headless: fillet and chamfer with their
//! joints and trimmed inputs, sharp corners, polyline forms, break at one and two points, join into
//! the simplest kind, lengthen of segments and arcs, stretch of what a window crosses, and explode of
//! polylines and block references, with undo and the refusals.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "test code"
)]

mod common;

use std::f64::consts::{FRAC_PI_2, PI};

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

fn exists(env: &Env, id: &str) -> bool {
    let wanted: ObjectId = id.parse().unwrap();
    entities(env).iter().any(|(i, _, _)| *i == wanted)
}

fn count(env: &Env, key: &str) -> usize {
    entities(env).iter().filter(|(_, k, _)| *k == key).count()
}

fn ends(curve: &Curve2) -> ([f64; 2], [f64; 2]) {
    match curve {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("not a segment: {other:?}"),
    }
}

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
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

fn quarter_arc(env: &mut Env, start: [f64; 2], end: [f64; 2]) -> String {
    let result = env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_end", "start": start, "center": [0.0, 0.0], "end": end}}),
    );
    result["ids"][0].as_str().unwrap().to_owned()
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

// ----- fillet and chamfer -----------------------------------------------------------------------

#[test]
fn fillet_of_two_lines_trims_both_and_joins_them_with_an_arc_tangent_to_both() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [0.0, 10.0]);
    let result = env.run(
        "draftbuilder.modify.fillet",
        json!({"first": {"object": a.clone(), "pick": [5.0, 0.0]},
               "second": {"object": b.clone(), "pick": [0.0, 5.0]}, "radius": 2.0}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 3, "{result}");
    assert_eq!(ids[1], a, "the first input keeps its identifier");
    assert_eq!(ids[2], b, "the second input keeps its identifier");
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([2.0, 0.0], [10.0, 0.0])
    );
    assert_eq!(
        ends(&curve_of(&env, &b).unwrap()),
        ([0.0, 2.0], [0.0, 10.0])
    );
    match curve_of(&env, &ids[0]).unwrap() {
        Curve2::Arc(arc) => {
            assert!((arc.center().x - 2.0).abs() < 1e-9 && (arc.center().y - 2.0).abs() < 1e-9);
            assert!((arc.radius() - 2.0).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    undo(&mut env);
    assert_eq!(count(&env, "core.draft.arc"), 0);
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn fillet_keep_mode_creates_the_joint_and_changes_no_input() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [0.0, 10.0]);
    env.run(
        "draftbuilder.modify.fillet",
        json!({"first": {"object": a.clone(), "pick": [5.0, 0.0]},
               "second": {"object": b.clone(), "pick": [0.0, 5.0]}, "radius": 2.0, "trim": false}),
    );
    assert_eq!(count(&env, "core.draft.arc"), 1);
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
    assert_eq!(
        ends(&curve_of(&env, &b).unwrap()),
        ([0.0, 0.0], [0.0, 10.0])
    );
}

#[test]
fn a_zero_radius_fillet_gives_the_sharp_corner_with_no_joint() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [5.0, -5.0], [5.0, 5.0]);
    let result = env.run(
        "draftbuilder.modify.fillet",
        json!({"first": {"object": a.clone(), "pick": [8.0, 0.0]},
               "second": {"object": b.clone(), "pick": [5.0, 4.0]}, "radius": 0.0}),
    );
    assert_eq!(ids_of(&result), vec![a.clone(), b.clone()]);
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([5.0, 0.0], [10.0, 0.0])
    );
    assert_eq!(ends(&curve_of(&env, &b).unwrap()), ([5.0, 0.0], [5.0, 5.0]));
    assert_eq!(count(&env, "core.draft.arc"), 0);
}

#[test]
fn chamfer_by_two_distances_bevels_the_corner() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [0.0, 10.0]);
    let result = env.run(
        "draftbuilder.modify.chamfer",
        json!({"corner": {"form": "pair",
                          "first": {"object": a.clone(), "pick": [5.0, 0.0]},
                          "second": {"object": b.clone(), "pick": [0.0, 5.0]},
                          "size": {"by": "distances", "distances": [2.0, 2.0]}}}),
    );
    let joint = ids_of(&result)[0].clone();
    let (s, e) = ends(&curve_of(&env, &joint).unwrap());
    let mut pts = [s, e];
    pts.sort_by(|p, q| p[0].partial_cmp(&q[0]).unwrap());
    assert!(
        close(pts[0], [0.0, 2.0]) && close(pts[1], [2.0, 0.0]),
        "{pts:?}"
    );
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([2.0, 0.0], [10.0, 0.0])
    );
    assert_eq!(
        ends(&curve_of(&env, &b).unwrap()),
        ([0.0, 2.0], [0.0, 10.0])
    );
}

#[test]
fn chamfer_refuses_the_same_entity_twice_and_a_circle_input() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.chamfer",
            json!({"corner": {"form": "pair",
                              "first": {"object": a.clone(), "pick": [5.0, 0.0]},
                              "second": {"object": a.clone(), "pick": [5.0, 0.0]},
                              "size": {"by": "distances", "distances": [1.0, 1.0]}}}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
    let circle = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 5.0}}),
    );
    let circle = circle["ids"][0].as_str().unwrap().to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.chamfer",
            json!({"corner": {"form": "pair",
                              "first": {"object": circle, "pick": [5.0, 0.0]},
                              "second": {"object": a.clone(), "pick": [5.0, 0.0]},
                              "size": {"by": "distances", "distances": [1.0, 1.0]}}}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "geom.corner.unsupported_pair");
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn fillet_of_a_polyline_applies_the_corner_at_every_vertex() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    );
    let source = source["ids"][0].as_str().unwrap().to_owned();
    let result = env.run(
        "draftbuilder.modify.fillet",
        json!({"polyline": source.clone(), "radius": 2.0}),
    );
    assert_eq!(ids_of(&result), vec![source.clone()]);
    assert_eq!(result["skipped"].as_array().unwrap().len(), 0, "{result}");
    let Curve2::Polyline(p) = curve_of(&env, &source).unwrap() else {
        panic!("not a polyline");
    };
    assert_eq!(p.vertices().len(), 4);
}

#[test]
fn fillet_refuses_a_negative_radius_with_the_foundation_code() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [0.0, 10.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.fillet",
            json!({"first": {"object": a.clone(), "pick": [5.0, 0.0]},
                   "second": {"object": b.clone(), "pick": [0.0, 5.0]}, "radius": -1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "geom.corner.radius_invalid");
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn fillet_prompt_form_takes_two_objects_their_picks_and_the_radius() {
    use arc_command::{CommandLine, Submitted, Target};
    use std::collections::BTreeMap;
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [0.0, 10.0]);
    let mut prompt = CommandLine::new();
    let mut seed = 700;
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
    assert!(matches!(submit(&env, "fillet"), Submitted::Prompt(_)));
    submit(&env, &a);
    submit(&env, "5,0");
    submit(&env, &b);
    submit(&env, "0,5");
    let Submitted::Ran(outcome) = submit(&env, "2") else {
        panic!("the command did not run");
    };
    assert_eq!(ids_of(&outcome.value).len(), 3);
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([2.0, 0.0], [10.0, 0.0])
    );
}

// ----- break ------------------------------------------------------------------------------------

#[test]
fn break_between_two_points_removes_the_middle_and_keeps_both_ends() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.break",
        json!({"object": target.clone(), "first": [3.0, 0.0], "second": [6.0, 0.0]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 2, "{result}");
    assert_eq!(ids[0], target);
    assert_eq!(
        ends(&curve_of(&env, &ids[0]).unwrap()),
        ([0.0, 0.0], [3.0, 0.0])
    );
    assert_eq!(
        ends(&curve_of(&env, &ids[1]).unwrap()),
        ([6.0, 0.0], [10.0, 0.0])
    );
    undo(&mut env);
    assert_eq!(count(&env, "core.draft.line"), 1);
    assert_eq!(
        ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn break_at_a_point_splits_into_two_objects_with_the_same_layer() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.break",
        json!({"object": target, "first": [4.0, 0.0]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 2);
    assert_eq!(
        ends(&curve_of(&env, &ids[0]).unwrap()),
        ([0.0, 0.0], [4.0, 0.0])
    );
    assert_eq!(
        ends(&curve_of(&env, &ids[1]).unwrap()),
        ([4.0, 0.0], [10.0, 0.0])
    );
}

#[test]
fn break_of_a_circle_removes_the_arc_counterclockwise_from_the_first_point() {
    let mut env = Env::new();
    setup(&mut env);
    let circle = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 5.0}}),
    );
    let circle = circle["ids"][0].as_str().unwrap().to_owned();
    let result = env.run(
        "draftbuilder.modify.break",
        json!({"object": circle.clone(), "first": [5.0, 0.0], "second": [0.0, 5.0]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 1, "{result}");
    assert!(
        !exists(&env, &circle),
        "a circle that becomes an arc is replaced"
    );
    match curve_of(&env, &ids[0]).unwrap() {
        Curve2::Arc(arc) => assert!(
            (arc.sweep().abs() - 3.0 * FRAC_PI_2).abs() < 1e-9,
            "{}",
            arc.sweep()
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn break_refuses_two_equal_points() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.break",
            json!({"object": target, "first": [3.0, 0.0], "second": [3.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid");
}

// ----- join -------------------------------------------------------------------------------------

#[test]
fn join_of_collinear_segments_gives_one_segment_and_erases_the_other() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [5.0, 0.0]);
    let b = line(&mut env, [5.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.join",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert_eq!(ids_of(&result), vec![a.clone()]);
    assert_eq!(result["unjoined"].as_array().unwrap().len(), 0);
    assert_eq!(count(&env, "core.draft.line"), 1);
    assert_eq!(
        ends(&curve_of(&env, &a).unwrap()),
        ([0.0, 0.0], [10.0, 0.0])
    );
    assert!(!exists(&env, &b));
}

#[test]
fn join_reports_entities_that_join_with_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [5.0, 5.0], [6.0, 5.0]);
    let result = env.run(
        "draftbuilder.modify.join",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert_eq!(result["objects"].as_array().unwrap().len(), 0);
    assert_eq!(result["unjoined"].as_array().unwrap().len(), 2);
    assert!(exists(&env, &a) && exists(&env, &b));
}

#[test]
fn join_of_four_quarter_arcs_closes_into_a_circle() {
    let mut env = Env::new();
    setup(&mut env);
    let arcs = vec![
        quarter_arc(&mut env, [5.0, 0.0], [0.0, 5.0]),
        quarter_arc(&mut env, [0.0, 5.0], [-5.0, 0.0]),
        quarter_arc(&mut env, [-5.0, 0.0], [0.0, -5.0]),
        quarter_arc(&mut env, [0.0, -5.0], [5.0, 0.0]),
    ];
    let result = env.run("draftbuilder.modify.join", json!({"objects": arcs}));
    assert_eq!(ids_of(&result).len(), 1, "{result}");
    assert_eq!(count(&env, "core.draft.circle"), 1);
    assert_eq!(count(&env, "core.draft.arc"), 0);
}

#[test]
fn join_of_two_quarter_arcs_is_one_half_arc() {
    let mut env = Env::new();
    setup(&mut env);
    let first = quarter_arc(&mut env, [5.0, 0.0], [0.0, 5.0]);
    let second = quarter_arc(&mut env, [0.0, 5.0], [-5.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.join",
        json!({"objects": [first.clone(), second]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids, vec![first.clone()]);
    match curve_of(&env, &first).unwrap() {
        Curve2::Arc(arc) => assert!((arc.sweep().abs() - PI).abs() < 1e-9, "{}", arc.sweep()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn join_of_a_segment_and_a_bent_segment_gives_a_polyline_that_replaces_the_first_object() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [10.0, 0.0], [10.0, 10.0]);
    let result = env.run(
        "draftbuilder.modify.join",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 1);
    assert!(
        !exists(&env, &a) && !exists(&env, &b),
        "both inputs are replaced by the polyline"
    );
    assert_eq!(count(&env, "core.draft.polyline"), 1);
}

// ----- lengthen ---------------------------------------------------------------------------------

#[test]
fn lengthen_by_a_delta_moves_the_end_nearest_the_pick() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": target.clone(), "pick": [9.0, 0.0], "mode": "delta", "value": 5.0}),
    );
    assert_eq!(
        ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [15.0, 0.0])
    );
}

#[test]
fn lengthen_by_a_percentage_and_to_a_total_from_the_start() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": target.clone(), "pick": [9.0, 0.0], "mode": "percent", "value": 50.0}),
    );
    assert_eq!(
        ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 0.0], [15.0, 0.0])
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": target.clone(), "pick": [1.0, 0.0], "mode": "total", "value": 4.0}),
    );
    assert_eq!(
        ends(&curve_of(&env, &target).unwrap()),
        ([11.0, 0.0], [15.0, 0.0])
    );
}

#[test]
fn lengthen_of_an_arc_keeps_its_center_and_radius() {
    let mut env = Env::new();
    setup(&mut env);
    let target = quarter_arc(&mut env, [5.0, 0.0], [0.0, 5.0]);
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": target.clone(), "pick": [0.0, 5.0], "mode": "total", "value": 5.0 * PI}),
    );
    match curve_of(&env, &target).unwrap() {
        Curve2::Arc(arc) => {
            assert!((arc.sweep() - PI).abs() < 1e-9, "{}", arc.sweep());
            assert!((arc.radius() - 5.0).abs() < 1e-12);
            assert!(arc.center().x.abs() < 1e-12 && arc.center().y.abs() < 1e-12);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn lengthen_refuses_a_non_positive_total_and_a_circle() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.lengthen",
            json!({"object": target, "pick": [9.0, 0.0], "mode": "total", "value": 0.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid");
    let circle = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 5.0}}),
    );
    let circle = circle["ids"][0].as_str().unwrap().to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.lengthen",
            json!({"object": circle, "pick": [5.0, 0.0], "mode": "delta", "value": 1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.geometry_unsupported");
}

// ----- stretch ----------------------------------------------------------------------------------

#[test]
fn stretch_moves_the_end_inside_a_crossing_window() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.stretch",
        json!({"first": [-1.0, -1.0], "second": [1.0, 1.0], "displacement": [0.0, 5.0]}),
    );
    assert_eq!(ids_of(&result), vec![target.clone()]);
    assert_eq!(
        ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 5.0], [10.0, 0.0])
    );
}

#[test]
fn stretch_moves_wholly_enclosed_entities_as_a_whole() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "draftbuilder.modify.stretch",
        json!({"first": [-1.0, -1.0], "second": [11.0, 1.0], "base": [0.0, 0.0], "to": [0.0, 5.0]}),
    );
    assert_eq!(
        ends(&curve_of(&env, &target).unwrap()),
        ([0.0, 5.0], [10.0, 5.0])
    );
}

#[test]
fn stretch_moves_only_the_polyline_vertices_inside_the_window() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    );
    let source = source["ids"][0].as_str().unwrap().to_owned();
    env.run(
        "draftbuilder.modify.stretch",
        json!({"first": [9.0, -1.0], "second": [11.0, 1.0], "displacement": [0.0, -3.0]}),
    );
    let Curve2::Polyline(p) = curve_of(&env, &source).unwrap() else {
        panic!("not a polyline");
    };
    let points: Vec<[f64; 2]> = p
        .vertices()
        .iter()
        .map(|v| [v.point.x, v.point.y])
        .collect();
    assert_eq!(points, vec![[0.0, 0.0], [10.0, -3.0], [10.0, 10.0]]);
}

#[test]
fn stretch_moves_the_end_of_an_arc_and_keeps_its_midpoint() {
    let mut env = Env::new();
    setup(&mut env);
    let target = quarter_arc(&mut env, [5.0, 0.0], [0.0, 5.0]);
    env.run(
        "draftbuilder.modify.stretch",
        json!({"first": [-1.0, 4.0], "second": [1.0, 6.0], "displacement": [-1.0, 1.0]}),
    );
    let curve = curve_of(&env, &target).unwrap();
    assert!(matches!(curve, Curve2::Arc(_)), "an arc stays an arc");
    let end = curve.end_point().unwrap();
    assert!(
        (end.x + 1.0).abs() < 1e-9 && (end.y - 6.0).abs() < 1e-9,
        "{end:?}"
    );
}

#[test]
fn stretch_refuses_a_circle_that_the_window_crosses() {
    let mut env = Env::new();
    setup(&mut env);
    let circle = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 5.0}}),
    );
    let circle = circle["ids"][0].as_str().unwrap().to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.stretch",
            json!({"first": [4.0, -1.0], "second": [6.0, 1.0], "displacement": [0.0, 2.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.stretch_unsupported");
    assert!(exists(&env, &circle));
}

// ----- explode ----------------------------------------------------------------------------------

#[test]
fn explode_of_a_polyline_gives_its_segments_and_erases_it() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    );
    let source = source["ids"][0].as_str().unwrap().to_owned();
    let result = env.run(
        "draftbuilder.modify.explode",
        json!({"objects": [source.clone()]}),
    );
    assert_eq!(ids_of(&result).len(), 2);
    assert!(!exists(&env, &source));
    assert_eq!(count(&env, "core.draft.line"), 2);
    undo(&mut env);
    assert!(exists(&env, &source));
}

#[test]
fn explode_of_a_block_reference_places_its_entities_by_the_reference() {
    let mut env = Env::new();
    setup(&mut env);
    let part = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let defined = env.run(
        "core.draft.block.define",
        json!({"name": "PART", "objects": [part]}),
    );
    let block = defined["id"].as_str().unwrap().to_owned();
    let space = env.space().to_string();
    let inserted = env.run(
        "core.draft.block.insert",
        json!({"owner": space, "block": block, "position": [10.0, 10.0],
               "rotation": FRAC_PI_2, "scale": [2.0, 2.0, 1.0]}),
    );
    let reference = inserted["id"].as_str().unwrap().to_owned();
    let result = env.run(
        "draftbuilder.modify.explode",
        json!({"objects": [reference.clone()]}),
    );
    let ids = ids_of(&result);
    assert_eq!(ids.len(), 1, "{result}");
    assert!(!exists(&env, &reference));
    let (s, e) = ends(&curve_of(&env, &ids[0]).unwrap());
    assert!(
        close(s, [10.0, 10.0]) && close(e, [10.0, 12.0]),
        "{s:?} {e:?}"
    );
}

#[test]
fn explode_refuses_a_kind_the_foundation_does_not_decompose() {
    let mut env = Env::new();
    setup(&mut env);
    let point = env.run("draftbuilder.draw.point", json!({"points": [[1.0, 1.0]]}));
    let point = point["ids"][0].as_str().unwrap().to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.explode",
            json!({"objects": [point.clone()]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.explode_unsupported");
    assert!(exists(&env, &point));
}

#[test]
fn stretch_of_an_arc_whose_ends_are_inside_the_window_moves_the_arc_as_a_whole() {
    let mut env = Env::new();
    setup(&mut env);
    let target = quarter_arc(&mut env, [5.0, 0.0], [0.0, 5.0]);
    // The window holds both ends and the bulge of this quarter arc, so the arc is enclosed and moves
    // as a whole, which is the result the end-inside rule gives as well.
    env.run(
        "draftbuilder.modify.stretch",
        json!({"first": [-0.5, -0.5], "second": [5.5, 5.5], "displacement": [1.0, 2.0]}),
    );
    let Curve2::Arc(arc) = curve_of(&env, &target).unwrap() else {
        panic!("an arc stays an arc");
    };
    assert!(
        (arc.center().x - 1.0).abs() < 1e-9 && (arc.center().y - 2.0).abs() < 1e-9,
        "{arc:?}"
    );
    assert!((arc.radius() - 5.0).abs() < 1e-9);
}
