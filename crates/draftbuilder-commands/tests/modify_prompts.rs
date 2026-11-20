//! The prompt form of the modify commands, typed as a command-line script: each answer is one line,
//! the same prompt machine answers it, and the command runs when its prompts are complete. Each test
//! checks the resulting geometry and that one undo returns the drawing to its state before the command.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code"
)]

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

/// The curves of the drawing: identifier and curve, in no particular order.
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
    let curve = curves(env)
        .into_iter()
        .find(|(i, _)| *i == wanted)
        .map(|(_, c)| c)
        .unwrap();
    match curve {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("not a segment: {other:?}"),
    }
}

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A command-line script over the environment.
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

    fn submit(&mut self, env: &Env, text: &str) -> Submitted {
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
        self.line.submit(&target, text).unwrap()
    }

    /// Submits each line; the last one must run the command and return its value.
    fn run(&mut self, env: &Env, lines: &[&str]) -> Json {
        let mut last = None;
        for text in lines {
            last = Some(self.submit(env, text));
        }
        match last.unwrap() {
            Submitted::Ran(outcome) => outcome.value,
            other => panic!("the command did not run: {other:?}"),
        }
    }
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

#[test]
fn scale_prompt_takes_the_objects_a_base_and_a_factor() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [1.0, 1.0], [2.0, 2.0]);
    let mut script = Script::new();
    script.run(&env, &["scale", &a, "0,0", "2"]);
    assert_eq!(segment_of(&env, &a), ([2.0, 2.0], [4.0, 4.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &a), ([1.0, 1.0], [2.0, 2.0]));
}

#[test]
fn mirror_prompt_keeps_the_source_unless_yes_is_answered() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 1.0], [1.0, 2.0]);
    let mut script = Script::new();
    let result = script.run(&env, &["mirror", &a, "0,0", "1,0", "no"]);
    assert_eq!(result["objects"].as_array().unwrap().len(), 1);
    assert_eq!(segment_of(&env, &a), ([0.0, 1.0], [1.0, 2.0]));
    undo(&mut env);
    assert_eq!(curves(&env).len(), 1);
}

#[test]
fn align_prompt_takes_a_source_and_destination_and_end() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut script = Script::new();
    script.run(&env, &["align", &a, "0,0", "5,5", "end"]);
    assert_eq!(segment_of(&env, &a), ([5.0, 5.0], [6.0, 5.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [1.0, 0.0]));
}

#[test]
fn trim_prompt_takes_the_edges_then_the_targets_with_their_picks() {
    let mut env = Env::new();
    setup(&mut env);
    let target = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let edge = line(&mut env, [3.0, -1.0], [3.0, 1.0]);
    let mut script = Script::new();
    let result = script.run(&env, &["trim", &edge, &target, "1,0", "end"]);
    assert_eq!(result["objects"].as_array().unwrap().len(), 1);
    assert_eq!(segment_of(&env, &target), ([3.0, 0.0], [10.0, 0.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &target), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn offset_prompt_takes_a_distance_the_objects_and_a_side_point() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    let result = script.run(&env, &["offset", "2", &a, "5,1"]);
    let copy: ObjectId = result["objects"][0].as_str().unwrap().parse().unwrap();
    let found = curves(&env)
        .into_iter()
        .find(|(i, _)| *i == copy)
        .unwrap()
        .1;
    match found {
        Curve2::Segment(s) => assert_eq!((s.start().y, s.end().y), (2.0, 2.0)),
        other => panic!("{other:?}"),
    }
    undo(&mut env);
    assert_eq!(curves(&env).len(), 1);
}

#[test]
fn chamfer_prompt_takes_two_entities_their_picks_and_two_distances() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let b = line(&mut env, [0.0, 0.0], [0.0, 10.0]);
    let mut script = Script::new();
    script.run(&env, &["chamfer", &a, "5,0", &b, "0,5", "2", "2"]);
    assert_eq!(segment_of(&env, &a), ([2.0, 0.0], [10.0, 0.0]));
    assert_eq!(segment_of(&env, &b), ([0.0, 2.0], [0.0, 10.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn break_prompt_takes_the_object_and_two_points() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    let result = script.run(&env, &["break", &a, "3,0", "6,0"]);
    assert_eq!(result["objects"].as_array().unwrap().len(), 2);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [3.0, 0.0]));
    undo(&mut env);
    assert_eq!(curves(&env).len(), 1);
}

#[test]
fn break_prompt_at_point_splits_with_the_keyword() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    let result = script.run(&env, &["break", &a, "4,0", "at_point"]);
    assert_eq!(result["objects"].as_array().unwrap().len(), 2);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [4.0, 0.0]));
}

#[test]
fn join_prompt_takes_the_objects_and_joins_them() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [5.0, 0.0]);
    let b = line(&mut env, [5.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    let result = script.run(&env, &["join", &format!("{a} {b}")]);
    assert_eq!(result["objects"].as_array().unwrap().len(), 1);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
    undo(&mut env);
    assert_eq!(curves(&env).len(), 2);
}

#[test]
fn lengthen_prompt_takes_the_object_the_pick_the_mode_and_the_value() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    script.run(&env, &["lengthen", &a, "9,0", "delta", "5"]);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [15.0, 0.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn stretch_prompt_takes_the_window_the_base_and_the_second_point() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut script = Script::new();
    script.run(&env, &["stretch", "-1,-1", "1,1", "0,0", "0,5"]);
    assert_eq!(segment_of(&env, &a), ([0.0, 5.0], [10.0, 0.0]));
    undo(&mut env);
    assert_eq!(segment_of(&env, &a), ([0.0, 0.0], [10.0, 0.0]));
}

#[test]
fn explode_prompt_takes_the_object() {
    let mut env = Env::new();
    setup(&mut env);
    let polyline = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]}]}),
    );
    let polyline = polyline["ids"][0].as_str().unwrap().to_owned();
    let mut script = Script::new();
    let result = script.run(&env, &["explode", &polyline]);
    assert_eq!(result["objects"].as_array().unwrap().len(), 2);
    undo(&mut env);
    assert_eq!(curves(&env).len(), 1);
}
