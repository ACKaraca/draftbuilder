//! The options added to the phase-1 modify commands: the polyline and arc forms of lengthen, the erase option of
//! trim and extend, the count array of copy, the current-layer and multiple options of offset, and the multiple
//! mode of fillet. Each is checked for its exact geometry, for its refusals and for one undo back to the drawing
//! before the command; the prompt form of each is checked as a command-line script.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use std::collections::BTreeMap;
use std::f64::consts::PI;

use arc_base::{Limits, ObjectId, Unit};
use arc_command::{CommandLine, Submitted, Target};
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

/// The curves of the drawing with their identifiers.
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

fn count(env: &Env) -> usize {
    curves(env).len()
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

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn point_close(p: [f64; 2], q: [f64; 2]) -> bool {
    close(p[0], q[0]) && close(p[1], q[1])
}

fn segment(curve: &Curve2) -> ([f64; 2], [f64; 2]) {
    match curve {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("not a segment: {other:?}"),
    }
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
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

// ----- lengthen of polylines --------------------------------------------------------------------

#[test]
fn lengthen_of_a_polyline_moves_the_last_vertex_along_its_end_segment() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]),
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": id.clone(), "pick": [10.0, 9.0], "mode": "delta", "value": 5.0}),
    );
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            let v = p.vertices();
            assert!(
                point_close([v[2].point.x, v[2].point.y], [10.0, 15.0]),
                "{v:?}"
            );
            assert!(
                point_close([v[1].point.x, v[1].point.y], [10.0, 0.0]),
                "{v:?}"
            );
        }
        other => panic!("{other:?}"),
    }
    undo(&mut env);
    match curve(&env, &id) {
        Curve2::Polyline(p) => assert!(point_close(
            [p.vertices()[2].point.x, p.vertices()[2].point.y],
            [10.0, 10.0]
        )),
        other => panic!("{other:?}"),
    }
}

#[test]
fn lengthen_of_a_polyline_at_its_start_moves_the_first_vertex() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}]),
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": id.clone(), "pick": [1.0, 0.0], "mode": "total", "value": 12.0}),
    );
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            assert!(point_close(
                [p.vertices()[0].point.x, p.vertices()[0].point.y],
                [-2.0, 0.0]
            ));
            assert!(point_close(
                [p.vertices()[1].point.x, p.vertices()[1].point.y],
                [10.0, 0.0]
            ));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn lengthen_of_an_arc_segment_of_a_polyline_keeps_its_center_and_radius() {
    let mut env = Env::new();
    setup(&mut env);
    // A half circle from (0,0) to (10,0), counterclockwise: center (5,0), radius 5.
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0], "bulge": 1.0}, {"point": [10.0, 0.0]}]),
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": id.clone(), "pick": [10.0, 0.0], "mode": "percent", "value": 20.0}),
    );
    let angle = 1.2 * PI;
    let end = [5.0 + 5.0 * (angle + PI).cos(), 5.0 * (angle + PI).sin()];
    match curve(&env, &id) {
        Curve2::Polyline(p) => {
            let v = p.vertices();
            assert!(point_close([v[1].point.x, v[1].point.y], end), "{v:?}");
            assert!(close(v[0].bulge, (angle / 4.0).tan()), "{v:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn lengthen_refuses_a_closed_polyline_and_changes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let closed = env
        .invoke(
            "draftbuilder.draw.polyline",
            json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [4.0, 0.0]}, {"point": [4.0, 3.0]}], "close": true}),
        )
        .unwrap();
    let closed = closed["ids"][0].as_str().unwrap().to_owned();
    let before = count(&env);
    let error = env
        .invoke(
            "draftbuilder.modify.lengthen",
            json!({"object": closed.clone(), "pick": [4.0, 3.0], "mode": "delta", "value": 1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.geometry_unsupported");
    assert_eq!(count(&env), before);
}

#[test]
fn lengthen_prompt_takes_a_polyline() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]),
    );
    let mut script = Script::new();
    script.run(&env, &["lengthen", &id, "10,9", "delta", "5"]);
    match curve(&env, &id) {
        Curve2::Polyline(p) => assert!(point_close(
            [p.vertices()[2].point.x, p.vertices()[2].point.y],
            [10.0, 15.0]
        )),
        other => panic!("{other:?}"),
    }
}

// ----- trim and extend, erase option ------------------------------------------------------------

#[test]
fn trim_erase_option_erases_the_listed_objects_with_the_trim() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [5.0, -5.0], [5.0, 5.0]);
    let spare = line(&mut env, [20.0, 0.0], [30.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({
            "edges": [edge],
            "targets": [{"object": target.clone(), "pick": [8.0, 0.0]}],
            "erase": [spare.clone()]
        }),
    );
    assert_eq!(result["erased"][0].as_str().unwrap(), spare);
    assert_eq!(segment(&curve(&env, &target)), ([0.0, 0.0], [5.0, 0.0]));
    let spare_id: ObjectId = spare.parse().unwrap();
    assert!(!curves(&env).iter().any(|(id, _)| *id == spare_id));
    undo(&mut env);
    assert_eq!(segment(&curve(&env, &target)), ([0.0, 0.0], [10.0, 0.0]));
    assert!(curves(&env).iter().any(|(id, _)| *id == spare_id));
}

#[test]
fn trim_erase_refuses_an_object_that_is_also_a_target() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [5.0, -5.0], [5.0, 5.0]);
    let before = count(&env);
    let error = env
        .invoke(
            "draftbuilder.modify.trim",
            json!({
                "edges": [edge],
                "targets": [{"object": target.clone(), "pick": [8.0, 0.0]}],
                "erase": [target.clone()]
            }),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
    assert_eq!(count(&env), before);
    assert_eq!(segment(&curve(&env, &target)), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn trim_prompt_erase_keyword_takes_objects_to_erase() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [5.0, -5.0], [5.0, 5.0]);
    let spare = line(&mut env, [20.0, 0.0], [30.0, 0.0]);
    let mut script = Script::new();
    script.run(
        &env,
        &["trim", &edge, &target, "8,0", "erase", &spare, "end"],
    );
    assert_eq!(segment(&curve(&env, &target)), ([0.0, 0.0], [5.0, 0.0]));
    let spare_id: ObjectId = spare.parse().unwrap();
    assert!(!curves(&env).iter().any(|(id, _)| *id == spare_id));
    undo(&mut env);
    assert_eq!(segment(&curve(&env, &target)), ([0.0, 0.0], [10.0, 0.0]));
}

// ----- copy as an array of a set count -----------------------------------------------------------

fn starts_of(env: &Env) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = curves(env).iter().map(|(_, c)| segment(c).0).collect();
    out.sort_by(|a, b| a[0].total_cmp(&b[0]));
    out
}

#[test]
fn copy_array_spaced_places_copy_k_at_k_times_the_base_to_second_vector() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.copy",
        json!({
            "objects": [source],
            "base": [0.0, 0.0],
            "array": {"second": [2.0, 0.0], "count": 3}
        }),
    );
    assert_eq!(result["objects"].as_array().unwrap().len(), 3);
    assert_eq!(
        starts_of(&env),
        vec![[0.0, 0.0], [2.0, 0.0], [4.0, 0.0], [6.0, 0.0]]
    );
    undo(&mut env);
    assert_eq!(starts_of(&env), vec![[0.0, 0.0]]);
}

#[test]
fn copy_array_fitted_puts_the_last_copy_on_the_second_point() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    env.run(
        "draftbuilder.modify.copy",
        json!({
            "objects": [source],
            "base": [0.0, 0.0],
            "array": {"second": [4.0, 0.0], "count": 4, "fitted": true}
        }),
    );
    assert_eq!(
        starts_of(&env),
        vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0], [4.0, 0.0]]
    );
}

#[test]
fn copy_array_refuses_a_zero_count_a_zero_spacing_and_targets_with_it() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let before = count(&env);
    for params in [
        json!({"objects": [source.clone()], "base": [0.0, 0.0], "array": {"second": [2.0, 0.0], "count": 0}}),
        json!({"objects": [source.clone()], "base": [0.0, 0.0], "array": {"second": [0.0, 0.0], "count": 2}}),
        json!({"objects": [source.clone()], "base": [0.0, 0.0], "targets": [[1.0, 1.0]], "array": {"second": [2.0, 0.0], "count": 2}}),
    ] {
        assert!(env.invoke("draftbuilder.modify.copy", params).is_err());
    }
    assert_eq!(count(&env), before);
}

#[test]
fn copy_array_prompt_takes_the_count_the_second_point_and_fit() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut script = Script::new();
    script.run(
        &env,
        &["copy", &source, "0,0", "array", "3", "4,0", "fitted"],
    );
    // Three copies spaced 4/3 apart, the last on the second point.
    let starts = starts_of(&env);
    let expected = [0.0, 4.0 / 3.0, 8.0 / 3.0, 4.0];
    assert_eq!(starts.len(), 4);
    assert!(
        starts
            .iter()
            .zip(expected)
            .all(|(s, e)| close(s[0], e) && close(s[1], 0.0)),
        "{starts:?}"
    );
    undo(&mut env);
    assert_eq!(starts_of(&env), vec![[0.0, 0.0]]);
}

// ----- offset: current layer and multiple mode --------------------------------------------------

#[test]
fn offset_on_the_current_layer_puts_the_copy_on_that_layer() {
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
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source], "distance": 2.0, "side": [5.0, 1.0], "layer": "current"}),
    );
    let copy: ObjectId = result["objects"][0].as_str().unwrap().parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(copy).unwrap();
    let layer = match snapshot
        .get(handle)
        .unwrap()
        .values
        .get(arc_draft::types::entity::LAYER)
    {
        Some(Some(Value::Reference(layer))) => *layer,
        other => panic!("{other:?}"),
    };
    assert_eq!(layer.to_string(), walls);
}

#[test]
fn offset_multiple_makes_each_offset_from_the_last_result() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source.clone()], "distance": 1.0, "side": [5.0, 5.0], "repeat": 3}),
    );
    assert_eq!(result["objects"].as_array().unwrap().len(), 3);
    let mut ys: Vec<f64> = curves(&env).iter().map(|(_, c)| segment(c).0[1]).collect();
    ys.sort_by(f64::total_cmp);
    assert!(
        ys.iter()
            .zip([0.0, 1.0, 2.0, 3.0])
            .all(|(a, b)| close(*a, b)),
        "{ys:?}"
    );
    undo(&mut env);
    assert_eq!(count(&env), 1);
}

#[test]
fn offset_multiple_refuses_a_through_point_and_a_zero_count() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    for params in [
        json!({"objects": [source.clone()], "through": [5.0, 2.0], "repeat": 2}),
        json!({"objects": [source.clone()], "distance": 1.0, "side": [5.0, 5.0], "repeat": 0}),
    ] {
        let error = env
            .invoke("draftbuilder.modify.offset", params)
            .unwrap_err();
        assert_eq!(error.code(), "db.cmd.value_invalid");
    }
    assert_eq!(count(&env), 1);
}

#[test]
fn offset_prompt_takes_the_current_layer_and_the_multiple_count() {
    let mut env = Env::new();
    setup(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    env.run(
        "draftbuilder.settings.current_layer",
        json!({"layer": "Walls"}),
    );
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    script.run(
        &env,
        &[
            "offset", "layer", "current", "multiple", "2", "1", &source, "5,5",
        ],
    );
    assert_eq!(count(&env), 3);
}

// ----- fillet: multiple mode --------------------------------------------------------------------

/// A rectangle from (0,0) to (10,10): the corner at (10,0) and the corner at (0,10).
fn rectangle(env: &mut Env) -> [String; 4] {
    [
        line(env, [0.0, 0.0], [10.0, 0.0]),
        line(env, [10.0, 0.0], [10.0, 10.0]),
        line(env, [10.0, 10.0], [0.0, 10.0]),
        line(env, [0.0, 10.0], [0.0, 0.0]),
    ]
}

fn arc_centers(env: &Env) -> Vec<(f64, f64, f64)> {
    curves(env)
        .iter()
        .filter_map(|(_, c)| match c {
            Curve2::Arc(a) => Some((a.center().x, a.center().y, a.radius())),
            _ => None,
        })
        .collect()
}

#[test]
fn fillet_more_fillets_each_further_corner_with_the_same_radius() {
    let mut env = Env::new();
    setup(&mut env);
    let [bottom, right, top, left] = rectangle(&mut env);
    env.run(
        "draftbuilder.modify.fillet",
        json!({
            "first": {"object": bottom.clone(), "pick": [5.0, 0.0]},
            "second": {"object": right.clone(), "pick": [10.0, 5.0]},
            "radius": 1.0,
            "more": [{
                "first": {"object": top.clone(), "pick": [5.0, 10.0]},
                "second": {"object": left.clone(), "pick": [0.0, 5.0]}
            }]
        }),
    );
    let centers = arc_centers(&env);
    assert_eq!(centers.len(), 2, "{centers:?}");
    assert!(
        centers
            .iter()
            .any(|(x, y, r)| close(*x, 9.0) && close(*y, 1.0) && close(*r, 1.0))
    );
    assert!(
        centers
            .iter()
            .any(|(x, y, r)| close(*x, 1.0) && close(*y, 9.0) && close(*r, 1.0))
    );
    assert_eq!(segment(&curve(&env, &top)), ([10.0, 10.0], [1.0, 10.0]));
    assert_eq!(segment(&curve(&env, &bottom)), ([0.0, 0.0], [9.0, 0.0]));
    undo(&mut env);
    assert!(arc_centers(&env).is_empty());
    assert_eq!(segment(&curve(&env, &bottom)), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn fillet_prompt_multiple_repeats_the_corner_pairs_until_end() {
    let mut env = Env::new();
    setup(&mut env);
    let [bottom, right, top, left] = rectangle(&mut env);
    let mut script = Script::new();
    script.run(
        &env,
        &[
            "fillet", "multiple", &bottom, "5,0", &right, "10,5", "1", &top, "5,10", &left, "0,5",
            "end",
        ],
    );
    let centers = arc_centers(&env);
    assert_eq!(centers.len(), 2, "{centers:?}");
    undo(&mut env);
    assert!(arc_centers(&env).is_empty());
}

#[test]
fn fillet_more_refuses_a_pair_of_one_entity_and_changes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let [bottom, right, top, _left] = rectangle(&mut env);
    let before = count(&env);
    let error = env
        .invoke(
            "draftbuilder.modify.fillet",
            json!({
                "first": {"object": bottom.clone(), "pick": [5.0, 0.0]},
                "second": {"object": right.clone(), "pick": [10.0, 5.0]},
                "radius": 1.0,
                "more": [{
                    "first": {"object": top.clone(), "pick": [5.0, 10.0]},
                    "second": {"object": top.clone(), "pick": [1.0, 10.0]}
                }]
            }),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
    assert_eq!(count(&env), before);
    assert!(arc_centers(&env).is_empty());
}

// ----- reference input: picked points in the prompt form ----------------------------------------

#[test]
fn rotate_prompt_takes_the_reference_angle_from_two_picked_points() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut script = Script::new();
    script.run(&env, &["rotate", &id, "0,0", "reference", "1,0", "0,1"]);
    let (start, end) = segment(&curve(&env, &id));
    assert!(close(start[0], 0.0) && close(start[1], 0.0), "{start:?}");
    assert!(close(end[0], 0.0) && close(end[1], 1.0), "{end:?}");
    undo(&mut env);
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [1.0, 0.0]));
}

#[test]
fn scale_prompt_takes_the_reference_length_from_two_picks_and_the_new_length_from_a_pick() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut script = Script::new();
    script.run(
        &env,
        &["scale", &id, "0,0", "reference", "1,0", "3,0", "6,0"],
    );
    let (start, end) = segment(&curve(&env, &id));
    assert!(
        close(start[0], 0.0) && close(end[0], 3.0) && close(end[1], 0.0),
        "{end:?}"
    );
    undo(&mut env);
    assert_eq!(segment(&curve(&env, &id)), ([0.0, 0.0], [1.0, 0.0]));
}

fn total_length(curve: &Curve2) -> f64 {
    match curve {
        Curve2::Polyline(p) => p
            .vertices()
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0].point, w[1].point);
                (b - a).norm()
            })
            .sum(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn lengthen_total_on_a_polyline_sets_the_length_of_the_whole_polyline() {
    let mut env = Env::new();
    setup(&mut env);
    // Segments of 30 and 40: the whole polyline is 70 long.
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [30.0, 0.0]}, {"point": [30.0, 40.0]}]),
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": id.clone(), "pick": [30.0, 39.0], "mode": "total", "value": 100.0}),
    );
    let c = curve(&env, &id);
    assert!(close(total_length(&c), 100.0), "{}", total_length(&c));
    assert_eq!(vertices(&c)[2], [30.0, 70.0]);
    undo(&mut env);
    assert!(close(total_length(&curve(&env, &id)), 70.0));
}

#[test]
fn lengthen_percent_on_a_polyline_scales_the_whole_length_onto_the_end_segment() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [30.0, 0.0]}, {"point": [30.0, 40.0]}]),
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": id.clone(), "pick": [30.0, 39.0], "mode": "percent", "value": 50.0}),
    );
    // The whole length 70 grows by 35, all of it on the end segment: 40 becomes 75.
    assert!(close(total_length(&curve(&env, &id)), 105.0));
    assert_eq!(vertices(&curve(&env, &id))[2], [30.0, 75.0]);
}

#[test]
fn lengthen_delta_on_a_polyline_changes_only_the_end_segment() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [30.0, 0.0]}, {"point": [30.0, 40.0]}]),
    );
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": id.clone(), "pick": [30.0, 39.0], "mode": "delta", "value": 10.0}),
    );
    assert_eq!(vertices(&curve(&env, &id))[2], [30.0, 50.0]);
}

#[test]
fn lengthen_total_refuses_a_change_that_would_shorten_the_end_segment_past_its_start() {
    let mut env = Env::new();
    setup(&mut env);
    let id = polyline(
        &mut env,
        json!([{"point": [0.0, 0.0]}, {"point": [30.0, 0.0]}, {"point": [30.0, 40.0]}]),
    );
    // Total 20 would take 50 off the end segment of 40.
    let error = env
        .invoke(
            "draftbuilder.modify.lengthen",
            json!({"object": id.clone(), "pick": [30.0, 39.0], "mode": "total", "value": 20.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid");
    assert!(close(total_length(&curve(&env, &id)), 70.0));
}

#[test]
fn offset_onto_a_locked_current_layer_creates_the_copy_there() {
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
    let source = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [walls.clone()], "locked": true}),
    );
    // Creating on a locked layer is allowed; only modifying the entities on it is refused.
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source], "distance": 2.0, "side": [5.0, 1.0], "layer": "current"}),
    );
    let copy: ObjectId = result["objects"][0].as_str().unwrap().parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(copy).unwrap();
    match snapshot
        .get(handle)
        .unwrap()
        .values
        .get(arc_draft::types::entity::LAYER)
    {
        Some(Some(Value::Reference(layer))) => assert_eq!(layer.to_string(), walls),
        other => panic!("{other:?}"),
    }
}

fn vertices(curve: &Curve2) -> Vec<[f64; 2]> {
    match curve {
        Curve2::Polyline(p) => p
            .vertices()
            .iter()
            .map(|v| [v.point.x, v.point.y])
            .collect(),
        other => panic!("{other:?}"),
    }
}
