//! Arrays on save, end to end: an array the export writes is saved as block references, one per item,
//! at each item's position, rotation and scale, and reported as approximated with the save's
//! `arrays_as_references` count. An array the export cannot place refuses the save, and nothing is written.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use common::{Env, dxf_records};
use serde_json::json;

fn circle_at(env: &mut Env, center: [f64; 2]) -> String {
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": center, "radius": 1.0}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The group `code` of a record as a number, `default` when absent.
fn number(record: &[(i64, String)], code: i64, default: f64) -> f64 {
    record
        .iter()
        .find(|(c, _)| *c == code)
        .map_or(default, |(_, v)| v.parse().unwrap())
}

/// An anonymous block: its name, its base point and the centers of its circles.
type BlockCircles = (String, (f64, f64), Vec<(f64, f64)>);

/// The displayed centers of the circles of the anonymous blocks, in model space: each circle of a block
/// moved by each reference's scale, rotation (degrees, counterclockwise) and insertion point. Sorted.
fn displayed_centers(text: &str) -> Vec<(f64, f64)> {
    let records = dxf_records(text);
    let name = |r: &[(i64, String)]| {
        r.iter()
            .find(|(c, _)| *c == 2)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let mut blocks: Vec<BlockCircles> = Vec::new();
    let mut open = None;
    for r in &records {
        match r[0].1.as_str() {
            "BLOCK" if name(r).starts_with("*A") => {
                blocks.push((
                    name(r),
                    (number(r, 10, 0.0), number(r, 20, 0.0)),
                    Vec::new(),
                ));
                open = Some(blocks.len() - 1);
            }
            "ENDBLK" => open = None,
            "CIRCLE" => {
                if let Some(i) = open {
                    blocks[i].2.push((number(r, 10, 0.0), number(r, 20, 0.0)));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for r in records.iter().filter(|r| r[0].1 == "INSERT") {
        let Some((_, base, centers)) = blocks.iter().find(|(n, _, _)| *n == name(r)) else {
            continue;
        };
        let (ix, iy) = (number(r, 10, 0.0), number(r, 20, 0.0));
        let (sx, sy) = (number(r, 41, 1.0), number(r, 42, 1.0));
        let theta = number(r, 50, 0.0).to_radians();
        for c in centers {
            let (vx, vy) = (sx * (c.0 - base.0), sy * (c.1 - base.1));
            out.push((
                ix + vx * theta.cos() - vy * theta.sin(),
                iy + vx * theta.sin() + vy * theta.cos(),
            ));
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

/// The rotations (degrees, in [0, 360)) and scales of the references of anonymous blocks, sorted.
fn references(text: &str) -> Vec<(f64, f64, f64)> {
    let mut out: Vec<(f64, f64, f64)> = dxf_records(text)
        .iter()
        .filter(|r| r[0].1 == "INSERT" && r.iter().any(|(c, v)| *c == 2 && v.starts_with("*A")))
        .map(|r| {
            (
                number(r, 50, 0.0).rem_euclid(360.0),
                number(r, 41, 1.0),
                number(r, 42, 1.0),
            )
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn assert_points(actual: &[(f64, f64)], expected: &[(f64, f64)]) {
    let mut expected = expected.to_vec();
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, e) in actual.iter().zip(&expected) {
        assert!(
            (a.0 - e.0).abs() < 1e-9 && (a.1 - e.1).abs() < 1e-9,
            "{a:?} is not {e:?}"
        );
    }
}

#[test]
fn a_polar_array_saves_with_each_item_at_its_position_rotation_and_scale() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let hole = circle_at(&mut env, [5.0, 0.0]);
    env.run(
        "draftbuilder.modify.array_polar",
        json!({"objects": [hole], "center": [0.0, 0.0], "count": 4}),
    );
    let path = env.path("polar.dxf");
    let saved = env.run("draftbuilder.drawing.save", json!({"path": path.clone()}));
    assert_eq!(saved["arrays_as_references"], 1, "{saved}");

    let text = std::fs::read_to_string(&path).unwrap();
    assert_points(
        &displayed_centers(&text),
        &[(5.0, 0.0), (0.0, 5.0), (-5.0, 0.0), (0.0, -5.0)],
    );
    let rotations: Vec<f64> = references(&text).iter().map(|r| r.0).collect();
    assert_eq!(rotations, vec![0.0, 90.0, 180.0, 270.0]);
    for (_, sx, sy) in references(&text) {
        assert_eq!((sx, sy), (1.0, 1.0));
    }
}

#[test]
fn a_path_array_the_export_cannot_place_refuses_the_save_and_writes_nothing() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let path_line = line(&mut env, [0.0, 0.0], [100.0, 0.0]);
    let hole = circle_at(&mut env, [0.0, 5.0]);
    env.run(
        "draftbuilder.modify.array_path",
        json!({"objects": [hole], "path": path_line.clone(), "spacing": 1.0}),
    );
    // The path grows past the item limit: the array now has more items than an array may hold.
    env.run(
        "draftbuilder.modify.lengthen",
        json!({"object": path_line, "pick": [50.0, 0.0], "mode": "delta", "value": 19900.0}),
    );
    let out = env.path("refused.dxf");
    let error = env
        .invoke("draftbuilder.drawing.save", json!({"path": out.clone()}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.export_drops_objects", "{error:?}");
    assert!(!std::path::Path::new(&out).exists(), "nothing is written");
}
