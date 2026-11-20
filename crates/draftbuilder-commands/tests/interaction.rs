//! The interaction controller, headless: event sequences drive the command line, the pointer, the
//! selection and the dynamic input, and the outputs and the drawing are checked.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_draft::snap::{ConstraintSource, SnapKind};
use draftbuilder_commands::interaction::{
    BandMode, Controller, CrossingPicker, DynamicMode, Event, Outputs,
};
use serde_json::json;

use common::{Env, lines};

const APERTURE: f64 = 0.05;

/// A new drawing and a controller for it.
fn setup() -> (Env, Controller) {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let controller = Controller::new().unwrap();
    (env, controller)
}

fn send(env: &Env, controller: &mut Controller, event: Event) -> Outputs {
    controller.handle(&env.document, event, &CrossingPicker)
}

fn submit(env: &Env, controller: &mut Controller, text: &str) -> Outputs {
    send(env, controller, Event::Submit(text.to_owned()))
}

fn click(env: &Env, controller: &mut Controller, x: f64, y: f64) -> Outputs {
    send(
        env,
        controller,
        Event::Click {
            at: [x, y],
            aperture: APERTURE,
        },
    )
}

fn move_to(env: &Env, controller: &mut Controller, x: f64, y: f64) -> Outputs {
    send(
        env,
        controller,
        Event::Move {
            at: [x, y],
            aperture: APERTURE,
        },
    )
}

fn errors_of(out: &Outputs) -> Vec<&'static str> {
    out.errors.iter().map(|e| e.code).collect()
}

fn space(env: &Env) -> String {
    env.space().to_string()
}

/// Adds a segment entity to the model space, the way a test fixture does.
fn add_segment(env: &mut Env, start: [f64; 2], end: [f64; 2]) -> String {
    let owner = space(env);
    env.run(
        "core.draft.entity.create",
        json!({"owner": owner, "geometry": {"kind": "segment", "start": start, "end": end}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The segments of the drawing as (start, end) pairs, in sorted order.
fn segments(env: &Env) -> Vec<([f64; 2], [f64; 2])> {
    lines(&env.document.snapshot())
        .into_iter()
        .map(|(_, start, end)| (start, end))
        .collect()
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

#[test]
fn a_line_is_drawn_by_two_clicks_and_enter_ends_the_chain() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    let out = click(&env, &mut c, 0.0, 0.0);
    let prompt = out.prompt.clone().unwrap();
    assert_eq!(prompt.command, "draftbuilder.draw.line");
    assert!(
        prompt
            .expects
            .contains(&draftbuilder_commands::interaction::Expect::Point)
    );
    let out = click(&env, &mut c, 10.0, 0.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.prompt.is_some());
    // The chain ends with the default (END) on an empty line.
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.prompt.is_none());
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 0.0])]);
}

#[test]
fn a_typed_relative_point_answers_the_second_point() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 1.0, 2.0);
    let out = submit(&env, &mut c, "@10,0");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([1.0, 2.0], [11.0, 2.0])]);
}

#[test]
fn ortho_constrains_the_second_click_to_the_axis() {
    let (mut env, mut c) = setup();
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.ortho", "value": true}),
    );
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    let out = move_to(&env, &mut c, 10.0, 3.0);
    let cursor = out.cursor.unwrap();
    assert_eq!(cursor.source, ConstraintSource::Ortho);
    assert!(near(cursor.point, [10.0, 0.0]), "{:?}", cursor.point);
    click(&env, &mut c, 10.0, 3.0);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 0.0])]);
}

#[test]
fn a_running_endpoint_snap_lands_exactly_on_the_endpoint_and_shows_its_marker() {
    let (mut env, mut c) = setup();
    add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.precision.osnap", json!({"modes": "END"}));
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 2.0, 3.0);
    let out = move_to(&env, &mut c, 10.02, 0.01);
    let snap = out.snap.expect("the endpoint is snapped");
    assert_eq!(snap.kind, SnapKind::Endpoint);
    assert!(near(snap.point, [10.0, 0.0]));
    click(&env, &mut c, 10.02, 0.01);
    submit(&env, &mut c, "");
    assert!(
        segments(&env)
            .iter()
            .any(|(s, e)| near(*s, [2.0, 3.0]) && near(*e, [10.0, 0.0])),
        "{:?}",
        segments(&env)
    );
}

#[test]
fn direct_distance_takes_the_length_along_the_pointer_direction() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    move_to(&env, &mut c, 5.0, 0.0);
    let out = submit(&env, &mut c, "20");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [20.0, 0.0])]);
}

#[test]
fn a_keyword_option_of_the_prompt_is_offered_and_activated() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    let out = click(&env, &mut c, 10.0, 10.0);
    let prompt = out.prompt.unwrap();
    let close = prompt.options.iter().find(|o| o.id == "close").unwrap();
    assert_eq!(close.shortcut, Some('c'));
    assert_eq!(prompt.default.as_deref(), Some("end"));
    let out = send(&env, &mut c, Event::Option("close".to_owned()));
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env).len(), 3);
}

#[test]
fn escape_cancels_the_command_and_leaves_no_partial_entity() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    let out = send(&env, &mut c, Event::Escape);
    assert!(out.prompt.is_none());
    assert!(
        out.history
            .iter()
            .any(|l| l.text == "draftbuilder.draw.line"
                && l.role == draftbuilder_commands::interaction::LineRole::Cancelled)
    );
    assert!(segments(&env).is_empty());
}

#[test]
fn enter_on_an_empty_line_repeats_the_last_command() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env).len(), 1);
    let out = submit(&env, &mut c, "");
    let prompt = out.prompt.expect("LINE starts again");
    assert_eq!(prompt.command, "draftbuilder.draw.line");
    assert_eq!(segments(&env).len(), 1, "no duplicate was drawn");
}

/// Three lines: `a` inside the window (0,0)-(6,6), `b` crossing its top edge, `c` outside.
fn selection_fixture(env: &mut Env) -> (String, String, String) {
    let a = add_segment(env, [1.0, 1.0], [4.0, 4.0]);
    let b = add_segment(env, [5.0, -1.0], [5.0, 20.0]);
    let c = add_segment(env, [20.0, 20.0], [30.0, 30.0]);
    (a, b, c)
}

#[test]
fn a_click_selects_an_object_and_a_window_or_crossing_follows_the_drag_direction() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);

    let out = click(&env, &mut c, 2.5, 2.5);
    assert_eq!(out.selection.len(), 1);
    assert_eq!(out.selection[0].to_string(), a);
    send(&env, &mut c, Event::Escape);
    assert!(send(&env, &mut c, Event::Escape).selection.is_empty());

    // Left to right: a window, only the object fully inside.
    click(&env, &mut c, 0.0, 0.0);
    let out = move_to(&env, &mut c, 3.0, 3.0);
    assert_eq!(out.band.unwrap().mode, BandMode::Window);
    let out = click(&env, &mut c, 6.0, 6.0);
    let picked: Vec<String> = out.selection.iter().map(ToString::to_string).collect();
    assert_eq!(picked, vec![a.clone()]);
    send(&env, &mut c, Event::Escape);

    // Right to left: a crossing, the objects it touches.
    click(&env, &mut c, 6.0, 6.0);
    let out = move_to(&env, &mut c, 0.0, 0.0);
    assert_eq!(out.band.unwrap().mode, BandMode::Crossing);
    let out = click(&env, &mut c, 0.0, 0.0);
    let picked: Vec<String> = out.selection.iter().map(ToString::to_string).collect();
    assert!(picked.contains(&a) && picked.contains(&b), "{picked:?}");
    assert!(!picked.contains(&outside));
}

#[test]
fn a_command_uses_the_selection_made_by_clicks() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 6.0, 6.0); // window: a
    click(&env, &mut c, 6.0, 6.0);
    click(&env, &mut c, 0.0, 0.0); // crossing: a and b, added
    let out = submit(&env, &mut c, "ERASE");
    assert_eq!(out.prompt.unwrap().command, "draftbuilder.modify.erase");
    let out = submit(&env, &mut c, "selection");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let snapshot = env.document.snapshot();
    let left = lines(&snapshot);
    assert_eq!(left.len(), 1, "a and b are erased");
    let kept = outside.parse().unwrap();
    assert!(snapshot.resolve(kept).is_ok(), "the outside line is kept");
    assert!(snapshot.resolve(a.parse().unwrap()).is_err());
    assert!(snapshot.resolve(b.parse().unwrap()).is_err());
}

#[test]
fn a_transparent_setting_runs_beside_a_waiting_line() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    let out = submit(&env, &mut c, "'SETVAR name=draft.ortho value=true");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.unwrap().command, "draftbuilder.draw.line");
    let out = move_to(&env, &mut c, 10.0, 3.0);
    assert_eq!(out.cursor.unwrap().source, ConstraintSource::Ortho);
}

#[test]
fn undo_steps_back_a_point_in_a_command_and_the_drawing_after_it() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    let out = send(&env, &mut c, Event::Undo);
    assert!(out.prompt.is_some(), "the command still waits");
    send(&env, &mut c, Event::Escape);
    assert!(segments(&env).is_empty());

    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env).len(), 1);
    send(&env, &mut c, Event::Undo);
    assert!(segments(&env).is_empty(), "the undo removed the line");
}

#[test]
fn the_snap_index_is_rebuilt_only_when_the_revision_changes() {
    let (mut env, mut c) = setup();
    add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.precision.osnap", json!({"modes": "END"}));
    submit(&env, &mut c, "LINE");
    assert_eq!(c.snap_index_builds(), 0);
    for step in 0..10 {
        move_to(&env, &mut c, 5.0 + f64::from(step) * 0.01, 1.0);
    }
    assert_eq!(c.snap_index_builds(), 1, "one build for ten moves");
    add_segment(&mut env, [0.0, 5.0], [10.0, 5.0]);
    move_to(&env, &mut c, 5.0, 1.0);
    move_to(&env, &mut c, 5.1, 1.0);
    assert_eq!(c.snap_index_builds(), 2, "a new revision builds once");
}

#[test]
fn typed_dynamic_fields_lock_and_compose_the_point() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    move_to(&env, &mut c, 3.0, 4.0);
    let out = send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "10".to_owned(),
        },
    );
    let dynamic = out.dynamic.clone().unwrap();
    assert_eq!(dynamic.mode, DynamicMode::Relative);
    assert!(dynamic.fields[0].locked && !dynamic.fields[1].locked);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "0".to_owned(),
        },
    );
    send(&env, &mut c, Event::Tab);
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 0.0])]);
}

#[test]
fn an_invalid_locked_field_is_an_error_and_keeps_the_prompt() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "ten".to_owned(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert_eq!(errors_of(&out), vec!["db.cmd.value_invalid"]);
    assert!(out.prompt.is_some());
}

#[test]
fn hovering_an_endpoint_acquires_it_and_a_tracking_path_runs_from_it() {
    let (mut env, mut c) = setup();
    add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.precision.osnap", json!({"modes": "END"}));
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 2.0, 3.0);
    // Hovering the endpoint acquires it.
    move_to(&env, &mut c, 10.01, 0.01);
    // Away from any snap, near the horizontal path through the acquired endpoint.
    let out = move_to(&env, &mut c, 5.0, 0.02);
    let cursor = out.cursor.unwrap();
    assert_eq!(cursor.source, ConstraintSource::Tracking);
    assert!(near(cursor.point, [5.0, 0.0]), "{:?}", cursor.point);
    assert!(!out.tracking.is_empty());
    assert!(near(out.tracking[0].origin, [10.0, 0.0]));
}
