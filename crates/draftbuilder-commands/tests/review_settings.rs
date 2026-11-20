//! The review findings on the settings, layers, coordinate systems and snaps of 8a50842 (review items
//! 2 to 8, 11, 13, 14, 15): the header values read and written, the current layer and linetype, the
//! lineweight and polar lists, the aperture, the coordinate system import, the one-pick override and
//! the open report.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use common::{Env, reasons};
use serde_json::json;

/// A DXF text with the given header variables (each a `(name, raw group pairs)` pair), the given
/// layers (`(name, flags)`), and a linetype table naming `DASHED` when `linetype` is set.
fn dxf(header: &[(&str, &str)], layers: &[(&str, i16)], linetype: bool) -> String {
    let mut out = String::from(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$INSUNITS\n 70\n6\n",
    );
    for (name, raw) in header {
        out.push_str(&format!("  9\n{name}\n{raw}"));
    }
    out.push_str("  0\nENDSEC\n  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLTYPE\n  0\nLTYPE\n  2\nCONTINUOUS\n 70\n0\n  3\nSolid line\n 72\n65\n 73\n0\n 40\n0.0\n");
    if linetype {
        out.push_str("  0\nLTYPE\n  2\nDASHED\n 70\n0\n  3\nDashed\n 72\n65\n 73\n2\n 40\n1.5\n 49\n1.0\n 49\n-0.5\n");
    }
    out.push_str("  0\nENDTAB\n  0\nTABLE\n  2\nLAYER\n");
    for (name, flags) in layers {
        out.push_str(&format!(
            "  0\nLAYER\n  2\n{name}\n 70\n{flags}\n 62\n7\n  6\nCONTINUOUS\n"
        ));
    }
    out.push_str("  0\nENDTAB\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n");
    out
}

fn value(env: &mut Env, name: &str) -> serde_json::Value {
    env.run("draftbuilder.settings.get", json!({"name": name}))["value"].clone()
}

fn code(result: Result<serde_json::Value, arc_base::Error>) -> &'static str {
    result.unwrap_err().code()
}

#[test]
fn a_frozen_current_layer_in_the_file_falls_back_to_layer_zero_with_a_report_item() {
    let mut env = Env::new();
    let path = env.write(
        "frozen.dxf",
        dxf(&[("$CLAYER", " 8\nFROZ\n")], &[("FROZ", 1)], false).as_bytes(),
    );
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(
        reasons(&opened).contains(&"db.io.current_layer_fallback".to_owned()),
        "{opened}"
    );
    assert_eq!(value(&mut env, "draft.layer.current"), json!("0"));
}

#[test]
fn an_out_of_range_header_value_is_clamped_and_reported() {
    let mut env = Env::new();
    // The length precision takes 0 to 8: a file's 99 is set to 8, and the report says so.
    let path = env.write(
        "clamp.dxf",
        dxf(&[("$LUPREC", " 70\n99\n")], &[], false).as_bytes(),
    );
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(
        reasons(&opened).contains(&"db.io.header_value_clamped".to_owned()),
        "{opened}"
    );
    assert_eq!(value(&mut env, "draft.units.length_precision"), json!(8));
}

#[test]
fn a_header_value_the_setting_cannot_take_is_ignored_and_reported() {
    let mut env = Env::new();
    let path = env.write(
        "ignored.dxf",
        dxf(&[("$LUPREC", "  1\nmany\n")], &[], false).as_bytes(),
    );
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(
        reasons(&opened).contains(&"db.io.header_value_ignored".to_owned()),
        "{opened}"
    );
    assert_eq!(
        value(&mut env, "draft.units.length_precision"),
        json!(4),
        "the default applies"
    );
}

#[test]
fn save_writes_only_the_settings_the_file_or_the_user_gave_a_value() {
    let mut env = Env::new();
    let path = env.write(
        "ltscale.dxf",
        dxf(&[("$LTSCALE", " 40\n2.5\n")], &[], false).as_bytes(),
    );
    env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    let out = env.path("out.dxf");
    env.run("draftbuilder.drawing.save", json!({"path": out.clone()}));
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("$LTSCALE"));
    assert!(
        !text.contains("$PDSIZE"),
        "a default the file never had is not written"
    );
    assert!(!text.contains("$GRIDMODE"));
}

#[test]
fn a_header_variable_the_save_does_not_write_back_is_reported_at_open() {
    let mut env = Env::new();
    let path = env.write(
        "note.dxf",
        dxf(&[("$ARCTEST_NOTE", "  1\npartner-owned\n")], &[], false).as_bytes(),
    );
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(
        reasons(&opened).contains(&"db.io.header_not_written".to_owned()),
        "{opened}"
    );
}

#[test]
fn the_angle_base_is_written_in_degrees_and_read_back_in_radians() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.angle.base", "value": std::f64::consts::FRAC_PI_2}),
    );
    let out = env.path("base.dxf");
    env.run("draftbuilder.drawing.save", json!({"path": out.clone()}));
    let text = std::fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = lines.iter().position(|l| *l == "$ANGBASE").unwrap();
    let written: f64 = lines[at + 2].parse().unwrap();
    assert!(
        (written - 90.0).abs() < 1e-9,
        "degrees in the header: {written}"
    );

    let mut again = Env::new();
    let path = again.write("base-in.dxf", text.as_bytes());
    again.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    let read = value(&mut again, "draft.angle.base").as_f64().unwrap();
    assert!(
        (read - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "radians in the setting: {read}"
    );
}

#[test]
fn a_current_layer_that_is_purged_stays_current() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run("draftbuilder.layer.create", json!({"name": "PLAN"}));
    env.run(
        "draftbuilder.settings.setvar",
        json!({"name": "draft.layer.current", "value": "PLAN"}),
    );
    env.run("draftbuilder.drawing.purge", json!({"repeat": true}));
    assert_eq!(value(&mut env, "draft.layer.current"), json!("PLAN"));
}

#[test]
fn a_current_linetype_is_stored_by_identifier_and_kept_by_purge() {
    let mut env = Env::new();
    let path = env.write("dashed.dxf", dxf(&[], &[], true).as_bytes());
    env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    env.run(
        "draftbuilder.settings.setvar",
        json!({"name": "draft.linetype.current", "value": "DASHED"}),
    );
    let purged = env.run("draftbuilder.drawing.purge", json!({"repeat": true}));
    assert!(
        !purged.to_string().contains("DASHED"),
        "the current linetype is not removed: {purged}"
    );
    assert_eq!(value(&mut env, "draft.linetype.current"), json!("DASHED"));
}

#[test]
fn the_current_lineweight_takes_a_standard_value_only() {
    let mut env = Env::new();
    assert_eq!(
        code(env.invoke(
            "draftbuilder.settings.set",
            json!({"name": "draft.lineweight.current", "value": 17})
        )),
        "db.cmd.setting_value"
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.lineweight.current", "value": 18}),
    );
    assert_eq!(value(&mut env, "draft.lineweight.current"), json!(18));
}

#[test]
fn more_than_thirty_two_additional_polar_angles_are_refused() {
    let mut env = Env::new();
    let too_many: Vec<f64> = (0..33).map(|_| 1.0).collect();
    assert_eq!(
        code(env.invoke(
            "draftbuilder.settings.set",
            json!({"name": "draft.polar.additional", "value": too_many})
        )),
        "db.cmd.setting_value"
    );
}

#[test]
fn a_pick_needs_an_aperture_in_drawing_units() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    assert_eq!(
        code(env.invoke("draftbuilder.precision.resolve", json!({"point": [0, 0]}))),
        "cmd.params.invalid"
    );
    assert_eq!(
        code(env.invoke(
            "draftbuilder.precision.resolve",
            json!({"point": [0, 0], "aperture": -1.0})
        )),
        "db.cmd.value_invalid"
    );
}

#[test]
fn a_one_pick_off_override_keeps_the_snap_step() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.snap.on", "value": true}),
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.snap.spacing", "value": [1.0, 1.0]}),
    );
    let pick = env.run(
        "draftbuilder.precision.resolve",
        json!({"point": [0.4, 0.6], "over": "NON", "aperture": 1.0}),
    );
    assert_eq!(
        pick["point"],
        json!([0.0, 1.0]),
        "the step still applies beside NON"
    );
}

#[test]
fn a_header_coordinate_system_keeps_its_direction_at_a_large_origin() {
    let mut env = Env::new();
    // The header's X direction is (0.001, 1) at an origin of (1e15, 0): forming a point from origin
    // plus direction rounds the 0.001 away, so the direction would come out as (0, 1).
    let path = env.write(
        "ucs.dxf",
        dxf(
            &[
                ("$UCSORG", " 10\n1000000000000000\n 20\n0\n 30\n0\n"),
                ("$UCSXDIR", " 10\n0.001\n 20\n1\n 30\n0\n"),
            ],
            &[],
            false,
        )
        .as_bytes(),
    );
    env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    let saved = env.run("draftbuilder.precision.ucs_save", json!({"name": "probe"}));
    let length = (1.0_f64 + 0.001 * 0.001).sqrt();
    let x = saved["x_axis"].as_array().unwrap();
    assert!(
        (x[0].as_f64().unwrap() - 0.001 / length).abs() < 1e-12,
        "{saved}"
    );
    assert!(
        (x[1].as_f64().unwrap() - 1.0 / length).abs() < 1e-12,
        "{saved}"
    );
}

#[test]
fn a_mirrored_header_coordinate_system_is_ignored_and_reported() {
    let mut env = Env::new();
    let path = env.write(
        "mirror.dxf",
        dxf(
            &[
                ("$UCSORG", " 10\n0\n 20\n0\n 30\n0\n"),
                ("$UCSXDIR", " 10\n1\n 20\n0\n 30\n0\n"),
                ("$UCSYDIR", " 10\n0\n 20\n-1\n 30\n0\n"),
            ],
            &[],
            false,
        )
        .as_bytes(),
    );
    let opened = env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert!(
        reasons(&opened).contains(&"db.io.header_value_ignored".to_owned()),
        "{opened}"
    );
    let saved = env.run("draftbuilder.precision.ucs_save", json!({"name": "probe"}));
    assert_eq!(saved["x_axis"], json!([1.0, 0.0]), "the world system");
}

#[test]
fn the_reserved_current_system_cannot_be_deleted_or_restored() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    assert_eq!(
        code(env.invoke(
            "draftbuilder.precision.ucs_delete",
            json!({"name": "*current*"})
        )),
        "db.cmd.ucs_name"
    );
    assert_eq!(
        code(env.invoke(
            "draftbuilder.precision.ucs_restore",
            json!({"name": "*previous*"})
        )),
        "db.cmd.ucs_name"
    );
}

#[test]
fn a_three_point_system_on_the_clockwise_side_is_refused() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    assert_eq!(
        code(env.invoke(
            "draftbuilder.precision.ucs",
            json!({"mode": "three_points", "origin": [0, 0], "x_point": [1, 0], "y_point": [0, -1]}),
        )),
        "db.cmd.ucs_handedness"
    );
}

#[test]
fn the_current_layer_cannot_be_frozen_and_a_locked_current_layer_takes_new_objects() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run("draftbuilder.layer.create", json!({"name": "Work"}));
    env.run(
        "draftbuilder.settings.setvar",
        json!({"name": "draft.layer.current", "value": "Work"}),
    );
    let line = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        code(env.invoke(
            "draftbuilder.layer.freeze",
            json!({"objects": [line.clone()]})
        )),
        "db.cmd.current_layer"
    );
    env.run("draftbuilder.layer.lock", json!({"objects": [line]}));
    let created = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 1.0], [1.0, 1.0]]}),
    );
    assert!(
        created["ids"][0].is_string(),
        "creation on a locked current layer is allowed: {created}"
    );
}

#[test]
fn a_session_setting_changes_no_document_state_and_no_undo_step() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let revision = env.document.snapshot().revision();
    let undo = env.document.can_undo();
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "snap.override.typed_first", "value": true}),
    );
    assert_eq!(env.document.snapshot().revision(), revision, "no revision");
    assert_eq!(env.document.can_undo(), undo, "no undo step");
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "snap.override.typed_first", "value": false}),
    );
}

#[test]
fn a_bulged_polyline_has_its_arc_in_its_area_and_perimeter() {
    // A closed polyline: a half circle of radius 1 from (0,0) to (2,0) and the straight way back. Its area
    // is the half disc, pi / 2, and its perimeter is the arc, pi, plus the segment, 2.
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let id = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0], "bulge": 1.0}, {"point": [2.0, 0.0]}], "close": true}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let area = env.run("draftbuilder.inquiry.area", json!({"objects": [id]}));
    let pi = std::f64::consts::PI;
    let value = area["areas"][0]["area"]
        .as_f64()
        .or_else(|| area["area"].as_f64())
        .unwrap();
    let perimeter = area["areas"][0]["perimeter"]
        .as_f64()
        .or_else(|| area["perimeter"].as_f64())
        .unwrap();
    assert!((value - pi / 2.0).abs() < 1e-9, "{area}");
    assert!((perimeter - (pi + 2.0)).abs() < 1e-9, "{area}");
}

#[test]
fn ortho_locks_to_the_axes_of_a_rotated_coordinate_system() {
    // The system's X axis is the world's Y axis (a quarter turn). A pick from the origin at (0.2, 5) is
    // locked to that axis: (0, 5), not to the world X axis.
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run(
        "draftbuilder.precision.ucs",
        json!({"mode": "origin", "origin": [0, 0], "x_point": [0, 1]}),
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.ortho", "value": true}),
    );
    let pick = env.run(
        "draftbuilder.precision.resolve",
        json!({"point": [0.2, 5.0], "from": [0.0, 0.0], "aperture": 1.0}),
    );
    let point = pick["point"].as_array().unwrap();
    assert!(
        point[0].as_f64().unwrap().abs() < 1e-9 && (point[1].as_f64().unwrap() - 5.0).abs() < 1e-9,
        "{pick}"
    );
}

#[test]
fn the_distance_angle_is_measured_from_the_angle_base_in_the_drawing_direction() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.angle.base", "value": std::f64::consts::FRAC_PI_2}),
    );
    let distance = env.run(
        "draftbuilder.inquiry.distance",
        json!({"from": [0.0, 0.0], "to": [1.0, 0.0]}),
    );
    let angle = distance["angle"].as_f64().unwrap();
    // The world direction of +X is a quarter turn clockwise from a base of +Y: -90 degrees, reported
    // in [0, 2 pi) as 270 degrees.
    assert!(
        (angle - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "{distance}"
    );
}
