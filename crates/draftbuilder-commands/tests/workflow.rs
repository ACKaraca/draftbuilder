//! Workflow W01 (`docs/product/workflows.md`), the steps the commands now cover, headless: a drawing
//! is drawn, its hole is arrayed, its corners are rounded, the outline is offset, a line is trimmed by
//! the holes, two steps are undone and one redone, and the drawing is saved as DXF and reopened with
//! the same entities. Steps the product does not build yet (the team template, the running snaps, the
//! snap-from offsets) are not in this test; see the end of the test for the list.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use std::collections::BTreeMap;

use arc_base::{Limits, ObjectId, Unit};
use arc_doc::Value;
use arc_draft::types::ENTITIES;
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use common::Env;
use serde_json::json;

fn kinds(env: &Env) -> BTreeMap<&'static str, usize> {
    let snapshot = env.document.snapshot();
    let mut out = BTreeMap::new();
    for key in ENTITIES {
        let n = snapshot.iter_type(key).map(Iterator::count).unwrap_or(0);
        if n > 0 {
            out.insert(*key, n);
        }
    }
    out
}

fn polyline_vertices(env: &Env, id: &str) -> usize {
    let wanted: ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    let object = snapshot.get(handle).unwrap();
    match Reader::new(snapshot.registry(), "core.draft.polyline", &object.values).get("geometry") {
        Some(Value::Geometry(bytes)) => match Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap() {
            Curve2::Polyline(p) => p.vertices().len(),
            other => panic!("{other:?}"),
        },
        _ => panic!("no geometry"),
    }
}

#[test]
fn workflow_w01_draw_array_round_offset_trim_undo_redo_save_and_reopen() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];

    // Step 3: the outline, a closed polyline of four corners.
    let outline = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [20.0, 0.0]},
                            {"point": [20.0, 10.0]}, {"point": [0.0, 10.0]}], "close": true}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();

    // Step 4: a hole, then an array of it in two columns.
    let hole = env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [5.0, 5.0], "radius": 1.0}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let array = env.run(
        "draftbuilder.modify.array_rect",
        json!({"objects": [hole], "rows": 1, "columns": 2, "column_spacing": 10.0}),
    );
    assert_eq!(array["objects"].as_array().unwrap().len(), 1, "{array}");

    // A centerline that the hole and a wall will cut, and the wall.
    let centerline = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 5.0], [20.0, 5.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let wall = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[12.0, -1.0], [12.0, 11.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();

    // Step 5: round the corners with a radius of 2, offset the outline inward by 1, trim the centerline
    // between the holes.
    let kinds_before_corners = kinds(&env);
    let rounded = env.run(
        "draftbuilder.modify.fillet",
        json!({"polyline": outline.clone(), "radius": 2.0}),
    );
    assert_eq!(rounded["skipped"].as_array().unwrap().len(), 0, "{rounded}");
    assert_eq!(
        polyline_vertices(&env, &outline),
        8,
        "each corner adds one vertex to each side"
    );
    let offset = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [outline.clone()], "distance": 1.0, "side": [10.0, 5.0]}),
    );
    let inner = offset["objects"][0].as_str().unwrap().to_owned();
    assert!(polyline_vertices(&env, &inner) >= 8);
    let trimmed = env.run(
        "draftbuilder.modify.trim",
        json!({"edges": [hole.clone(), wall], "targets": [{"object": centerline.clone(), "pick": [9.0, 5.0]}]}),
    );
    assert_eq!(trimmed["objects"].as_array().unwrap().len(), 2, "{trimmed}");
    let after_trim = kinds(&env);
    assert_eq!(
        after_trim["core.draft.polyline"], 2,
        "the outline and its offset"
    );
    assert_eq!(
        after_trim["core.draft.line"], 3,
        "the wall and the centerline in two parts"
    );
    assert_eq!(kinds_before_corners["core.draft.circle"], 1);

    // Undo two steps (the trim, the offset) and redo one (the offset).
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(
        kinds(&env)["core.draft.line"],
        2,
        "the wall and the centerline are whole again"
    );
    assert_eq!(
        kinds(&env)["core.draft.polyline"],
        1,
        "no offset after two undos"
    );
    assert!(env.document.redo().wait().unwrap().value);
    assert_eq!(
        kinds(&env)["core.draft.polyline"],
        2,
        "the offset is back after one redo"
    );
    assert_eq!(kinds(&env)["core.draft.line"], 2);

    // Step 6: save as DXF, reopen, and compare what is there. DXF has no array entity in the
    // foundation's export, so the array is dropped by the save and reported as such; the block that
    // holds the hole's content is kept.
    let before = kinds(&env);
    let path = env.dir.join("w01.dxf");
    draftbuilder_commands::save_document(&env.document, &path, &env.session).unwrap();
    let saved = env.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": env.path("w01-report.dxf")}),
    );
    let dropped: Vec<&str> = saved["report"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["reason"] == "draft.dxf.export_unmapped")
        .filter_map(|i| i["arguments"]["kind"].as_str())
        .collect();
    assert_eq!(dropped, vec!["core.draft.array"], "{saved}");
    let reopened = draftbuilder_commands::open_document(
        &path,
        arc_doc::DocumentConfig {
            workers: 0,
            ..arc_doc::DocumentConfig::default()
        },
        &env.session,
    )
    .unwrap();
    let snapshot = reopened.snapshot();
    let mut after = BTreeMap::new();
    for key in ENTITIES {
        let n = snapshot.iter_type(key).map(Iterator::count).unwrap_or(0);
        if n > 0 {
            after.insert(*key, n);
        }
    }
    let mut expected = before.clone();
    expected.remove("core.draft.array");
    assert_eq!(
        after, expected,
        "the reopened drawing has the same entities by kind, but no array"
    );
    // Not covered by this workflow (not built): the team template (step 1), the running snaps and
    // polar tracking (step 2), the snap-from offsets of the hole (step 4), and the fillet of two
    // lines by pick (the polyline form is used); see docs/architecture/overview.md.
}
