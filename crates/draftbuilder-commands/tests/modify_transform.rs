//! The transform modify commands, headless: the typed form of move, copy, rotate, scale, mirror and
//! align with the exact geometry each produces, the prompt form driven through the command line,
//! undo of one command, the refusals, and the layer lock.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "test code"
)]

mod common;

use std::collections::BTreeMap;
use std::f64::consts::FRAC_PI_2;

use arc_base::Unit;
use arc_command::{CommandLine, Submitted, Target};
use common::{Env, lines};
use serde_json::{Value as Json, json};

fn setup(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
}

/// A line from `a` to `b` in the model space, and its identifier.
fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    let result = env.run("draftbuilder.draw.line", json!({"points": [a, b]}));
    result["ids"][0]["0"].as_str().map_or_else(
        || result["ids"][0].as_str().unwrap().to_owned(),
        str::to_owned,
    )
}

/// The start and end of every line of the model space, sorted by start.
fn segments(env: &Env) -> Vec<([f64; 2], [f64; 2])> {
    lines(&env.document.snapshot())
        .into_iter()
        .map(|(_, a, b)| (a, b))
        .collect()
}

fn count_lines(env: &Env) -> usize {
    segments(env).len()
}

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

/// The identifiers of a result's `objects`.
fn objects(result: &Json) -> Vec<String> {
    result["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

/// A command line over the environment, with the seed of each submitted line.
struct Prompts {
    line: CommandLine,
    seed: u64,
}

impl Prompts {
    fn new() -> Self {
        Self {
            line: CommandLine::new(),
            seed: 500,
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
}

fn ran(submitted: Submitted) -> Json {
    match submitted {
        Submitted::Ran(outcome) => outcome.value,
        other => panic!("the command did not run: {other:?}"),
    }
}

#[test]
fn move_by_a_displacement_keeps_the_identifier_and_undoes_in_one_step() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let moved = env.run(
        "draftbuilder.modify.move",
        json!({"objects": [id.clone()], "displacement": [5.0, 2.0]}),
    );
    assert_eq!(objects(&moved), vec![id.clone()]);
    assert_eq!(segments(&env), vec![([5.0, 2.0], [15.0, 2.0])]);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 0.0])]);
}

#[test]
fn move_from_a_base_to_a_second_point_is_the_difference() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [1.0, 1.0], [2.0, 1.0]);
    env.run(
        "draftbuilder.modify.move",
        json!({"objects": [id], "base": [1.0, 1.0], "second": [4.0, 5.0]}),
    );
    assert_eq!(segments(&env), vec![([4.0, 5.0], [5.0, 5.0])]);
}

#[test]
fn move_refuses_a_displacement_together_with_a_base_point() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.move",
            json!({"objects": [id], "displacement": [1.0, 0.0], "base": [0.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [1.0, 0.0])]);
}

#[test]
fn move_prompt_form_takes_the_object_then_two_points() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [10.0, 0.0]);
    let mut prompts = Prompts::new();
    assert!(matches!(prompts.submit(&env, "move"), Submitted::Prompt(_)));
    prompts.submit(&env, &id);
    prompts.submit(&env, "0,0");
    let result = ran(prompts.submit(&env, "3,4"));
    assert_eq!(objects(&result), vec![id]);
    assert_eq!(segments(&env), vec![([3.0, 4.0], [13.0, 4.0])]);
}

#[test]
fn copy_makes_one_copy_per_target_and_keeps_the_source() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.copy",
        json!({"objects": [id.clone()], "base": [0.0, 0.0], "targets": [[10.0, 0.0], [0.0, 5.0]]}),
    );
    assert_eq!(objects(&result).len(), 2, "{result}");
    assert!(!objects(&result).contains(&id));
    assert_eq!(
        segments(&env),
        vec![
            ([0.0, 0.0], [1.0, 0.0]),
            ([0.0, 5.0], [1.0, 5.0]),
            ([10.0, 0.0], [11.0, 0.0]),
        ]
    );
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count_lines(&env), 1);
}

#[test]
fn copy_prompt_form_takes_targets_until_end() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut prompts = Prompts::new();
    prompts.submit(&env, "copy");
    prompts.submit(&env, &id);
    prompts.submit(&env, "0,0");
    prompts.submit(&env, "10,0");
    let result = ran(prompts.submit(&env, "end"));
    assert_eq!(objects(&result).len(), 1);
    assert_eq!(count_lines(&env), 2);
}

#[test]
fn rotate_about_a_base_by_an_angle_and_by_a_reference_pair() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [1.0, 0.0], [2.0, 0.0]);
    env.run(
        "draftbuilder.modify.rotate",
        json!({"objects": [id.clone()], "base": [0.0, 0.0], "angle": FRAC_PI_2}),
    );
    let s = segments(&env);
    assert!(
        close(s[0].0, [0.0, 1.0]) && close(s[0].1, [0.0, 2.0]),
        "{s:?}"
    );
    // A reference pair from 0 to 90 degrees is the same quarter turn about the same base.
    env.run(
        "draftbuilder.modify.rotate",
        json!({"objects": [id], "base": [0.0, 0.0],
               "reference": {"from": 0.0, "to": FRAC_PI_2}}),
    );
    let s = segments(&env);
    assert!(
        close(s[0].0, [-1.0, 0.0]) && close(s[0].1, [-2.0, 0.0]),
        "{s:?}"
    );
}

#[test]
fn rotate_about_a_point_other_than_the_origin_keeps_the_base_fixed() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [3.0, 2.0], [4.0, 2.0]);
    env.run(
        "draftbuilder.modify.rotate",
        json!({"objects": [id], "base": [2.0, 2.0], "angle": FRAC_PI_2}),
    );
    let s = segments(&env);
    assert!(
        close(s[0].0, [2.0, 3.0]) && close(s[0].1, [2.0, 4.0]),
        "{s:?}"
    );
}

#[test]
fn rotate_copy_keeps_the_source_and_refuses_two_angle_forms() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [1.0, 0.0], [2.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.rotate",
        json!({"objects": [id.clone()], "base": [0.0, 0.0], "angle": FRAC_PI_2, "copy": true}),
    );
    assert_eq!(objects(&result).len(), 1);
    assert_eq!(count_lines(&env), 2);
    let error = env
        .invoke(
            "draftbuilder.modify.rotate",
            json!({"objects": [id], "base": [0.0, 0.0], "angle": 1.0,
                   "reference": {"from": 0.0, "to": 1.0}}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
}

#[test]
fn rotate_prompt_form_reads_the_angle_in_degrees() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [1.0, 0.0], [2.0, 0.0]);
    let mut prompts = Prompts::new();
    prompts.submit(&env, "rotate");
    prompts.submit(&env, &id);
    prompts.submit(&env, "0,0");
    let result = ran(prompts.submit(&env, "90"));
    assert_eq!(objects(&result), vec![id]);
    let s = segments(&env);
    assert!(
        close(s[0].0, [0.0, 1.0]) && close(s[0].1, [0.0, 2.0]),
        "{s:?}"
    );
}

#[test]
fn scale_by_a_factor_about_a_base_and_by_a_reference_length() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [1.0, 1.0], [2.0, 2.0]);
    env.run(
        "draftbuilder.modify.scale",
        json!({"objects": [id.clone()], "base": [0.0, 0.0], "factor": 2.0}),
    );
    assert_eq!(segments(&env), vec![([2.0, 2.0], [4.0, 4.0])]);
    env.run(
        "draftbuilder.modify.scale",
        json!({"objects": [id], "base": [4.0, 4.0], "reference": {"from": 1.0, "to": 0.5}}),
    );
    assert_eq!(segments(&env), vec![([3.0, 3.0], [4.0, 4.0])]);
}

#[test]
fn scale_refuses_a_zero_or_negative_factor() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [1.0, 1.0], [2.0, 2.0]);
    for factor in [0.0, -2.0] {
        let error = env
            .invoke(
                "draftbuilder.modify.scale",
                json!({"objects": [id.clone()], "base": [0.0, 0.0], "factor": factor}),
            )
            .unwrap_err();
        assert_eq!(error.code(), "db.cmd.value_invalid", "{factor}");
    }
    assert_eq!(segments(&env), vec![([1.0, 1.0], [2.0, 2.0])]);
}

#[test]
fn mirror_keeps_the_source_or_erases_it() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 1.0], [1.0, 2.0]);
    let kept = env.run(
        "draftbuilder.modify.mirror",
        json!({"objects": [id.clone()], "first": [0.0, 0.0], "second": [1.0, 0.0]}),
    );
    assert_eq!(objects(&kept).len(), 1);
    let s = segments(&env);
    assert_eq!(s.len(), 2);
    assert!(s.contains(&([0.0, 1.0], [1.0, 2.0])));
    assert!(s.contains(&([0.0, -1.0], [1.0, -2.0])), "{s:?}");
    env.run(
        "draftbuilder.modify.mirror",
        json!({"objects": [id], "first": [0.0, 0.0], "second": [0.0, 1.0], "erase_source": true}),
    );
    // The source (0,1)-(1,2) is erased; its mirror across the y axis is (0,1)-(-1,2).
    let s = segments(&env);
    assert_eq!(s.len(), 2, "{s:?}");
    assert!(!s.contains(&([0.0, 1.0], [1.0, 2.0])));
    assert!(s.contains(&([0.0, 1.0], [-1.0, 2.0])), "{s:?}");
}

#[test]
fn mirror_refuses_a_line_of_no_length() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 1.0], [1.0, 2.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.mirror",
            json!({"objects": [id], "first": [0.0, 0.0], "second": [0.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.points_collinear");
}

#[test]
fn align_with_one_pair_moves_and_with_two_pairs_turns_and_scales() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    env.run(
        "draftbuilder.modify.align",
        json!({"objects": [id.clone()], "pairs": [{"from": [0.0, 0.0], "to": [5.0, 5.0]}]}),
    );
    assert_eq!(segments(&env), vec![([5.0, 5.0], [6.0, 5.0])]);
    // Two pairs: the segment from the first to the second source point turns onto the destination
    // direction and is scaled to its length.
    env.run(
        "draftbuilder.modify.align",
        json!({"objects": [id], "scale": true,
               "pairs": [{"from": [5.0, 5.0], "to": [0.0, 0.0]},
                         {"from": [6.0, 5.0], "to": [0.0, 2.0]}]}),
    );
    let s = segments(&env);
    assert!(
        close(s[0].0, [0.0, 0.0]) && close(s[0].1, [0.0, 2.0]),
        "{s:?}"
    );
}

#[test]
fn align_refuses_three_pairs_that_are_not_a_similarity() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.align",
            json!({"objects": [id], "pairs": [
                {"from": [0.0, 0.0], "to": [0.0, 0.0]},
                {"from": [1.0, 0.0], "to": [1.0, 0.0]},
                {"from": [0.0, 1.0], "to": [3.0, 0.0]}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.align_not_similarity");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [1.0, 0.0])]);
}

#[test]
fn a_transform_on_a_locked_layer_is_refused_with_the_foundation_code() {
    let mut env = Env::new();
    setup(&mut env);
    let id = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    env.run("draftbuilder.layer.lock", json!({"objects": [id.clone()]}));
    let error = env
        .invoke(
            "draftbuilder.modify.move",
            json!({"objects": [id.clone()], "displacement": [1.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "draft.layer.locked");
    let error = env
        .invoke(
            "draftbuilder.modify.mirror",
            json!({"objects": [id], "first": [0.0, 0.0], "second": [0.0, 1.0], "erase_source": true}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "draft.layer.locked");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [1.0, 0.0])]);
}

#[test]
fn mirror_of_several_objects_keeps_each_one_as_a_copy() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 1.0], [1.0, 1.0]);
    let b = line(&mut env, [2.0, 3.0], [2.0, 4.0]);
    let result = env.run(
        "draftbuilder.modify.mirror",
        json!({"objects": [a, b], "first": [0.0, 0.0], "second": [1.0, 0.0]}),
    );
    assert_eq!(objects(&result).len(), 2);
    let s = segments(&env);
    assert_eq!(s.len(), 4);
    assert!(s.contains(&([0.0, -1.0], [1.0, -1.0])));
    assert!(s.contains(&([2.0, -3.0], [2.0, -4.0])), "{s:?}");
}

#[test]
fn erase_removes_entities_and_undo_restores_them() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    line(&mut env, [5.0, 0.0], [6.0, 0.0]);
    let erased = env.run("draftbuilder.modify.erase", json!({"objects": [a]}));
    assert_eq!(objects(&erased).len(), 1);
    assert_eq!(segments(&env), vec![([5.0, 0.0], [6.0, 0.0])]);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count_lines(&env), 2);
}

#[test]
fn erase_prompt_form_takes_the_selection() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut prompts = Prompts::new();
    prompts.submit(&env, "erase");
    ran(prompts.submit(&env, &a));
    assert_eq!(count_lines(&env), 0);
}

#[test]
fn erase_refuses_a_point_that_names_no_entity() {
    let mut env = Env::new();
    setup(&mut env);
    let error = env
        .invoke(
            "draftbuilder.modify.erase",
            json!({"objects": ["00000000-0000-0000-0000-000000000009"]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "doc.object.not_found", "{}", error.code());
}

#[test]
fn the_keyword_last_names_the_topmost_object_when_the_session_has_the_product_hook() {
    let mut env = Env::new();
    setup(&mut env);
    let first = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let second = line(&mut env, [0.0, 2.0], [1.0, 2.0]);
    // Without the product's hook the keyword fails with its own code.
    let error = env
        .invoke(
            "draftbuilder.modify.move",
            json!({"objects": ["last"], "displacement": [1.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "cmd.selection.unavailable");
    env.session = draftbuilder_commands::with_hooks(env.session.clone());
    let moved = env.run(
        "draftbuilder.modify.move",
        json!({"objects": ["last"], "displacement": [1.0, 0.0]}),
    );
    assert_eq!(objects(&moved), vec![second]);
    assert_eq!(segments(&env)[0], ([0.0, 0.0], [1.0, 0.0]), "{first}");
    assert_eq!(segments(&env)[1], ([1.0, 2.0], [2.0, 2.0]));
}
