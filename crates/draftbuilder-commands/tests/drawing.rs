//! The drawing commands, headless: open of DWG and DXF (content detection, refusals, units and
//! scaling), new, save (names, targets, permissions), and audit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "test code"
)]

mod common;

use arc_draft::types::LAYER;
use common::{Env, FIXTURE, entity_count, lines, reasons, text_heights, with_units};
use draftbuilder_model::Database;
use serde_json::{Value, json};

/// A DXF file with one line whose two ends coincide.
const ZERO_LINE: &str = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n4\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  5\n1A\n  8\n0\n 10\n1.0\n 20\n1.0\n 30\n0.0\n 11\n1.0\n 21\n1.0\n 31\n0.0\n  0\nENDSEC\n  0\nEOF\n";

/// The conversion items of a result: the mapper's `units_converted` items.
fn conversions(result: &Value) -> Vec<Value> {
    result["report"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["reason"] == "draft.dxf.units_converted")
        .cloned()
        .collect()
}

#[test]
fn a_dwg_file_of_an_unsupported_version_is_refused_and_not_touched() {
    let mut env = Env::new();
    let bytes = b"AC1033 not read, the version is newer than R2018".to_vec();
    let path = env.write("newer.dwg", &bytes);
    let before = env.document.snapshot().revision();
    let error = env
        .invoke("draftbuilder.drawing.open", json!({"path": path}))
        .unwrap_err();
    assert_eq!(error.code(), "io.dwg.version_unsupported");
    assert_eq!(std::fs::read(env.dir.join("newer.dwg")).unwrap(), bytes);
    assert_eq!(env.document.snapshot().revision(), before);
}

#[test]
fn a_dwg_file_without_a_dwg_signature_is_refused() {
    let mut env = Env::new();
    let path = env.write("bad.dwg", b"AD1032 is not a DWG signature");
    let error = env
        .invoke("draftbuilder.drawing.open", json!({"path": path}))
        .unwrap_err();
    assert_eq!(error.code(), "io.dwg.signature_invalid");
}

#[test]
fn a_truncated_dwg_file_fails_inside_the_reader_and_the_document_stays_empty() {
    let mut env = Env::new();
    let mut bytes = b"AC1032".to_vec();
    bytes.resize(64, 0);
    let path = env.write("truncated.dwg", &bytes);
    let before = env.document.snapshot().revision();
    let error = env
        .invoke("draftbuilder.drawing.open", json!({"path": path}))
        .unwrap_err();
    assert!(error.code().starts_with("io.dwg."), "{}", error.code());
    assert_eq!(env.document.snapshot().revision(), before);
}

#[test]
fn a_dxf_file_named_dwg_is_read_by_its_content() {
    let mut env = Env::new();
    let path = env.write("m0.dwg", FIXTURE.as_bytes());
    let opened = env.run("draftbuilder.drawing.open", json!({"path": path}));
    assert_eq!(opened["format"], "dxf");
    assert_eq!(opened["handles"], 15, "{opened}");
}

#[test]
fn a_dwg_signature_under_a_dxf_name_is_read_as_dwg() {
    let mut env = Env::new();
    let path = env.write("newer.dxf", b"AC1033 newer than the reader");
    let error = env
        .invoke("draftbuilder.drawing.open", json!({"path": path}))
        .unwrap_err();
    assert_eq!(error.code(), "io.dwg.version_unsupported");
}

#[test]
fn the_extension_names_the_format_when_the_content_does_not() {
    let mut env = Env::new();
    let dwg = env.write("drawing.dwg", b"AC1032");
    let error = env
        .invoke("draftbuilder.drawing.open_dxf", json!({"path": dwg}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.format_mismatch");
    let txt = env.write("drawing.txt", b"nothing a reader knows");
    let error = env
        .invoke("draftbuilder.drawing.open", json!({"path": txt}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.format_unknown");
}

#[test]
fn open_reports_exactly_one_conversion_with_the_factor_the_mapper_applied() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    let opened = env.run(
        "draftbuilder.drawing.open",
        json!({"path": path, "unit": "inch"}),
    );
    assert_eq!(opened["unit"], "inch");
    let items = conversions(&opened);
    assert_eq!(items.len(), 1, "{opened}");
    assert_eq!(items[0]["arguments"]["from"], 4);
    let factor: f64 = items[0]["arguments"]["factor"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((factor - 1.0 / 25.4).abs() < 1e-12, "{factor}");
    assert!(!reasons(&opened).contains(&"db.io.import_transform".to_owned()));
}

#[test]
fn open_without_a_unit_keeps_the_files_unit_and_converts_nothing() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    let opened = env.run("draftbuilder.drawing.open", json!({"path": path}));
    assert_eq!(opened["unit"], "millimeter");
    assert!(conversions(&opened).is_empty());
}

#[test]
fn a_decimeter_file_is_scaled_by_the_mapped_factor_and_reported_once() {
    let mut scaled = Env::new();
    let path = scaled.write("dm.dxf", with_units("14").as_bytes());
    let opened = scaled.run(
        "draftbuilder.drawing.open",
        json!({"path": path, "unit": "inch"}),
    );
    let items = conversions(&opened);
    assert_eq!(items.len(), 1, "{opened}");
    let factor: f64 = items[0]["arguments"]["factor"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let expected = 0.1 / 0.0254;
    assert!((factor - expected).abs() < 1e-9, "{factor} {expected}");

    // The same file opened without a unit keeps its numbers: the geometry scaled by the reported
    // factor is what the unit-changing open produced.
    let mut plain = Env::new();
    let path = plain.write("dm.dxf", with_units("14").as_bytes());
    let unscaled = plain.run("draftbuilder.drawing.open", json!({"path": path}));
    assert_eq!(unscaled["unit"], "unitless");
    assert!(reasons(&unscaled).contains(&"draft.dxf.units_unitless".to_owned()));
    // Model-space lines are the ones the factor applies to; the block's lines keep their unit.
    let space = scaled.space();
    let plain_space = plain.space();
    let before: Vec<_> = lines(&plain.document.snapshot())
        .into_iter()
        .filter(|r| r.0 == Some(plain_space))
        .collect();
    let after: Vec<_> = lines(&scaled.document.snapshot())
        .into_iter()
        .filter(|r| r.0 == Some(space))
        .collect();
    assert!(!before.is_empty());
    assert_eq!(before.len(), after.len());
    for (a, b) in before.iter().zip(after.iter()) {
        for k in 0..2 {
            assert!((b.1[k] - a.1[k] * expected).abs() < 1e-9, "{a:?} {b:?}");
            assert!((b.2[k] - a.2[k] * expected).abs() < 1e-9, "{a:?} {b:?}");
        }
    }
}

#[test]
fn opening_with_a_unit_scales_model_geometry_and_text_heights_and_keeps_block_content() {
    let mut plain = Env::new();
    let path = plain.write("m0.dxf", FIXTURE.as_bytes());
    plain.run("draftbuilder.drawing.open", json!({"path": path}));
    let mut scaled = Env::new();
    let path = scaled.write("m0.dxf", FIXTURE.as_bytes());
    scaled.run(
        "draftbuilder.drawing.open",
        json!({"path": path, "unit": "inch"}),
    );
    let factor = 1.0 / 25.4;
    let (p_snap, s_snap) = (plain.document.snapshot(), scaled.document.snapshot());
    let model_space = plain.space();

    // Model-space lines are scaled; the lines of the block keep their coordinates, because a block
    // keeps the unit of its block record and the drafting model converts a reference when it draws.
    let (mut pm, mut sm, mut pb, mut sb) = (vec![], vec![], vec![], vec![]);
    for row in lines(&p_snap) {
        if row.0 == Some(model_space) {
            pm.push(row)
        } else {
            pb.push(row)
        }
    }
    for row in lines(&s_snap) {
        if row.0 == Some(model_space) {
            sm.push(row)
        } else {
            sb.push(row)
        }
    }
    assert!(
        !pm.is_empty() && !pb.is_empty(),
        "the fixture has model and block lines"
    );
    assert_eq!(pm.len(), sm.len());
    for (a, b) in pm.iter().zip(sm.iter()) {
        for k in 0..2 {
            assert!((b.1[k] - a.1[k] * factor).abs() < 1e-9, "{a:?} {b:?}");
            assert!((b.2[k] - a.2[k] * factor).abs() < 1e-9, "{a:?} {b:?}");
        }
    }
    assert_eq!(pb, sb, "block content is not converted on open");

    // Text heights of the model space are scaled.
    let (pt, st): (Vec<_>, Vec<_>) = (
        text_heights(&p_snap)
            .into_iter()
            .filter(|r| r.0 == Some(model_space))
            .collect(),
        text_heights(&s_snap)
            .into_iter()
            .filter(|r| r.0 == Some(model_space))
            .collect(),
    );
    assert!(!pt.is_empty());
    for (a, b) in pt.iter().zip(st.iter()) {
        assert!((b.1 - a.1 * factor).abs() < 1e-9, "{a:?} {b:?}");
    }
}

#[test]
fn new_creates_an_empty_drawing_from_a_unit_system() {
    let mut env = Env::new();
    let created = env.run("draftbuilder.drawing.new", json!({}));
    assert_eq!(created["unit"], "millimeter");
    assert!(created["report"].is_null());
    assert_eq!(entity_count(&env.document.snapshot()), 0);
    assert!(
        Database::of(&env.document.snapshot())
            .fingerprint()
            .is_some()
    );

    let mut imperial = Env::new();
    let created = imperial.run("draftbuilder.drawing.new", json!({"system": "imperial"}));
    assert_eq!(created["unit"], "inch");
}

#[test]
fn new_from_a_template_takes_its_tables_and_unit_but_not_its_geometry() {
    let mut env = Env::new();
    let template = env.write("template.dxf", FIXTURE.as_bytes());
    let created = env.run("draftbuilder.drawing.new", json!({"template": template}));
    assert_eq!(created["unit"], "millimeter");
    assert_eq!(created["layers"], 3, "{created}");
    assert!(
        created["report"].is_null(),
        "a template is not a transfer: no report"
    );
    let snapshot = env.document.snapshot();
    assert_eq!(
        snapshot.iter_type(LAYER).unwrap().count(),
        3,
        "the template's layers"
    );
    assert_eq!(entity_count(&snapshot), 0);
}

#[test]
fn new_from_a_template_leaves_no_source_bindings_in_the_document() {
    let mut env = Env::new();
    let template = env.write("template.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.new", json!({"template": template}));
    let snapshot = env.document.snapshot();
    let bindings = snapshot
        .foreign_rows()
        .filter(|r| r.record_kind == arc_doc::source_map::RECORD_KIND)
        .count();
    assert_eq!(
        bindings, 0,
        "the template's handles are not the new drawing's"
    );
    assert_eq!(Database::of(&snapshot).len(), 0);
}

#[test]
fn a_new_drawing_can_be_saved_and_the_report_says_the_fingerprint_was_assigned() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let saved = env.run(
        "draftbuilder.drawing.save",
        json!({"path": env.path("new.dxf")}),
    );
    assert!(reasons(&saved).contains(&"db.io.fingerprint_assigned".to_owned()));
    let text = std::fs::read_to_string(env.dir.join("new.dxf")).unwrap();
    assert!(text.contains("$FINGERPRINTGUID"));
}

#[test]
fn save_replaces_the_target_and_leaves_no_temporary_file() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let target = env.path("out.dxf");
    std::fs::write(env.dir.join("out.dxf"), "an older file").unwrap();
    env.run("draftbuilder.drawing.save", json!({"path": target}));
    let text = std::fs::read_to_string(env.dir.join("out.dxf")).unwrap();
    assert!(text.contains("ENTITIES"), "the target is the new drawing");
    let names: Vec<String> = std::fs::read_dir(&env.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
}

#[test]
fn a_long_file_name_saves() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let name = format!("{}.dxf", "a".repeat(250));
    env.run(
        "draftbuilder.drawing.save",
        json!({"path": env.path(&name)}),
    );
    assert!(env.dir.join(&name).is_file());
}

#[test]
fn a_dwg_target_is_refused_including_a_file_named_just_dwg() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    for name in ["out.dwg", "OUT.DWG", ".dwg"] {
        let error = env
            .invoke("draftbuilder.drawing.save", json!({"path": env.path(name)}))
            .unwrap_err();
        assert_eq!(error.code(), "db.cmd.format_unsupported", "{name}");
        assert!(!env.dir.join(name).exists(), "{name}");
    }
}

#[test]
fn a_target_that_is_a_directory_is_refused_before_anything_is_written() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    std::fs::create_dir(env.dir.join("out.dxf")).unwrap();
    let error = env
        .invoke(
            "draftbuilder.drawing.save",
            json!({"path": env.path("out.dxf")}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.target_not_file");
    let names: Vec<String> = std::fs::read_dir(&env.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_target_is_refused() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    std::fs::write(env.dir.join("real.txt"), "keep").unwrap();
    std::os::unix::fs::symlink(env.dir.join("real.txt"), env.dir.join("link.dxf")).unwrap();
    let error = env
        .invoke(
            "draftbuilder.drawing.save",
            json!({"path": env.path("link.dxf")}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.target_not_file");
    assert_eq!(
        std::fs::read_to_string(env.dir.join("real.txt")).unwrap(),
        "keep"
    );
}

#[cfg(unix)]
#[test]
fn a_save_over_a_file_keeps_its_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let target = env.write("out.dxf", b"older");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    env.run(
        "draftbuilder.drawing.save",
        json!({"path": env.path("out.dxf")}),
    );
    let mode = std::fs::metadata(env.dir.join("out.dxf"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn audit_reports_nothing_for_an_opened_drawing_and_changes_nothing() {
    let mut env = Env::new();
    let path = env.write("m0.dxf", FIXTURE.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    let before = env.document.snapshot().revision();
    let audited = env.run("draftbuilder.drawing.audit", json!({}));
    assert_eq!(audited["findings"], json!([]), "{audited}");
    assert_eq!(audited["omitted"], 0);
    assert_eq!(audited["bindings"], 15);
    assert!(audited["objects"].as_u64().unwrap() > 0);
    assert_eq!(env.document.snapshot().revision(), before);
}

#[test]
fn a_zero_length_line_in_a_file_is_not_imported_and_the_report_says_so() {
    let mut env = Env::new();
    let path = env.write("zero.dxf", ZERO_LINE.as_bytes());
    let opened = env.run("draftbuilder.drawing.open", json!({"path": path}));
    assert_eq!(
        env.document
            .snapshot()
            .iter_type(arc_draft::types::LINE)
            .unwrap()
            .count(),
        0,
        "{opened}"
    );
    assert!(
        reasons(&opened).contains(&"draft.dxf.geometry_invalid".to_owned()),
        "{opened}"
    );
    assert_eq!(opened["handles"], 0);
}
