//! The array modify commands, headless: rectangular, polar and path arrays whose items are the
//! foundation's own item mapping of the stored layout, the source moved into the anonymous block,
//! undo, the prompt form, and the refusals.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code"
)]

mod common;

use arc_base::{ObjectId, Tolerance, Unit};
use arc_doc::{Snapshot, Value};
use arc_draft::array::ARRAY;
use arc_draft::array::{self, Params};
use common::Env;
use serde_json::json;

fn setup(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
}

/// The arrays of the model space: identifier and layout parameters.
fn arrays(snapshot: &Snapshot) -> Vec<(ObjectId, Params)> {
    snapshot
        .iter_type(ARRAY)
        .map(|iter| {
            iter.map(|(_, o)| (o.id, Params::from_values(&o.values)))
                .collect()
        })
        .unwrap_or_default()
}

/// The number of lines whose owner is the model space (block content is owned by its block).
fn model_lines(env: &Env) -> usize {
    let snapshot = env.document.snapshot();
    let space = env.space();
    snapshot
        .iter_type("core.draft.line")
        .map(|iter| iter.filter(|(_, o)| o.header.owner == Some(space)).count())
        .unwrap_or(0)
}

/// The item positions of an array, rounded to micrometers.
fn positions(params: &Params) -> Vec<(f64, f64)> {
    let items = array::items(params, None, &Tolerance::DEFAULT).unwrap();
    items
        .iter()
        .map(|i| {
            (
                (i.position.x * 1e6).round() / 1e6,
                (i.position.y * 1e6).round() / 1e6,
            )
        })
        .collect()
}

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn rectangular_array_has_rows_and_columns_at_the_given_spacings() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let result = env.run(
        "draftbuilder.modify.array_rect",
        json!({"objects": [source.clone()], "rows": 2, "columns": 3,
               "row_spacing": 3.0, "column_spacing": 2.0}),
    );
    assert_eq!(result["objects"].as_array().unwrap().len(), 1, "{result}");
    // The source is now the content of the block: no line stays in the model space.
    assert_eq!(model_lines(&env), 0);
    let found = arrays(&env.document.snapshot());
    assert_eq!(found.len(), 1);
    assert_eq!(
        positions(&found[0].1),
        vec![
            (0.0, 0.0),
            (2.0, 0.0),
            (4.0, 0.0),
            (0.0, 3.0),
            (2.0, 3.0),
            (4.0, 3.0)
        ]
    );
    env.document.undo().wait().unwrap();
    assert_eq!(model_lines(&env), 1);
    assert_eq!(arrays(&env.document.snapshot()).len(), 0);
}

#[test]
fn rectangular_array_at_an_angle_turns_its_axes() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    env.run(
        "draftbuilder.modify.array_rect",
        json!({"objects": [source], "rows": 1, "columns": 2, "column_spacing": 1.0,
               "angle": std::f64::consts::FRAC_PI_2}),
    );
    let found = arrays(&env.document.snapshot());
    let p = positions(&found[0].1);
    assert_eq!(p.len(), 2);
    assert!(
        (p[1].0).abs() < 1e-9 && (p[1].1 - 1.0).abs() < 1e-9,
        "{p:?}"
    );
}

#[test]
fn polar_array_turns_the_item_about_its_center() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    env.run(
        "draftbuilder.modify.array_polar",
        json!({"objects": [source], "center": [0.0, 0.0], "count": 4}),
    );
    let found = arrays(&env.document.snapshot());
    let params = &found[0].1;
    let items = array::items(params, None, &Tolerance::DEFAULT).unwrap();
    assert_eq!(items.len(), 4);
    let turns: Vec<f64> = items
        .iter()
        .map(|i| (i.rotation / std::f64::consts::FRAC_PI_2).round())
        .collect();
    assert_eq!(turns, vec![0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn path_array_places_its_items_along_the_path_and_keeps_the_path() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let path = line(&mut env, [0.0, 5.0], [10.0, 5.0]);
    env.run(
        "draftbuilder.modify.array_path",
        json!({"objects": [source], "path": path.clone(), "count": 3}),
    );
    let snapshot = env.document.snapshot();
    let found = arrays(&snapshot);
    let path_id: ObjectId = path.parse().unwrap();
    let path_curve = common::curve_of_entity(&snapshot, path_id);
    let items = array::items(&found[0].1, Some(&path_curve), &Tolerance::DEFAULT).unwrap();
    assert_eq!(items.len(), 3);
    assert!(
        snapshot.resolve(path_id).is_ok(),
        "the path stays in the drawing"
    );
}

#[test]
fn path_array_refuses_the_path_as_one_of_its_entities() {
    let mut env = Env::new();
    setup(&mut env);
    let path = line(&mut env, [0.0, 5.0], [10.0, 5.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.array_path",
            json!({"objects": [path.clone()], "path": path, "count": 3}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.params_conflict");
}

#[test]
fn an_array_with_no_rows_is_refused_before_anything_changes() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let error = env
        .invoke(
            "draftbuilder.modify.array_rect",
            json!({"objects": [source.clone()], "rows": 0, "columns": 2,
                   "row_spacing": 1.0, "column_spacing": 1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid", "{}", error.code());
    assert_eq!(model_lines(&env), 1);
    assert_eq!(arrays(&env.document.snapshot()).len(), 0);
}

#[test]
fn rectangular_array_prompt_form_takes_the_entities_the_counts_and_the_spacings() {
    use arc_command::{CommandLine, Submitted, Target};
    use std::collections::BTreeMap;
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut prompts = CommandLine::new();
    let mut seed = 300;
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
        prompts.submit(&t, text).unwrap()
    };
    submit(&env, "arrayrect");
    submit(&env, &source);
    submit(&env, "2");
    submit(&env, "3");
    submit(&env, "4");
    submit(&env, "2");
    let Submitted::Ran(outcome) = submit(&env, "0") else {
        panic!("the array did not run");
    };
    assert_eq!(outcome.value["objects"].as_array().unwrap().len(), 1);
    let found = arrays(&env.document.snapshot());
    assert_eq!(positions(&found[0].1).len(), 6);
}

#[test]
fn array_items_read_the_layer_of_the_source() {
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    env.run(
        "draftbuilder.modify.array_polar",
        json!({"objects": [source], "center": [0.0, 0.0], "count": 2}),
    );
    let snapshot = env.document.snapshot();
    let found = snapshot.iter_type(ARRAY).unwrap().next().unwrap().1;
    assert!(matches!(
        found.values[arc_draft::types::entity::LAYER],
        Some(Value::Reference(_))
    ));
}

#[test]
fn polar_and_path_array_prompt_forms_take_their_values_in_order() {
    use arc_command::{CommandLine, Submitted, Target};
    use std::collections::BTreeMap;
    let mut env = Env::new();
    setup(&mut env);
    let source = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let path = line(&mut env, [0.0, 5.0], [10.0, 5.0]);
    let mut prompts = CommandLine::new();
    let mut seed = 400;
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
        prompts.submit(&t, text).unwrap()
    };
    submit(&env, "arraypolar");
    submit(&env, &source);
    submit(&env, "0,0");
    submit(&env, "4");
    let Submitted::Ran(polar) = submit(&env, "360") else {
        panic!("the polar array did not run");
    };
    assert_eq!(polar.value["objects"].as_array().unwrap().len(), 1);
    let polar_found = arrays(&env.document.snapshot());
    assert_eq!(
        array::items(&polar_found[0].1, None, &Tolerance::DEFAULT)
            .unwrap()
            .len(),
        4
    );

    let source2 = line(&mut env, [0.0, 0.0], [1.0, 0.0]);
    let mut prompts = CommandLine::new();
    let mut seed = 500;
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
        prompts.submit(&t, text).unwrap()
    };
    submit(&env, "arraypath");
    submit(&env, &source2);
    submit(&env, &path);
    let Submitted::Ran(along) = submit(&env, "3") else {
        panic!("the path array did not run");
    };
    assert_eq!(along.value["objects"].as_array().unwrap().len(), 1);
    assert_eq!(arrays(&env.document.snapshot()).len(), 2);
}
