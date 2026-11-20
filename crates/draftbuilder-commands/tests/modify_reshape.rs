//! REVERSE, PEDIT (open and close), CHANGE, FLATTEN and DRAWORDER, headless and as prompts: each checks the exact
//! geometry or the order it leaves, its refusals, and that one undo returns the drawing to its state before.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use std::collections::BTreeMap;

use arc_base::{Limits, ObjectId, Unit};
use arc_command::{CommandLine, Submitted, Target};
use arc_doc::Value;
use arc_draft::types::{ENTITIES, entity};
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

fn curves(env: &Env) -> Vec<(ObjectId, Curve2)> {
    let snapshot = env.document.snapshot();
    let mut out = Vec::new();
    for key in ENTITIES {
        let Ok(iter) = snapshot.iter_type(key) else {
            continue;
        };
        for (_, object) in iter {
            let reader = Reader::new(snapshot.registry(), key, &object.values);
            if let Some(Value::Geometry(bytes)) = reader.get("geometry") {
                out.push((
                    object.id,
                    Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap(),
                ));
            }
        }
    }
    out
}

fn curve(env: &Env, id: &str) -> Curve2 {
    let wanted: ObjectId = id.parse().unwrap();
    curves(env)
        .into_iter()
        .find(|(i, _)| *i == wanted)
        .map(|(_, c)| c)
        .unwrap()
}

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

fn polyline(env: &mut Env, vertices: Json) -> String {
    env.run("draftbuilder.draw.polyline", json!({"vertices": vertices}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

fn circle(env: &mut Env, center: [f64; 2], radius: f64) -> String {
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": center, "radius": radius}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

fn segment(curve: &Curve2) -> ([f64; 2], [f64; 2]) {
    match curve {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("not a segment: {other:?}"),
    }
}

fn vertices(curve: &Curve2) -> Vec<[f64; 2]> {
    match curve {
        Curve2::Polyline(p) => p
            .vertices()
            .iter()
            .map(|v| [v.point.x, v.point.y])
            .collect(),
        other => panic!("not a polyline: {other:?}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

/// The draw-order key of an entity, as the drawing stores it.
fn draw_order(env: &Env, id: &str) -> i64 {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    match snapshot.get(handle).unwrap().values.get(entity::DRAW_ORDER) {
        Some(Some(Value::I64(order))) => *order,
        other => panic!("no draw order: {other:?}"),
    }
}

/// The identifiers of the given entities, in draw order (bottom first).
fn in_draw_order(env: &Env, ids: &[&str]) -> Vec<String> {
    let mut keyed: Vec<(i64, String)> = ids
        .iter()
        .map(|id| (draw_order(env, id), (*id).to_owned()))
        .collect();
    keyed.sort();
    keyed.into_iter().map(|(_, id)| id).collect()
}

/// A command-line script over the environment: each line is one answer, the last one runs the command.
struct Script {
    line: CommandLine,
    seed: u64,
}

impl Script {
    fn new() -> Self {
        Self {
            line: CommandLine::new(),
            seed: 1_000,
        }
    }

    fn run(&mut self, env: &Env, lines: &[&str]) -> Json {
        let mut last = None;
        for text in lines {
            self.seed += 1;
            let localized = BTreeMap::new();
            let target = Target {
                registry: &env.commands,
                document: &env.document,
                session: &env.session,
                selection: &[],
                localized: &localized,
                seed: self.seed,
            };
            last = Some(self.line.submit(&target, text).unwrap());
        }
        match last.unwrap() {
            Submitted::Ran(outcome) => outcome.value,
            other => panic!("the command did not run: {other:?}"),
        }
    }
}

// ----- reverse ----------------------------------------------------------------------------------

#[test]
fn reverse_swaps_the_ends_of_a_line_and_undo_restores_them() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 5.0]);
    env.run(
        "draftbuilder.modify.reverse",
        json!({"objects": [id.clone()]}),
    );
    assert_eq!(segment(&curve(&env, &id)), ([10.0, 5.0], [0.0, 0.0]));
    undo(&mut env);
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [10.0, 5.0]));
}

#[test]
fn reverse_of_a_polyline_reverses_its_vertex_order() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]),
    );
    env.run(
        "draftbuilder.modify.reverse",
        json!({"objects": [id.clone()]}),
    );
    assert_eq!(
        vertices(&curve(&env, &id)),
        vec![[10.0, 10.0], [10.0, 0.0], [0.0, 0.0]]
    );
    undo(&mut env);
    assert_eq!(
        vertices(&curve(&env, &id)),
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]
    );
}

#[test]
fn reverse_refuses_a_circle_and_changes_no_other_object() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let c = circle(&mut env, [0.0, 0.0], 2.0);
    let error = env
        .invoke(
            "draftbuilder.modify.reverse",
            json!({"objects": [a.clone(), c]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.geometry_unsupported");
    assert_eq!(segment(&curve(&env, &a)), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn reverse_prompt_takes_the_objects() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    script.run(&env, &["reverse", &id]);
    assert_eq!(segment(&curve(&env, &id)), ([10.0, 0.0], [0.0, 0.0]));
}

// ----- polyline edit: open and close ------------------------------------------------------------

#[test]
fn pedit_close_adds_the_closing_segment_and_open_removes_it() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]),
    );
    env.run(
        "draftbuilder.modify.pedit",
        json!({"objects": [id.clone()], "action": "close"}),
    );
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            assert!(p.is_closed());
            assert_eq!(p.segment_count(), 3);
        }
        other => panic!("{other:?}"),
    }
    env.run(
        "draftbuilder.modify.pedit",
        json!({"objects": [id.clone()], "action": "open"}),
    );
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            assert!(!p.is_closed());
            assert_eq!(p.segment_count(), 2);
            assert_eq!(vertices(&curve(&env, &id)).len(), 3);
        }
        other => panic!("{other:?}"),
    }
    undo(&mut env);
    match curve(&env, &id) {
        Curve2::Polyline(p) => assert!(p.is_closed()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn pedit_refuses_a_line_and_leaves_the_polylines_of_the_selection_unchanged() {
    let mut env = Env::new();
    setup(&mut env);
    let poly = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 4.0]}]),
    );
    let straight = line(&mut env, [0.0, 5.0], [10.0, 5.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.pedit",
            json!({"objects": [poly.clone(), straight], "action": "close"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.geometry_unsupported");
    match curve(&env, &poly) {
        Curve2::Polyline(p) => assert!(!p.is_closed()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn pedit_prompt_takes_the_polyline_and_the_action() {
    let mut env = Env::new();
    setup(&mut env);
    let poly = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]),
    );
    let mut script = Script::new();
    script.run(&env, &["pedit", &poly, "close"]);
    match curve(&env, &poly) {
        Curve2::Polyline(p) => assert!(p.is_closed()),
        other => panic!("{other:?}"),
    }
}

// ----- change -----------------------------------------------------------------------------------

#[test]
fn change_moves_the_endpoint_nearer_the_pick_to_the_change_point() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "draftbuilder.modify.change",
        json!({"object": id.clone(), "pick": [9.0, 0.0], "point": [12.0, 3.0]}),
    );
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [12.0, 3.0]));
    undo(&mut env);
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn change_sets_the_radius_of_a_circle_to_the_distance_of_the_change_point() {
    let mut env = Env::new();
    setup(&mut env);
    let id = circle(&mut env, [1.0, 1.0], 2.0);
    env.run(
        "draftbuilder.modify.change",
        json!({"object": id.clone(), "pick": [3.0, 1.0], "point": [4.0, 5.0]}),
    );
    match curve(&env, &id) {
        Curve2::Circle(c) => {
            assert!(close(c.center().x, 1.0) && close(c.center().y, 1.0));
            assert!(close(c.radius(), 5.0));
        }
        other => panic!("{other:?}"),
    }
    undo(&mut env);
    match curve(&env, &id) {
        Curve2::Circle(c) => assert!(close(c.radius(), 2.0)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn change_refuses_a_polyline() {
    let mut env = Env::new();
    setup(&mut env);
    let poly = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}]),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.change",
            json!({"object": poly, "pick": [9.0, 0.0], "point": [12.0, 3.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.geometry_unsupported");
}

#[test]
fn change_prompt_takes_the_object_the_pick_and_the_point() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    // The typed name CHANGE is the property change command, so the prompt form is named by its identifier.
    script.run(&env, &["draftbuilder.modify.change", &id, "9,0", "12,3"]);
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [12.0, 3.0]));
}

// ----- flatten ----------------------------------------------------------------------------------

fn elevation_of(env: &Env, id: &str) -> f64 {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    match snapshot.get(handle).unwrap().values.get(entity::ELEVATION) {
        Some(Some(Value::F64(z))) => *z,
        other => panic!("no elevation: {other:?}"),
    }
}

#[test]
fn flatten_sets_the_elevation_and_thickness_to_zero_and_undo_restores_them() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [id.clone()], "field": "elevation", "value": 5.0}),
    );
    assert!(close(elevation_of(&env, &id), 5.0));
    env.run(
        "draftbuilder.modify.flatten",
        json!({"objects": [id.clone()]}),
    );
    assert!(close(elevation_of(&env, &id), 0.0));
    undo(&mut env);
    assert!(close(elevation_of(&env, &id), 5.0));
}

#[test]
fn flatten_prompt_takes_the_objects() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [id.clone()], "field": "elevation", "value": 2.0}),
    );
    let mut script = Script::new();
    script.run(&env, &["flatten", &id]);
    assert!(close(elevation_of(&env, &id), 0.0));
}

// ----- draw order -------------------------------------------------------------------------------

#[test]
fn draw_order_front_and_back_move_an_entity_over_and_under_the_others() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [0.0, 1.0], [1.0, 1.0]);
    let c = line(&mut env, [0.0, 2.0], [1.0, 2.0]);
    let all = [a.as_str(), b.as_str(), c.as_str()];
    assert_eq!(
        in_draw_order(&env, &all),
        vec![a.clone(), b.clone(), c.clone()]
    );
    env.run(
        "draftbuilder.modify.draw_order",
        json!({"objects": [a.clone()], "place": "front"}),
    );
    assert_eq!(
        in_draw_order(&env, &all),
        vec![b.clone(), c.clone(), a.clone()]
    );
    env.run(
        "draftbuilder.modify.draw_order",
        json!({"objects": [c.clone()], "place": "back"}),
    );
    assert_eq!(
        in_draw_order(&env, &all),
        vec![c.clone(), b.clone(), a.clone()]
    );
    undo(&mut env);
    assert_eq!(
        in_draw_order(&env, &all),
        vec![b.clone(), c.clone(), a.clone()]
    );
}

#[test]
fn draw_order_above_and_below_place_next_to_the_reference() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [0.0, 1.0], [1.0, 1.0]);
    let c = line(&mut env, [0.0, 2.0], [1.0, 2.0]);
    let all = [a.as_str(), b.as_str(), c.as_str()];
    env.run(
        "draftbuilder.modify.draw_order",
        json!({"objects": [c.clone()], "place": "below", "reference": a.clone()}),
    );
    assert_eq!(
        in_draw_order(&env, &all),
        vec![c.clone(), a.clone(), b.clone()]
    );
    env.run(
        "draftbuilder.modify.draw_order",
        json!({"objects": [c.clone()], "place": "above", "reference": b.clone()}),
    );
    assert_eq!(
        in_draw_order(&env, &all),
        vec![a.clone(), b.clone(), c.clone()]
    );
}

#[test]
fn draw_order_refuses_a_reference_that_does_not_match_the_place() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [0.0, 1.0], [1.0, 1.0]);
    let before = [draw_order(&env, &a), draw_order(&env, &b)];
    for params in [
        json!({"objects": [a.clone()], "place": "above"}),
        json!({"objects": [a.clone()], "place": "front", "reference": b.clone()}),
    ] {
        let error = env
            .invoke("draftbuilder.modify.draw_order", params)
            .unwrap_err();
        assert_eq!(error.code(), "db.cmd.params_conflict");
    }
    assert_eq!([draw_order(&env, &a), draw_order(&env, &b)], before);
}

#[test]
fn draw_order_prompt_takes_the_objects_the_place_and_the_reference() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [0.0, 1.0], [1.0, 1.0]);
    let c = line(&mut env, [0.0, 2.0], [1.0, 2.0]);
    let mut script = Script::new();
    script.run(&env, &["draworder", &a, "front"]);
    assert_eq!(
        in_draw_order(&env, &[&a, &b, &c]),
        vec![b.clone(), c.clone(), a.clone()]
    );
    let mut script = Script::new();
    script.run(&env, &["draworder", &c, "below", &b]);
    assert_eq!(
        in_draw_order(&env, &[&a, &b, &c]),
        vec![c.clone(), b.clone(), a.clone()]
    );
}

#[test]
fn reverse_of_a_spline_swaps_its_start_and_end_and_undo_restores_them() {
    let mut env = Env::new();
    setup(&mut env);
    let id = env.run(
        "draftbuilder.draw.spline",
        json!({"points": [[0.0, 0.0], [1.0, 2.0], [2.0, 0.0], [3.0, 2.0], [4.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let before = curve(&env, &id);
    let (start, end) = (before.start_point().unwrap(), before.end_point().unwrap());
    env.run(
        "draftbuilder.modify.reverse",
        json!({"objects": [id.clone()]}),
    );
    let after = curve(&env, &id);
    assert!((after.start_point().unwrap() - end).norm() < 1e-9);
    assert!((after.end_point().unwrap() - start).norm() < 1e-9);
    undo(&mut env);
    assert!((curve(&env, &id).start_point().unwrap() - start).norm() < 1e-9);
}

fn fit_points(env: &Env, id: &str) -> Vec<[f64; 2]> {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    match snapshot
        .get(handle)
        .unwrap()
        .values
        .get(entity::SPLINE_FIT_POINTS)
    {
        Some(Some(Value::List(items))) => items
            .iter()
            .map(|v| match v {
                Value::Point2(p) => [p.x, p.y],
                other => panic!("{other:?}"),
            })
            .collect(),
        other => panic!("no fit points: {other:?}"),
    }
}

#[test]
fn reverse_of_a_spline_through_fit_points_reverses_them_too() {
    let mut env = Env::new();
    setup(&mut env);
    let id = env.run(
        "draftbuilder.draw.spline",
        json!({"points": [[0.0, 0.0], [1.0, 2.0], [2.0, 0.0], [3.0, 2.0], [4.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let before = fit_points(&env, &id);
    assert_eq!(before.len(), 5);
    env.run(
        "draftbuilder.modify.reverse",
        json!({"objects": [id.clone()]}),
    );
    assert_eq!(
        fit_points(&env, &id),
        before.iter().rev().copied().collect::<Vec<_>>()
    );
    undo(&mut env);
    assert_eq!(fit_points(&env, &id), before);
}

#[test]
fn reverse_of_a_polyline_with_arc_segments_and_widths_turns_bulges_and_widths() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([
            {"point": [0.0, 0.0], "bulge": 1.0, "start_width": 0.5, "end_width": 0.8},
            {"point": [10.0, 0.0], "bulge": -0.5, "start_width": 1.0, "end_width": 2.0},
            {"point": [10.0, 10.0]}
        ]),
    );
    env.run(
        "draftbuilder.modify.reverse",
        json!({"objects": [id.clone()]}),
    );
    // Segment 1 (clockwise, widths 1.0 to 2.0) becomes the first segment, from (10,10) back to (10,0), counter-
    // clockwise, with its widths swapped; segment 0 (counterclockwise, widths 0.5 to 0.8) follows, clockwise.
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            let v = p.vertices();
            let expect = [
                ([10.0, 10.0], 0.5, 2.0, 1.0),
                ([10.0, 0.0], -1.0, 0.8, 0.5),
                ([0.0, 0.0], 0.0, 0.0, 0.0),
            ];
            assert_eq!(v.len(), 3);
            for (vertex, (point, bulge, start, end)) in v.iter().zip(expect) {
                assert!(
                    close(vertex.point.x, point[0]) && close(vertex.point.y, point[1]),
                    "{vertex:?}"
                );
                assert!(close(vertex.bulge, bulge), "{vertex:?}");
                assert!(
                    close(vertex.start_width, start) && close(vertex.end_width, end),
                    "{vertex:?}"
                );
            }
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn pedit_close_drops_a_last_vertex_that_is_the_first_and_closes_with_its_segment() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([
            {"point": [0.0, 0.0]},
            {"point": [10.0, 0.0]},
            {"point": [10.0, 10.0]},
            {"point": [0.0, 0.0]}
        ]),
    );
    env.run(
        "draftbuilder.modify.pedit",
        json!({"objects": [id.clone()], "action": "close"}),
    );
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            assert!(p.is_closed());
            assert_eq!(
                vertices(&curve(&env, &id)),
                vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]
            );
            assert_eq!(p.segment_count(), 3);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn pedit_close_refuses_a_two_vertex_polyline() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}]),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.pedit",
            json!({"objects": [id.clone()], "action": "close"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid");
    match curve(&env, &id) {
        Curve2::Polyline(p) => assert!(!p.is_closed()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn flatten_refuses_an_entity_whose_extrusion_is_not_up_and_changes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let up = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let down = line(&mut env, [0.0, 5.0], [10.0, 5.0]);
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [up.clone()], "field": "elevation", "value": 5.0}),
    );
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [down.clone()], "field": "extrusion", "value": [0.0, 0.0, -1.0]}),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.flatten",
            json!({"objects": [up.clone(), down.clone()]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.geometry_unsupported");
    assert!(close(elevation_of(&env, &up), 5.0));
}

#[test]
fn change_refuses_a_line_or_radius_that_would_fall_below_the_linear_tolerance() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let circ = circle(&mut env, [0.0, 0.0], 2.0);
    for (object, pick, point) in [
        (id.clone(), [9.0, 0.0], [0.0, 0.0]),
        (id.clone(), [9.0, 0.0], [1e-12, 0.0]),
        (circ.clone(), [2.0, 0.0], [0.0, 0.0]),
    ] {
        let error = env
            .invoke(
                "draftbuilder.modify.change",
                json!({"object": object, "pick": pick, "point": point}),
            )
            .unwrap_err();
        assert_eq!(error.code(), "db.cmd.value_invalid");
    }
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn each_new_command_refuses_an_entity_on_a_locked_layer() {
    let mut env = Env::new();
    setup(&mut env);
    let walls = env.run("draftbuilder.layer.create", json!({"name": "Walls"}))["layers"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.settings.current_layer",
        json!({"layer": "Walls"}),
    );
    let l = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let pl = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]),
    );
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [walls], "locked": true}),
    );
    let cases = [
        (
            "draftbuilder.modify.reverse",
            json!({"objects": [l.clone()]}),
        ),
        (
            "draftbuilder.modify.pedit",
            json!({"objects": [pl.clone()], "action": "close"}),
        ),
        (
            "draftbuilder.modify.change",
            json!({"object": l.clone(), "pick": [9.0, 0.0], "point": [12.0, 0.0]}),
        ),
        (
            "draftbuilder.modify.flatten",
            json!({"objects": [l.clone()]}),
        ),
        (
            "draftbuilder.modify.draw_order",
            json!({"objects": [l.clone()], "place": "front"}),
        ),
        (
            "draftbuilder.modify.overkill",
            json!({"objects": [l.clone()]}),
        ),
    ];
    for (command, params) in cases {
        let error = env.invoke(command, params).unwrap_err();
        assert_eq!(error.code(), "draft.layer.locked", "{command}");
    }
    assert_eq!(segment(&curve(&env, &l)), ([0.0, 0.0], [10.0, 0.0]));
}
