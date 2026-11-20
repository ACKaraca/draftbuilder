//! The selection commands, headless: each geometric method on a fixture with exact expected sets,
//! the layer and visibility rules, add and remove, previous and last across commands, filters and
//! similar selection, and undo.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use std::collections::BTreeSet;

use common::Env;
use serde_json::{Value, json};

/// The named entities of the fixture, by identifier text.
struct Fixture {
    /// Inside the window (-1,-1)-(20,20) and the triangle (0,0),(12,0),(0,12).
    a: String,
    /// Crosses the window's right edge at x = 20.
    b: String,
    /// Outside everything.
    c: String,
    /// A circle inside the window; crosses the triangle.
    d: String,
    /// A circle crossing the window's right edge.
    e: String,
    /// An arc inside the window.
    f: String,
    /// An open polyline inside the window.
    g: String,
}

fn add(env: &mut Env, geometry: Value, layer: Option<&str>) -> String {
    let space = env.space().to_string();
    let mut params = json!({"owner": space, "geometry": geometry});
    if let Some(layer) = layer {
        params["layer"] = json!(layer);
    }
    env.run("core.draft.entity.create", params)["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn fixture(env: &mut Env) -> Fixture {
    env.run("draftbuilder.drawing.new", json!({}));
    Fixture {
        a: add(
            env,
            json!({"kind": "segment", "start": [1.0, 1.0], "end": [4.0, 4.0]}),
            None,
        ),
        b: add(
            env,
            json!({"kind": "segment", "start": [15.0, 15.0], "end": [30.0, 15.0]}),
            None,
        ),
        c: add(
            env,
            json!({"kind": "segment", "start": [40.0, 40.0], "end": [50.0, 50.0]}),
            None,
        ),
        d: add(
            env,
            json!({"kind": "circle", "center": [5.0, 5.0], "radius": 2.0}),
            None,
        ),
        e: add(
            env,
            json!({"kind": "circle", "center": [20.0, 5.0], "radius": 3.0}),
            None,
        ),
        f: add(
            env,
            json!({"kind": "arc", "center": [10.0, 10.0], "radius": 1.0,
                   "start_angle": 0.0, "sweep": std::f64::consts::FRAC_PI_2}),
            None,
        ),
        g: add(
            env,
            json!({"kind": "polyline", "closed": false, "vertices": [
                {"point": [0.0, 15.0], "bulge": 0.0, "start_width": 0.0, "end_width": 0.0},
                {"point": [5.0, 18.0], "bulge": 0.0, "start_width": 0.0, "end_width": 0.0},
                {"point": [10.0, 15.0], "bulge": 0.0, "start_width": 0.0, "end_width": 0.0}
            ]}),
            None,
        ),
    }
}

/// The identifiers a selection result names, as a set.
fn names(result: &Value) -> BTreeSet<String> {
    result["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

const WINDOW_FIRST: [f64; 2] = [-1.0, -1.0];
const WINDOW_SECOND: [f64; 2] = [20.0, 20.0];

fn window(env: &mut Env) -> Value {
    env.run(
        "draftbuilder.select.window",
        json!({"first": WINDOW_FIRST, "second": WINDOW_SECOND}),
    )
}

fn crossing(env: &mut Env) -> Value {
    env.run(
        "draftbuilder.select.crossing",
        json!({"first": WINDOW_FIRST, "second": WINDOW_SECOND}),
    )
}

/// Runs a selection command with `current` as its explicit current set.
fn run_with_selection(
    env: &mut Env,
    command: &str,
    mut params: Value,
    current: &[&str],
) -> Result<Value, arc_base::Error> {
    params["current"] = json!(current);
    env.invoke(command, params)
}

#[test]
fn window_selects_only_what_lies_entirely_inside() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let result = window(&mut env);
    assert_eq!(names(&result), set(&[&f.a, &f.d, &f.f, &f.g]), "{result}");
    assert_eq!(result["types"]["line"], json!(1), "{result}");
    assert_eq!(result["types"]["circle"], json!(1), "{result}");
}

#[test]
fn crossing_selects_anything_with_a_part_inside_or_on_the_boundary() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let result = crossing(&mut env);
    assert_eq!(
        names(&result),
        set(&[&f.a, &f.b, &f.d, &f.e, &f.f, &f.g]),
        "{result}"
    );
}

#[test]
fn a_window_polygon_selects_what_lies_entirely_inside_the_polygon() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    // The triangle x + y <= 12: the circle d reaches beyond its hypotenuse.
    let triangle = json!({"points": [[0.0, 0.0], [12.0, 0.0], [0.0, 12.0]]});
    let result = env.run("draftbuilder.select.window_polygon", triangle.clone());
    assert_eq!(names(&result), set(&[&f.a]), "{result}");
    let result = env.run("draftbuilder.select.crossing_polygon", triangle);
    assert_eq!(names(&result), set(&[&f.a, &f.d]), "{result}");
}

#[test]
fn a_fence_selects_what_it_crosses() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let result = env.run(
        "draftbuilder.select.fence",
        json!({"points": [[25.0, 0.0], [25.0, 30.0]]}),
    );
    assert_eq!(names(&result), set(&[&f.b]), "{result}");
}

#[test]
fn a_fence_that_misses_everything_selects_nothing() {
    let mut env = Env::new();
    fixture(&mut env);
    let result = env.run(
        "draftbuilder.select.fence",
        json!({"points": [[-50.0, -50.0], [-40.0, -40.0]]}),
    );
    assert!(names(&result).is_empty(), "{result}");
}

#[test]
fn all_and_last_follow_the_draw_order() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let all = env.run("draftbuilder.select.all", json!({}));
    assert_eq!(names(&all).len(), 7, "{all}");
    assert!(names(&all).contains(&f.c));
    // The last created entity is the topmost one.
    let last = env.run("draftbuilder.select.last", json!({}));
    assert_eq!(names(&last), set(&[&f.g]), "{last}");
}

#[test]
fn an_invalid_polygon_or_fence_is_refused_with_its_code() {
    let mut env = Env::new();
    fixture(&mut env);
    let error = env
        .invoke(
            "draftbuilder.select.window_polygon",
            json!({"points": [[0.0, 0.0], [1.0, 1.0]]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.selection_invalid", "{error:?}");
    let error = env
        .invoke("draftbuilder.select.fence", json!({"points": [[0.0, 0.0]]}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.selection_invalid", "{error:?}");
}

#[test]
fn layers_that_are_off_or_frozen_are_never_selected_and_locked_ones_only_when_allowed() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    // Each of a, d and f moves to its own layer, which is then turned off, frozen or locked.
    for (name, object, command) in [
        ("Off", &f.a, "draftbuilder.layer.off"),
        ("Frozen", &f.d, "draftbuilder.layer.freeze"),
        ("Locked", &f.f, "draftbuilder.layer.lock"),
    ] {
        env.run("draftbuilder.layer.create", json!({"name": name}));
        env.run(
            "draftbuilder.layer.change",
            json!({"objects": [object.clone()], "layer": name}),
        );
        env.run(command, json!({"objects": [object.clone()]}));
    }

    let result = window(&mut env);
    assert_eq!(
        names(&result),
        set(&[&f.g]),
        "off and frozen hidden, locked left out: {result}"
    );

    let result = env.run(
        "draftbuilder.select.window",
        json!({"first": WINDOW_FIRST, "second": WINDOW_SECOND, "include_locked": true}),
    );
    assert_eq!(names(&result), set(&[&f.f, &f.g]), "{result}");
}

#[test]
fn an_invisible_entity_is_not_selected() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [f.g.clone()], "field": "visible", "value": false}),
    );
    let result = window(&mut env);
    assert_eq!(names(&result), set(&[&f.a, &f.d, &f.f]), "{result}");
}

#[test]
fn add_and_remove_combine_with_the_current_selection_in_order() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let first = window(&mut env);
    assert_eq!(names(&first), set(&[&f.a, &f.d, &f.f, &f.g]));

    // Add the fence (b) to the window's set.
    let added = run_with_selection(
        &mut env,
        "draftbuilder.select.fence",
        json!({"points": [[25.0, 0.0], [25.0, 30.0]], "mode": "add"}),
        &[&f.a, &f.d, &f.f, &f.g],
    )
    .unwrap();
    assert_eq!(
        names(&added),
        set(&[&f.a, &f.d, &f.f, &f.g, &f.b]),
        "{added}"
    );
    // The picked set is the fence's alone.
    assert_eq!(added["picked"], json!(1), "{added}");

    // Remove the circle d, which lies inside the window (2,2)-(8,8); the line a does not.
    let removed = run_with_selection(
        &mut env,
        "draftbuilder.select.window",
        json!({"first": [2.0, 2.0], "second": [8.0, 8.0], "mode": "remove"}),
        &[&f.a, &f.d, &f.f, &f.g, &f.b],
    )
    .unwrap();
    assert_eq!(names(&removed), set(&[&f.a, &f.f, &f.g, &f.b]), "{removed}");
}

#[test]
fn previous_returns_the_set_the_caller_passes_and_drops_unknown_identifiers() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let picked = crossing(&mut env);
    // The caller records the crossing set and passes it back: previous is explicit.
    let objects: Vec<Value> = picked["objects"].as_array().unwrap().clone();
    let previous = env.run("draftbuilder.select.previous", json!({"previous": objects}));
    assert_eq!(
        names(&previous),
        set(&[&f.a, &f.b, &f.d, &f.e, &f.f, &f.g]),
        "{previous}"
    );
    // Identifiers that name no object are dropped.
    let unknown = "00000000-0000-0000-0000-000000000001";
    let with_unknown = env.run(
        "draftbuilder.select.previous",
        json!({"previous": [f.a.clone(), unknown]}),
    );
    assert_eq!(names(&with_unknown), set(&[&f.a]), "{with_unknown}");
    // Previous combines with the current set by mode, as the other methods do.
    let added = run_with_selection(
        &mut env,
        "draftbuilder.select.previous",
        json!({"previous": [f.a.clone()], "mode": "add"}),
        &[&f.g],
    )
    .unwrap();
    assert_eq!(names(&added), set(&[&f.a, &f.g]), "{added}");
}

#[test]
fn previous_drops_an_object_that_was_erased() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let picked = window(&mut env);
    env.run(
        "core.draft.entity.delete",
        json!({"objects": [f.d.clone()]}),
    );
    let objects: Vec<Value> = picked["objects"].as_array().unwrap().clone();
    let previous = env.run("draftbuilder.select.previous", json!({"previous": objects}));
    assert_eq!(names(&previous), set(&[&f.a, &f.f, &f.g]), "{previous}");
}

#[test]
fn a_filter_selects_by_type_layer_and_wildcards_with_boolean_groups() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let circles = env.run(
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "type", "op": "equal", "value": "circle"}}}),
    );
    assert_eq!(names(&circles), set(&[&f.d, &f.e]), "{circles}");

    let wild = env.run(
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "type", "op": "wildcard", "value": "LINE*"}}}),
    );
    assert_eq!(names(&wild), set(&[&f.a, &f.b, &f.c]), "{wild}");

    // Not lines, and on layer 0.
    let not_lines = env.run(
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "type", "op": "equal", "value": "line"}},
               "invert": true}),
    );
    assert_eq!(
        names(&not_lines),
        set(&[&f.d, &f.e, &f.f, &f.g]),
        "{not_lines}"
    );

    let xor = env.run(
        "draftbuilder.select.filter",
        json!({"filter": {"xor": [
            {"test": {"property": "type", "op": "equal", "value": "circle"}},
            {"test": {"property": "layer", "op": "equal", "value": "0"}}
        ]}}),
    );
    // Every entity is on layer 0, so xor of "circle" and "on layer 0" leaves the non-circles.
    assert_eq!(names(&xor), set(&[&f.a, &f.b, &f.c, &f.f, &f.g]), "{xor}");

    let within = run_with_selection(
        &mut env,
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "type", "op": "equal", "value": "line"}},
               "within_current": true}),
        &[&f.a, &f.d],
    )
    .unwrap();
    assert_eq!(names(&within), set(&[&f.a]), "{within}");
}

#[test]
fn a_filter_with_a_wildcard_on_a_number_or_a_bad_limit_is_refused() {
    let mut env = Env::new();
    fixture(&mut env);
    let error = env
        .invoke(
            "draftbuilder.select.filter",
            json!({"filter": {"test": {"property": "lineweight", "op": "wildcard", "value": "1*"}}}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.selection_invalid", "{error:?}");
    let mut deep = json!({"test": {"property": "type", "op": "equal", "value": "line"}});
    for _ in 0..20 {
        deep = json!({"not": deep});
    }
    let error = env
        .invoke("draftbuilder.select.filter", json!({"filter": deep}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.selection_invalid", "{error:?}");
}

#[test]
fn similar_selects_objects_sharing_the_chosen_properties() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let result = env.run(
        "draftbuilder.select.similar",
        json!({"objects": [f.a.clone()], "properties": ["type"]}),
    );
    assert_eq!(names(&result), set(&[&f.a, &f.b, &f.c]), "{result}");
    let result = env.run(
        "draftbuilder.select.similar",
        json!({"objects": [f.d.clone()], "properties": ["type", "layer"]}),
    );
    assert_eq!(names(&result), set(&[&f.d, &f.e]), "{result}");
}

#[test]
fn a_selection_changes_no_document_state_and_creates_no_undo_step() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    let before = env.document.snapshot().revision();
    window(&mut env);
    crossing(&mut env);
    assert_eq!(env.document.snapshot().revision(), before);
    // The one undo step left is the creation of the last fixture line, not a selection.
    assert!(env.document.undo().wait().unwrap().value);
    let all = env.run("draftbuilder.select.all", json!({}));
    assert!(!names(&all).contains(&f.g), "{all}");
    assert_eq!(names(&all).len(), 6, "{all}");
}

#[test]
fn a_curve_touching_the_window_from_inside_is_inside_and_a_crossing_counts_its_corner() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    // A circle whose box is exactly the window (0,0)-(20,20) lies inside it, touching all four edges.
    let inside = add(
        &mut env,
        json!({"kind": "circle", "center": [10.0, 10.0], "radius": 10.0}),
        None,
    );
    // A segment from (25,25) to (15,15) meets the window only where it passes the corner (20,20).
    let corner = add(
        &mut env,
        json!({"kind": "segment", "start": [25.0, 25.0], "end": [15.0, 15.0]}),
        None,
    );
    let window = env.run(
        "draftbuilder.select.window",
        json!({"first": [0.0, 0.0], "second": [20.0, 20.0]}),
    );
    assert_eq!(names(&window), set(&[&inside]), "{window}");
    let crossing = env.run(
        "draftbuilder.select.crossing",
        json!({"first": [0.0, 0.0], "second": [20.0, 20.0]}),
    );
    assert_eq!(names(&crossing), set(&[&inside, &corner]), "{crossing}");
}

#[test]
fn turning_off_layer_zero_hides_the_entities_that_name_no_layer() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    // No fixture entity names a layer, so all of them are on layer 0, which a new drawing holds.
    env.run("draftbuilder.layer.off", json!({"objects": [f.a.clone()]}));
    let all = env.run("draftbuilder.select.all", json!({}));
    assert!(names(&all).is_empty(), "{all}");
    let window = window(&mut env);
    assert!(names(&window).is_empty(), "{window}");
}

#[test]
fn a_block_reference_is_selected_by_the_geometry_it_draws() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let space = env.space().to_string();
    let line = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [4.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let block = env.run(
        "core.draft.block.define",
        json!({"name": "UNIT", "objects": [line]}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // The definition's line is drawn at (10,10) through the reference.
    let reference = env.run(
        "core.draft.block.insert",
        json!({"owner": space, "block": block, "position": [10.0, 10.0]}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let inside = env.run(
        "draftbuilder.select.window",
        json!({"first": [8.0, 8.0], "second": [20.0, 20.0]}),
    );
    assert_eq!(names(&inside), set(&[&reference]), "{inside}");
    let away = env.run(
        "draftbuilder.select.window",
        json!({"first": [0.0, 0.0], "second": [5.0, 5.0]}),
    );
    assert!(names(&away).is_empty(), "{away}");
    let crossing = env.run(
        "draftbuilder.select.crossing",
        json!({"first": [12.0, 9.0], "second": [13.0, 11.0]}),
    );
    assert_eq!(names(&crossing), set(&[&reference]), "{crossing}");
}

#[test]
fn the_space_searched_can_be_a_block_definition_as_well_as_the_model_space() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let space = env.space().to_string();
    let inner = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [2.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let outer = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [30.0, 0.0], "end": [32.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let block = env.run(
        "core.draft.block.define",
        json!({"name": "HOLDER", "objects": [inner.clone()]}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let window_all = json!({"first": [-100.0, -100.0], "second": [100.0, 100.0]});
    // The model space default holds the outer line only; the block definition holds the inner one.
    let model = env.run("draftbuilder.select.all", json!({}));
    assert_eq!(names(&model), set(&[&outer]), "{model}");
    let inside_block = env.run(
        "draftbuilder.select.window",
        json!({"first": window_all["first"], "second": window_all["second"], "space": block}),
    );
    assert_eq!(names(&inside_block), set(&[&inner]), "{inside_block}");
    let not_a_space = env.invoke("draftbuilder.select.all", json!({"space": outer}));
    assert_eq!(not_a_space.unwrap_err().code(), "draft.object.wrong_type");
}

#[test]
fn previous_keeps_only_what_the_selection_rules_allow_in_its_order() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    // d moves to a frozen layer, g becomes invisible, and a layer identifier is offered as an object.
    let frozen = env.run("draftbuilder.layer.create", json!({"name": "Fr"}))["layers"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [f.d.clone()], "layer": "Fr"}),
    );
    env.run(
        "draftbuilder.layer.freeze",
        json!({"objects": [f.d.clone()]}),
    );
    env.run(
        "core.draft.entity.modify",
        json!({"objects": [f.g.clone()], "field": "visible", "value": false}),
    );
    let previous = env.run(
        "draftbuilder.select.previous",
        json!({"previous": [f.b.clone(), f.d.clone(), f.a.clone(), f.g.clone(), frozen, f.f.clone(), f.a.clone()]}),
    );
    // In the order given, each once: b, a, f. The frozen d, the invisible g and the layer are out.
    assert_eq!(
        previous["objects"],
        json!([f.b.clone(), f.a.clone(), f.f.clone()]),
        "{previous}"
    );
    // A locked previous object is out unless locked objects are allowed.
    env.run("draftbuilder.layer.create", json!({"name": "Lk"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [f.a.clone()], "layer": "Lk"}),
    );
    env.run("draftbuilder.layer.lock", json!({"objects": [f.a.clone()]}));
    let without = env.run(
        "draftbuilder.select.previous",
        json!({"previous": [f.a.clone(), f.f.clone()]}),
    );
    assert_eq!(names(&without), set(&[&f.f]), "{without}");
    let with = env.run(
        "draftbuilder.select.previous",
        json!({"previous": [f.a.clone(), f.f.clone()], "include_locked": true}),
    );
    assert_eq!(names(&with), set(&[&f.a, &f.f]), "{with}");
}

#[test]
fn a_zero_width_or_zero_height_window_is_refused_with_its_code() {
    let mut env = Env::new();
    fixture(&mut env);
    for (command, params) in [
        (
            "draftbuilder.select.window",
            json!({"first": [5.0, 0.0], "second": [5.0, 10.0]}),
        ),
        (
            "draftbuilder.select.crossing",
            json!({"first": [0.0, 7.0], "second": [10.0, 7.0]}),
        ),
    ] {
        let error = env.invoke(command, params).unwrap_err();
        assert_eq!(
            error.code(),
            "db.cmd.selection_invalid",
            "{command}: {error:?}"
        );
    }
}

#[test]
fn the_current_set_is_limited_and_the_limit_is_named() {
    let mut env = Env::new();
    fixture(&mut env);
    let too_many: Vec<String> = (0..10_001)
        .map(|i| format!("00000000-0000-0000-0000-{i:012x}"))
        .collect();
    let error = env
        .invoke(
            "draftbuilder.select.window",
            json!({"first": WINDOW_FIRST, "second": WINDOW_SECOND, "current": too_many}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.limit_exceeded", "{error:?}");
}

#[test]
fn a_filter_value_folds_a_multi_character_uppercase_form_like_the_names_do() {
    let mut env = Env::new();
    let f = fixture(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Straße"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [f.a.clone()], "layer": "Straße"}),
    );
    let selected = env.run(
        "draftbuilder.select.filter",
        json!({"filter": {"test": {"property": "layer", "op": "equal", "value": "STRASSE"}}}),
    );
    assert_eq!(names(&selected), set(&[&f.a]), "{selected}");
}

#[test]
fn an_entity_whose_display_list_fails_is_counted_as_skipped_not_hidden() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let space = env.space().to_string();
    let line = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [4.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let block = env.run(
        "core.draft.block.define",
        json!({"name": "FAR", "objects": [line]}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // Placed so far that its transformed geometry overflows: its display list cannot be built.
    env.run(
        "core.draft.block.insert",
        json!({"owner": space, "block": block, "position": [1e308, 1e308],
               "scale": [1e308, 1e308, 1.0]}),
    );
    let all = env.run("draftbuilder.select.all", json!({}));
    assert!(names(&all).is_empty(), "{all}");
    assert_eq!(all["skipped"], json!(1), "{all}");
    let window = env.run(
        "draftbuilder.select.window",
        json!({"first": [-1e300, -1e300], "second": [1e300, 1e300]}),
    );
    assert_eq!(window["skipped"], json!(1), "{window}");
}
