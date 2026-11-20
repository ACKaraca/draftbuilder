//! Workflow W05 (`docs/product/workflows.md`) run as a script, for its open, preservation report and
//! DXF save steps: the partner drawing of `tests/data/partner_drawing.dxf` (written by this project
//! from the DXF text format) is opened, its preservation report is read, the plan is moved and a wall
//! changes its layer, and the drawing is saved. A DWG drawing written by the foundation's fixture
//! helper is opened as well. The three ignored tests state what the workflow needs and the foundation
//! does not yet give; they are run with `--ignored`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use std::collections::BTreeSet;

use common::{Harness, dxf_records, fill, handle_of, section, value_of, values_of};
use draftbuilder_model::database::Database;
use serde_json::Value;

const W05: &str = include_str!("data/w05.scr");
const PARTNER: &[u8] = include_bytes!("data/partner_drawing.dxf");

/// The handles of the entities of the partner drawing, as its file gives them.
const PARTNER_ENTITY_HANDLES: &[&str] = &["20", "21", "22", "23", "24", "25"];

/// Runs the workflow on the partner drawing and returns the harness, the report and the saved file.
fn edited(h: &mut Harness) -> (Value, String) {
    let partner = h.write("partner_drawing.dxf", PARTNER);
    let output = h.path("partner_out.dxf");
    let text = fill(W05, &[("partner", &partner), ("output", &output)]);
    let report = h.run(&text);
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    (report.to_json(), std::fs::read_to_string(&output).unwrap())
}

/// The entity record with handle `handle` in the ENTITIES section of a DXF text.
fn entity(text: &str, handle: &str) -> Vec<(i64, String)> {
    section(text, "ENTITIES")
        .into_iter()
        .find(|r| handle_of(r).as_deref() == Some(handle))
        .unwrap()
}

/// Whether two group values are the same value: the same text, or the same number.
fn same_value(a: &str, b: &str) -> bool {
    a == b || matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if x == y)
}

/// The records of the OBJECTS section whose class is `class`, as their group pairs.
fn objects_of(text: &str, class: &str) -> Vec<Vec<(i64, String)>> {
    section(text, "OBJECTS")
        .into_iter()
        .filter(|r| r[0].1 == class)
        .collect()
}

#[test]
fn w05_reports_what_the_open_kept_and_what_it_dropped() {
    let mut h = Harness::new();
    let (report, _) = edited(&mut h);
    let open = &report["reports"][0];
    assert_eq!(open["command"], "draftbuilder.drawing.open");
    let items = open["report"]["items"].as_array().unwrap();
    let preserved = |kind: &str| {
        items
            .iter()
            .find(|i| i["reason"] == "draft.dxf.record_preserved" && i["arguments"]["kind"] == kind)
    };
    let glyph = preserved("ARCTEST_GLYPH").expect("the custom entity is reported");
    assert_eq!(glyph["arguments"]["count"], 1);
    assert_eq!(glyph["outcome"], "preserved");
    let record = preserved("ARCTEST_RECORD").expect("the custom object is reported");
    assert_eq!(record["arguments"]["count"], 1);
    assert!(
        items
            .iter()
            .any(|i| i["reason"] == "draft.dxf.table_skipped" && i["record_kind"] == "APPID"),
        "the application table is reported as dropped"
    );
    assert!(
        items
            .iter()
            .any(|i| i["reason"] == "draft.dxf.objects_skipped"),
        "the root dictionary is reported as dropped"
    );
}

#[test]
fn w05_edits_the_plan_and_the_layer_and_keeps_the_handles() {
    let mut h = Harness::new();
    let (report, saved) = edited(&mut h);
    assert_eq!(report["skipped"], 0);

    // The window took every entity of the plan; the custom entity is not an object of the drawing,
    // so the move leaves it (see the ignored tests).
    let moved_line = entity(&saved, "20");
    assert_eq!(values_of(&moved_line, 10), vec![1000.0]);
    assert_eq!(values_of(&moved_line, 20), vec![500.0]);
    assert_eq!(values_of(&moved_line, 11), vec![5000.0]);
    assert_eq!(values_of(&moved_line, 21), vec![500.0]);
    assert_eq!(
        value_of(&moved_line, 8),
        Some("WALLS"),
        "the first wall keeps its layer"
    );

    let polyline = entity(&saved, "21");
    let vertices: Vec<f64> = polyline
        .iter()
        .filter(|(c, _)| *c == 10 || *c == 20)
        .map(|(_, v)| v.parse::<f64>().unwrap())
        .collect();
    assert_eq!(
        vertices,
        vec![1000.0, 500.0, 5000.0, 500.0, 5000.0, 3500.0, 1000.0, 3500.0]
    );
    assert_eq!(value_of(&polyline, 70), Some("1"), "still closed");

    let text = entity(&saved, "22");
    assert_eq!(values_of(&text, 10), vec![1500.0]);
    assert_eq!(values_of(&text, 20), vec![1000.0]);
    assert_eq!(value_of(&text, 1), Some("PARTNER PLAN"));

    let insert = entity(&saved, "23");
    assert_eq!(values_of(&insert, 10), vec![2000.0]);
    assert_eq!(values_of(&insert, 20), vec![500.0]);
    assert_eq!(value_of(&insert, 2), Some("DOOR"));
    assert_eq!(
        value_of(&insert, 8),
        Some("WALLS"),
        "the door keeps its layer"
    );

    let second_wall = entity(&saved, "25");
    assert_eq!(values_of(&second_wall, 10), vec![5000.0]);
    assert_eq!(values_of(&second_wall, 21), vec![3500.0]);
    assert_eq!(
        value_of(&second_wall, 8),
        Some("PLAN"),
        "the wall that was picked changed its layer"
    );
    assert_eq!(
        value_of(&moved_line, 8),
        Some("WALLS"),
        "the wall that was not picked keeps its layer"
    );

    // Every entity of the file keeps its handle.
    let handles: BTreeSet<String> = section(&saved, "ENTITIES")
        .iter()
        .filter_map(|r| handle_of(r))
        .collect();
    let wanted: BTreeSet<String> = PARTNER_ENTITY_HANDLES
        .iter()
        .map(|h| (*h).to_owned())
        .collect();
    assert_eq!(handles, wanted);
}

#[test]
fn w05_keeps_the_custom_object_and_the_custom_entity_byte_for_byte() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let source = String::from_utf8(PARTNER.to_vec()).unwrap();

    // The custom object is written back exactly: every group, in order, as text.
    let record_before = objects_of(&source, "ARCTEST_RECORD");
    let record_after = objects_of(&saved, "ARCTEST_RECORD");
    assert_eq!(record_before.len(), 1);
    assert_eq!(record_after, record_before);

    // The custom entity keeps its groups and its extended data; a number may be written with another
    // spelling of the same value.
    let before = section(&source, "ENTITIES")
        .into_iter()
        .find(|r| r[0].1 == "ARCTEST_GLYPH")
        .unwrap();
    let after = entity(&saved, "24");
    // The owner (group 330) is the model space record, which the export writes with its own handle
    // (see the ignored test below); every other group is the same.
    assert_eq!(before.len(), after.len(), "{after:?}");
    for ((c1, v1), (c2, v2)) in before.iter().zip(&after) {
        assert_eq!(c1, c2);
        if *c1 != 330 {
            assert!(same_value(v1, v2), "group {c1}: {v1} is not {v2}");
        }
    }
}

#[test]
fn w05_without_edits_a_round_trip_keeps_the_handles_and_the_stable_identifiers() {
    // Open, save, reopen: the objects that were not edited keep their handles and their identifiers.
    let mut h = Harness::new();
    let partner = h.write("partner_drawing.dxf", PARTNER);
    let first = h.path("first.dxf");
    let text = format!(
        "OPEN path={}\nSAVE path={}\n",
        common::quoted(&partner),
        common::quoted(&first)
    );
    let report = h.run(&text);
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let saved = std::fs::read_to_string(&first).unwrap();
    let handles: BTreeSet<String> = section(&saved, "ENTITIES")
        .iter()
        .filter_map(|r| handle_of(r))
        .collect();
    assert_eq!(
        handles,
        PARTNER_ENTITY_HANDLES
            .iter()
            .map(|h| (*h).to_owned())
            .collect()
    );

    let before = h.document.snapshot();
    let before_ids = stable_ids(&before);
    let reopened = h.reopen(&first);
    assert_eq!(
        stable_ids(&reopened.snapshot()),
        before_ids,
        "the identifiers are the same"
    );
}

/// The stable identifiers of the entities of a snapshot (lines, polylines, texts and inserts): the
/// objects the workflow edits and keeps.
fn stable_ids(snapshot: &arc_doc::Snapshot) -> BTreeSet<String> {
    let database = Database::of(snapshot);
    snapshot
        .iter()
        .filter(|(handle, _)| {
            [
                "core.draft.line",
                "core.draft.polyline",
                "core.draft.text",
                "core.draft.block_ref",
            ]
            .contains(&common::type_of(snapshot, *handle).as_str())
        })
        .filter_map(|(_, object)| database.stable_id_of(object.id).map(|id| id.to_string()))
        .collect()
}

#[test]
fn w05_dwg_variant_opens_and_saves_as_dxf() {
    // The DWG fixture of the foundation: a layer WALLS, a line, a block DOOR with a circle, and an
    // insert of it. DWG writing is M3, so the save is DXF.
    let mut h = Harness::new();
    let dwg = h.write(
        "sample.dwg",
        &arc_io_dwg::testing::sample_dwg_bytes().unwrap(),
    );
    let output = h.path("sample.dxf");
    let text = format!(
        "OPEN path={}\nSAVE path={}\n",
        common::quoted(&dwg),
        common::quoted(&output)
    );
    let report = h.run(&text);
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    assert_eq!(report.reports.len(), 2, "the open and the save both report");
    let saved = std::fs::read_to_string(&output).unwrap();
    let records = dxf_records(&saved);
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "LINE" && value_of(r, 8) == Some("WALLS"))
    );
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "INSERT" && value_of(r, 2) == Some("DOOR"))
    );
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "BLOCK" && value_of(r, 2) == Some("DOOR"))
    );
}

#[test]
#[ignore = "foundation gap: the extended data of an ordinary entity is not written back (arc-draft-dxf export)"]
fn the_extended_data_of_an_ordinary_entity_survives_the_save() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let wall = entity(&saved, "25");
    assert!(
        wall.iter().any(|(c, v)| *c == 1001 && v == "ARCTESTAPP"),
        "the wall's extended data is written back"
    );
}

#[test]
#[ignore = "foundation gap: a custom entity is a preserved record, not an object, so the window cannot take it"]
fn the_custom_entity_moves_with_the_plan() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let glyph = entity(&saved, "24");
    assert_eq!(
        values_of(&glyph, 10),
        vec![3000.0],
        "the glyph moves by the plan's displacement"
    );
    assert_eq!(values_of(&glyph, 20), vec![2000.0]);
}

#[test]
#[ignore = "foundation gap: the owner of a preserved record is the export's model space handle, not the file's"]
fn the_owner_of_a_preserved_record_keeps_its_handle() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let source = String::from_utf8(PARTNER.to_vec()).unwrap();
    let before = section(&source, "ENTITIES")
        .into_iter()
        .find(|r| r[0].1 == "ARCTEST_GLYPH")
        .unwrap();
    assert_eq!(value_of(&entity(&saved, "24"), 330), value_of(&before, 330));
}

#[test]
#[ignore = "foundation gap: the class records and the application table of a file are not written back"]
fn the_class_records_and_the_application_table_survive() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    assert!(
        section(&saved, "CLASSES")
            .iter()
            .any(|r| value_of(r, 1) == Some("ARCTEST_GLYPH")),
        "the class of the custom entity is written"
    );
    assert!(
        section(&saved, "TABLES")
            .iter()
            .any(|r| value_of(r, 2) == Some("ARCTESTAPP")),
        "the application is registered"
    );
}

#[test]
#[ignore = "foundation gap: the root dictionary entry that names a preserved object is not written back"]
fn the_root_dictionary_names_the_preserved_object() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let names = section(&saved, "OBJECTS")
        .iter()
        .filter(|r| r[0].1 == "DICTIONARY")
        .any(|r| r.iter().any(|(c, v)| *c == 3 && v == "ARCTEST"));
    assert!(names, "the root dictionary has the entry ARCTEST");
}

#[test]
fn the_unknown_header_variable_survives_the_save() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    assert_eq!(
        common::header_value(&saved, "$ARCTEST_NOTE").as_deref(),
        Some("partner-owned header value")
    );
}
