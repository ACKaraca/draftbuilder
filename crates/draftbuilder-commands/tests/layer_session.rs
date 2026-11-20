//! The session layer state, headless: the previous layer settings (`LAYERP`), isolation
//! (`LAYISO`, `LAYUNISO`) and the layer walk (`LAYWALK`, `draftbuilder.layer.walk_end`). Each
//! command's exact flags, its refusals, and undo of its changes.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_doc::{ColorSource, Value};
use arc_draft::types::{LAYER, layer};
use common::{Env, name_of};
use serde_json::{Value as Json, json};

/// A drawing with no entities; the space is ready for lines.
fn drawing(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({}));
}

/// A line on the layer named `name`, drawn in the model space.
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

fn layer_named(env: &Env, name: &str) -> arc_base::ObjectId {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .map(|(_, o)| o.id)
        .find(|id| name_of(&snapshot, *id).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no layer {name}"))
}

fn field(env: &Env, id: arc_base::ObjectId, index: usize) -> Option<Value> {
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id).unwrap();
    snapshot.get(handle).unwrap().values[index].clone()
}

fn flag(env: &Env, name: &str, index: usize) -> bool {
    match field(env, layer_named(env, name), index) {
        Some(Value::Bool(b)) => b,
        other => panic!("not a flag: {other:?}"),
    }
}

fn is_on(env: &Env, name: &str) -> bool {
    flag(env, name, layer::ON)
}

fn is_locked(env: &Env, name: &str) -> bool {
    flag(env, name, layer::LOCKED)
}

fn index_color_of(env: &Env, name: &str) -> Option<u8> {
    match field(env, layer_named(env, name), layer::COLOR) {
        Some(Value::Color(c)) => match c.source {
            Some(ColorSource::Indexed(i)) => Some(i),
            _ => None,
        },
        _ => None,
    }
}

fn code_of(result: Result<Json, arc_base::Error>) -> &'static str {
    result.unwrap_err().code()
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

#[test]
fn previous_restores_the_flag_of_the_last_kept_change_and_refuses_when_none_is_kept() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    let line = line_on(&mut env, "Walls");
    env.run("draftbuilder.layer.off", json!({"objects": [line]}));
    assert!(!is_on(&env, "Walls"));
    env.run("draftbuilder.layer.previous", json!({}));
    assert!(is_on(&env, "Walls"));
    // The one change is used up: nothing is left to go back to.
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.previous", json!({}))),
        "db.cmd.layer_previous_none"
    );
}

#[test]
fn previous_steps_back_one_change_per_use_in_reverse_order() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run(
        "draftbuilder.layer.create",
        json!({"name": "Walls", "color": 3}),
    );
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [layer_named(&env, "Walls").to_string()], "color": 1}),
    );
    let line = line_on(&mut env, "Walls");
    env.run("draftbuilder.layer.off", json!({"objects": [line]}));
    assert_eq!(index_color_of(&env, "Walls"), Some(1));
    assert!(!is_on(&env, "Walls"));

    env.run("draftbuilder.layer.previous", json!({}));
    assert!(is_on(&env, "Walls"));
    assert_eq!(index_color_of(&env, "Walls"), Some(1));

    env.run("draftbuilder.layer.previous", json!({}));
    assert_eq!(index_color_of(&env, "Walls"), Some(3));
}

#[test]
fn previous_is_one_undoable_step() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    let line = line_on(&mut env, "Walls");
    env.run("draftbuilder.layer.off", json!({"objects": [line]}));
    env.run("draftbuilder.layer.previous", json!({}));
    assert!(is_on(&env, "Walls"));
    undo(&mut env);
    assert!(!is_on(&env, "Walls"));
}

#[test]
fn isolate_keeps_the_selected_layers_and_unisolate_restores_the_prior_flags() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    env.run("draftbuilder.layer.create", json!({"name": "C"}));
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [layer_named(&env, "C").to_string()], "on": false}),
    );
    let la = line_on(&mut env, "A");
    line_on(&mut env, "B");

    env.run(
        "draftbuilder.layer.isolate",
        json!({"objects": [la.clone()]}),
    );
    assert!(is_on(&env, "A"));
    assert!(!is_on(&env, "B"));
    assert!(!is_on(&env, "C"));
    assert!(!is_on(&env, "0"));

    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(is_on(&env, "A"));
    assert!(is_on(&env, "B"));
    assert!(!is_on(&env, "C"), "C was off before the isolation");
    assert!(is_on(&env, "0"));
}

#[test]
fn isolate_lock_mode_locks_the_other_layers_and_keeps_them_on() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    env.run(
        "draftbuilder.layer.isolate",
        json!({"objects": [la], "mode": "lock"}),
    );
    assert!(!is_locked(&env, "A"));
    assert!(is_locked(&env, "B"));
    assert!(is_on(&env, "B"));
    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(!is_locked(&env, "B"));
}

#[test]
fn a_second_isolation_unisolates_back_to_the_state_before_the_first() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    let lb = line_on(&mut env, "B");
    env.run("draftbuilder.layer.isolate", json!({"objects": [la]}));
    env.run("draftbuilder.layer.isolate", json!({"objects": [lb]}));
    assert!(!is_on(&env, "A"));
    assert!(is_on(&env, "B"));
    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(is_on(&env, "A"));
    assert!(is_on(&env, "B"));
    assert!(is_on(&env, "0"));
}

#[test]
fn unisolate_without_an_isolation_refuses_and_isolate_undoes_in_one_step() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.unisolate", json!({}))),
        "db.cmd.layer_not_active"
    );
    env.run("draftbuilder.layer.isolate", json!({"objects": [la]}));
    assert!(!is_on(&env, "B"));
    undo(&mut env);
    assert!(is_on(&env, "B"));
}

#[test]
fn walk_shows_each_group_and_the_end_restores_every_visibility() {
    let mut env = Env::new();
    drawing(&mut env);
    for name in ["A", "B", "C"] {
        env.run("draftbuilder.layer.create", json!({"name": name}));
    }
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [layer_named(&env, "C").to_string()], "on": false}),
    );
    env.run("draftbuilder.layer.walk", json!({"layers": ["A"]}));
    assert!(is_on(&env, "A"));
    assert!(!is_on(&env, "B"));
    assert!(!is_on(&env, "C"));
    env.run("draftbuilder.layer.walk", json!({"layers": ["b"]}));
    assert!(!is_on(&env, "A"));
    assert!(is_on(&env, "B"));
    env.run("draftbuilder.layer.walk_end", json!({}));
    assert!(is_on(&env, "A"));
    assert!(is_on(&env, "B"));
    assert!(!is_on(&env, "C"));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.walk_end", json!({}))),
        "db.cmd.layer_not_active"
    );
}

#[test]
fn walk_keep_leaves_the_shown_layers_visible_and_matches_wildcards() {
    let mut env = Env::new();
    drawing(&mut env);
    for name in ["A", "B", "Dims"] {
        env.run("draftbuilder.layer.create", json!({"name": name}));
    }
    env.run("draftbuilder.layer.walk", json!({"layers": ["A"]}));
    env.run("draftbuilder.layer.walk_end", json!({"keep": true}));
    assert!(is_on(&env, "A"));
    assert!(!is_on(&env, "B"));
    assert!(!is_on(&env, "Dims"));
    env.run("draftbuilder.layer.walk", json!({"layers": ["D*"]}));
    assert!(!is_on(&env, "A"));
    assert!(is_on(&env, "Dims"));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.walk", json!({"layers": ["Nope"]}))),
        "db.cmd.layer_unknown"
    );
}

#[test]
fn walk_over_many_layers_keeps_its_record_across_chunks() {
    let mut env = Env::new();
    drawing(&mut env);
    for index in 0..120 {
        env.run(
            "draftbuilder.layer.create",
            json!({"name": format!("L{index:03}")}),
        );
    }
    env.run("draftbuilder.layer.walk", json!({"layers": ["L000"]}));
    assert!(is_on(&env, "L000"));
    assert!(!is_on(&env, "L119"));
    env.run("draftbuilder.layer.walk_end", json!({}));
    assert!(is_on(&env, "L119"));
    assert!(is_on(&env, "L000"));
}

#[test]
fn previous_stops_at_a_change_too_large_to_keep_until_a_newer_change_comes() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    let line = line_on(&mut env, "A");
    env.run("draftbuilder.layer.off", json!({"objects": [line]}));
    // Two hundred layer flags in one change do not fit one history value: the command still
    // succeeds, and the history stops there.
    let names: Vec<String> = (0..200).map(|i| format!("M{i:03}")).collect();
    for name in &names {
        env.run("draftbuilder.layer.create", json!({"name": name}));
    }
    let ids: Vec<String> = names
        .iter()
        .map(|name| layer_named(&env, name).to_string())
        .collect();
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": ids, "on": false}),
    );
    assert!(!is_on(&env, "M000"));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.previous", json!({}))),
        "db.cmd.layer_previous_unkept"
    );
    // The stop is not consumed by the failure: it still stops, and nothing changed.
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.previous", json!({}))),
        "db.cmd.layer_previous_unkept"
    );
    assert!(!is_on(&env, "M000"));
    // A newer change puts a restorable entry in front of the stop.
    let again = line_on(&mut env, "A");
    env.run("draftbuilder.layer.on", json!({"objects": [again]}));
    env.run("draftbuilder.layer.previous", json!({}));
    assert!(
        !is_on(&env, "A"),
        "the newer change restored A's state before it"
    );
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.previous", json!({}))),
        "db.cmd.layer_previous_unkept"
    );
}

#[test]
fn previous_refuses_with_nothing_to_restore_when_undo_took_the_only_change_back() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    let line = line_on(&mut env, "A");
    env.run("draftbuilder.layer.off", json!({"objects": [line]}));
    undo(&mut env);
    assert!(is_on(&env, "A"));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.previous", json!({}))),
        "db.cmd.layer_previous_none"
    );
    assert!(is_on(&env, "A"), "a refused LAYERP changes nothing");
}

#[test]
fn previous_skips_an_undone_change_and_restores_the_older_one() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    let line = line_on(&mut env, "A");
    env.run("draftbuilder.layer.off", json!({"objects": [line.clone()]}));
    env.run("draftbuilder.layer.lock", json!({"objects": [line]}));
    undo(&mut env);
    assert!(!is_locked(&env, "A"));
    env.run("draftbuilder.layer.previous", json!({}));
    assert!(is_on(&env, "A"), "the older change is restored");
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.previous", json!({}))),
        "db.cmd.layer_previous_none"
    );
}

#[test]
fn unisolate_keeps_a_flag_the_user_changed_since_the_isolation() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    env.run("draftbuilder.layer.isolate", json!({"objects": [la]}));
    assert!(!is_on(&env, "B"));
    let other_line = line_on(&mut env, "B");
    env.run("draftbuilder.layer.on", json!({"objects": [other_line]}));
    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(is_on(&env, "B"), "the user turned B on after the isolation");
    assert!(is_on(&env, "A"));
    assert!(
        is_on(&env, "0"),
        "0 had no change by the user and comes back"
    );
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.unisolate", json!({}))),
        "db.cmd.layer_not_active"
    );
}

#[test]
fn walk_end_keeps_a_visibility_the_user_changed_during_the_walk() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    env.run("draftbuilder.layer.walk", json!({"layers": ["A"]}));
    assert!(!is_on(&env, "B"));
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [layer_named(&env, "B").to_string()], "on": true}),
    );
    env.run("draftbuilder.layer.walk_end", json!({}));
    assert!(is_on(&env, "B"));
    assert!(is_on(&env, "A"));
}

#[test]
fn undo_then_unisolate_changes_nothing_and_clears_the_record() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    env.run("draftbuilder.layer.isolate", json!({"objects": [la]}));
    undo(&mut env);
    assert!(is_on(&env, "B"));
    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(is_on(&env, "B"));
    assert!(is_on(&env, "A"));
    assert!(is_on(&env, "0"));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.unisolate", json!({}))),
        "db.cmd.layer_not_active"
    );
}

#[test]
fn undo_of_an_unisolate_leaves_the_isolation_without_a_record() {
    // The limit of the record: it is consumed by the unisolate, and undo brings the drawing's
    // isolated state back without it. Unisolate then reports that no isolation is active.
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    env.run("draftbuilder.layer.isolate", json!({"objects": [la]}));
    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(is_on(&env, "B"));
    undo(&mut env);
    assert!(!is_on(&env, "B"));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.unisolate", json!({}))),
        "db.cmd.layer_not_active"
    );
}

#[test]
fn a_record_of_another_drawing_is_refused_and_kept() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "A"}));
    env.run("draftbuilder.layer.create", json!({"name": "B"}));
    let la = line_on(&mut env, "A");
    env.run("draftbuilder.layer.isolate", json!({"objects": [la]}));
    // Another drawing, in the same session: its document takes the place of this one.
    let mut other = Env::new();
    // Fingerprints come from the command seed: the other drawing is made at a different seed.
    for _ in 0..3 {
        other.run("draftbuilder.settings.get", json!({"name": "command.echo"}));
    }
    drawing(&mut other);
    std::mem::swap(&mut env.document, &mut other.document);
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.unisolate", json!({}))),
        "db.cmd.layer_state_other_drawing"
    );
    std::mem::swap(&mut env.document, &mut other.document);
    env.run("draftbuilder.layer.unisolate", json!({}));
    assert!(is_on(&env, "B"), "the record survived the refusal");
}
