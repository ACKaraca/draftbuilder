//! The history directives in a script (DB-DBASE-038, DB-DBASE-039): counted undo and redo, marks,
//! groups and OOPS, read by the script runner and checked on the drawing it saves.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_command::Permission;
use common::{Harness, section, values_of};

const THREE: &str = "NEW unit=meter\n\
LINE\n0,0\n10,0\n\n\
LINE\n0,5\n10,5\n\n\
LINE\n0,9\n10,9\n\n";

/// The endpoints of the LINE entities of a saved DXF text.
fn line_ends(text: &str) -> Vec<([f64; 2], [f64; 2])> {
    section(text, "ENTITIES")
        .into_iter()
        .filter(|r| r[0].1 == "LINE")
        .map(|r| {
            let at = |x: i64, y: i64| [values_of(&r, x)[0], values_of(&r, y)[0]];
            (at(10, 20), at(11, 21))
        })
        .collect()
}

/// Runs `script` on a new harness and saves the drawing; returns the line endpoints saved.
fn saved_lines(script: &str) -> Vec<([f64; 2], [f64; 2])> {
    let mut h = Harness::new();
    let out = h.path("out.dxf");
    let report = h.run(&format!("{script}SAVE path={}\n", out.display()));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.map(|f| f.error.code())
    );
    line_ends(&std::fs::read_to_string(&out).unwrap())
}

fn failure_code(report: &draftbuilder_commands::script::Report) -> &str {
    report.failure.as_ref().unwrap().error.code()
}

#[test]
fn undo_three_then_redo_three_returns_the_same_drawing() {
    let before = saved_lines(THREE);
    assert_eq!(before.len(), 3);
    let after = saved_lines(&format!("{THREE}UNDO 3\nREDO 3\n"));
    assert_eq!(after, before);
    assert!(saved_lines(&format!("{THREE}UNDO 3\n")).is_empty());
}

#[test]
fn a_count_above_the_stack_is_refused_and_changes_nothing() {
    let mut h = Harness::new();
    assert!(h.run(THREE).failure.is_none());
    let revision = h.document.snapshot().revision();
    let report = h.run("UNDO 50\n");
    assert_eq!(failure_code(&report), "doc.undo.sequence_invalid");
    assert_eq!(h.document.snapshot().revision(), revision);
}

#[test]
fn a_malformed_count_is_refused_as_a_directive() {
    let mut h = Harness::new();
    assert!(h.run(THREE).failure.is_none());
    assert_eq!(failure_code(&h.run("UNDO 0\n")), "db.cmd.history_form");
    assert_eq!(failure_code(&h.run("UNDO 1001\n")), "db.cmd.history_form");
    assert_eq!(failure_code(&h.run("REDO MARK m\n")), "db.cmd.history_form");
}

#[test]
fn a_mark_then_later_work_then_back_to_mark_keeps_the_marked_drawing() {
    let script = "NEW unit=meter\n\
LINE\n0,0\n10,0\n\n\
UNDO MARK start\n\
LINE\n0,5\n10,5\n\n\
LINE\n0,9\n10,9\n\n\
UNDO BACK start\n";
    assert_eq!(saved_lines(script), vec![([0.0, 0.0], [10.0, 0.0])]);
}

#[test]
fn a_group_of_three_commands_undoes_as_one_step() {
    let grouped = format!(
        "{}UNDO ENDGROUP\n",
        THREE.replacen("NEW unit=meter\n", "NEW unit=meter\nUNDO GROUP batch\n", 1)
    );
    assert_eq!(saved_lines(&grouped).len(), 3);
    assert!(saved_lines(&format!("{grouped}UNDO\n")).is_empty());
}

#[test]
fn a_group_left_open_at_the_end_of_a_script_is_closed() {
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nUNDO GROUP open\nLINE\n0,0\n10,0\n\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.map(|f| f.error.code())
    );
    let report = h.run("UNDO\n");
    assert!(report.failure.is_none(), "the group closed with the script");
}

#[test]
fn erase_draw_then_oops_restores_the_erased_line_and_a_second_oops_reports_nothing() {
    let mut h = Harness::new();
    let report = h.run(
        "NEW unit=meter\n\
LINE\n0,0\n10,0\n\n\
LINE\n0,5\n10,5\n\n\
ERASE\n5,0\n\
LINE\n0,9\n10,9\n\n",
    );
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.map(|f| (f.line, f.text, f.error.code()))
    );
    let report = h.run("OOPS\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.map(|f| f.error.code())
    );
    let restored = report.commands.last().unwrap();
    assert_eq!(restored.command, "history.restore_erased");
    assert_eq!(restored.created, 1, "the erased line came back");

    assert_eq!(failure_code(&h.run("OOPS\n")), "db.cmd.nothing_to_restore");
}

#[test]
fn a_session_without_edit_is_refused_every_history_form() {
    let mut h = Harness::new();
    assert!(h.run(THREE).failure.is_none());
    let revision = h.document.snapshot().revision();
    h.session.permissions.remove(&Permission::Edit);
    for script in [
        "UNDO\n",
        "REDO\n",
        "UNDO MARK m\n",
        "UNDO BACK m\n",
        "UNDO GROUP g\n",
        "OOPS\n",
    ] {
        assert_eq!(
            failure_code(&h.run(script)),
            "cmd.permission_denied",
            "{script}"
        );
    }
    assert_eq!(h.document.snapshot().revision(), revision);
}

#[test]
fn undo_three_then_redo_three_returns_the_whole_document() {
    let mut h = Harness::new();
    assert!(h.run(THREE).failure.is_none());
    let before = h.document.snapshot();
    let report = h.run("UNDO 3\nREDO 3\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.map(|f| f.error.code())
    );
    let comparison = arc_doc::compare(&before, &h.document.snapshot());
    assert!(comparison.is_empty(), "{:?}", comparison.objects);
}

#[test]
fn an_end_group_the_script_did_not_open_is_refused_and_leaves_the_group_open() {
    let mut h = Harness::new();
    assert!(h.run("NEW unit=meter\n").failure.is_none());
    h.document.begin_group("elsewhere").wait().unwrap();
    let report = h.run("UNDO ENDGROUP\n");
    assert_eq!(failure_code(&report), "db.cmd.no_group_open");
    // The group opened elsewhere is still open: ending it here is the only way to close it.
    h.document.end_group().wait().unwrap();
}

#[test]
fn a_script_group_nested_in_a_group_the_script_did_not_open_closes_only_its_own() {
    let mut h = Harness::new();
    assert!(h.run("NEW unit=meter\n").failure.is_none());
    h.document.begin_group("elsewhere").wait().unwrap();
    let report = h.run("UNDO GROUP inside\nUNDO ENDGROUP\nUNDO ENDGROUP\n");
    assert_eq!(failure_code(&report), "db.cmd.no_group_open");
    h.document.end_group().wait().unwrap();
}
