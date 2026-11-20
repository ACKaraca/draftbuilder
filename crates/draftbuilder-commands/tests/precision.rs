//! Precision: the resolution of a pick under the running snap set, the overrides of one pick, ortho,
//! polar and the snap step; the inquiry commands with their results in the display format; and the
//! planar user coordinate systems with their undo.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use common::Env;
use draftbuilder_commands::precision::ucs;
use serde_json::{Value as Json, json};

/// A new drawing in meters with the model space ready for entities.
fn drawing(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *arc_base::Unit::by_name("meter").unwrap(),
        *arc_base::Unit::by_name("degree").unwrap(),
    ];
}

/// Draws the segment from `a` to `b` through the line command.
fn segment(env: &mut Env, a: [f64; 2], b: [f64; 2]) -> String {
    env.run("draftbuilder.draw.line", json!({"points": [a, b]}))["ids"][0]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Sets the running object snap set of the drawing by codes.
fn osnap(env: &mut Env, modes: &str) {
    env.run("draftbuilder.precision.osnap", json!({"modes": modes}));
}

/// Resolves a pick and returns the result.
fn resolve(env: &mut Env, params: Json) -> Json {
    env.run("draftbuilder.precision.resolve", params)
}

fn code_of(result: Result<Json, arc_base::Error>) -> &'static str {
    result.unwrap_err().code()
}

#[test]
fn a_pick_snaps_to_the_running_endpoint_and_reports_the_source() {
    let mut env = Env::new();
    drawing(&mut env);
    segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    osnap(&mut env, "END");
    let pick = resolve(&mut env, json!({"point": [0.2, 0.1]}));
    assert_eq!(pick["point"], json!([0.0, 0.0]));
    assert_eq!(pick["source"], "object_snap");
    assert_eq!(pick["snap"], "END");
}

#[test]
fn an_override_replaces_the_running_set_for_one_pick_only() {
    let mut env = Env::new();
    drawing(&mut env);
    segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    osnap(&mut env, "END");
    let mid = resolve(&mut env, json!({"point": [5.2, 0.1], "over": "MID"}));
    assert_eq!(mid["point"], json!([5.0, 0.0]));
    assert_eq!(mid["snap"], "MID");
    let running = resolve(&mut env, json!({"point": [5.2, 0.1]}));
    assert_eq!(running["point"], json!([10.0, 0.0]));
    assert_eq!(running["snap"], "END");
}

#[test]
fn non_disables_object_snap_for_one_pick() {
    let mut env = Env::new();
    drawing(&mut env);
    segment(&mut env, [0.0, 0.0], [10.0, 0.0]);
    osnap(&mut env, "END");
    let pick = resolve(&mut env, json!({"point": [0.2, 0.1], "over": "NON"}));
    assert_eq!(pick["source"], "raw");
    assert_eq!(pick["point"], json!([0.2, 0.1]));
}

#[test]
fn an_unknown_override_code_is_a_typed_error() {
    let mut env = Env::new();
    drawing(&mut env);
    let error = env.invoke(
        "draftbuilder.precision.resolve",
        json!({"point": [0, 0], "over": "XYZ"}),
    );
    assert_eq!(code_of(error), "db.cmd.snap_unknown");
    let osnap_error = env.invoke("draftbuilder.precision.osnap", json!({"modes": "END,XYZ"}));
    assert_eq!(code_of(osnap_error), "db.cmd.snap_unknown");
}

#[test]
fn ortho_constrains_the_pick_to_the_axis_from_the_last_point() {
    let mut env = Env::new();
    drawing(&mut env);
    osnap(&mut env, "NONE");
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "ORTHOMODE", "value": true}),
    );
    let pick = resolve(&mut env, json!({"point": [5.0, 1.0], "from": [0.0, 0.0]}));
    assert_eq!(pick["point"], json!([5.0, 0.0]));
    assert_eq!(pick["source"], "ortho");
}

#[test]
fn polar_locks_to_the_increment_angle_within_the_tolerance() {
    let mut env = Env::new();
    drawing(&mut env);
    osnap(&mut env, "NONE");
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "POLARMODE", "value": true}),
    );
    let locked = resolve(&mut env, json!({"point": [7.0, 0.02], "from": [0.0, 0.0]}));
    assert_eq!(locked["source"], "polar");
    assert!((locked["point"][0].as_f64().unwrap() - 7.0).abs() < 1e-9);
    assert!(locked["point"][1].as_f64().unwrap().abs() < 1e-9);
    let free = resolve(&mut env, json!({"point": [7.0, 3.0], "from": [0.0, 0.0]}));
    assert_eq!(free["source"], "raw");
}

#[test]
fn the_snap_step_rounds_the_pick_to_the_grid() {
    let mut env = Env::new();
    drawing(&mut env);
    osnap(&mut env, "NONE");
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "SNAPMODE", "value": true}),
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "SNAPUNIT", "value": [1.0, 1.0]}),
    );
    let pick = resolve(&mut env, json!({"point": [1.2, 2.7]}));
    assert_eq!(pick["point"], json!([1.0, 3.0]));
    assert_eq!(pick["source"], "step");
}

#[test]
fn distance_reports_the_distance_deltas_and_angle_in_the_display_format() {
    let mut env = Env::new();
    drawing(&mut env);
    let result = env.run(
        "draftbuilder.inquiry.distance",
        json!({"from": [0.0, 0.0], "to": [3.0, 4.0]}),
    );
    assert_eq!(result["distance"], json!(5.0));
    assert_eq!(result["delta"], json!([3.0, 4.0]));
    assert_eq!(result["texts"]["distance"], "5 m");
    assert_eq!(result["texts"]["delta_x"], "3 m");
    assert_eq!(result["texts"]["delta_y"], "4 m");
    assert_eq!(result["texts"]["angle"], "53°");
}

#[test]
fn identity_reports_coordinates_in_the_current_user_coordinate_system() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run(
        "draftbuilder.precision.ucs",
        json!({"mode": "origin", "origin": [1.0, 1.0], "x_point": [2.0, 1.0]}),
    );
    let result = env.run("draftbuilder.inquiry.id", json!({"point": [3.0, 4.0]}));
    assert_eq!(result["point"], json!([2.0, 3.0]));
    assert_eq!(result["x"], "2 m");
    assert_eq!(result["y"], "3 m");
}

#[test]
fn area_of_a_polygon_with_running_totals_adds_and_subtracts() {
    let mut env = Env::new();
    drawing(&mut env);
    let square = json!([[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]);
    let single = env.run("draftbuilder.inquiry.area", json!({"points": square}));
    assert_eq!(single["area"], json!(16.0));
    assert_eq!(single["perimeter"], json!(16.0));
    assert_eq!(single["texts"]["area"], "16 m²");
    let added = env.run(
        "draftbuilder.inquiry.area",
        json!({"points": square, "running": "add", "total": 10.0}),
    );
    assert_eq!(added["total"], json!(26.0));
    let subtracted = env.run(
        "draftbuilder.inquiry.area",
        json!({"points": square, "running": "subtract", "total": 10.0}),
    );
    assert_eq!(subtracted["total"], json!(-6.0));
}

#[test]
fn area_of_a_closed_circle_and_the_refusal_of_an_open_object() {
    let mut env = Env::new();
    drawing(&mut env);
    let space = env.space().to_string();
    let circle = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "circle", "center": [0.0, 0.0], "radius": 2.0}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run("draftbuilder.inquiry.area", json!({"objects": [circle]}));
    let expected = std::f64::consts::PI * 4.0;
    assert!((result["area"].as_f64().unwrap() - expected).abs() < 1e-9);
    let open = segment(&mut env, [0.0, 0.0], [3.0, 4.0]);
    let refused = env.invoke("draftbuilder.inquiry.area", json!({"objects": [open]}));
    assert_eq!(code_of(refused), "db.cmd.not_closed");
}

#[test]
fn length_of_a_curve_and_the_properties_of_an_object() {
    let mut env = Env::new();
    drawing(&mut env);
    let line = segment(&mut env, [0.0, 0.0], [3.0, 4.0]);
    let length = env.run("draftbuilder.inquiry.length", json!({"objects": [line]}));
    assert!((length["total"].as_f64().unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(length["total_text"], "5 m");
    let listed = env.run("draftbuilder.inquiry.list", json!({"objects": [line]}));
    let item = &listed["items"][0];
    assert_eq!(item["layer"], "0");
    assert_eq!(item["color"], "layer");
    assert_eq!(item["closed"], json!(false));
    assert!((item["length"].as_f64().unwrap() - 5.0).abs() < 1e-9);
}

#[test]
fn a_coordinate_system_is_defined_saved_restored_listed_and_deleted_with_undo() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run(
        "draftbuilder.precision.ucs",
        json!({"mode": "origin", "origin": [1.0, 2.0], "x_point": [1.0, 3.0]}),
    );
    let current = ucs::current(&env.document.snapshot());
    assert_eq!(current.origin, [1.0, 2.0]);
    assert!((current.x_axis[1] - 1.0).abs() < 1e-12);
    env.run("draftbuilder.precision.ucs_save", json!({"name": "Plate"}));
    env.run("draftbuilder.precision.ucs", json!({"mode": "world"}));
    assert_eq!(ucs::current(&env.document.snapshot()).origin, [0.0, 0.0]);
    env.run(
        "draftbuilder.precision.ucs_restore",
        json!({"name": "Plate"}),
    );
    assert_eq!(ucs::current(&env.document.snapshot()).origin, [1.0, 2.0]);
    let listed = env.run("draftbuilder.precision.ucs_list", json!({}));
    assert_eq!(listed["systems"][0]["name"], "Plate");
    env.run(
        "draftbuilder.precision.ucs_delete",
        json!({"name": "Plate"}),
    );
    assert_eq!(
        env.run("draftbuilder.precision.ucs_list", json!({}))["systems"],
        json!([])
    );
    let missing = env.invoke(
        "draftbuilder.precision.ucs_restore",
        json!({"name": "Plate"}),
    );
    assert_eq!(code_of(missing), "db.cmd.ucs_unknown");
    let bad_name = env.invoke("draftbuilder.precision.ucs_save", json!({"name": "a*b"}));
    assert_eq!(code_of(bad_name), "db.cmd.ucs_name");
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(ucs::current(&env.document.snapshot()).origin, [1.0, 2.0]);
}

#[test]
fn typed_coordinates_in_the_current_system_map_to_world_coordinates() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run(
        "draftbuilder.precision.ucs",
        json!({"mode": "origin", "origin": [1.0, 1.0], "x_point": [2.0, 1.0]}),
    );
    let transform = ucs::plane_transform(&env.document.snapshot()).unwrap();
    let (x, y) = transform.apply(2.0, 3.0);
    assert!(
        (x - 3.0).abs() < 1e-12 && (y - 4.0).abs() < 1e-12,
        "{x} {y}"
    );
}

#[test]
fn the_running_snap_set_is_set_by_codes_and_undone() {
    let mut env = Env::new();
    drawing(&mut env);
    osnap(&mut env, "END,MID");
    let value = env.run("draftbuilder.settings.get", json!({"name": "OSMODE"}));
    assert_eq!(value["value"], json!(3));
    assert!(env.document.undo().wait().unwrap().value);
    let restored = env.run("draftbuilder.settings.get", json!({"name": "OSMODE"}));
    assert_eq!(restored["value"], json!(63));
}

#[test]
fn a_point_filter_takes_x_and_y_from_different_points() {
    let mut env = Env::new();
    drawing(&mut env);
    let point = env.run(
        "draftbuilder.precision.filter",
        json!({"x": [1.0, 2.0], "y": [5.0, 7.0]}),
    );
    assert_eq!(point["point"], json!([1.0, 7.0]));
}

/// A command line over the environment, for the typed and prompt forms.
struct Typed {
    line: arc_command::CommandLine,
    seed: u64,
}

impl Typed {
    fn new() -> Self {
        Self {
            line: arc_command::CommandLine::new(),
            seed: 700,
        }
    }

    fn submit(&mut self, env: &Env, text: &str) -> arc_command::Submitted {
        self.seed += 1;
        let localized = std::collections::BTreeMap::new();
        let target = arc_command::Target {
            registry: &env.commands,
            document: &env.document,
            session: &env.session,
            selection: &[],
            localized: &localized,
            seed: self.seed,
        };
        self.line.submit(&target, text).unwrap()
    }
}

/// The value of a command that ran from the line.
fn ran(submitted: arc_command::Submitted) -> Json {
    match submitted {
        arc_command::Submitted::Ran(outcome) => outcome.value,
        other => panic!("the command did not run: {other:?}"),
    }
}

#[test]
fn distance_prompt_form_asks_two_points_and_reports_the_same_result() {
    let mut env = Env::new();
    drawing(&mut env);
    let mut line = Typed::new();
    assert!(matches!(
        line.submit(&env, "dist"),
        arc_command::Submitted::Prompt(_)
    ));
    assert!(matches!(
        line.submit(&env, "0,0"),
        arc_command::Submitted::Prompt(_)
    ));
    let result = ran(line.submit(&env, "3,4"));
    assert_eq!(result["distance"], json!(5.0));
    assert_eq!(result["texts"]["distance"], "5 m");
}

#[test]
fn osnap_prompt_form_sets_the_running_set_from_codes() {
    let mut env = Env::new();
    drawing(&mut env);
    let mut line = Typed::new();
    assert!(matches!(
        line.submit(&env, "osnap"),
        arc_command::Submitted::Prompt(_)
    ));
    ran(line.submit(&env, "END,MID"));
    assert_eq!(
        env.run("draftbuilder.settings.get", json!({"name": "OSMODE"}))["value"],
        json!(3)
    );
}

#[test]
fn ucs_prompt_form_takes_a_mode_keyword_then_its_points() {
    let mut env = Env::new();
    drawing(&mut env);
    let mut line = Typed::new();
    assert!(matches!(
        line.submit(&env, "ucs"),
        arc_command::Submitted::Prompt(_)
    ));
    assert!(matches!(
        line.submit(&env, "origin"),
        arc_command::Submitted::Prompt(_)
    ));
    assert!(matches!(
        line.submit(&env, "1,1"),
        arc_command::Submitted::Prompt(_)
    ));
    ran(line.submit(&env, "2,1"));
    let current = ucs::current(&env.document.snapshot());
    assert_eq!(current.origin, [1.0, 1.0]);
    assert!((current.x_axis[0] - 1.0).abs() < 1e-12);
}

#[test]
fn id_prompt_form_reports_the_point_in_the_current_system() {
    let mut env = Env::new();
    drawing(&mut env);
    let mut line = Typed::new();
    assert!(matches!(
        line.submit(&env, "id"),
        arc_command::Submitted::Prompt(_)
    ));
    let result = ran(line.submit(&env, "1,2"));
    assert_eq!(result["point"], json!([1.0, 2.0]));
}

#[test]
fn mass_properties_of_a_closed_circle_are_its_area_and_moments() {
    let mut env = Env::new();
    drawing(&mut env);
    let space = env.space().to_string();
    let circle = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "circle", "center": [1.0, 2.0], "radius": 2.0}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run("draftbuilder.inquiry.mass", json!({"object": circle}));
    let pi = std::f64::consts::PI;
    assert!((result["area"].as_f64().unwrap() - 4.0 * pi).abs() < 1e-9);
    assert!((result["centroid"][0].as_f64().unwrap() - 1.0).abs() < 1e-9);
    assert!((result["centroid"][1].as_f64().unwrap() - 2.0).abs() < 1e-9);
    let moments = result["moments"].as_array().unwrap();
    assert!((moments[0].as_f64().unwrap() - 4.0 * pi).abs() < 1e-6);
    assert!((moments[1].as_f64().unwrap() - 4.0 * pi).abs() < 1e-6);
    assert!(moments[2].as_f64().unwrap().abs() < 1e-6);
    let open = segment(&mut env, [0.0, 0.0], [3.0, 4.0]);
    assert_eq!(
        code_of(env.invoke("draftbuilder.inquiry.mass", json!({"object": open}))),
        "db.cmd.not_closed"
    );
}
