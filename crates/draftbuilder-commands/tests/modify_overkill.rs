//! OVERKILL, headless and as a prompt: duplicate, overlapping, touching and chained lines merge into the first line
//! of the selection, the others are erased, collinearity is judged by the tolerance, and one undo restores them.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use std::collections::BTreeMap;

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

fn segment_of(env: &Env, id: &str) -> ([f64; 2], [f64; 2]) {
    let wanted: ObjectId = id.parse().unwrap();
    match curves(env)
        .into_iter()
        .find(|(i, _)| *i == wanted)
        .map(|(_, c)| c)
    {
        Some(Curve2::Segment(s)) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("not a segment: {other:?}"),
    }
}

fn exists(env: &Env, id: &str) -> bool {
    let wanted: ObjectId = id.parse().unwrap();
    curves(env).iter().any(|(i, _)| *i == wanted)
}

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

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

#[test]
fn duplicate_lines_merge_into_the_first_and_the_second_is_erased() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [10.0, 0.0], [0.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert_eq!(result["erased"][0].as_str().unwrap(), b);
    assert!(result["changed"].as_array().unwrap().is_empty());
    assert!(exists(&env, &a));
    assert!(!exists(&env, &b));
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    undo(&mut env);
    assert!(exists(&env, &b));
}

#[test]
fn overlapping_lines_become_one_line_spanning_both() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [6.0, 0.0]);
    let b = line(&mut env, [4.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert_eq!(result["changed"][0].as_str().unwrap(), a);
    assert_eq!(result["erased"][0].as_str().unwrap(), b);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [6.0, 0.0]));
    assert!(exists(&env, &b));
}

#[test]
fn end_to_end_lines_that_touch_join_and_chained_lines_merge_in_one_run() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [3.0, 0.0]);
    let c = line(&mut env, [6.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [2.0, 0.0], [7.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), c.clone(), b.clone()]}),
    );
    assert_eq!(result["erased"].as_array().unwrap().len(), 2);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    assert!(!exists(&env, &b) && !exists(&env, &c));
}

#[test]
fn lines_off_the_same_line_or_apart_are_left_alone() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let parallel = line(&mut env, [0.0, 1.0], [10.0, 1.0]);
    let apart = line(&mut env, [20.0, 0.0], [30.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), parallel.clone(), apart.clone()]}),
    );
    assert!(result["erased"].as_array().unwrap().is_empty());
    assert!(result["changed"].as_array().unwrap().is_empty());
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn the_tolerance_decides_whether_a_nearly_collinear_line_merges() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [5.0, 0.05], [15.0, 0.05]);
    let kept = env
        .invoke(
            "draftbuilder.modify.overkill",
            json!({"objects": [a.clone(), b.clone()]}),
        )
        .unwrap();
    assert!(kept["erased"].as_array().unwrap().is_empty());
    let merged = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), b.clone()], "tolerance": 0.1}),
    );
    assert_eq!(merged["erased"][0].as_str().unwrap(), b);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [15.0, 0.0]));
}

#[test]
fn overkill_refuses_a_negative_tolerance_and_changes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.overkill",
            json!({"objects": [a.clone(), b.clone()], "tolerance": -1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid");
    assert!(exists(&env, &b));
}

#[test]
fn overkill_prompt_takes_the_objects() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [6.0, 0.0]);
    let b = line(&mut env, [4.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    script.run(&env, &["overkill", &format!("{a},{b}")]);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    assert!(!exists(&env, &b));
}

fn remaining(env: &Env, ids: &[&str]) -> Vec<String> {
    ids.iter()
        .filter(|id| exists(env, id))
        .map(|id| (*id).to_owned())
        .collect()
}

#[test]
fn chained_lines_keep_their_whole_extent_in_selection_order() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [2.5, 0.0], [3.5, 0.0]);
    let c = line(&mut env, [3.5, 0.0], [5.0, 0.0]);
    let d = line(&mut env, [1.0, 0.0], [2.5, 0.0]);
    env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), b.clone(), c.clone(), d.clone()]}),
    );
    assert_eq!(remaining(&env, &[&a, &b, &c, &d]), vec![a.clone()]);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [5.0, 0.0]));
    undo(&mut env);
    assert_eq!(remaining(&env, &[&a, &b, &c, &d]).len(), 4);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [1.0, 0.0]));
}

#[test]
fn chained_lines_keep_their_whole_extent_in_reversed_selection_order() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [2.5, 0.0], [3.5, 0.0]);
    let c = line(&mut env, [3.5, 0.0], [5.0, 0.0]);
    let d = line(&mut env, [1.0, 0.0], [2.5, 0.0]);
    env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [d.clone(), c.clone(), b.clone(), a.clone()]}),
    );
    // The first selected line, D, keeps its identifier and takes the whole extent.
    assert_eq!(remaining(&env, &[&a, &b, &c, &d]), vec![d.clone()]);
    let (start, end) = segment_of(&env, &d);
    let (lo, hi) = if start[0] < end[0] {
        (start, end)
    } else {
        (end, start)
    };
    assert!(
        close(lo[0], 0.0) && close(hi[0], 5.0) && close(lo[1], 0.0) && close(hi[1], 0.0),
        "{start:?} {end:?}"
    );
}

#[test]
fn a_few_thousand_collinear_lines_in_adversarial_order_become_one_line() {
    let mut env = Env::new();
    setup(&mut env);
    let count: usize = 3000;
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        // A permutation of the unit segments: 7919 is prime and does not divide 3000.
        let k = ((i * 7919) % count) as f64;
        ids.push(line(&mut env, [k, 0.0], [k + 1.0, 0.0]));
    }
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": ids.clone()}),
    );
    assert_eq!(result["erased"].as_array().unwrap().len(), count - 1);
    assert_eq!(remaining(&env, &refs).len(), 1);
    assert_eq!(segment_of(&env, &ids[0]), ([0.0, 0.0], [3000.0, 0.0]));
}

#[test]
fn lines_on_different_layers_are_both_kept() {
    let mut env = Env::new();
    setup(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Other"}));
    let a = line(&mut env, [0.0, 0.0], [6.0, 0.0]);
    env.run(
        "draftbuilder.settings.current_layer",
        json!({"layer": "Other"}),
    );
    let b = line(&mut env, [4.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert!(result["erased"].as_array().unwrap().is_empty());
    assert!(result["changed"].as_array().unwrap().is_empty());
    assert_eq!(remaining(&env, &[&a, &b]).len(), 2);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [6.0, 0.0]));
}

#[test]
fn lines_of_different_color_are_both_kept() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [6.0, 0.0]);
    let b = line(&mut env, [4.0, 0.0], [10.0, 0.0]);
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [b.clone()], "field": "color_by", "value": "object"}),
    );
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [b.clone()], "field": "color", "value": [1.0, 0.0, 0.0, 1.0]}),
    );
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [a.clone(), b.clone()]}),
    );
    assert!(result["erased"].as_array().unwrap().is_empty());
    assert_eq!(remaining(&env, &[&a, &b]).len(), 2);
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_point_or_polyline_in_the_selection_is_left_alone_and_the_lines_still_merge() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [6.0, 0.0]);
    let b = line(&mut env, [4.0, 0.0], [10.0, 0.0]);
    let point = env.run("draftbuilder.draw.point", json!({"points": [[2.0, 0.0]]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let poly = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [8.0, 0.0]}]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [point.clone(), a.clone(), poly.clone(), b.clone()]}),
    );
    assert_eq!(result["erased"][0].as_str().unwrap(), b);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    let point_id: ObjectId = point.parse().unwrap();
    assert!(
        env.document.snapshot().resolve(point_id).is_ok(),
        "the point is kept"
    );
    assert!(
        exists(&env, &poly),
        "the polyline is kept, and is not merged into the line"
    );
}

#[test]
fn a_keeper_keeps_its_own_direction_and_an_unchanged_keeper_is_not_rewritten() {
    let mut env = Env::new();
    setup(&mut env);
    // The first selected line runs from (5,0) to (0,0); the second overlaps it from (3,0) to (10,0).
    let first = line(&mut env, [5.0, 0.0], [0.0, 0.0]);
    let second = line(&mut env, [3.0, 0.0], [10.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [first.clone(), second.clone()]}),
    );
    assert_eq!(result["changed"][0].as_str().unwrap(), first);
    assert_eq!(segment_of(&env, &first), ([10.0, 0.0], [0.0, 0.0]));

    // A keeper that already spans the run is not rewritten and is not reported as changed.
    let mut env = Env::new();
    setup(&mut env);
    let spanning = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let inner = line(&mut env, [2.0, 0.0], [5.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [spanning.clone(), inner.clone()]}),
    );
    assert!(result["changed"].as_array().unwrap().is_empty());
    assert_eq!(result["erased"][0].as_str().unwrap(), inner);
    assert_eq!(segment_of(&env, &spanning), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn a_later_keeper_takes_the_run_in_its_own_direction() {
    let mut env = Env::new();
    setup(&mut env);
    // F is alone on [0,1]. B (9,0)->(3,0) and C (2,0)->(5,0) overlap into the run [2,9]; B, selected first, keeps
    // its direction, from (9,0) to (2,0), and C is erased.
    let f = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let b = line(&mut env, [9.0, 0.0], [3.0, 0.0]);
    let c = line(&mut env, [2.0, 0.0], [5.0, 0.0]);
    env.run(
        "draftbuilder.modify.overkill",
        json!({"objects": [f.clone(), b.clone(), c.clone()]}),
    );
    assert_eq!(segment_of(&env, &b), ([9.0, 0.0], [2.0, 0.0]));
    assert_eq!(segment_of(&env, &f), ([0.0, 0.0], [1.0, 0.0]));
    assert!(!exists(&env, &c));
}

#[test]
fn a_merge_that_would_erase_a_dependent_line_writes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let path = line(&mut env, [5.0, 0.0], [15.0, 0.0]);
    let source = line(&mut env, [20.0, 0.0], [21.0, 0.0]);
    env.run(
        "draftbuilder.modify.array_path",
        json!({"objects": [source], "path": path.clone(), "count": 3}),
    );
    // Merging would extend A to 15 and erase the path the array uses: refused, and A is not written first.
    let error = env
        .invoke(
            "draftbuilder.modify.overkill",
            json!({"objects": [a.clone(), path.clone()]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.erase_dependent");
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    assert!(exists(&env, &path));
}
