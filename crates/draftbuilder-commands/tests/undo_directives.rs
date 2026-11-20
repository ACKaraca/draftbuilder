//! The history directives at the interaction controller (DB-DBASE-038, DB-DBASE-039): counted undo
//! and redo, the step list, marks, groups and restore of the last erased objects, with the
//! permission and the controller's own state checked.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_command::Permission;
use arc_command::history::Direction;
use draftbuilder_commands::interaction::{Controller, CrossingPicker, Event, LineRole, Outputs};
use draftbuilder_commands::undo::Request;
use serde_json::json;
use std::collections::BTreeSet;

use common::{Env, lines};

const APERTURE: f64 = 0.05;

fn setup() -> (Env, Controller) {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    (env, Controller::new().unwrap())
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

fn step(env: &Env, controller: &mut Controller, direction: Direction, count: usize) -> Outputs {
    send(
        env,
        controller,
        Event::History(Request::Step { direction, count }),
    )
}

fn codes(out: &Outputs) -> Vec<&'static str> {
    out.errors.iter().map(|e| e.code).collect()
}

fn segments(env: &Env) -> Vec<([f64; 2], [f64; 2])> {
    lines(&env.document.snapshot())
        .into_iter()
        .map(|(_, start, end)| (start, end))
        .collect()
}

/// Draws a line from `a` to `b` through the controller: LINE, two points, Enter.
fn draw(env: &Env, c: &mut Controller, a: [f64; 2], b: [f64; 2]) {
    submit(env, c, "LINE");
    click(env, c, a[0], a[1]);
    click(env, c, b[0], b[1]);
    let out = submit(env, c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
}

/// Adds a segment with the entity command (a command that is not the controller's).
fn add_segment(env: &mut Env, start: [f64; 2], end: [f64; 2]) -> String {
    let owner = env.space().to_string();
    env.run(
        "core.draft.entity.create",
        json!({"owner": owner, "geometry": {"kind": "segment", "start": start, "end": end}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn three_lines(env: &Env, c: &mut Controller) -> Vec<([f64; 2], [f64; 2])> {
    draw(env, c, [0.0, 0.0], [10.0, 0.0]);
    draw(env, c, [0.0, 5.0], [10.0, 5.0]);
    draw(env, c, [0.0, 9.0], [10.0, 9.0]);
    segments(env)
}

#[test]
fn undo_three_then_redo_three_returns_the_same_drawing() {
    let (env, mut c) = setup();
    let before = three_lines(&env, &mut c);
    assert_eq!(before.len(), 3);

    let out = step(&env, &mut c, Direction::Undo, 3);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(segments(&env).is_empty());

    let out = step(&env, &mut c, Direction::Redo, 3);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env), before);
}

#[test]
fn a_count_above_the_stack_is_refused_and_changes_nothing() {
    let (env, mut c) = setup();
    three_lines(&env, &mut c);
    let revision = env.document.snapshot().revision();
    // The stack holds the new drawing's step as well as the three lines, so 4 would fit.
    let out = step(&env, &mut c, Direction::Undo, 5);
    assert_eq!(codes(&out), vec!["doc.undo.sequence_invalid"]);
    assert_eq!(env.document.snapshot().revision(), revision);
    assert_eq!(segments(&env).len(), 3);

    let out = submit(&env, &mut c, "UNDO 5");
    assert_eq!(codes(&out), vec!["doc.undo.sequence_invalid"]);
    assert_eq!(segments(&env).len(), 3);
}

#[test]
fn a_typed_count_is_read_and_a_malformed_one_refused() {
    let (env, mut c) = setup();
    three_lines(&env, &mut c);
    let out = submit(&env, &mut c, "UNDO 2");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env).len(), 1);
    let out = submit(&env, &mut c, "REDO 2");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env).len(), 3);
    let out = submit(&env, &mut c, "UNDO 0");
    assert_eq!(codes(&out), vec!["db.cmd.history_form"]);
    assert_eq!(segments(&env).len(), 3);
}

#[test]
fn the_step_list_names_the_commands_next_to_step_first() {
    let (mut env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    add_segment(&mut env, [0.0, 5.0], [10.0, 5.0]);
    let listed = c.undo_steps(&env.document, 10).unwrap();
    let names: Vec<_> = listed
        .undo
        .iter()
        .map(|s| s.command.clone().unwrap_or_default())
        .collect();
    // The new drawing's step is the oldest one, at the bottom of the stack.
    assert_eq!(
        names,
        vec![
            "core.draft.entity.create",
            "draftbuilder.draw.line",
            "draftbuilder.drawing.new"
        ]
    );
    assert!(listed.redo.is_empty());

    step(&env, &mut c, Direction::Undo, 1);
    let listed = c.undo_steps(&env.document, 10).unwrap();
    assert_eq!(listed.undo.len(), 2);
    assert_eq!(listed.redo.len(), 1);
    assert_eq!(
        listed.redo[0].command.as_deref(),
        Some("core.draft.entity.create")
    );
}

#[test]
fn a_mark_is_set_drawing_continues_and_back_to_mark_undoes_only_the_later_work() {
    let (env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    let out = submit(&env, &mut c, "UNDO MARK start");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    draw(&env, &mut c, [0.0, 5.0], [10.0, 5.0]);
    draw(&env, &mut c, [0.0, 9.0], [10.0, 9.0]);
    assert_eq!(segments(&env).len(), 3);

    let out = submit(&env, &mut c, "UNDO BACK start");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env), vec![([0.0, 0.0], [10.0, 0.0])]);
    assert_eq!(out.history.last().map(|l| l.role), Some(LineRole::Undo));

    // Back to the mark again is a no-op, and an unknown mark is refused.
    let out = submit(&env, &mut c, "UNDO BACK start");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env).len(), 1);
    let out = submit(&env, &mut c, "UNDO BACK missing");
    assert_eq!(codes(&out), vec!["doc.undo.sequence_invalid"]);
    assert_eq!(segments(&env).len(), 1);
}

#[test]
fn a_group_of_three_commands_undoes_as_one_step() {
    let (env, mut c) = setup();
    assert!(submit(&env, &mut c, "UNDO GROUP batch").errors.is_empty());
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    draw(&env, &mut c, [0.0, 5.0], [10.0, 5.0]);
    draw(&env, &mut c, [0.0, 9.0], [10.0, 9.0]);
    assert!(submit(&env, &mut c, "UNDO ENDGROUP").errors.is_empty());
    assert_eq!(segments(&env).len(), 3);

    let listed = c.undo_steps(&env.document, 10).unwrap();
    // The stack holds the new drawing's step below the group, which is one step of three commands.
    assert_eq!(listed.undo.len(), 2);
    assert_eq!(listed.undo[0].transactions, 3);

    let out = step(&env, &mut c, Direction::Undo, 1);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(segments(&env).is_empty());
}

#[test]
fn an_end_group_without_a_group_is_refused() {
    let (env, mut c) = setup();
    let out = submit(&env, &mut c, "UNDO ENDGROUP");
    assert_eq!(codes(&out), vec!["db.cmd.no_group_open"]);
}

#[test]
fn restore_brings_back_the_erased_objects_and_keeps_later_work() {
    let (mut env, mut c) = setup();
    let a = add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    add_segment(&mut env, [0.0, 5.0], [10.0, 5.0]);
    env.run("draftbuilder.modify.erase", json!({"objects": [a]}));
    assert_eq!(segments(&env).len(), 1);

    draw(&env, &mut c, [0.0, 9.0], [10.0, 9.0]);
    let out = submit(&env, &mut c, "OOPS");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(segments(&env).len(), 3, "the erased line is back");
    assert_eq!(
        out.history.last(),
        Some(&draftbuilder_commands::interaction::HistoryLine {
            role: LineRole::Restored,
            text: "1".into(),
        })
    );

    let out = submit(&env, &mut c, "OOPS");
    assert_eq!(codes(&out), vec!["db.cmd.nothing_to_restore"]);
    assert_eq!(segments(&env).len(), 3, "a second restore changes nothing");
}

#[test]
fn a_history_step_cancels_the_waiting_command_and_ends_its_state() {
    let (env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    submit(&env, &mut c, "LINE");
    let out = click(&env, &mut c, 0.0, 5.0);
    assert!(out.prompt.is_some(), "the line waits for its second point");

    let out = step(&env, &mut c, Direction::Undo, 1);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.prompt.is_none(), "the waiting command ended");
    assert!(segments(&env).is_empty());
    assert!(out.history.iter().any(|l| l.role == LineRole::Cancelled));
}

#[test]
fn a_session_without_edit_is_refused_every_history_form() {
    let (env, mut c) = setup();
    three_lines(&env, &mut c);
    let read: BTreeSet<Permission> = [Permission::Read].into_iter().collect();
    c.set_permissions(&read);
    let revision = env.document.snapshot().revision();

    for out in [
        step(&env, &mut c, Direction::Undo, 1),
        step(&env, &mut c, Direction::Redo, 2),
        submit(&env, &mut c, "UNDO MARK m"),
        submit(&env, &mut c, "UNDO BACK m"),
        submit(&env, &mut c, "UNDO GROUP g"),
        submit(&env, &mut c, "OOPS"),
    ] {
        assert_eq!(codes(&out), vec!["cmd.permission_denied"]);
    }
    assert_eq!(env.document.snapshot().revision(), revision);
    assert_eq!(segments(&env).len(), 3);
    assert!(
        c.undo_steps(&env.document, 10).is_ok(),
        "the list needs only read"
    );
}

fn send_history(env: &Env, controller: &mut Controller, request: Request) -> Outputs {
    send(env, controller, Event::History(request))
}

/// The undo steps and the revision of the document, before and after a line.
fn history_state(env: &Env, controller: &Controller) -> (usize, u64) {
    let steps = controller
        .undo_steps(&env.document, 1_000)
        .unwrap()
        .undo
        .len();
    (steps, env.document.snapshot().revision())
}

#[test]
fn a_refused_history_step_leaves_the_waiting_command_at_its_first_point() {
    let (env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 5.0);

    let out = step(&env, &mut c, Direction::Undo, 50);
    assert_eq!(codes(&out), vec!["doc.undo.sequence_invalid"]);
    assert!(
        out.prompt.is_some(),
        "a count above the stack cancels nothing"
    );

    let out = send_history(&env, &mut c, Request::BackToMark("unknown".into()));
    assert_eq!(codes(&out), vec!["doc.undo.sequence_invalid"]);
    assert!(out.prompt.is_some(), "a missing mark cancels nothing");

    let out = send_history(&env, &mut c, Request::RestoreErased);
    assert_eq!(codes(&out), vec!["db.cmd.nothing_to_restore"]);
    assert!(
        out.prompt.is_some(),
        "a restore with nothing to restore cancels nothing"
    );

    click(&env, &mut c, 10.0, 0.0);
    let out = submit(&env, &mut c, "");
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
        segments(&env).len(),
        2,
        "the command completes from its first point"
    );
}

#[test]
fn a_back_to_mark_that_steps_nothing_keeps_the_waiting_command() {
    let (env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    submit(&env, &mut c, "UNDO MARK here");
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 5.0);
    let out = send_history(&env, &mut c, Request::BackToMark("here".into()));
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        out.prompt.is_some(),
        "no step was taken, so the command waits"
    );
    assert_eq!(segments(&env).len(), 1);
}

#[test]
fn a_typed_count_at_a_prompt_is_an_answer_and_never_a_history_step() {
    let (env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);

    let before = history_state(&env, &c);
    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 5.0);
    let out = submit(&env, &mut c, "UNDO 5");
    assert!(
        !codes(&out).contains(&"doc.undo.sequence_invalid")
            && !codes(&out).contains(&"db.cmd.history_form"),
        "a point prompt takes the text, not the history word: {:?}",
        codes(&out)
    );
    assert!(out.prompt.is_some(), "the point prompt still waits");
    assert_eq!(history_state(&env, &c), before);
    send(&env, &mut c, Event::Escape);

    let before = history_state(&env, &c);
    submit(&env, &mut c, "CIRCLE");
    click(&env, &mut c, 0.0, 5.0);
    let out = submit(&env, &mut c, "UNDO 5");
    assert!(
        !codes(&out).contains(&"doc.undo.sequence_invalid")
            && !codes(&out).contains(&"db.cmd.history_form"),
        "the radius prompt takes the text, not the history word: {:?}",
        codes(&out)
    );
    assert_eq!(history_state(&env, &c), before);
    send(&env, &mut c, Event::Escape);

    let before = history_state(&env, &c);
    submit(&env, &mut c, "ERASE");
    let out = submit(&env, &mut c, "UNDO 5");
    assert!(
        !codes(&out).contains(&"doc.undo.sequence_invalid")
            && !codes(&out).contains(&"db.cmd.history_form"),
        "the selection prompt takes the text, not the history word: {:?}",
        codes(&out)
    );
    assert!(out.prompt.is_some(), "the selection prompt still waits");
    assert_eq!(history_state(&env, &c), before);
    assert_eq!(segments(&env).len(), 1);
}

#[test]
fn a_group_is_counted_shown_and_closed_by_close_groups() {
    let (env, mut c) = setup();
    let out = submit(&env, &mut c, "UNDO GROUP batch");
    assert_eq!(out.open_groups, 1);
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    assert_eq!(c.open_groups(), 1);

    let out = step(&env, &mut c, Direction::Undo, 1);
    assert_eq!(codes(&out), vec!["doc.undo.sequence_invalid"]);
    assert_eq!(
        segments(&env).len(),
        1,
        "a step inside an open group changes nothing"
    );
    assert_eq!(c.open_groups(), 1);

    assert_eq!(c.close_groups(&env.document), 1);
    assert_eq!(c.open_groups(), 0);

    let mut fresh = Controller::new().unwrap();
    let out = step(&env, &mut fresh, Direction::Undo, 1);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        segments(&env).is_empty(),
        "the group undoes as one step in a fresh controller"
    );
}

#[test]
fn an_end_group_is_refused_when_this_controller_opened_no_group() {
    let (env, mut c) = setup();
    env.document.begin_group("elsewhere").wait().unwrap();
    let out = submit(&env, &mut c, "UNDO ENDGROUP");
    assert_eq!(codes(&out), vec!["db.cmd.no_group_open"]);
    assert_eq!(out.open_groups, 0);
    env.document.end_group().wait().unwrap();
}

#[test]
fn nested_groups_undo_as_one_step_when_the_outer_group_ends() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "UNDO GROUP outer");
    submit(&env, &mut c, "UNDO GROUP inner");
    assert_eq!(c.open_groups(), 2);
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    assert!(submit(&env, &mut c, "UNDO ENDGROUP").errors.is_empty());
    assert_eq!(c.open_groups(), 1);
    draw(&env, &mut c, [0.0, 5.0], [10.0, 5.0]);
    assert!(submit(&env, &mut c, "UNDO ENDGROUP").errors.is_empty());
    assert_eq!(c.open_groups(), 0);
    assert_eq!(segments(&env).len(), 2);

    let listed = c.undo_steps(&env.document, 10).unwrap();
    assert_eq!(
        listed.undo.len(),
        2,
        "the new drawing, then the outer group"
    );
    assert_eq!(listed.undo[0].transactions, 2);

    let out = step(&env, &mut c, Direction::Undo, 1);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(segments(&env).is_empty());
}

#[test]
fn oops_with_a_command_waiting_restores_and_ends_the_command() {
    let (mut env, mut c) = setup();
    let a = add_segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    env.run("draftbuilder.modify.erase", json!({"objects": [a]}));
    assert!(segments(&env).is_empty());

    submit(&env, &mut c, "LINE");
    click(&env, &mut c, 0.0, 5.0);
    let out = send_history(&env, &mut c, Request::RestoreErased);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        out.prompt.is_none(),
        "the restore ended the waiting command"
    );
    assert_eq!(segments(&env).len(), 1);
    assert!(out.history.iter().any(|l| l.role == LineRole::Cancelled));
    assert_eq!(out.history.last().map(|l| l.role), Some(LineRole::Restored));
}

#[test]
fn after_an_undo_the_selection_holds_no_removed_object() {
    let (env, mut c) = setup();
    draw(&env, &mut c, [0.0, 0.0], [10.0, 0.0]);
    let out = click(&env, &mut c, 5.0, 0.0);
    assert_eq!(out.selection.len(), 1, "the click selects the line");
    let out = step(&env, &mut c, Direction::Undo, 1);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(
        out.selection.is_empty(),
        "the removed line leaves the selection"
    );
}

#[test]
fn undo_three_then_redo_three_returns_the_whole_document() {
    let (env, mut c) = setup();
    three_lines(&env, &mut c);
    let before = env.document.snapshot();
    step(&env, &mut c, Direction::Undo, 3);
    step(&env, &mut c, Direction::Redo, 3);
    let comparison = arc_doc::compare(&before, &env.document.snapshot());
    assert!(comparison.is_empty(), "{:?}", comparison.objects);
}

#[test]
fn the_turkish_history_form_names_only_words_the_parser_accepts() {
    const TR: &str = include_str!("../../../locales/tr-TR/draftbuilder-commands.ftl");
    let line = TR
        .lines()
        .find(|l| l.starts_with("db-cmd-history-form = "))
        .unwrap();
    for word in ["UNDO", "REDO", "OOPS", "MARK", "BACK", "GROUP", "ENDGROUP"] {
        assert!(line.contains(word), "{word} is missing");
    }
    assert!(!line.contains("GERİ") && !line.contains("YİNELE"));
}
