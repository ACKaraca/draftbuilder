//! JOIN, headless: pieces that overlap or fold back keep their exact length (the result is their
//! polyline), pieces that run the same way merge into one segment, and a single open polyline whose ends
//! touch is closed.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_base::{Limits, Unit};
use arc_doc::Value;
use arc_draft::types::{ENTITIES, LINE, POLYLINE};
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

fn line(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Every curve of the drawing, with its type key.
fn curves(env: &Env) -> Vec<(&'static str, Curve2)> {
    let snapshot = env.document.snapshot();
    let mut out = Vec::new();
    for key in ENTITIES {
        let Ok(iter) = snapshot.iter_type(key) else {
            continue;
        };
        for (_, object) in iter {
            let reader = Reader::new(snapshot.registry(), key, &object.values);
            if let Some(Value::Geometry(bytes)) = reader.get("geometry") {
                out.push((*key, Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap()));
            }
        }
    }
    out
}

fn count(env: &Env, key: &str) -> usize {
    curves(env).iter().filter(|(k, _)| *k == key).count()
}

#[test]
fn join_of_a_segment_and_one_that_folds_back_keeps_every_unit_of_length() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [2.0, 0.0]);
    let b = line(&mut env, [1.0, 0.0], [2.0, 0.0]);
    let result = env.run("draftbuilder.modify.join", json!({"objects": [a, b]}));
    assert_eq!(result["objects"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(
        count(&env, LINE),
        0,
        "no segment is left: the pieces fold, so they are one polyline"
    );
    let polylines: Vec<Vec<[f64; 2]>> = curves(&env)
        .into_iter()
        .filter_map(|(key, curve)| match (key, curve) {
            (POLYLINE, Curve2::Polyline(p)) => Some(
                p.vertices()
                    .iter()
                    .map(|v| [v.point.x, v.point.y])
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(polylines, vec![vec![[0.0, 0.0], [2.0, 0.0], [1.0, 0.0]]]);
}

#[test]
fn join_of_pieces_that_run_the_same_way_is_one_segment_spanning_them() {
    let mut env = Env::new();
    setup(&mut env);
    let a = line(&mut env, [0.0, 0.0], [5.0, 0.0]);
    let b = line(&mut env, [5.0, 0.0], [10.0, 0.0]);
    env.run(
        "draftbuilder.modify.join",
        json!({"objects": [a.clone(), b]}),
    );
    assert_eq!(count(&env, LINE), 1);
    let segment = curves(&env)
        .into_iter()
        .find_map(|(_, c)| match c {
            Curve2::Segment(s) => Some((s.start(), s.end())),
            _ => None,
        })
        .unwrap();
    assert_eq!((segment.0.x, segment.1.x), (0.0, 10.0));
}

#[test]
fn join_closes_a_single_open_polyline_whose_ends_touch() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0]}, {"point": [10.0, 0.0]}, {"point": [10.0, 10.0]},
                            {"point": [0.0, 0.0]}]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.modify.join",
        json!({"objects": [source.clone()]}),
    );
    assert_eq!(result["unjoined"].as_array().unwrap().len(), 0, "{result}");
    let closed = curves(&env)
        .into_iter()
        .find_map(|(_, c)| match c {
            Curve2::Polyline(p) => Some(p),
            _ => None,
        })
        .unwrap();
    assert!(closed.is_closed());
    assert_eq!(closed.vertices().len(), 3);
}
