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
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use arc_base::{Limits, Unit};
use arc_command::{CommandLine, Submitted, Target};
use arc_doc::Value;
use arc_draft::types::ENTITIES;
use arc_draft_dxf::fields::Reader;
use arc_geom::encoding::GeomCodec;
use arc_geom::{Arc2, Curve2, Polyline2};
use common::Env;
use draftbuilder_commands::error::codes;
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

/// The positions of the point entities of the drawing, in creation order.
fn point_positions(env: &Env) -> Vec<(f64, f64)> {
    let snapshot = env.document.snapshot();
    let Ok(iter) = snapshot.iter_type(arc_draft::types::POINT) else {
        return Vec::new();
    };
    iter.filter_map(|(_, object)| {
        let reader = Reader::new(snapshot.registry(), arc_draft::types::POINT, &object.values);
        match reader.get("position") {
            Some(Value::Point2(p)) => Some((p.x, p.y)),
            _ => None,
        }
    })
    .collect()
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
}

#[test]
fn divide_open_line_places_segments_minus_one_points_and_undoes() {
    let mut env = Env::new();
    setup(&mut env);
    let line_id = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let result = env.run(
        "draftbuilder.draw.divide",
        json!({"object": line_id, "segments": 4}),
    );
    assert_eq!(result["ids"].as_array().unwrap().len(), 3);
    let points = point_positions(&env);
    assert_eq!(points, vec![(2.5, 0.0), (5.0, 0.0), (7.5, 0.0)]);
    assert!(env.document.undo().wait().unwrap().value);
    assert!(point_positions(&env).is_empty());
    assert_eq!(count(&env, "core.draft.line"), 1);
}

#[test]
fn divide_closed_circle_places_one_point_per_segment_at_equal_angles() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 1.0}}),
    );
    let circle_id = env
        .document
        .snapshot()
        .iter_type(arc_draft::types::CIRCLE)
        .unwrap()
        .next()
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    env.run(
        "draftbuilder.draw.divide",
        json!({"object": circle_id, "segments": 4}),
    );
    let points = point_positions(&env);
    assert_eq!(points.len(), 4);
    for (point, angle) in points.iter().zip([0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2]) {
        assert!(
            near(*point, (angle.cos(), angle.sin())),
            "{point:?} at {angle}"
        );
    }
}

#[test]
fn divide_refuses_an_infinite_line_and_too_few_segments() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.xline",
        json!({"lines": [{"kind": "through", "point": [0.0, 0.0], "through": [1.0, 0.0]}]}),
    );
    let infinite = env
        .document
        .snapshot()
        .iter_type(arc_draft::types::CONSTRUCTION_LINE)
        .unwrap()
        .next()
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    assert!(
        env.invoke(
            "draftbuilder.draw.divide",
            json!({"object": infinite, "segments": 4})
        )
        .is_err()
    );
    let line_id = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        env.invoke(
            "draftbuilder.draw.divide",
            json!({"object": line_id.clone(), "segments": 1})
        )
        .is_err()
    );
    assert!(point_positions(&env).is_empty());
}

#[test]
fn measure_places_points_from_the_end_nearest_the_pick() {
    let mut env = Env::new();
    setup(&mut env);
    let line_id = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.draw.measure",
        json!({"object": line_id.clone(), "length": 3.0, "pick": [1.0, 0.0]}),
    );
    assert_eq!(
        point_positions(&env),
        vec![(3.0, 0.0), (6.0, 0.0), (9.0, 0.0)]
    );
    assert!(env.document.undo().wait().unwrap().value);
    env.run(
        "draftbuilder.draw.measure",
        json!({"object": line_id, "length": 3.0, "pick": [9.0, 0.0]}),
    );
    assert_eq!(
        point_positions(&env),
        vec![(7.0, 0.0), (4.0, 0.0), (1.0, 0.0)]
    );
}

#[test]
fn measure_on_a_closed_circle_starts_at_the_pick_and_runs_forward() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 1.0}}),
    );
    let circle_id = env
        .document
        .snapshot()
        .iter_type(arc_draft::types::CIRCLE)
        .unwrap()
        .next()
        .map(|(_, o)| o.id.to_string())
        .unwrap();
    // Total length 2π; the pick is at angle 0, the points lie at arc 2, 4 and 6.
    env.run(
        "draftbuilder.draw.measure",
        json!({"object": circle_id, "length": 2.0, "pick": [1.0, 0.0]}),
    );
    let points = point_positions(&env);
    assert_eq!(points.len(), 3);
    for (point, arc) in points.iter().zip([2.0_f64, 4.0, 6.0]) {
        assert!(near(*point, (arc.cos(), arc.sin())), "{point:?} at {arc}");
    }
}

#[test]
fn divide_and_measure_prompt_forms_take_the_object_and_the_values() {
    let mut env = Env::new();
    setup(&mut env);
    let line_id = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let mut script = Line::new();
    assert!(matches!(
        script.submit(&env, "divide"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, &line_id);
    ran(script.submit(&env, "5"));
    assert_eq!(
        point_positions(&env),
        vec![(2.0, 0.0), (4.0, 0.0), (6.0, 0.0), (8.0, 0.0)]
    );
    assert!(env.document.undo().wait().unwrap().value);

    let mut script = Line::new();
    assert!(matches!(
        script.submit(&env, "measure"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, &line_id);
    script.submit(&env, "4");
    ran(script.submit(&env, "0,0"));
    assert_eq!(point_positions(&env), vec![(4.0, 0.0), (8.0, 0.0)]);
}

fn wipeout_points(env: &Env) -> Vec<Curve2> {
    curves(env)
        .into_iter()
        .filter(|(key, _)| *key == arc_draft::types::WIPEOUT)
        .map(|(_, curve)| curve)
        .collect()
}

#[test]
fn wipeout_from_points_is_a_closed_mask_and_undoes() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.wipeout",
        json!({"boundary": {"mode": "points", "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 2.0], [0.0, 2.0]]}}),
    );
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 1);
    let boundary = wipeout_points(&env);
    let polyline = polyline_of(&boundary[0]);
    assert!(polyline.is_closed());
    assert_eq!(polyline.vertices().len(), 4);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 0);
}

#[test]
fn wipeout_from_a_closed_polyline_keeps_the_source_and_refuses_an_open_one() {
    let mut env = Env::new();
    setup(&mut env);
    let source = env.run(
        "draftbuilder.draw.polygon",
        json!({"sides": 6, "shape": {"mode": "inscribed", "center": [0.0, 0.0], "point": [1.0, 0.0]}}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.draw.wipeout",
        json!({"boundary": {"mode": "object", "object": source.clone()}}),
    );
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 1);
    assert_eq!(count(&env, arc_draft::types::POLYLINE), 1);
    let open = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        env.invoke(
            "draftbuilder.draw.wipeout",
            json!({"boundary": {"mode": "object", "object": open}})
        )
        .is_err()
    );
    assert!(
        env.invoke(
            "draftbuilder.draw.wipeout",
            json!({"boundary": {"mode": "points", "points": [[0.0, 0.0], [1.0, 0.0]]}}),
        )
        .is_err()
    );
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 1);
}

#[test]
fn wipeout_prompt_form_takes_points_until_enter() {
    let mut env = Env::new();
    setup(&mut env);
    let mut script = Line::new();
    assert!(matches!(
        script.submit(&env, "wipeout"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, "0,0");
    script.submit(&env, "4,0");
    script.submit(&env, "4,2");
    ran(script.submit(&env, "end"));
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 1);
    let boundary = wipeout_points(&env);
    assert_eq!(polyline_of(&boundary[0]).vertices().len(), 3);
}

/// The vertices of the polyline entities of the drawing, in order.
fn polyline_vertices(env: &Env) -> Vec<[f64; 2]> {
    let curves = curves(env);
    let polyline = curves
        .iter()
        .find_map(|(key, curve)| match (key, curve) {
            (&"core.draft.polyline", Curve2::Polyline(p)) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    polyline
        .vertices()
        .iter()
        .map(|v| [v.point.x, v.point.y])
        .collect()
}

fn has_vertex(vertices: &[[f64; 2]], wanted: [f64; 2]) -> bool {
    vertices
        .iter()
        .any(|v| near((v[0], v[1]), (wanted[0], wanted[1])))
}

#[test]
fn rectangle_chamfer_cuts_every_corner_and_elevation_and_thickness_are_stored() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.rectangle",
        json!({
            "shape": {"mode": "corners", "first": [0.0, 0.0], "other": [4.0, 2.0]},
            "chamfer": 0.5,
            "elevation": 1.5,
            "thickness": 2.5
        }),
    );
    let vertices = polyline_vertices(&env);
    assert_eq!(vertices.len(), 8);
    for wanted in [
        [0.5, 0.0],
        [3.5, 0.0],
        [4.0, 0.5],
        [4.0, 1.5],
        [3.5, 2.0],
        [0.5, 2.0],
        [0.0, 1.5],
        [0.0, 0.5],
    ] {
        assert!(has_vertex(&vertices, wanted), "missing {wanted:?}");
    }
    let snapshot = env.document.snapshot();
    let object = snapshot
        .iter_type("core.draft.polyline")
        .unwrap()
        .next()
        .map(|(_, o)| o)
        .unwrap();
    let reader = Reader::new(snapshot.registry(), "core.draft.polyline", &object.values);
    assert_eq!(reader.get("elevation"), Some(&Value::F64(1.5)));
    assert_eq!(reader.get("thickness"), Some(&Value::F64(2.5)));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count(&env, "core.draft.polyline"), 0);
}

#[test]
fn rectangle_refuses_a_chamfer_with_a_fillet_and_a_chamfer_too_large() {
    let mut env = Env::new();
    setup(&mut env);
    assert!(
        env.invoke(
            "draftbuilder.draw.rectangle",
            json!({
                "shape": {"mode": "corners", "first": [0.0, 0.0], "other": [4.0, 2.0]},
                "chamfer": 0.5,
                "fillet": 0.5
            }),
        )
        .is_err()
    );
    assert!(
        env.invoke(
            "draftbuilder.draw.rectangle",
            json!({
                "shape": {"mode": "corners", "first": [0.0, 0.0], "other": [4.0, 2.0]},
                "chamfer": 1.5
            }),
        )
        .is_err()
    );
    assert_eq!(count(&env, "core.draft.polyline"), 0);
}

#[test]
fn polyline_prompt_length_continues_along_the_last_segment() {
    let mut env = Env::new();
    setup(&mut env);
    let mut script = Line::new();
    script.submit(&env, "PLINE");
    script.submit(&env, "0,0");
    script.submit(&env, "4,0");
    assert!(matches!(
        script.submit(&env, "length"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, "3");
    ran(script.submit(&env, "end"));
    assert_eq!(
        polyline_vertices(&env),
        vec![[0.0, 0.0], [4.0, 0.0], [7.0, 0.0]]
    );
}

#[test]
fn polyline_prompt_halfwidth_sets_twice_the_half_widths() {
    let mut env = Env::new();
    setup(&mut env);
    let mut script = Line::new();
    script.submit(&env, "PLINE");
    script.submit(&env, "0,0");
    script.submit(&env, "halfwidth");
    script.submit(&env, "1");
    script.submit(&env, "0.5");
    script.submit(&env, "4,0");
    ran(script.submit(&env, "end"));
    let polyline = match curves(&env).into_iter().next().unwrap().1 {
        Curve2::Polyline(p) => p,
        other => panic!("{other:?}"),
    };
    let widths: Vec<(f64, f64)> = polyline
        .vertices()
        .iter()
        .map(|v| (v.start_width, v.end_width))
        .collect();
    assert_eq!(widths, vec![(2.0, 1.0), (0.0, 0.0)]);
}

#[test]
fn line_continue_last_starts_at_the_end_of_the_last_line_and_refuses_without_one() {
    let mut env = Env::new();
    setup(&mut env);
    assert!(
        env.invoke(
            "draftbuilder.draw.line",
            json!({"points": [[2.0, 2.0]], "continue_last": true}),
        )
        .is_err()
    );
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
    );
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[2.0, 2.0]], "continue_last": true}),
    );
    let segments: Vec<_> = curves(&env)
        .into_iter()
        .map(|(_, c)| match c {
            Curve2::Segment(s) => (s.start(), s.end()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1].0, arc_base::math::Point2::new(2.0, 0.0));
    assert_eq!(segments[1].1, arc_base::math::Point2::new(2.0, 2.0));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(count(&env, "core.draft.line"), 1);
}

#[test]
fn line_prompt_continue_takes_the_last_end_and_undo_stops_at_it() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
    );
    let mut script = Line::new();
    assert!(matches!(script.submit(&env, "LINE"), Submitted::Prompt(_)));
    script.submit(&env, "continue");
    script.submit(&env, "undo");
    script.submit(&env, "2,2");
    ran(script.submit(&env, "end"));
    assert_eq!(count(&env, "core.draft.line"), 2);
}

#[test]
fn arc_continue_is_tangent_to_the_last_line_and_ends_at_the_point() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
    );
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "continue", "end": [2.0, 2.0]}}),
    );
    // Leaves (2,0) along +x and ends at (2,2): the half circle about (2,1), counterclockwise.
    match &curves(&env)[1].1 {
        Curve2::Arc(arc) => {
            assert!(close(arc.radius(), 1.0));
            assert!(close(arc.center().x, 2.0) && close(arc.center().y, 1.0));
            assert!(close(arc.sweep(), PI));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn arc_continue_is_refused_without_a_line_or_arc_to_continue_from() {
    let mut env = Env::new();
    setup(&mut env);
    assert!(
        env.invoke(
            "draftbuilder.draw.arc",
            json!({"shape": {"method": "continue", "end": [2.0, 2.0]}}),
        )
        .is_err()
    );
    assert_eq!(count(&env, "core.draft.arc"), 0);
}

#[test]
fn arc_continue_prompt_form_and_chord_length_forms() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
    );
    let mut script = Line::new();
    assert!(matches!(script.submit(&env, "ARC"), Submitted::Prompt(_)));
    script.submit(&env, "continue");
    ran(script.submit(&env, "2,2"));
    assert_eq!(count(&env, "core.draft.arc"), 1);

    // Start (1,0) about the origin with a chord of √2: a quarter turn, counterclockwise; negative
    // is clockwise.
    let chord = std::f64::consts::SQRT_2;
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_chord", "start": [1.0, 0.0], "center": [0.0, 0.0], "chord": chord}}),
    );
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_chord", "start": [0.0, 1.0], "center": [0.0, 0.0], "chord": -chord}}),
    );
    let arcs: Vec<Arc2> = curves(&env)
        .into_iter()
        .filter_map(|(_, c)| match c {
            Curve2::Arc(a) => Some(a),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 3);
    assert!(close(arcs[1].sweep(), FRAC_PI_2));
    assert!(close(arcs[2].sweep(), -FRAC_PI_2));
    assert!(
        env.invoke(
            "draftbuilder.draw.arc",
            json!({"shape": {"method": "start_center_chord", "start": [1.0, 0.0], "center": [0.0, 0.0], "chord": 3.0}}),
        )
        .is_err()
    );
}

#[test]
fn rectangle_prompt_form_takes_the_chamfer_option_on_the_first_corner_prompt() {
    let mut env = Env::new();
    setup(&mut env);
    let mut script = Line::new();
    script.submit(&env, "RECTANG");
    assert!(matches!(
        script.submit(&env, "chamfer"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, "0.5");
    script.submit(&env, "0,0");
    ran(script.submit(&env, "4,2"));
    assert_eq!(polyline_vertices(&env).len(), 8);
}

#[test]
fn rectangle_two_corners_prompt_finishes_without_an_extra_enter() {
    let mut env = Env::new();
    setup(&mut env);
    let mut script = Line::new();
    assert!(matches!(
        script.submit(&env, "RECTANG"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, "0,0");
    ran(script.submit(&env, "4,2"));
    assert_eq!(polyline_vertices(&env).len(), 4);
    assert_eq!(count(&env, "core.draft.polyline"), 1);
}

/// The code of the refusal of an answer that the prompt rejects.
fn rejection(line: &mut Line, env: &Env, text: &str) -> String {
    match try_submit(line, env, text).unwrap() {
        Submitted::Rejected { error, .. } => error.code().to_owned(),
        other => panic!("not rejected: {other:?}"),
    }
}

/// A command line over the environment that returns refusals instead of panicking on them.
fn try_submit(line: &mut Line, env: &Env, text: &str) -> Result<Submitted, arc_base::Error> {
    line.seed += 1;
    let localized = BTreeMap::new();
    let target = Target {
        registry: &env.commands,
        document: &env.document,
        session: &env.session,
        selection: &[],
        localized: &localized,
        seed: line.seed,
    };
    line.line.submit(&target, text)
}

#[test]
fn wipeout_prompt_refuses_enter_with_too_few_corners_and_a_repeated_corner() {
    let mut env = Env::new();
    setup(&mut env);
    let mut script = Line::new();
    assert!(matches!(
        script.submit(&env, "WIPEOUT"),
        Submitted::Prompt(_)
    ));
    script.submit(&env, "0,0");
    script.submit(&env, "4,0");
    assert_eq!(
        rejection(&mut script, &env, "end"),
        codes::VALUE_INVALID.code
    );
    assert_eq!(
        rejection(&mut script, &env, "4,0"),
        codes::POINTS_COLLINEAR.code
    );
    script.submit(&env, "4,2");
    ran(script.submit(&env, "end"));
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 1);
}

#[test]
fn wipeout_refuses_a_boundary_with_no_area() {
    let mut env = Env::new();
    setup(&mut env);
    assert!(
        env.invoke(
            "draftbuilder.draw.wipeout",
            json!({"boundary": {"mode": "points", "points": [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]}}),
        )
        .is_err()
    );
    assert_eq!(count(&env, arc_draft::types::WIPEOUT), 0);
}

#[test]
fn measure_refuses_a_length_that_places_no_point_and_writes_nothing() {
    let mut env = Env::new();
    setup(&mut env);
    let line_id = env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [10.0, 0.0]]}),
    )["ids"][0]
        .as_str()
        .unwrap()
        .to_owned();
    for length in [10.0, 12.0] {
        let error = env
            .invoke(
                "draftbuilder.draw.measure",
                json!({"object": line_id.clone(), "length": length, "pick": [0.0, 0.0]}),
            )
            .unwrap_err();
        assert_eq!(error.code(), codes::VALUE_INVALID.code);
    }
    assert!(point_positions(&env).is_empty());
}

#[test]
fn chord_equal_to_the_diameter_is_a_half_turn() {
    let mut env = Env::new();
    setup(&mut env);
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_chord", "start": [1.0, 0.0], "center": [0.0, 0.0], "chord": 2.0}}),
    );
    match &curves(&env)[0].1 {
        Curve2::Arc(arc) => {
            assert!(close(arc.radius(), 1.0));
            assert!(close(arc.sweep().abs(), PI));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn line_refuses_more_points_than_the_limit_before_writing_anything() {
    let mut env = Env::new();
    setup(&mut env);
    let points: Vec<[f64; 2]> = (0..=10_000).map(|i| [f64::from(i), 0.0]).collect();
    assert!(
        env.invoke("draftbuilder.draw.line", json!({"points": points}))
            .is_err()
    );
    assert_eq!(count(&env, "core.draft.line"), 0);
}

fn undo(env: &mut Env) {
    assert!(env.document.undo().wait().unwrap().value);
}

fn last_id(env: &Env, key: &str) -> String {
    env.document
        .snapshot()
        .iter_type(key)
        .unwrap()
        .last()
        .map(|(_, o)| o.id.to_string())
        .unwrap()
}

#[test]
fn divide_and_measure_on_a_clockwise_arc_and_a_wrapped_arc_use_arc_length() {
    let mut env = Env::new();
    setup(&mut env);
    // Clockwise half turn from (1,0): quarter-length points at -45, -90 and -135 degrees.
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_angle", "start": [1.0, 0.0], "center": [0.0, 0.0], "angle": -PI}}),
    );
    let arc_id = last_id(&env, arc_draft::types::ARC);
    env.run(
        "draftbuilder.draw.divide",
        json!({"object": arc_id, "segments": 4}),
    );
    let points = point_positions(&env);
    assert_eq!(points.len(), 3);
    for (point, angle) in points
        .iter()
        .zip([-FRAC_PI_4, -FRAC_PI_2, -3.0 * FRAC_PI_4])
    {
        assert!(
            near(*point, (angle.cos(), angle.sin())),
            "{point:?} at {angle}"
        );
    }
    undo(&mut env);

    // Counterclockwise from (0,-1) across angle 0: the half-length point is (1,0).
    env.run(
        "draftbuilder.draw.arc",
        json!({"shape": {"method": "start_center_angle", "start": [0.0, -1.0], "center": [0.0, 0.0], "angle": PI}}),
    );
    let wrapped = last_id(&env, arc_draft::types::ARC);
    env.run(
        "draftbuilder.draw.measure",
        json!({"object": wrapped, "length": PI / 2.0, "pick": [0.0, -1.0]}),
    );
    let points = point_positions(&env);
    assert_eq!(points.len(), 1);
    assert!(near(points[0], (1.0, 0.0)), "{:?}", points[0]);
}

#[test]
fn divide_and_measure_on_a_polyline_with_a_bulged_segment_use_arc_length() {
    let mut env = Env::new();
    setup(&mut env);
    // A counterclockwise semicircle from (0,0) to (2,0): centre (1,0), radius 1, through (1,-1).
    env.run(
        "draftbuilder.draw.polyline",
        json!({"vertices": [{"point": [0.0, 0.0], "bulge": 1.0}, {"point": [2.0, 0.0]}]}),
    );
    let id = last_id(&env, arc_draft::types::POLYLINE);
    env.run(
        "draftbuilder.draw.divide",
        json!({"object": id.clone(), "segments": 2}),
    );
    let points = point_positions(&env);
    assert_eq!(points.len(), 1);
    assert!(near(points[0], (1.0, -1.0)), "{:?}", points[0]);
    undo(&mut env);

    // Points at quarter-length steps along the arc: 45 degrees from (0,0) is (1 - sin45, -sin45).
    env.run(
        "draftbuilder.draw.measure",
        json!({"object": id, "length": PI / 4.0, "pick": [0.0, 0.0]}),
    );
    let points = point_positions(&env);
    assert_eq!(points.len(), 3);
    let s = FRAC_PI_4.sin();
    for (point, expected) in points
        .iter()
        .zip([(1.0 - s, -s), (1.0, -1.0), (1.0 + s, -s)])
    {
        assert!(near(*point, expected), "{point:?} vs {expected:?}");
    }
}
