//! Guards of the geometry commands, headless: STRETCH refuses a locked entity that its window moves or
//! crosses (it does not leave the entity behind and tear the drawing), an arc whose stretched form would
//! turn the other way is refused rather than stored reversed, and LENGTHEN refuses an arc that would become
//! a full turn.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_base::{Limits, Unit};
use arc_doc::Value;
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

fn curve_of(env: &Env, id: &str) -> Curve2 {
    let wanted: arc_base::ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    let object = snapshot.get(handle).unwrap();
    let key = snapshot.type_key(handle).unwrap().as_str().to_owned();
    match Reader::new(snapshot.registry(), &key, &object.values).get("geometry") {
        Some(Value::Geometry(bytes)) => Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap(),
        _ => panic!("no geometry"),
    }
}

fn quarter_arc(env: &mut Env) -> String {
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_end", "start": [5.0, 0.0], "center": [0.0, 0.0], "end": [0.0, 5.0]}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn stretch_refuses_a_locked_line_inside_its_window_and_moves_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let locked = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let free = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 3.0], [10.0, 3.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [locked.clone()]}),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.stretch",
            json!({"first": [-1.0, -1.0], "second": [2.0, 1.0], "displacement": [0.0, 5.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "draft.layer.locked", "{error:?}");
    assert!(matches!(curve_of(&env, &locked), Curve2::Segment(s) if s.start().y == 0.0));
    assert!(matches!(curve_of(&env, &free), Curve2::Segment(s) if s.start().y == 3.0));
}

#[test]
fn stretch_refuses_a_locked_line_the_window_crosses_and_moves_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let locked = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [locked.clone()]}),
    );
    let error = env
        .invoke(
            "draftbuilder.modify.stretch",
            json!({"first": [-1.0, -1.0], "second": [1.0, 1.0], "displacement": [0.0, 5.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "draft.layer.locked", "{error:?}");
    assert!(matches!(curve_of(&env, &locked), Curve2::Segment(s) if s.end().y == 0.0));
}

#[test]
fn stretch_refuses_an_arc_whose_stretched_form_would_turn_the_other_way() {
    let mut env = Env::new();
    setup(&mut env);
    let arc = quarter_arc(&mut env);
    // The window holds the end (0, 5); moving it by (6, 1) puts it past the start's side, which turns the
    // arc through the moved ends the other way.
    let error = env
        .invoke(
            "draftbuilder.modify.stretch",
            json!({"first": [-1.0, 4.0], "second": [1.0, 6.0], "displacement": [6.0, 1.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.stretch_unsupported", "{error:?}");
    match curve_of(&env, &arc) {
        Curve2::Arc(a) => assert!(
            a.sweep() > 0.0,
            "the arc keeps its direction: {}",
            a.sweep()
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn lengthen_refuses_an_arc_that_would_become_a_full_turn() {
    let mut env = Env::new();
    setup(&mut env);
    let arc = quarter_arc(&mut env);
    let error = env
        .invoke(
            "draftbuilder.modify.lengthen",
            json!({"object": arc.clone(), "pick": [0.0, 5.0], "mode": "total", "value": 10.0 * std::f64::consts::PI}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid", "{error:?}");
    match curve_of(&env, &arc) {
        Curve2::Arc(a) => assert!((a.sweep() - std::f64::consts::FRAC_PI_2).abs() < 1e-9),
        other => panic!("{other:?}"),
    }
}

#[test]
fn break_at_an_end_point_of_an_open_line_is_refused_and_the_line_is_unchanged() {
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
            "draftbuilder.modify.break",
            json!({"object": line.clone(), "first": [0.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.pick_invalid", "{error:?}");
    assert!(
        matches!(curve_of(&env, &line), Curve2::Segment(s) if s.start().x == 0.0 && s.end().x == 10.0)
    );
}

#[test]
fn break_between_two_points_closer_than_the_tolerance_is_refused() {
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
            "draftbuilder.modify.break",
            json!({"object": line.clone(), "first": [3.0, 0.0], "second": [3.0 + 1e-12, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.value_invalid", "{error:?}");
    assert_eq!(
        env.document
            .snapshot()
            .iter_type(arc_draft::types::LINE)
            .unwrap()
            .count(),
        1
    );
}

fn ends_of(env: &Env, id: &str) -> ([f64; 2], [f64; 2]) {
    match curve_of(env, id) {
        Curve2::Segment(s) => ([s.start().x, s.start().y], [s.end().x, s.end().y]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn trim_with_several_targets_skips_the_one_that_cannot_change_and_reports_it() {
    let mut env = Env::new();
    setup(&mut env);
    let cut = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[5.0, -1.0], [5.0, 1.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let cut_target = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let far = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 20.0], [10.0, 20.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.modify.trim",
        json!({"edges": [cut.clone()], "targets": [
            {"object": far.clone(), "pick": [5.0, 20.0]},
            {"object": cut_target.clone(), "pick": [8.0, 0.0]}]}),
    );
    let skipped = result["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{result}");
    assert_eq!(skipped[0]["object"], json!(far.clone()));
    assert_eq!(skipped[0]["reason"], json!("geom.edit.no_intersection"));
    assert_eq!(ends_of(&env, &cut_target), ([0.0, 0.0], [5.0, 0.0]));
    assert_eq!(
        ends_of(&env, &far),
        ([0.0, 20.0], [10.0, 20.0]),
        "the skipped line is untouched"
    );
}

#[test]
fn trim_fails_with_the_first_error_when_no_target_changes_and_changes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let cut = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[5.0, -1.0], [5.0, 1.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let far = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 20.0], [10.0, 20.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let error = env
        .invoke(
            "draftbuilder.modify.trim",
            json!({"edges": [cut], "targets": [{"object": far.clone(), "pick": [5.0, 20.0]}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "geom.edit.no_intersection");
    assert_eq!(ends_of(&env, &far), ([0.0, 20.0], [10.0, 20.0]));
}

#[test]
fn trim_of_two_targets_by_one_edge_does_not_depend_on_their_order() {
    let mut env = Env::new();
    setup(&mut env);
    let cut = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[5.0, -1.0], [5.0, 11.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let a = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let b = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 4.0], [10.0, 4.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let targets = |first: &str, second: &str| {
        json!({"edges": [cut.clone()], "targets": [
            {"object": first, "pick": [8.0, if first == a.as_str() { 0.0 } else { 4.0 }]},
            {"object": second, "pick": [8.0, if second == a.as_str() { 0.0 } else { 4.0 }]}]})
    };
    env.run("draftbuilder.modify.trim", targets(&a, &b));
    let forward = (ends_of(&env, &a), ends_of(&env, &b));
    env.document.undo().wait().unwrap();
    env.run("draftbuilder.modify.trim", targets(&b, &a));
    let backward = (ends_of(&env, &a), ends_of(&env, &b));
    assert_eq!(forward, backward);
    assert_eq!(forward.0, ([0.0, 0.0], [5.0, 0.0]));
}

#[test]
fn copy_refuses_more_copies_than_the_array_limit_before_making_any() {
    let mut env = Env::new();
    setup(&mut env);
    let line = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let other = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 2.0], [1.0, 2.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.session.limits.array_length = 3;
    let error = env
        .invoke(
            "draftbuilder.modify.copy",
            json!({"objects": [line, other], "base": [0.0, 0.0], "targets": [[5.0, 0.0], [9.0, 0.0]]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.limit_exceeded", "{error:?}");
    assert_eq!(
        env.document
            .snapshot()
            .iter_type(arc_draft::types::LINE)
            .unwrap()
            .count(),
        2
    );
}

/// The stored values of an entity, by identifier.
fn values_of(env: &Env, id: &str) -> Vec<Option<Value>> {
    let wanted: arc_base::ObjectId = id.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(wanted).unwrap();
    snapshot.get(handle).unwrap().values.clone()
}

#[test]
fn an_exploded_component_with_a_by_block_color_takes_the_references_layer_color() {
    use arc_draft::types::entity;
    let mut env = Env::new();
    setup(&mut env);
    let red = env.run(
        "draftbuilder.layer.create",
        json!({"name": "Red", "color": 1}),
    )["layers"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.create",
        json!({"name": "Walls", "color": 5}),
    );
    let component = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [component.clone()], "layer": "Walls"}),
    );
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [component.clone()], "field": "color_by", "value": "block"}),
    );
    let block = env.run(
        "core.draft.block.define",
        json!({"name": "PART", "objects": [component]}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let space = env.space().to_string();
    let reference = env.run(
        "core.draft.block.insert",
        json!({"owner": space, "block": block, "position": [10.0, 0.0], "layer": red}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exploded = env.run(
        "draftbuilder.modify.explode",
        json!({"objects": [reference]}),
    );
    let piece = exploded["objects"][0].as_str().unwrap().to_owned();
    let values = values_of(&env, &piece);
    let red_color = values_of(&env, &red)[arc_draft::types::layer::COLOR].clone();
    assert_eq!(
        values[entity::COLOR_BY],
        Some(Value::Enum(2)),
        "the component takes an explicit color"
    );
    assert_eq!(
        values[entity::COLOR],
        red_color,
        "the reference's layer color, red"
    );
    assert_ne!(red_color, None, "the red layer has a color");
}
