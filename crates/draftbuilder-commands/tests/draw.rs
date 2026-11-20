//! The drawing commands, headless: the typed form of each command with its exact geometry, the
//! prompt form driven through the command line as a typed script, undo of one command, the replay
//! of a recorded script to an identical document, and the refusals.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "test code"
)]

mod common;

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI};

use arc_base::{Limits, Unit};
use arc_command::{CommandLine, Submitted, Target};
use arc_doc::Value;
use arc_draft::types::ENTITIES;
use arc_draft_dxf::fields::Reader;
use arc_geom::encoding::GeomCodec;
use arc_geom::{Curve2, Polyline2};
use common::Env;
use serde_json::{Value as Json, json};

/// A metric-free drawing in meters, and the context units of typed numbers: meters and degrees.
fn setup(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *Unit::by_name("meter").unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
}

/// Every curve of the drawing's entities, with the type key it is stored under.
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

/// The number of entities of one type key.
fn count(env: &Env, key: &str) -> usize {
    env.document
        .snapshot()
        .iter_type(key)
        .map(Iterator::count)
        .unwrap_or(0)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn polyline_of(curve: &Curve2) -> &Polyline2 {
    match curve {
        Curve2::Polyline(polyline) => polyline,
        other => panic!("not a polyline: {other:?}"),
    }
}

/// A command line over the environment, with the seed of each submitted line.
struct Line {
    line: CommandLine,
    seed: u64,
}

impl Line {
    fn new() -> Self {
        Self {
            line: CommandLine::new(),
            seed: 100,
        }
    }

    fn submit(&mut self, env: &Env, text: &str) -> Submitted {
        self.seed += 1;
        let localized = BTreeMap::new();
        let target = Target {
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

fn ran(submitted: Submitted) -> Json {
    match submitted {
        Submitted::Ran(outcome) => outcome.value,
        other => panic!("the command did not run: {other:?}"),
    }
}

#[test]
fn line_typed_makes_one_segment_per_pair_and_undoes_in_one_step() {
    let mut env = Env::new();
    setup(&mut env);
    let result = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]], "close": true}),
    );
    assert_eq!(result["ids"].as_array().unwrap().len(), 3);
    assert_eq!(count(&env, "core.draft.line"), 3);
    let segments: Vec<_> = curves(&env)
        .into_iter()
        .map(|(_, c)| match c {
            Curve2::Segment(s) => (s.start(), s.end()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(segments[1].0, arc_base::math::Point2::new(10.0, 0.0));
    assert_eq!(segments[2].1, arc_base::math::Point2::new(0.0, 0.0));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count(&env, "core.draft.line"), 0);
}

#[test]
fn line_prompt_form_takes_relative_polar_pointer_direction_and_undo() {
    let mut env = Env::new();
    setup(&mut env);
    let mut line = Line::new();
    assert!(matches!(line.submit(&env, "line"), Submitted::Prompt(_)));
    line.submit(&env, "0,0");
    // Relative polar: 5 along the x axis from the last point.
    line.submit(&env, "@5<0");
    line.submit(&env, "10,0");
    line.submit(&env, "undo");
    line.submit(&env, "5,5");
    // Direct distance along the pointer direction set by the host.
    line.line.set_pointer_direction(Some([-1.0, 0.0])).unwrap();
    line.submit(&env, "5");
    let result = ran(line.submit(&env, "close"));
    assert_eq!(result["ids"].as_array().unwrap().len(), 4);
    let points: Vec<[f64; 2]> = curves(&env)
        .iter()
        .map(|(_, c)| match c {
            Curve2::Segment(s) => [s.start().x, s.start().y],
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(points, vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0], [0.0, 5.0]]);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count(&env, "core.draft.line"), 0);
}

#[test]
fn point_typed_and_prompt_forms_place_points() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.point",
        json!({"points": [[1.0, 2.0], [3.0, 4.0]]}),
    );
    assert_eq!(count(&env, "core.draft.point"), 2);
    let mut line = Line::new();
    line.submit(&env, "point");
    line.submit(&env, "0,0");
    ran(line.submit(&env, "end"));
    assert_eq!(count(&env, "core.draft.point"), 3);
}

#[test]
fn xline_and_ray_typed_forms_create_infinite_and_semi_infinite_lines() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.xline",
        json!({"lines": [
            {"kind": "through", "point": [0.0, 0.0], "through": [1.0, 1.0]},
            {"kind": "horizontal", "point": [0.0, 2.0]},
            {"kind": "vertical", "point": [3.0, 0.0]},
            {"kind": "angle", "point": [0.0, 0.0], "angle": FRAC_PI_2}
        ]}),
    );
    assert_eq!(count(&env, "core.draft.construction_line"), 4);
    env.run(
        "draftbuilder.draw.ray",
        json!({"start": [0.0, 0.0], "through": [[2.0, 0.0], [0.0, 3.0]]}),
    );
    assert_eq!(count(&env, "core.draft.ray"), 2);
}

#[test]
fn polyline_prompt_form_draws_an_arc_by_second_point_and_typed_bulges_agree() {
    let mut env = Env::new();
    setup(&mut env);
    let mut line = Line::new();
    line.submit(&env, "pline");
    line.submit(&env, "0,0");
    line.submit(&env, "10,0");
    line.submit(&env, "arc");
    line.submit(&env, "second");
    line.submit(&env, "5,5");
    line.submit(&env, "0,0");
    ran(line.submit(&env, "end"));
    let curves = curves(&env);
    assert_eq!(curves.len(), 1);
    let polyline = polyline_of(&curves[0].1);
    assert_eq!(polyline.vertices().len(), 3);
    // The circle through (10,0), (5,5) and (0,0) is centered at (5,0): a half circle, bulge 1.
    assert!(close(polyline.vertices()[1].bulge, 1.0));
    assert!(close(polyline.vertices()[0].bulge, 0.0));
    // The typed form of the same polyline, with the bulge written out.
    let typed = json!({"vertices": [
        {"point": [0.0, 0.0]}, {"point": [10.0, 0.0], "bulge": 1.0}, {"point": [0.0, 0.0]}
    ]});
    env.run("draftbuilder.draw.polyline", typed);
    assert_eq!(count(&env, "core.draft.polyline"), 2);
}

#[test]
fn polyline_widths_and_undo_of_the_last_vertex() {
    let mut env = Env::new();
    setup(&mut env);
    let mut line = Line::new();
    line.submit(&env, "pline");
    line.submit(&env, "0,0");
    line.submit(&env, "width");
    line.submit(&env, "0.5");
    line.submit(&env, "0.25");
    line.submit(&env, "10,0");
    line.submit(&env, "undo");
    line.submit(&env, "3,4");
    ran(line.submit(&env, "end"));
    let curves = curves(&env);
    let polyline = polyline_of(&curves[0].1);
    assert_eq!(polyline.vertices().len(), 2);
    assert!(close(polyline.vertices()[0].start_width, 0.5));
    assert!(close(polyline.vertices()[0].end_width, 0.25));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count(&env, "core.draft.polyline"), 0);
}

#[test]
fn rectangle_corners_rotation_and_dimensions_with_the_side_quadrant() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.rectangle",
        json!({"shape": {"mode": "corners", "first": [0.0, 0.0], "other": [4.0, 2.0]}}),
    );
    let mut line = Line::new();
    line.submit(&env, "rectang");
    line.submit(&env, "0,0");
    line.submit(&env, "dimensions");
    line.submit(&env, "4");
    line.submit(&env, "2");
    ran(line.submit(&env, "-1,-1"));
    let curves = curves(&env);
    assert_eq!(curves.len(), 2);
    let corners = |c: &Curve2| {
        polyline_of(c)
            .vertices()
            .iter()
            .map(|v| (v.point.x, v.point.y))
            .collect::<Vec<_>>()
    };
    assert_eq!(corners(&curves[0].1)[2], (4.0, 2.0));
    assert_eq!(corners(&curves[1].1)[2], (-4.0, -2.0));
}

#[test]
fn circle_typed_and_three_point_prompt() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "three_points", "first": [1.0, 0.0], "second": [0.0, 1.0], "third": [-1.0, 0.0]}}),
    );
    let mut line = Line::new();
    line.submit(&env, "circle");
    line.submit(&env, "three");
    line.submit(&env, "0,2");
    line.submit(&env, "2,0");
    ran(line.submit(&env, "0,-2"));
    let circles: Vec<_> = curves(&env)
        .into_iter()
        .map(|(_, c)| match c {
            Curve2::Circle(circle) => (circle.center().x, circle.center().y, circle.radius()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(circles.len(), 2);
    // (1,0), (0,1), (-1,0) and (0,2), (2,0), (0,-2) are both centered at the origin.
    let mut radii: Vec<f64> = circles.iter().map(|c| c.2).collect();
    radii.sort_by(f64::total_cmp);
    assert!(close(radii[0], 1.0) && close(radii[1], 2.0));
    assert!(circles.iter().all(|c| close(c.0, 0.0) && close(c.1, 0.0)));
}

#[test]
fn circle_by_center_and_radius_prompt_and_diameter_option() {
    let mut env = Env::new();
    setup(&mut env);
    let mut line = Line::new();
    line.submit(&env, "circle");
    line.submit(&env, "1,1");
    line.submit(&env, "diameter");
    ran(line.submit(&env, "4"));
    match &curves(&env)[0].1 {
        Curve2::Circle(circle) => {
            assert!(close(circle.radius(), 2.0));
            assert!(close(circle.center().x, 1.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn circle_tangent_to_two_lines_takes_the_solution_nearest_the_picks() {
    let mut env = Env::new();
    setup(&mut env);
    let x_axis = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let y_axis = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [0.0, 10.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "tangent_radius",
            "first": {"object": x_axis, "pick": [1.0, 0.1]},
            "second": {"object": y_axis, "pick": [0.1, 1.0]},
            "radius": 1.0}}),
    );
    let last = curves(&env).pop().unwrap().1;
    match last {
        Curve2::Circle(circle) => {
            assert!(close(circle.center().x, 1.0) && close(circle.center().y, 1.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn arc_typed_methods_have_exact_geometry() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_end", "start": [1.0, 0.0], "center": [0.0, 0.0], "end": [0.0, 1.0]}}),
    );
    match &curves(&env)[0].1 {
        Curve2::Arc(arc) => {
            assert!(close(arc.radius(), 1.0));
            assert!(close(arc.sweep(), FRAC_PI_2));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn arc_prompt_form_by_center_and_angle() {
    let mut env = Env::new();
    setup(&mut env);
    let mut line = Line::new();
    line.submit(&env, "arc");
    line.submit(&env, "1,0");
    line.submit(&env, "center");
    line.submit(&env, "0,0");
    line.submit(&env, "angle");
    ran(line.submit(&env, "90"));
    match &curves(&env)[0].1 {
        Curve2::Arc(arc) => assert!(close(arc.sweep(), FRAC_PI_2)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn polygon_square_prompt_and_typed_edge() {
    let mut env = Env::new();
    setup(&mut env);
    let mut line = Line::new();
    line.submit(&env, "polygon");
    line.submit(&env, "4");
    line.submit(&env, "0,0");
    line.submit(&env, "inscribed");
    ran(line.submit(&env, "1,0"));
    let polygon = polyline_of(&curves(&env)[0].1).clone();
    assert_eq!(polygon.vertices().len(), 4);
    assert!(close(polygon.vertices()[1].point.y, 1.0));
    env.run(
        "draftbuilder.draw.polygon",
        json!({"sides": 3, "shape": {"mode": "edge", "first": [0.0, 0.0], "second": [1.0, 0.0]}}),
    );
    assert_eq!(count(&env, "core.draft.polyline"), 2);
}

#[test]
fn donut_is_a_closed_wide_polyline_and_a_zero_inner_diameter_is_a_disc() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.donut",
        json!({"inner_diameter": 0.0, "outer_diameter": 2.0, "centers": [[0.0, 0.0]]}),
    );
    let curves = curves(&env);
    let polyline = polyline_of(&curves[0].1);
    assert!(polyline.is_closed());
    assert!(close(polyline.vertices()[0].start_width, 1.0));
    assert!(close(polyline.vertices()[0].point.x, 0.5));
}

#[test]
fn ellipse_axis_and_arc_and_spline_fit_points() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.ellipse",
        json!({"shape": {"mode": "axis", "first": [-2.0, 0.0], "second": [2.0, 0.0], "minor": 1.0}}),
    );
    match &curves(&env)[0].1 {
        Curve2::Ellipse(ellipse) => assert!(close(ellipse.ratio(), 0.5)),
        other => panic!("{other:?}"),
    }
    env.run(
        "draftbuilder.draw.ellipse",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "end": [2.0, 0.0], "minor": 1.0},
               "start_angle": 0.0, "end_angle": FRAC_PI_2}),
    );
    assert_eq!(count(&env, "core.draft.elliptical_arc"), 1);
    env.run(
        "draftbuilder.draw.spline",
        json!({"points": [[0.0, 0.0], [1.0, 2.0], [2.0, 0.0], [3.0, 2.0], [4.0, 0.0]]}),
    );
    assert_eq!(count(&env, "core.draft.spline"), 1);
}

#[test]
fn rotated_ellipse_uses_the_cosine_of_the_rotation() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.ellipse",
        json!({"shape": {"mode": "rotation", "center": [0.0, 0.0], "end": [2.0, 0.0], "rotation": PI / 3.0}}),
    );
    match &curves(&env)[0].1 {
        Curve2::Ellipse(ellipse) => assert!(close(ellipse.ratio(), 0.5)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn refusals_change_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    // Coincident line points.
    assert!(
        env.invoke(
            "draftbuilder.draw.line",
            json!({"points": [[1.0, 1.0], [1.0, 1.0]]})
        )
        .is_err()
    );
    // Collinear three points for a circle.
    assert!(
        env.invoke(
            "draftbuilder.draw.circle",
            json!({"shape": {"mode": "three_points", "first": [0.0, 0.0], "second": [1.0, 1.0], "third": [2.0, 2.0]}}),
        )
        .is_err()
    );
    // Fewer than three sides.
    assert!(
        env.invoke(
            "draftbuilder.draw.polygon",
            json!({"sides": 2, "shape": {"mode": "edge", "first": [0.0, 0.0], "second": [1.0, 0.0]}}),
        )
        .is_err()
    );
    // A tangent to an arc is not a supported object.
    let arc = env
        .run(
            "draftbuilder.draw.arc",
            json!({"shape": {"method": "three_points", "start": [1.0, 0.0], "second": [0.0, 1.0], "end": [-1.0, 0.0]}}),
        )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = env.invoke(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "tangent_radius",
            "first": {"object": arc, "pick": [0.0, 0.0]},
            "second": {"object": arc, "pick": [0.0, 0.0]},
            "radius": 1.0}}),
    );
    assert!(refused.is_err());
    assert_eq!(count(&env, "core.draft.line"), 0);
    assert_eq!(count(&env, "core.draft.circle"), 0);
    // The first point prompt takes no option: an option there is refused.
    let mut line = Line::new();
    line.submit(&env, "line");
    let rejected = line.submit(&env, "undo");
    assert!(matches!(rejected, Submitted::Rejected { .. }));
}

#[test]
fn a_recorded_script_replays_to_an_identical_document() {
    let script = |env: &Env| {
        let mut line = Line::new();
        line.submit(env, "pline");
        line.submit(env, "0,0");
        line.submit(env, "@4<0");
        line.submit(env, "second");
        line.submit(env, "5,1");
        line.submit(env, "4,2");
        line.submit(env, "end");
        line.submit(env, "line");
        line.submit(env, "0,0");
        line.submit(env, "1,1");
        line.submit(env, "end");
    };
    let mut first = Env::new();
    setup(&mut first);
    script(&first);
    let mut second = Env::new();
    setup(&mut second);
    script(&second);
    assert!(!curves(&first).is_empty());
    assert_eq!(
        format!("{:?}", curves(&first)),
        format!("{:?}", curves(&second))
    );
}
