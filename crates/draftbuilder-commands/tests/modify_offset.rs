//! OFFSET, headless: rays and construction lines offset through their own parallel constructions, a side
//! point on the entity is refused, and a sharp polyline corner is beveled beyond the miter limit.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_base::{Limits, Unit};
use arc_doc::Value;
use arc_draft::types::ENTITIES;
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use common::Env;
use serde_json::json;

fn setup(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
}

fn created_curve(env: &Env, id: &str) -> Curve2 {
    let wanted: arc_base::ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    for key in ENTITIES {
        if let Ok(iter) = snapshot.iter_type(key) {
            for (_, object) in iter {
                if object.id == wanted {
                    let reader = Reader::new(snapshot.registry(), key, &object.values);
                    if let Some(Value::Geometry(bytes)) = reader.get("geometry") {
                        return Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap();
                    }
                }
            }
        }
    }
    panic!("no entity {id}");
}

#[test]
fn a_ray_offsets_parallel_to_itself_on_the_side_of_the_point() {
    let mut env = Env::new();
    setup(&mut env);
    let ray = env.run(
        "draftbuilder.draw.ray",
        json!({"start": [0.0, 0.0], "through": [[1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [ray], "distance": 2.0, "side": [5.0, 1.0]}),
    );
    let copy = result["objects"][0].as_str().unwrap().to_owned();
    match created_curve(&env, &copy) {
        Curve2::Ray(r) => {
            assert!(
                (r.origin().x - 0.0).abs() < 1e-9 && (r.origin().y - 2.0).abs() < 1e-9,
                "{r:?}"
            );
            assert!((r.direction().x - 1.0).abs() < 1e-9 && r.direction().y.abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_construction_line_offsets_parallel_to_itself() {
    let mut env = Env::new();
    setup(&mut env);
    let line = env.run(
        "draftbuilder.draw.xline",
        json!({"lines": [{"kind": "through", "point": [0.0, 0.0], "through": [1.0, 0.0]}]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [line], "distance": 3.0, "side": [5.0, 1.0]}),
    );
    let copy = result["objects"][0].as_str().unwrap().to_owned();
    match created_curve(&env, &copy) {
        Curve2::Line(l) => assert!((l.point().y - 3.0).abs() < 1e-9, "{l:?}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_side_point_on_the_entity_is_refused_instead_of_choosing_a_side() {
    let mut env = Env::new();
    setup(&mut env);
    let line = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.offset",
            json!({"objects": [line], "distance": 1.0, "side": [5.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.pick_invalid", "{error:?}");
    assert_eq!(
        env.document
            .snapshot()
            .iter_type(arc_draft::types::LINE)
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn a_sharp_polyline_corner_is_beveled_beyond_the_miter_limit() {
    let mut env = Env::new();
    setup(&mut env);
    // The corner at (10, 0) turns by about 174 degrees: its miter point lies about 20 offsets from the corner,
    // beyond the limit of four, so the outer side is beveled.
    let source = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [0.0, 1.0]}]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.modify.offset",
        json!({"objects": [source], "distance": 1.0, "side": [5.0, -1.0]}),
    );
    let copy = result["objects"][0].as_str().unwrap().to_owned();
    match created_curve(&env, &copy) {
        Curve2::Polyline(p) => {
            // A miter would reach about x = 20 here (the corner's miter length is about 20 offsets); the bevel
            // keeps every vertex within about one offset of the corner.
            let farthest = p
                .vertices()
                .iter()
                .map(|v| v.point.x)
                .fold(f64::MIN, f64::max);
            assert!(
                farthest < 12.0,
                "the corner is beveled, not mitered: {:?}",
                p.vertices()
            );
        }
        other => panic!("{other:?}"),
    }
}
