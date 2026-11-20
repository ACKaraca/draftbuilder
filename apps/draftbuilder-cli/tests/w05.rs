//! Workflow W05 (`docs/product/workflows.md`) run as a script, for its open, preservation report and
//! DXF save steps: the partner drawing of `tests/data/partner_drawing.dxf` (written by this project
//! from the DXF text format) is opened, its preservation report is read, the plan is moved and a wall
//! changes its layer, and the drawing is saved. A DWG drawing written by the foundation's fixture
//! helper is opened as well. The one ignored test states what the workflow needs and the foundation
//! does not give; it is run with `--ignored`.

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

/// The reasons of the save's preservation items that the partner drawing can trigger. None fires on
/// it: its references all name written objects, its owners are written, its application names are in
/// its table, its extended data is within the bounds and its header variables are storable.
const SAVE_ITEM_REASONS: &[&str] = &[
    "draft.dxf.reference_nulled",
    "draft.dxf.owner_substituted",
    "draft.dxf.appid_added",
    "draft.dxf.xdata_omitted",
    "draft.dxf.header_derived_omitted",
    "draft.dxf.header_handle_omitted",
    "draft.dxf.header_name_omitted",
    "draft.dxf.header_value_omitted",
    "draft.dxf.export_unmapped",
];

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

/// The groups of a record from its first application group (1001) on: the extended data.
fn extended_data(record: &[(i64, String)]) -> Vec<(i64, String)> {
    let start = record.iter().position(|(c, _)| *c == 1001).unwrap();
    record[start..].to_vec()
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
fn w05_reports_what_the_open_kept_and_dropped_nothing() {
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
    assert_eq!(
        preserved("DICTIONARY").expect("the root dictionary is reported")["arguments"]["count"],
        1
    );
    assert_eq!(
        preserved("CLASS").expect("the classes are reported")["arguments"]["count"],
        2
    );
    assert_eq!(
        preserved("APPID").expect("the application table is reported")["arguments"]["count"],
        1
    );
    assert!(
        items.iter().all(|i| i["outcome"] != "dropped"),
        "the partner drawing loses nothing on open"
    );
}

#[test]
fn w05_save_reports_the_items_the_fixture_triggers() {
    let mut h = Harness::new();
    let (report, _) = edited(&mut h);
    let save = &report["reports"][1];
    assert_eq!(save["command"], "draftbuilder.drawing.save");
    let items = save["report"]["items"].as_array().unwrap();
    // None of the save's preservation items fires on this drawing, so each reason counts zero; a
    // reason that appears here is a change in what the foundation keeps for this file.
    for reason in SAVE_ITEM_REASONS {
        let count = items.iter().filter(|i| i["reason"] == *reason).count();
        assert_eq!(count, 0, "{reason} is reported for the partner drawing");
    }
    assert!(
        items.iter().all(|i| i["outcome"] != "dropped"),
        "the save drops nothing of the partner drawing"
    );
}

#[test]
fn w05_every_preserved_record_the_open_reports_is_in_the_saved_file() {
    let mut h = Harness::new();
    let (report, saved) = edited(&mut h);
    let items = report["reports"][0]["report"]["items"].as_array().unwrap();
    let all = dxf_records(&saved);
    // The counts of the open report for the preserved records of the file, against the records of the
    // saved file with that type. The root dictionary is not counted: the save writes its own layouts
    // dictionary as a second one.
    for kind in ["ARCTEST_GLYPH", "ARCTEST_RECORD", "CLASS"] {
        let reported = items
            .iter()
            .find(|i| i["reason"] == "draft.dxf.record_preserved" && i["arguments"]["kind"] == kind)
            .unwrap()["arguments"]["count"]
            .as_u64()
            .unwrap();
        let written = u64::try_from(all.iter().filter(|r| r[0].1 == kind).count()).unwrap();
        assert_eq!(
            written, reported,
            "{kind} is written as many times as it was read"
        );
    }
    assert!(
        section(&saved, "TABLES")
            .iter()
            .any(|r| value_of(r, 2) == Some("ARCTESTAPP")),
        "the application the preserved records name is in the table"
    );
    assert_eq!(
        common::header_value(&saved, "$ARCTEST_NOTE").as_deref(),
        Some("partner-owned header value")
    );
}

#[test]
fn w05_edits_the_plan_and_the_layer_and_keeps_the_handles() {
    let mut h = Harness::new();
    let (report, saved) = edited(&mut h);
    assert_eq!(report["skipped"], 0);

    // The window took every entity of the plan; the custom entity is a preserved record, not an object
    // of the drawing, so the move leaves it (see the ignored test).
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
    // spelling of the same value. The owner (group 330) is checked by its own test.
    let before = section(&source, "ENTITIES")
        .into_iter()
        .find(|r| r[0].1 == "ARCTEST_GLYPH")
        .unwrap();
    let after = entity(&saved, "24");
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
fn the_extended_data_of_an_ordinary_entity_survives_the_save() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let source = String::from_utf8(PARTNER.to_vec()).unwrap();
    // The wall's extended data, group by group and in order, from its application name on. The window
    // moves the wall's geometry; the extended data is kept as stored, so its points do not move.
    let before = extended_data(&entity(&source, "25"));
    let after = extended_data(&entity(&saved, "25"));
    assert_eq!(
        after.first(),
        Some(&(1001, "ARCTESTAPP".to_owned())),
        "the wall's application name opens its extended data"
    );
    assert_eq!(after.len(), before.len(), "{after:?}");
    for ((c1, v1), (c2, v2)) in before.iter().zip(&after) {
        assert_eq!(c1, c2, "group code of {v1}");
        assert!(same_value(v1, v2), "group {c1}: {v1} is not {v2}");
    }
}

#[test]
#[ignore = "foundation gap: a custom entity is a preserved record, not a drafting object, so the window cannot select it and MOVE cannot move it (arc-draft-dxf keeps its groups as stored)"]
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
fn the_owner_of_a_preserved_record_names_the_written_model_space() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    // The owner of an entity is its container (arc-draft-dxf, References): the written model space
    // block record, whose handle is the export's, not the file's (`10`).
    let model_space = section(&saved, "TABLES")
        .iter()
        .find(|r| r[0].1 == "BLOCK_RECORD" && value_of(r, 2) == Some("*Model_Space"))
        .and_then(|r| handle_of(r))
        .unwrap();
    for handle in PARTNER_ENTITY_HANDLES {
        assert_eq!(
            value_of(&entity(&saved, handle), 330),
            Some(model_space.as_str()),
            "entity {handle} is owned by the written model space"
        );
    }
}

#[test]
fn the_class_records_and_the_application_table_survive() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let classes = section(&saved, "CLASSES");
    for (name, count) in [("ARCTEST_GLYPH", "1"), ("ARCTEST_RECORD", "1")] {
        let class = classes
            .iter()
            .find(|r| value_of(r, 1) == Some(name))
            .unwrap_or_else(|| panic!("the class {name} is written"));
        assert_eq!(
            value_of(class, 3),
            Some("ARCTESTAPP"),
            "the class of {name}"
        );
        // The instance count is recomputed: one written record of each class.
        assert_eq!(value_of(class, 91), Some(count), "the instances of {name}");
    }
    let tables = section(&saved, "TABLES");
    let apps: Vec<&str> = tables
        .iter()
        .filter(|r| r[0].1 == "APPID")
        .filter_map(|r| value_of(r, 2))
        .collect();
    assert_eq!(
        apps.iter().filter(|n| **n == "ARCTESTAPP").count(),
        1,
        "the application is registered once"
    );
    assert!(
        apps.contains(&"ACAD"),
        "the standard application is written"
    );
}

#[test]
fn the_root_dictionary_names_the_preserved_object() {
    let mut h = Harness::new();
    let (_, saved) = edited(&mut h);
    let objects = section(&saved, "OBJECTS");
    let root = objects
        .iter()
        .find(|r| r[0].1 == "DICTIONARY" && r.iter().any(|(c, v)| *c == 3 && v == "ARCTEST"))
        .unwrap();
    assert_eq!(value_of(root, 330), Some("0"), "the root has no owner");
    let record = objects.iter().find(|r| r[0].1 == "ARCTEST_RECORD").unwrap();
    // The entry names the object under the handle the object is written with, and the object's owner
    // is the root.
    let at = root
        .iter()
        .position(|(c, v)| *c == 3 && v == "ARCTEST")
        .unwrap();
    assert_eq!(root.get(at + 1), Some(&(350, handle_of(record).unwrap())));
    assert_eq!(value_of(record, 330), handle_of(root).as_deref());
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
