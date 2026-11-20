//! The interaction controller, headless: event sequences drive the command line, the pointer, the
//! selection and the dynamic input, and the outputs and the drawing are checked.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_command::Permission;
use arc_draft::snap::{ConstraintSource, SnapKind};
use draftbuilder_commands::interaction::{
    BandMode, Controller, CrossingPicker, DynamicMode, Event, Expect, LineRole, ModifierStep,
    Outputs,
};
use draftbuilder_commands::precision::points::Modifier;
use serde_json::json;
use std::collections::BTreeSet;

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
fn a_command_uses_the_selection_made_by_clicks_without_a_prompt() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 6.0, 6.0); // window: a
    click(&env, &mut c, 6.0, 6.0);
    click(&env, &mut c, 0.0, 0.0); // crossing: a and b, added
    // Noun-verb: the selection is the command's objects, so no prompt waits.
    let out = submit(&env, &mut c, "ERASE");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.prompt.is_none());
    assert!(out.selection.is_empty(), "the used selection is consumed");
    let snapshot = env.document.snapshot();
    assert_eq!(lines(&snapshot).len(), 1, "a and b are erased");
    let kept = outside.parse().unwrap();
    assert!(snapshot.resolve(kept).is_ok(), "the outside line is kept");
    assert!(snapshot.resolve(a.parse().unwrap()).is_err());
    assert!(snapshot.resolve(b.parse().unwrap()).is_err());
}

#[test]
fn a_command_with_a_selection_still_asks_for_its_points() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    click(&env, &mut c, 2.5, 2.5); // a
    let out = submit(&env, &mut c, "ROTATE");
    let prompt = out.prompt.unwrap();
    assert_eq!(prompt.command, "draftbuilder.modify.rotate");
    assert!(prompt.expects.contains(&Expect::Point), "{prompt:?}");
    assert_eq!(
        out.selection.len(),
        0,
        "a is consumed by the rotation's objects"
    );
    let _ = a;
}

#[test]
fn the_selection_keywords_are_offered_and_an_option_activates_the_window_keyword() {
    let (mut env, mut c) = setup();
    selection_fixture(&mut env);
    let out = submit(&env, &mut c, "ERASE");
    let prompt = out.prompt.unwrap();
    let window = prompt.options.iter().find(|o| o.id == "window").unwrap();
    assert_eq!(window.label, "db-sel-option-window");
    assert!(prompt.options.iter().any(|o| o.id == "wpolygon"));
    send(&env, &mut c, Event::Option("window".to_owned()));
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 6.0, 6.0);
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(lines(&env.document.snapshot()).len(), 2, "only a is erased");
}

#[test]
fn a_window_keyword_takes_two_corners_whatever_their_order() {
    let (mut env, mut c) = setup();
    let (_, b, outside) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "WINDOW");
    // Right to left, but the keyword forces a window: a is inside, b crosses the edge.
    click(&env, &mut c, 6.0, 6.0);
    let out = move_to(&env, &mut c, 0.0, 0.0);
    assert_eq!(out.band.unwrap().mode, BandMode::Window);
    click(&env, &mut c, 0.0, 0.0);
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.prompt.is_none(), "the one erase answer ran");
    let snapshot = env.document.snapshot();
    assert!(snapshot.resolve(b.parse().unwrap()).is_ok(), "b is kept");
    assert!(snapshot.resolve(outside.parse().unwrap()).is_ok());
    assert_eq!(lines(&snapshot).len(), 2);
}

#[test]
fn a_crossing_keyword_takes_what_the_box_touches_whatever_the_drag_direction() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "crossing");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 6.0, 6.0);
    // Enter finishes the selection that the box gathered.
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let snapshot = env.document.snapshot();
    assert!(snapshot.resolve(a.parse().unwrap()).is_err());
    assert!(snapshot.resolve(b.parse().unwrap()).is_err());
    assert!(snapshot.resolve(outside.parse().unwrap()).is_ok());
}

#[test]
fn a_window_polygon_keyword_takes_vertices_until_an_empty_line() {
    let (mut env, mut c) = setup();
    let (a, b, _) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "wpolygon");
    for (x, y) in [(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)] {
        let out = click(&env, &mut c, x, y);
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert!(out.prompt.is_some(), "the polygon waits for its vertices");
    }
    // An empty line closes a valid polygon; a second one finishes the gathered set.
    submit(&env, &mut c, "");
    let out = submit(&env, &mut c, "");
    assert!(out.prompt.is_none(), "an empty line closes a valid polygon");
    let snapshot = env.document.snapshot();
    assert!(snapshot.resolve(a.parse().unwrap()).is_err(), "a is inside");
    assert!(
        snapshot.resolve(b.parse().unwrap()).is_ok(),
        "b crosses the boundary"
    );
}

#[test]
fn a_crossing_polygon_and_a_fence_keyword_select_what_their_outline_touches() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "cpolygon");
    for (x, y) in [(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)] {
        click(&env, &mut c, x, y);
    }
    submit(&env, &mut c, "");
    submit(&env, &mut c, "");
    let snapshot = env.document.snapshot();
    assert!(snapshot.resolve(a.parse().unwrap()).is_err());
    assert!(snapshot.resolve(b.parse().unwrap()).is_err());
    assert!(snapshot.resolve(outside.parse().unwrap()).is_ok());

    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "FENCE");
    click(&env, &mut c, -5.0, 2.0);
    click(&env, &mut c, 6.5, 2.0);
    submit(&env, &mut c, "");
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let snapshot = env.document.snapshot();
    assert!(
        snapshot.resolve(a.parse().unwrap()).is_err(),
        "the fence crosses a"
    );
    assert!(snapshot.resolve(b.parse().unwrap()).is_err(), "and b");
    assert!(snapshot.resolve(outside.parse().unwrap()).is_ok());
}

#[test]
fn a_fence_with_one_vertex_is_refused_and_the_fence_keeps_waiting() {
    let (mut env, mut c) = setup();
    selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "fence");
    click(&env, &mut c, -5.0, 2.0);
    let out = submit(&env, &mut c, "");
    assert_eq!(errors_of(&out), vec!["db.cmd.selection_invalid"]);
    assert!(out.prompt.is_some());
    assert_eq!(lines(&env.document.snapshot()).len(), 3, "nothing erased");
}

#[test]
fn multiple_mode_gathers_gestures_until_an_empty_line_and_undo_takes_back_the_last() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "multiple");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 6.0, 6.0); // window: a, gathered
    let out = click(&env, &mut c, 19.0, 19.0);
    assert!(out.prompt.is_some(), "multiple mode keeps the prompt");
    let out = click(&env, &mut c, 31.0, 31.0); // window: c, gathered
    assert_eq!(out.selection.len(), 2, "the gathered set is shown");
    let out = submit(&env, &mut c, "undo");
    assert_eq!(out.selection.len(), 1, "undo takes back the window of c");
    let out = submit(&env, &mut c, "");
    assert!(out.prompt.is_none(), "the empty line answers with a");
    let snapshot = env.document.snapshot();
    assert!(snapshot.resolve(a.parse().unwrap()).is_err());
    assert!(snapshot.resolve(b.parse().unwrap()).is_ok());
    assert!(snapshot.resolve(outside.parse().unwrap()).is_ok());
}

#[test]
fn remove_takes_an_object_out_of_the_gathered_set_and_add_puts_it_back() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "multiple");
    let out = click(&env, &mut c, 2.5, 2.5);
    assert_eq!(out.selection.len(), 1);
    submit(&env, &mut c, "remove");
    let out = click(&env, &mut c, 2.5, 2.5);
    assert!(out.selection.is_empty());
    submit(&env, &mut c, "ADD");
    click(&env, &mut c, 2.5, 2.5);
    let out = submit(&env, &mut c, "");
    assert!(out.prompt.is_none());
    assert!(env.document.snapshot().resolve(a.parse().unwrap()).is_err());
}

#[test]
fn escape_drops_the_gathered_set_and_keeps_the_prompt() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "multiple");
    click(&env, &mut c, 2.5, 2.5);
    // One Escape ends the command with the set it gathered; nothing is erased.
    let out = send(&env, &mut c, Event::Escape);
    assert!(out.selection.is_empty());
    assert!(out.prompt.is_none(), "one Escape cancels the command");
    assert!(env.document.snapshot().resolve(a.parse().unwrap()).is_ok());
}

#[test]
fn a_click_on_empty_space_at_a_selection_prompt_starts_a_window_by_drag_direction() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    click(&env, &mut c, 0.0, 0.0);
    let out = move_to(&env, &mut c, 6.0, 6.0);
    assert_eq!(out.band.unwrap().mode, BandMode::Window);
    click(&env, &mut c, 6.0, 6.0);
    submit(&env, &mut c, "");
    let snapshot = env.document.snapshot();
    assert!(
        snapshot.resolve(a.parse().unwrap()).is_err(),
        "the window takes a"
    );
    assert!(snapshot.resolve(b.parse().unwrap()).is_ok());

    submit(&env, &mut c, "ERASE");
    click(&env, &mut c, 6.0, 6.0);
    let out = move_to(&env, &mut c, 0.0, 0.0);
    assert_eq!(out.band.unwrap().mode, BandMode::Crossing);
    click(&env, &mut c, 0.0, 0.0);
    submit(&env, &mut c, "");
    let snapshot = env.document.snapshot();
    assert!(
        snapshot.resolve(b.parse().unwrap()).is_err(),
        "the crossing takes b"
    );
    assert!(snapshot.resolve(outside.parse().unwrap()).is_ok());
}

#[test]
fn shift_click_takes_an_object_out_of_the_selection_without_a_command() {
    let (mut env, mut c) = setup();
    let (a, b, _) = selection_fixture(&mut env);
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 6.0, 6.0);
    click(&env, &mut c, 6.0, 6.0);
    click(&env, &mut c, 0.0, 0.0); // crossing: a and b
    let out = send(
        &env,
        &mut c,
        Event::ShiftClick {
            at: [5.0, 10.0],
            aperture: APERTURE,
        },
    );
    let picked: Vec<String> = out.selection.iter().map(ToString::to_string).collect();
    assert_eq!(picked, vec![a], "b leaves the selection, a stays");
    assert!(!picked.contains(&b));
}

#[test]
fn a_lasso_selects_by_window_or_crossing_polygon_as_the_drag_ends() {
    let (mut env, mut c) = setup();
    let (a, b, outside) = selection_fixture(&mut env);
    // Ends at the start's x or to the right: a window, so only a is inside.
    let out = send(
        &env,
        &mut c,
        Event::Lasso {
            path: vec![
                [-1.0, -1.0],
                [7.0, -1.0],
                [7.0, 7.0],
                [-1.0, 7.0],
                [-1.0, -1.0],
            ],
        },
    );
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let picked: Vec<String> = out.selection.iter().map(ToString::to_string).collect();
    assert_eq!(picked, vec![a.clone()]);
    // Ends left of its start: a crossing polygon, so b is picked too, the outside line is not.
    let out = send(
        &env,
        &mut c,
        Event::Lasso {
            path: vec![
                [7.0, -1.0],
                [-1.0, -1.0],
                [-1.0, 7.0],
                [7.0, 7.0],
                [6.5, -1.0],
            ],
        },
    );
    assert_eq!(out.selection.len(), 2, "{:?}", out.selection);
    assert!(out.selection.iter().any(|id| id.to_string() == b));
    assert!(!out.selection.iter().any(|id| id.to_string() == outside));
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

/// The end of a line from (0, 0) whose distance field holds `distance` (angle 0) in a millimeter
/// drawing, as a dynamic field types it (DB-SNAP-024).
fn field_distance_end(distance: &str) -> [f64; 2] {
    let (env, mut c) = setup_in("millimeter");
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: distance.to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "0".to_owned(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    let lines = segments(&env);
    assert_eq!(lines.len(), 1, "{distance}");
    lines[0].1
}

/// The same distance typed on the command line, as `@distance,0` from the same start.
fn typed_distance_end(distance: &str) -> [f64; 2] {
    let (env, mut c) = setup_in("millimeter");
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "0,0");
    let out = submit(&env, &mut c, &format!("@{distance},0"));
    assert!(out.errors.is_empty(), "{distance}: {:?}", out.errors);
    submit(&env, &mut c, "");
    segments(&env)[0].1
}

#[test]
fn a_typed_field_length_gives_the_value_the_command_line_gives() {
    // DB-SNAP-024: feet and inches, a unit suffix and an expression, in a millimeter drawing; a bare
    // number is in the drawing's unit.
    for (text, expected_mm) in [("3'-6\"", 1066.8), ("1.5 m", 1500.0), ("2*250", 500.0)] {
        let field = field_distance_end(text);
        let typed = typed_distance_end(text);
        assert!(
            near(field, typed),
            "{text}: field {field:?}, command line {typed:?}"
        );
        assert!(
            (field[0] - expected_mm).abs() < 1e-9 && field[1].abs() < 1e-9,
            "{text}: {field:?}"
        );
    }
}

#[test]
fn a_locked_length_field_shows_the_value_it_reads_in_drawing_units() {
    let (env, mut c) = setup_in("millimeter");
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    let out = send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "3'-6\"".to_owned(),
        },
    );
    let dynamic = out.dynamic.unwrap();
    assert!(
        (dynamic.fields[0].value - 1066.8).abs() < 1e-9,
        "{dynamic:?}"
    );
    assert!(dynamic.fields[0].locked);
}

#[test]
fn a_locked_angle_field_reads_units_the_command_line_reads() {
    // A gon (gradian) is a hundredth of a right angle: 100 gon is 90 degrees, so the line goes up.
    let (env, mut c) = setup_in("millimeter");
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "1000".to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "100 gon".to_owned(),
        },
    );
    submit(&env, &mut c, "");
    submit(&env, &mut c, "");
    let end = segments(&env)[0].1;
    assert!(near(end, [0.0, 1000.0]), "{end:?}");
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

#[test]
fn the_line_state_and_refresh_report_the_active_command() {
    let (env, mut c) = setup();
    assert!(c.active_prompt().is_none());
    assert!(!c.repeatable());
    submit(&env, &mut c, "LINE");
    assert!(c.active_prompt().is_some());
    assert_eq!(c.answers(), 0);
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    assert_eq!(c.answers(), 2);
    submit(&env, &mut c, "");
    assert!(c.active_prompt().is_none());
    assert_eq!(c.answers(), 0);
    assert!(c.repeatable());
    let out = c.refresh(&env.document);
    assert!(out.prompt.is_none());
    assert_eq!(segments(&env).len(), 1);
}

#[test]
fn a_session_without_edit_cannot_draw_a_line_or_step_the_history() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env).len(), 1);

    let read: BTreeSet<Permission> = [Permission::Read].into_iter().collect();
    c.set_permissions(&read);
    let mut codes = Vec::new();
    codes.extend(errors_of(&submit(&env, &mut c, "LINE")));
    codes.extend(errors_of(&click(&env, &mut c, 20.0, 0.0)));
    codes.extend(errors_of(&click(&env, &mut c, 30.0, 0.0)));
    codes.extend(errors_of(&submit(&env, &mut c, "")));
    assert!(!codes.is_empty(), "a read-only session must be refused");
    assert_eq!(segments(&env).len(), 1, "the refused line wrote nothing");

    let undo = send(&env, &mut c, Event::Undo);
    assert!(!errors_of(&undo).is_empty(), "undo needs Edit");
    let redo = send(&env, &mut c, Event::Redo);
    assert!(!errors_of(&redo).is_empty(), "redo needs Edit");
    assert_eq!(segments(&env).len(), 1);
}

#[test]
fn redo_restores_the_line_that_undo_removed() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    submit(&env, &mut c, "");
    let out = send(&env, &mut c, Event::Undo);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(segments(&env).is_empty());

    let out = send(&env, &mut c, Event::Redo);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 0.0])]);
    assert_eq!(out.history.last().map(|l| l.role), Some(LineRole::Redo));
}

/// The circles of the model space as (center, radius), in sorted order.
fn circles(env: &Env) -> Vec<([f64; 2], f64)> {
    let snapshot = env.document.snapshot();
    let mut out: Vec<([f64; 2], f64)> = common::entity_ids(&snapshot)
        .iter()
        .filter_map(|id| {
            let id = id.parse::<arc_base::ObjectId>().ok()?;
            match common::curve_of_entity(&snapshot, id) {
                arc_geom::Curve2::Circle(circle) => {
                    let c = circle.center();
                    Some(([c.x, c.y], circle.radius()))
                }
                _ => None,
            }
        })
        .collect();
    out.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]).then(a.0[1].total_cmp(&b.0[1])));
    out
}

fn modifier_at(out: &Outputs) -> Option<(Modifier, ModifierStep)> {
    out.modifier.map(|m| (m.modifier, m.step))
}

#[test]
fn from_takes_an_endpoint_base_and_an_offset_from_it_in_the_dynamic_fields() {
    let (mut env, mut c) = setup();
    add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.precision.osnap", json!({"modes": "END"}));
    submit(&env, &mut c, "LINE");
    let out = submit(&env, &mut c, "FROM");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::First))
    );
    // The base is snapped to the endpoint; it is not the answer of the first point.
    let out = click(&env, &mut c, 10.01, 0.01);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::Second))
    );
    assert!(out.prompt.is_some());
    // The offset is 5 at 90 degrees from the base.
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "5".to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "90".to_owned(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.modifier, None);
    assert_eq!(c.answers(), 1);
    click(&env, &mut c, 20.0, 5.0);
    submit(&env, &mut c, "");
    assert_eq!(
        segments(&env),
        vec![([0.0, 0.0], [10.0, 0.0]), ([10.0, 5.0], [20.0, 5.0])]
    );
}

#[test]
fn a_hole_centre_from_an_endpoint_offset_takes_a_typed_radius() {
    // W01 step 4: a circle whose centre is an offset from a snapped endpoint.
    let (mut env, mut c) = setup();
    add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.precision.osnap", json!({"modes": "END"}));
    submit(&env, &mut c, "CIRCLE");
    submit(&env, &mut c, "FROM");
    click(&env, &mut c, 10.0, 0.01);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "4".to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "180".to_owned(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let out = submit(&env, &mut c, "2");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.prompt.is_none());
    let found = circles(&env);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(near(found[0].0, [6.0, 0.0]), "{found:?}");
    assert!((found[0].1 - 2.0).abs() < 1e-9, "{found:?}");
}

#[test]
fn a_temporary_track_origin_aligns_the_next_point_only() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    let out = submit(&env, &mut c, "TEMPTRACK");
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::TempTrack, ModifierStep::First))
    );
    let out = click(&env, &mut c, 3.0, 4.0);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::TempTrack, ModifierStep::Second))
    );
    // The horizontal path through the origin pulls the point to y = 4.
    let out = move_to(&env, &mut c, 8.0, 4.02);
    assert_eq!(
        out.cursor.map(|c| c.source),
        Some(ConstraintSource::Tracking)
    );
    assert!(near(out.cursor.unwrap().point, [8.0, 4.0]));
    let out = click(&env, &mut c, 8.0, 4.02);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.modifier, None);
    // One-shot: the origin no longer offers a path for the next point.
    let out = move_to(&env, &mut c, 20.0, 4.02);
    assert!(out.tracking.is_empty(), "{:?}", out.tracking);
    click(&env, &mut c, 20.0, 4.0);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([8.0, 4.0], [20.0, 4.0])]);
}

#[test]
fn mid_between_answers_with_the_midpoint_of_two_points() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "MIDBETWEEN");
    let out = click(&env, &mut c, 0.0, 0.0);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::MidBetween, ModifierStep::Second))
    );
    let out = click(&env, &mut c, 10.0, 4.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.modifier, None);
    click(&env, &mut c, 20.0, 2.0);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([5.0, 2.0], [20.0, 2.0])]);
}

#[test]
fn a_waiting_modifier_reads_typed_text_as_its_point_and_an_empty_line_is_refused() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "FROM");
    // A typed point is the base; the empty line has no point to give.
    let out = submit(&env, &mut c, "5,5");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::Second))
    );
    let out = submit(&env, &mut c, "");
    assert_eq!(errors_of(&out), vec!["db.cmd.point_pending"]);
    let out = send(&env, &mut c, Event::Escape);
    assert_eq!(out.modifier, None);
    assert!(
        out.prompt.is_some(),
        "Escape ends the modifier, not the command"
    );
    let out = send(&env, &mut c, Event::Option("MIDBETWEEN".to_owned()));
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::MidBetween, ModifierStep::First))
    );
}

#[test]
fn a_world_prefix_reads_past_the_user_coordinate_system_and_a_typed_point_reads_in_it() {
    // DB-SNAP-021: `*` forces world coordinates; a plain typed point is in the current system.
    let (mut env, mut c) = setup();
    env.run(
        "draftbuilder.precision.ucs",
        json!({"mode": "origin", "origin": [1.0, 1.0], "x_point": [2.0, 1.0]}),
    );
    submit(&env, &mut c, "LINE");
    let out = submit(&env, &mut c, "*3,4");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let out = submit(&env, &mut c, "3,4");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([3.0, 4.0], [4.0, 5.0])]);
}

#[test]
fn a_typed_architectural_length_is_read_in_the_drawing_units() {
    // DB-SNAP-024 on the command line: feet and inches are read by the prompt's solver.
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "0,0");
    let out = submit(&env, &mut c, "@3'-6\",0");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    let (start, end) = segments(&env)[0];
    assert!(near(start, [0.0, 0.0]));
    assert!(
        (end[0] - 1.0668).abs() < 1e-9 && end[1].abs() < 1e-9,
        "{end:?}"
    );
}

/// A new drawing of `unit` and a controller for it.
fn setup_in(unit: &str) -> (Env, Controller) {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": unit}));
    (env, Controller::new().unwrap())
}

fn set_ortho(env: &mut Env) {
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.ortho", "value": true}),
    );
}

#[test]
fn an_option_that_answers_the_prompt_ends_a_waiting_modifier() {
    // The circle's first prompt offers the options two and three; the keyword answers the prompt
    // and the waiting FROM ends with it, so the next click is a point of the prompt it answers.
    let (env, mut c) = setup();
    submit(&env, &mut c, "CIRCLE");
    submit(&env, &mut c, "FROM");
    let out = click(&env, &mut c, 10.0, 0.0);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::Second))
    );
    let out = send(&env, &mut c, Event::Option("two".to_owned()));
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.modifier, None);
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("p1"));
    let out = click(&env, &mut c, 20.0, 0.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("p2"));
    assert_eq!(out.modifier, None);
}

#[test]
fn midbetween_at_the_second_point_of_a_line_ignores_the_last_point_under_ortho() {
    let (mut env, mut c) = setup();
    set_ortho(&mut env);
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    submit(&env, &mut c, "MIDBETWEEN");
    // Without ortho from the last point, the picks are exact; the midpoint is (15, 5).
    click(&env, &mut c, 10.0, 3.0);
    let out = click(&env, &mut c, 20.0, 7.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [15.0, 5.0])]);
}

#[test]
fn from_at_the_second_point_of_a_line_ignores_the_last_point_under_ortho() {
    let (mut env, mut c) = setup();
    set_ortho(&mut env);
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    submit(&env, &mut c, "FROM");
    click(&env, &mut c, 10.0, 3.0);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "5".to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "90".to_owned(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 8.0])]);
}

#[test]
fn a_direct_distance_typed_right_after_from_has_no_direction_from_the_base() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    submit(&env, &mut c, "FROM");
    click(&env, &mut c, 10.0, 3.0);
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "5".to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "90".to_owned(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        out.cursor.is_none(),
        "the cursor the modifier aligned is dropped at once"
    );
    // The FROM base (10, 3) to the offset point is straight up; a direction from it would
    // place the distance 5 at (10, 13). No direction is set, so the typed distance is refused.
    let out = submit(&env, &mut c, "5");
    assert_eq!(errors_of(&out), vec!["cmd.prompt.no_direction"]);
    assert!(out.prompt.is_some());
}

#[test]
fn from_in_a_millimeter_drawing_takes_its_base_and_offset_in_drawing_units() {
    let (mut env, mut c) = setup_in("millimeter");
    add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.precision.osnap", json!({"modes": "END"}));
    // Events are in meters; one drawing unit is a millimeter. The aperture is 0.05 mm.
    let tight = 0.000_05;
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "FROM");
    let out = send(
        &env,
        &mut c,
        Event::Click {
            at: [0.010, 0.000_02],
            aperture: tight,
        },
    );
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::Second))
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "5".to_owned(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "90".to_owned(),
        },
    );
    submit(&env, &mut c, "");
    let out = send(
        &env,
        &mut c,
        Event::Click {
            at: [0.020, 0.005],
            aperture: tight,
        },
    );
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    submit(&env, &mut c, "");
    assert_eq!(
        segments(&env),
        vec![([0.0, 0.0], [10.0, 0.0]), ([10.0, 5.0], [20.0, 5.0])]
    );
}

#[test]
fn from_under_a_rotated_user_coordinate_system_reads_typed_base_and_offset_in_that_system() {
    let (mut env, mut c) = setup();
    // The X axis of the system is the world Y axis; its Y axis is the world negative X axis.
    env.run(
        "draftbuilder.precision.ucs",
        json!({"mode": "origin", "origin": [1.0, 1.0], "x_point": [1.0, 2.0]}),
    );
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "FROM");
    // (2, 0) in the system is the world point (1, 3).
    let out = submit(&env, &mut c, "2,0");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::Second))
    );
    // Unprefixed coordinates are an offset from the base, in the system: (0, 4) in the world.
    let out = submit(&env, &mut c, "4,0");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.modifier, None);
    // (5, 5) absolute in the system is the world point (-4, 6).
    submit(&env, &mut c, "5,5");
    submit(&env, &mut c, "");
    assert_eq!(segments(&env), vec![([1.0, 7.0], [-4.0, 6.0])]);
}

#[test]
fn a_typed_text_the_point_reader_refuses_keeps_the_modifier_waiting() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    submit(&env, &mut c, "FROM");
    let out = submit(&env, &mut c, "not a point");
    assert_eq!(errors_of(&out), vec!["cmd.prompt.unreadable"]);
    assert_eq!(
        modifier_at(&out),
        Some((Modifier::From, ModifierStep::First))
    );
}

/// Moves `id` onto a new layer `name`, a layer of the drawing made for the test.
fn move_to_layer(env: &mut Env, id: &str, name: &str) {
    env.run("draftbuilder.layer.create", json!({"name": name}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [id], "layer": name}),
    );
}

#[test]
fn a_frozen_object_is_not_preselected_and_the_prompt_asks_instead() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    move_to_layer(&mut env, &a, "F");
    click(&env, &mut c, 2.5, 2.5); // a is the selection
    env.run("draftbuilder.layer.freeze", json!({"objects": [a.clone()]}));
    let out = submit(&env, &mut c, "ERASE");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        out.prompt.is_some(),
        "nothing usable is left: the prompt asks"
    );
    assert_eq!(lines(&env.document.snapshot()).len(), 3, "a is not erased");
}

#[test]
fn previous_at_a_prompt_does_not_return_a_frozen_object() {
    let (mut env, mut c) = setup();
    let (a, b, _) = selection_fixture(&mut env);
    move_to_layer(&mut env, &a, "F");
    click(&env, &mut c, 2.5, 2.5); // a
    click(&env, &mut c, 5.0, 10.0); // b, so the previous selection is a
    env.run("draftbuilder.layer.freeze", json!({"objects": [a.clone()]}));
    send(&env, &mut c, Event::Escape); // clears: previous is now a and b
    let out = submit(&env, &mut c, "ERASE");
    assert!(out.prompt.is_some());
    submit(&env, &mut c, "previous");
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let snapshot = env.document.snapshot();
    assert!(
        snapshot.resolve(a.parse().unwrap()).is_ok(),
        "the frozen a stays"
    );
    assert!(snapshot.resolve(b.parse().unwrap()).is_err(), "b is erased");
}

#[test]
fn a_locked_preselection_surfaces_the_command_error_and_keeps_the_selection() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    click(&env, &mut c, 2.5, 2.5); // a
    move_to_layer(&mut env, &a, "L");
    env.run("draftbuilder.layer.lock", json!({"objects": [a.clone()]}));
    let out = submit(&env, &mut c, "ERASE");
    assert!(
        !out.errors.is_empty(),
        "the locked layer refuses, and the error shows"
    );
    assert!(out.prompt.is_none(), "the command is ended");
    assert!(
        matches!(out.history.last(), Some(line) if line.role == LineRole::Cancelled),
        "{:?}",
        out.history
    );
    assert_eq!(
        out.selection,
        vec![a.parse().unwrap()],
        "the selection is kept"
    );
    assert!(env.document.snapshot().resolve(a.parse().unwrap()).is_ok());
}

#[test]
fn a_refused_preselection_is_reported_once_and_the_prompt_waits_with_the_selection() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    click(&env, &mut c, 2.5, 2.5); // a, a line: PEDIT takes polylines only
    let out = submit(&env, &mut c, "PEDIT");
    assert_eq!(out.errors.len(), 1, "{:?}", out.errors);
    let prompt = out.prompt.expect("the prompt waits");
    assert_eq!(prompt.command, "draftbuilder.modify.pedit");
    assert_eq!(out.selection, vec![a.parse().unwrap()]);
}

#[test]
fn a_consumed_preselection_comes_back_with_previous_after_a_cancel() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    click(&env, &mut c, 2.5, 2.5); // a
    let out = submit(&env, &mut c, "TRIM");
    assert!(out.prompt.is_some(), "trim asks for its targets next");
    assert!(
        out.selection.is_empty(),
        "a is consumed as the cutting edge"
    );
    send(&env, &mut c, Event::Escape); // cancels trim
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "previous");
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(env.document.snapshot().resolve(a.parse().unwrap()).is_err());
}

#[test]
fn a_gesture_that_changes_nothing_keeps_the_previous_selection() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    click(&env, &mut c, 2.5, 2.5); // a
    send(&env, &mut c, Event::Escape); // previous is a, current is empty
    click(&env, &mut c, -10.0, -10.0); // an empty window: selects nothing
    click(&env, &mut c, -9.0, -9.0);
    send(
        &env,
        &mut c,
        Event::ShiftClick {
            at: [5.0, 10.0],
            aperture: APERTURE,
        },
    ); // b is not selected: nothing leaves
    let out = submit(&env, &mut c, "ERASE");
    assert!(out.prompt.is_some(), "the selection is still empty");
    submit(&env, &mut c, "previous");
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(env.document.snapshot().resolve(a.parse().unwrap()).is_err());
}

#[test]
fn undo_with_nothing_gathered_steps_back_the_prompt_answer() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "OFFSET");
    let out = submit(&env, &mut c, "1");
    assert_eq!(out.prompt.unwrap().key, "objects");
    // Nothing is gathered: typed undo steps back the distance answer, as Undo does.
    let out = submit(&env, &mut c, "undo");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
        out.prompt.unwrap().key,
        "distance",
        "the distance prompt asks again"
    );
}

#[test]
fn the_typed_selection_keyword_answers_a_prompt_with_the_usable_selection() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    click(&env, &mut c, 2.5, 2.5); // a is the selection; OFFSET asks its distance first
    submit(&env, &mut c, "OFFSET");
    let out = submit(&env, &mut c, "1");
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("objects"));
    submit(&env, &mut c, "selection");
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let prompt = out.prompt.expect("offset asks the side");
    assert_ne!(
        prompt.key, "objects",
        "the selection answered the objects prompt"
    );
    assert!(env.document.snapshot().resolve(a.parse().unwrap()).is_ok());
}

#[test]
fn a_bowtie_lasso_is_refused_as_an_invalid_polygon_and_selects_nothing() {
    let (mut env, mut c) = setup();
    selection_fixture(&mut env);
    let out = send(
        &env,
        &mut c,
        Event::Lasso {
            path: vec![[0.0, 0.0], [6.0, 6.0], [6.0, 0.0], [0.0, 6.0]],
        },
    );
    assert_eq!(errors_of(&out), vec!["db.cmd.selection_invalid"]);
    assert!(out.selection.is_empty());
}

#[test]
fn a_lasso_while_a_polygon_is_pending_is_refused_and_keeps_its_vertices() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    submit(&env, &mut c, "ERASE");
    submit(&env, &mut c, "wpolygon");
    click(&env, &mut c, 0.0, 0.0);
    let out = send(
        &env,
        &mut c,
        Event::Lasso {
            path: vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]],
        },
    );
    assert_eq!(errors_of(&out), vec!["db.cmd.selection_invalid"]);
    for (x, y) in [(6.0, 0.0), (6.0, 6.0), (0.0, 6.0)] {
        click(&env, &mut c, x, y);
    }
    submit(&env, &mut c, "");
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        env.document.snapshot().resolve(a.parse().unwrap()).is_err(),
        "a is in the kept polygon"
    );
}

#[test]
fn a_lasso_drops_vertices_within_the_linear_tolerance_of_the_one_before() {
    let (mut env, mut c) = setup();
    let (a, _, _) = selection_fixture(&mut env);
    let out = send(
        &env,
        &mut c,
        Event::Lasso {
            path: vec![
                [-1.0, -1.0],
                [-1.0 + 1e-12, -1.0],
                [7.0, -1.0],
                [7.0, 7.0],
                [-1.0, 7.0],
                [-1.0 + 1e-12, -1.0 + 1e-12],
            ],
        },
    );
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let picked: Vec<String> = out.selection.iter().map(ToString::to_string).collect();
    assert_eq!(picked, vec![a]);
}

#[test]
fn a_typed_undo_steps_back_trim_from_its_target_prompt_to_the_edges_answer() {
    // TRIM: the edges answer is a click, the next prompt is the target; the typed word `undo` steps
    // back to the edges prompt (it was refused as an unreadable answer), which answers again by a
    // click and by a typed identifier.
    let (env, mut c) = setup();
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 0.0);
    click(&env, &mut c, 10.0, 0.0);
    submit(&env, &mut c, "");
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 5.0, -5.0);
    click(&env, &mut c, 5.0, 5.0);
    submit(&env, &mut c, "");
    let edge = common::entity_ids(&env.document.snapshot())[0].clone();

    submit(&env, &mut c, "TRIM");
    click(&env, &mut c, 0.0, 0.0);
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("target"));

    let out = submit(&env, &mut c, "undo");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("edges"));

    click(&env, &mut c, 0.0, 0.0);
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("target"));

    let out = submit(&env, &mut c, "undo");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let out = submit(&env, &mut c, &edge);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.as_ref().map(|p| p.key.as_str()), Some("target"));
    send(&env, &mut c, Event::Escape);
}

#[test]
fn a_set_prompt_gathers_picks_and_windows_until_enter_and_one_escape_cancels() {
    let (mut env, mut c) = setup();
    let (_a, _b, _c) = selection_fixture(&mut env);
    let before = segments(&env).len();
    // Picks add to the set; nothing is answered until Enter.
    submit(&env, &mut c, "ERASE");
    let first = click(&env, &mut c, 2.5, 2.5);
    assert_eq!(
        first
            .prompt
            .as_ref()
            .map(|p| p.expects.contains(&Expect::Selection)),
        Some(true)
    );
    assert_eq!(first.selection.len(), 1, "the pick is gathered");
    let second = click(&env, &mut c, 5.0, 10.0);
    assert_eq!(second.selection.len(), 2, "a second pick adds to the set");
    assert_eq!(segments(&env).len(), before, "nothing was erased yet");
    let done = submit(&env, &mut c, "");
    assert!(errors_of(&done).is_empty(), "{:?}", errors_of(&done));
    assert_eq!(
        segments(&env).len(),
        before - 2,
        "Enter finished the selection"
    );
    // Enter with nothing gathered ends the command instead of failing.
    submit(&env, &mut c, "ERASE");
    let none = submit(&env, &mut c, "");
    assert!(errors_of(&none).is_empty());
    assert!(none.prompt.is_none());
    // One Escape ends a command that collected answers.
    submit(&env, &mut c, "COPY");
    click(&env, &mut c, 2.5, 2.5);
    submit(&env, &mut c, "");
    let out = send(&env, &mut c, Event::Escape);
    assert!(out.prompt.is_none(), "one Escape cancels the whole command");
}

#[test]
fn zoom_hands_the_window_a_view_request_and_leaves_the_document_alone() {
    use draftbuilder_commands::interaction::ViewOut;
    let (env, mut c) = setup();
    let revision = env.document.snapshot().revision();
    let out = submit(&env, &mut c, "Z");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(submit(&env, &mut c, "E").view, Some(ViewOut::Extents));
    submit(&env, &mut c, "ZOOM");
    submit(&env, &mut c, "W");
    click(&env, &mut c, 1.0, 1.0);
    let out = click(&env, &mut c, 3.0, 2.0);
    assert_eq!(
        out.view,
        Some(ViewOut::Window {
            a: [1.0, 1.0],
            b: [3.0, 2.0]
        })
    );
    submit(&env, &mut c, "ZOOM");
    let out = submit(&env, &mut c, "2");
    assert_eq!(out.view, Some(ViewOut::Scale(2.0)), "{:?}", out.errors);
    assert_eq!(env.document.snapshot().revision(), revision);
    assert!(
        send(
            &env,
            &mut c,
            Event::Move {
                at: [0.0, 0.0],
                aperture: APERTURE
            }
        )
        .view
        .is_none()
    );
}

#[test]
fn trim_changes_each_pick_at_once_and_asks_again_until_enter() {
    let (mut env, mut c) = setup();
    add_segment(&mut env, [0.0, -5.0], [0.0, 5.0]);
    add_segment(&mut env, [-5.0, 1.0], [5.0, 1.0]);
    add_segment(&mut env, [-5.0, -1.0], [5.0, -1.0]);
    submit(&env, &mut c, "TRIM");
    // The vertical segment is the one cutting edge.
    click(&env, &mut c, 0.0, 3.0);
    let out = submit(&env, &mut c, "");
    assert_eq!(out.prompt.unwrap().key, "target");
    let out = click(&env, &mut c, 3.0, 1.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.prompt.unwrap().key, "target");
    let out = click(&env, &mut c, -3.0, -1.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let spans: Vec<(f64, f64, f64)> = segments(&env)
        .into_iter()
        .filter(|(a, b)| (a[1] - b[1]).abs() < 1e-9)
        .map(|(a, b)| (a[1], a[0].min(b[0]), a[0].max(b[0])))
        .collect();
    assert!(
        spans
            .iter()
            .any(|s| s.0 == 1.0 && (s.1 + 5.0).abs() < 1e-9 && s.2.abs() < 1e-9)
    );
    assert!(
        spans
            .iter()
            .any(|s| s.0 == -1.0 && s.1.abs() < 1e-9 && (s.2 - 5.0).abs() < 1e-9)
    );
    // A pick on the edge itself cannot be trimmed: the history says so and it still asks.
    let out = click(&env, &mut c, 0.0, -4.0);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.history.iter().any(|l| l.role == LineRole::Failed));
    assert_eq!(out.prompt.unwrap().key, "target");
    let out = submit(&env, &mut c, "");
    assert!(out.prompt.is_none());
    assert!(!out.history.iter().any(|l| l.role == LineRole::Cancelled));
}

#[test]
fn a_rectangle_corner_has_two_size_fields_that_compose_the_corner_on_the_cursor_side() {
    use draftbuilder_commands::interaction::DynamicMode;
    let (env, mut c) = setup();
    submit(&env, &mut c, "RECTANG");
    click(&env, &mut c, 1.0, 1.0);
    let out = move_to(&env, &mut c, -4.0, 3.0);
    let dynamic = out.dynamic.unwrap();
    assert_eq!(dynamic.mode, DynamicMode::Size);
    assert!(
        (dynamic.fields[0].value - 5.0).abs() < 1e-9
            && (dynamic.fields[1].value - 2.0).abs() < 1e-9
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 0,
            text: "30".into(),
        },
    );
    send(
        &env,
        &mut c,
        Event::Field {
            index: 1,
            text: "20".into(),
        },
    );
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    let snapshot = env.document.snapshot();
    let ids = common::entity_ids(&snapshot);
    assert_eq!(ids.len(), 1);
    let curve = common::curve_of_entity(&snapshot, ids[0].parse().unwrap());
    let perimeter = arc_geom::CurveEval::length(&curve, &arc_base::Tolerance::DEFAULT).unwrap();
    assert!((perimeter - 100.0).abs() < 1e-6, "{perimeter}");
}

#[test]
fn a_status_toggle_snaps_the_cursor_by_the_step_without_touching_the_drawing() {
    let (mut env, mut c) = setup();
    let revision = env.document.snapshot().revision();
    let at = |out: Outputs| out.cursor.unwrap().point;
    assert!(!near(at(move_to(&env, &mut c, 12.0, 7.0)), [10.0, 10.0]));
    // The command the status bar runs is no document change, so it is no undo step.
    env.run(
        "draftbuilder.view.toggle",
        json!({"tab": "t", "name": "snap", "on": true}),
    );
    c.set_toggles(&[("snap", Some(true))]);
    assert!(near(at(move_to(&env, &mut c, 12.0, 7.0)), [10.0, 10.0]));
    c.set_toggles(&[("snap", Some(false))]);
    assert!(!near(at(move_to(&env, &mut c, 12.0, 7.0)), [10.0, 10.0]));
    assert_eq!(env.document.snapshot().revision(), revision);
}
