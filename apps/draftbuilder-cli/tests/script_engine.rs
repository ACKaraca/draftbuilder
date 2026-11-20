//! The script engine (`draftbuilder_commands::script`) on its own: how a script's lines are read,
//! how Enter, the apostrophe and the history directives act, where a script stops, and what the
//! typed numbers and the coordinate system mean. The workflows are in `w01.rs` and `w05.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use common::{Harness, quoted, section, value_of, values_of};
use draftbuilder_commands::script;

fn codes(report: &script::Report) -> Vec<&str> {
    report.commands.iter().map(|c| c.command.as_str()).collect()
}

#[test]
fn comments_are_skipped_and_the_failing_line_keeps_its_number() {
    let mut h = Harness::new();
    let report = h.run(
        "; a header comment\nNEW unit=meter\n; a comment\nLINE\n0,0\n5,0\n\nNOTACOMMAND\nLINE\n",
    );
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.line, 8, "the numbers count the comments");
    assert_eq!(failure.text, "NOTACOMMAND");
    assert_eq!(failure.error.code(), "cmd.unknown");
    assert_eq!(report.lines, 6, "the comments are not lines of the script");
    assert_eq!(
        codes(&report),
        ["draftbuilder.drawing.new", "draftbuilder.draw.line"]
    );
}

#[test]
fn a_script_that_ends_inside_a_command_fails_and_names_the_prompt() {
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nPLINE\n0,0\n");
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "db.cmd.script_pending");
    assert_eq!(
        failure.command.as_deref(),
        Some("draftbuilder.draw.polyline")
    );
    assert_eq!(failure.line, 3);
}

#[test]
fn enter_with_no_command_waiting_repeats_the_last_command() {
    // DB-CMD-011: an empty line repeats the last command with its last parameters.
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nC\n5,5\n1\n\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let circles: Vec<_> = report
        .commands
        .iter()
        .filter(|c| c.command == "draftbuilder.draw.circle")
        .map(|c| c.line)
        .collect();
    assert_eq!(circles, vec![4, 5], "the empty line 5 ran the circle again");
}

#[test]
fn an_apostrophe_runs_a_transparent_command_beside_the_waiting_prompt() {
    // DB-CMD-012: SETVAR is transparent, so it runs while the polyline waits for points, and the
    // polyline goes on with its points.
    let mut h = Harness::new();
    let report = h.run(
        "NEW unit=meter\nLAYER name=WORK\nPLINE\n0,0\n'SETVAR name=CLAYER value=WORK\n10,0\n10,10\nC\n",
    );
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    assert_eq!(
        codes(&report),
        [
            "draftbuilder.drawing.new",
            "draftbuilder.layer.create",
            "draftbuilder.settings.setvar",
            "draftbuilder.draw.polyline",
        ]
    );
}

#[test]
fn an_apostrophe_refuses_a_command_that_is_not_transparent() {
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nPLINE\n0,0\n'LINE\n");
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "cmd.transparent.refused");
    assert_eq!(failure.line, 4);
}

#[test]
fn undo_with_nothing_to_undo_stops_the_script_and_a_bad_count_is_refused() {
    let mut h = Harness::new();
    let report = h.run("UNDO\n");
    assert_eq!(
        report.failure.as_ref().unwrap().error.code(),
        "cmd.history.empty"
    );

    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nUNDO 0\n");
    assert_eq!(
        report.failure.as_ref().unwrap().error.code(),
        "db.cmd.script_directive"
    );
}

#[test]
fn a_point_on_no_object_stops_a_pick_with_its_error() {
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nLINE\n0,0\n10,0\n\nERASE\n5,5\n");
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "db.cmd.pick_none");
    assert_eq!(failure.line, 7);
}

#[test]
fn typed_numbers_are_read_in_the_drawing_unit() {
    // A millimeter drawing: 1000 is 1000 millimeters, saved as 1000 in the file's unit.
    let mut h = Harness::new();
    let output = h.path("mm.dxf");
    let report = h.run(&format!(
        "NEW unit=millimeter\nLINE\n0,0\n1000,0\n\nSAVE path={}\n",
        quoted(&output)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let text = std::fs::read_to_string(&output).unwrap();
    let line = section(&text, "ENTITIES")
        .into_iter()
        .find(|r| r[0].1 == "LINE")
        .unwrap();
    assert_eq!(values_of(&line, 11), vec![1000.0]);
    assert_eq!(value_of(&line, 8), Some("0"));
}

#[test]
fn the_user_coordinate_system_applies_to_typed_points() {
    // UCS with its origin at (10, 10): a typed 0,0 is the drawing's (10, 10).
    let mut h = Harness::new();
    let output = h.path("ucs.dxf");
    let report = h.run(&format!(
        "NEW unit=meter\nUCS mode=origin origin=10,10\nLINE\n0,0\n5,0\n\nSAVE path={}\n",
        quoted(&output)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let text = std::fs::read_to_string(&output).unwrap();
    let line = section(&text, "ENTITIES")
        .into_iter()
        .find(|r| r[0].1 == "LINE")
        .unwrap();
    assert_eq!(values_of(&line, 10), vec![10.0]);
    assert_eq!(values_of(&line, 20), vec![10.0]);
    assert_eq!(values_of(&line, 11), vec![15.0]);
    assert_eq!(values_of(&line, 21), vec![10.0]);
}

#[test]
fn the_report_counts_what_each_command_created_changed_and_deleted() {
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nLINE\n0,0\n10,0\n\nLINE\n0,5\n10,5\n\nUNDO\n");
    assert!(report.failure.is_none());
    let json = report.to_json();
    assert_eq!(json["totals"]["created"], 2 + 1 + 1);
    assert_eq!(json["totals"]["deleted"], 1);
    let undo = report.commands.last().unwrap();
    assert_eq!(undo.command, "history.undo");
    assert_eq!((undo.created, undo.changed, undo.deleted), (0, 0, 1));
}

#[test]
fn a_layer_name_typed_in_a_script_keeps_its_case() {
    let mut h = Harness::new();
    let output = h.path("layer.dxf");
    let report = h.run(&format!(
        "NEW unit=meter\nLAYER name=Walls\nSETVAR name=CLAYER value=Walls\nSAVE path={}\n",
        quoted(&output)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let text = std::fs::read_to_string(&output).unwrap();
    let layers: Vec<String> = section(&text, "TABLES")
        .iter()
        .filter(|r| r.iter().any(|(c, v)| *c == 0 && v == "LAYER"))
        .filter_map(|r| value_of(r, 2).map(str::to_owned))
        .collect();
    assert!(layers.iter().any(|name| name == "Walls"), "{layers:?}");
}

#[test]
fn the_name_forms_run_the_same_command() {
    // `_LINE` is the English name, `.LINE` the built-in command and `-LINE` the prompt form: each
    // resolves to the same command and gives the same line (DB-CMD-011 and the name rules).
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\n_LINE\n0,0\n1,0\n\n.LINE\n0,1\n1,1\n\n-LINE\n0,2\n1,2\n\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let lines: Vec<&str> = report
        .commands
        .iter()
        .filter(|c| c.command == "draftbuilder.draw.line")
        .map(|c| c.command.as_str())
        .collect();
    assert_eq!(lines.len(), 3);
}

fn details(error: &arc_base::Error) -> String {
    format!("{:?}", error.details())
}

#[test]
fn a_line_over_the_length_cap_stops_the_script_and_names_the_cap() {
    let mut h = Harness::new();
    let long = format!(
        "; {}",
        "x".repeat(draftbuilder_commands::script::MAX_LINE_BYTES)
    );
    let report = h.run(&format!("NEW unit=meter\n{long}\nLINE\n"));
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "db.cmd.script_limit");
    assert_eq!(failure.line, 2);
    assert!(
        details(&failure.error).contains("line_bytes"),
        "{}",
        details(&failure.error)
    );
}

#[test]
fn a_line_over_the_token_cap_stops_the_script() {
    let mut h = Harness::new();
    let tokens = "1 ".repeat(draftbuilder_commands::script::MAX_TOKENS_PER_LINE + 1);
    let report = h.run(&format!("NEW unit=meter\nLINE {tokens}\n"));
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "db.cmd.script_limit");
    assert_eq!(failure.line, 2);
    assert!(details(&failure.error).contains("tokens_per_line"));
}

#[test]
fn a_script_over_the_line_count_cap_stops_at_the_first_line_past_it() {
    let mut h = Harness::new();
    let mut text = ";\n".repeat(draftbuilder_commands::script::MAX_LINES);
    text.push_str("NEW unit=meter\n");
    let report = h.run(&text);
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "db.cmd.script_limit");
    assert_eq!(failure.line, draftbuilder_commands::script::MAX_LINES + 1);
    assert!(details(&failure.error).contains("line_count"));
    assert_eq!(report.lines, 0, "nothing is run");
}

#[test]
fn a_byte_order_mark_at_the_start_is_ignored() {
    let mut h = Harness::new();
    let report = h.run("\u{feff}NEW unit=meter\nLINE\n0,0\n1,0\n\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    assert_eq!(report.commands[0].command, "draftbuilder.drawing.new");
}

#[test]
fn a_pick_tolerance_directive_changes_what_a_point_picks() {
    // The line is at y = 0; the point is 0.3 away: picked with a tolerance of 0.5, not with the default.
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nLINE\n0,0\n10,0\n\nPICKTOLERANCE 0.5\nERASE\n5,0.3\n");
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    assert_eq!(
        report.commands.last().unwrap().command,
        "draftbuilder.modify.erase"
    );

    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nLINE\n0,0\n10,0\n\nERASE\n5,0.3\n");
    assert_eq!(
        report.failure.as_ref().unwrap().error.code(),
        "db.cmd.pick_none"
    );
}

#[test]
fn a_pick_box_that_does_not_exist_at_large_coordinates_picks_nothing() {
    // At 1e20 the tolerance is below the spacing of the numbers: the box has no width, so nothing is
    // picked, and the error is the pick's, not a selection's.
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nERASE\n1e20,0\n");
    assert_eq!(
        report.failure.as_ref().unwrap().error.code(),
        "db.cmd.pick_none"
    );
}

#[test]
fn a_setting_that_may_not_change_while_a_prompt_waits_is_refused() {
    // The units are read by the answers already given: refused beside the polyline's prompt.
    let mut h = Harness::new();
    let report = h.run("NEW unit=meter\nPLINE\n0,0\n'SETVAR name=LUNITS value=2\n");
    let failure = report.failure.as_ref().unwrap();
    assert_eq!(failure.error.code(), "db.cmd.setting_not_transparent");
    assert_eq!(failure.line, 4);
}

/// The start and end of every LINE of the DXF text, in file order.
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

#[test]
fn typed_polar_and_relative_points_are_exact_in_a_millimeter_drawing() {
    // DB-SNAP-020 (planar part) and DB-SNAP-024: the polar and relative forms in millimeters.
    let mut h = Harness::new();
    let output = h.path("polar-mm.dxf");
    let report = h.run(&format!(
        "NEW unit=millimeter\nLINE\n0,0\n1000<90\n@500<0\n\nSAVE path={}\n",
        quoted(&output)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let text = std::fs::read_to_string(&output).unwrap();
    assert_points(
        &line_ends(&text),
        &[
            ([0.0, 0.0], [0.0, 1000.0]),
            ([0.0, 1000.0], [500.0, 1000.0]),
        ],
    );
}

#[test]
fn typed_polar_and_relative_points_follow_a_rotated_user_coordinate_system() {
    // The UCS origin is (10, 10) millimeters (UCS parameters are meters) and its X axis points along world Y, so its linear part turns
    // every offset by 90 degrees: local 3<0 is world (10, 13), and @2<90 from it is world (8, 13).
    let mut h = Harness::new();
    let output = h.path("polar-ucs.dxf");
    let report = h.run(&format!(
        "NEW unit=millimeter\nUCS mode=origin origin=0.01,0.01 x_point=0.01,0.011\nLINE\n0,0\n3<0\n@2<90\n\nSAVE path={}\n",
        quoted(&output)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let text = std::fs::read_to_string(&output).unwrap();
    assert_points(
        &line_ends(&text),
        &[([10.0, 10.0], [10.0, 13.0]), ([10.0, 13.0], [8.0, 13.0])],
    );
}

#[test]
fn cylindrical_and_spherical_points_are_refused_by_planar_prompts() {
    // DB-SNAP-020 gap: the foundation reads `d<a,z` and `d<a<e`, but every point prompt of the
    // product is planar (Point2 entities), so a height or an elevation is refused.
    for typed in ["1000<45,200", "@1000<45<30"] {
        let mut h = Harness::new();
        let report = h.run(&format!("NEW unit=millimeter\nLINE\n0,0\n{typed}\n"));
        let failure = report.failure.as_ref().unwrap();
        assert_eq!(failure.error.code(), "cmd.prompt.range", "{typed}");
        let details = format!("{:?}", failure.error.details());
        assert!(details.contains("dimensions"), "{typed}: {details}");
        assert_eq!(failure.line, 4, "{typed}");
    }
}

/// Asserts that the LINEs `actual` are `expected` within a nanometer-scale tolerance, since the
/// trigonometry of a polar point is not exact in binary floating point.
fn assert_points(actual: &[([f64; 2], [f64; 2])], expected: &[([f64; 2], [f64; 2])]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for ((a0, a1), (e0, e1)) in actual.iter().zip(expected) {
        for (a, e) in a0.iter().chain(a1).zip(e0.iter().chain(e1)) {
            assert!((a - e).abs() < 1e-9, "{actual:?} != {expected:?}");
        }
    }
}
