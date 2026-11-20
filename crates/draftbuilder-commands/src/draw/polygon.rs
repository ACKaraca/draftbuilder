//! POLYGON (`draftbuilder.draw.polygon`, typed `POLYGON`): a closed regular polyline of 3 to 1024
//! sides, inscribed in or circumscribed about a circle given by its center and a point, or with
//! one edge given by two points, counterclockwise.

use arc_base::Error;
use arc_base::Result;
use arc_base::math::Point2;
use arc_base::{Dimension, Quantity};
use arc_command::{
    Accept, CommandSpec, Context, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, Polyline2, PolylineVertex};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{Created, Creator, done, keyword_of, machine, option, point_of};
use super::geometry::{PolygonMode, polygon_about, polygon_edge};
use crate::error::{Message, codes};
use crate::settings::remember::{POLYGON_SIDES, offered, remembering};

/// The fewest sides of a polygon.
const MIN_SIDES: usize = 3;
/// The most sides of a polygon.
const MAX_SIDES: usize = 1024;

/// How a polygon is placed.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolygonShape {
    /// The vertices lie on the circle of `center` through `point`.
    Inscribed {
        /// The center, in drawing units.
        center: [f64; 2],
        /// A vertex, in drawing units.
        point: [f64; 2],
    },
    /// The middle of an edge lies on the circle of `center` through `point`.
    Circumscribed {
        /// The center, in drawing units.
        center: [f64; 2],
        /// The middle of an edge, in drawing units.
        point: [f64; 2],
    },
    /// The polygon has the edge from `first` to `second` and lies on its left.
    Edge {
        /// The first point of the edge, in drawing units.
        first: [f64; 2],
        /// The second point of the edge, in drawing units.
        second: [f64; 2],
    },
}

/// Parameters of `draftbuilder.draw.polygon`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolygonParams {
    /// The number of sides, from 3 to 1024.
    #[schemars(range(min = 3, max = 1024))]
    pub sides: usize,
    /// The placement of the polygon.
    pub shape: PolygonShape,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-polygon-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-polygon-description",
};
const SIDES: Message = Message {
    message_id: "db-cmd-draw-polygon-sides",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-polygon-first",
};
const MODE: Message = Message {
    message_id: "db-cmd-draw-polygon-mode",
};
const POINT: Message = Message {
    message_id: "db-cmd-draw-polygon-point",
};
const SECOND: Message = Message {
    message_id: "db-cmd-draw-polygon-second",
};
const EDGE_FIRST: Message = Message {
    message_id: "db-cmd-draw-polygon-edge-first",
};
const OPTION_EDGE: Message = Message {
    message_id: "db-cmd-option-edge",
};
const OPTION_INSCRIBED: Message = Message {
    message_id: "db-cmd-option-inscribed",
};
const OPTION_CIRCUMSCRIBED: Message = Message {
    message_id: "db-cmd-option-circumscribed",
};

/// The number of sides answered, when it is a whole number in range.
fn sides_of(state: &PromptState<'_>) -> Option<usize> {
    match state.answer("sides") {
        Some(PromptInput::Number(quantity)) => {
            (MIN_SIDES..=MAX_SIDES).find(|n| (*n as f64) == quantity.value())
        }
        _ => None,
    }
}

fn point_json(p: Point2) -> serde_json::Value {
    serde_json::json!([p.x, p.y])
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    if sides_of(state).is_none() {
        let prompt = Prompt::new(
            "sides",
            SIDES.message_id,
            Accept::Number {
                dimension: Dimension::NONE,
                min: Some(MIN_SIDES as f64),
                max: Some(MAX_SIDES as f64),
                integer: true,
            },
        )
        .with_default(PromptInput::Number(Quantity::new(4.0, Dimension::NONE)));
        return Ok(Step::Ask(offered(state, POLYGON_SIDES, prompt)));
    }
    let sides = sides_of(state).unwrap_or(MIN_SIDES);
    match state.answer("first") {
        None => {
            let edge = option("edge", OPTION_EDGE, Some('e'));
            let prompt = point_prompt("first", FIRST).or(Accept::keywords([edge]));
            Ok(Step::Ask(prompt))
        }
        Some(PromptInput::Keyword(_)) => edge_next(state, sides),
        Some(first) => center_next(state, sides, first),
    }
}

/// The edge form: the first edge point, then the second.
fn edge_next(state: &PromptState<'_>, sides: usize) -> Result<Step> {
    let (Some(first), Some(second)) = (state.answer("edge_first"), state.answer("second")) else {
        return Ok(Step::Ask(match state.answer("edge_first") {
            None => point_prompt("edge_first", EDGE_FIRST),
            Some(_) => point_prompt("second", SECOND),
        }));
    };
    let first = point_of(state, first).unwrap_or(Point2::origin());
    let second = point_of(state, second).unwrap_or(first);
    done(serde_json::json!({
        "sides": sides,
        "shape": {"mode": "edge", "first": point_json(first), "second": point_json(second)}
    }))
}

/// The center form: the placement keyword, then the point on the circle.
fn center_next(state: &PromptState<'_>, sides: usize, center: &PromptInput) -> Result<Step> {
    let Some(mode) = state.answer("mode") else {
        let inscribed = option("inscribed", OPTION_INSCRIBED, Some('i'));
        let circumscribed = option("circumscribed", OPTION_CIRCUMSCRIBED, Some('c'));
        let prompt = Prompt::new(
            "mode",
            MODE.message_id,
            Accept::keywords([inscribed, circumscribed]),
        )
        .with_default(PromptInput::Keyword("inscribed".to_owned()));
        return Ok(Step::Ask(prompt));
    };
    let Some(radius) = state.answer("point") else {
        return Ok(Step::Ask(point_prompt("point", POINT)));
    };
    let center = point_of(state, center).unwrap_or(Point2::origin());
    let point = point_of(state, radius).unwrap_or(center);
    let mode = keyword_of(mode).unwrap_or("inscribed");
    done(serde_json::json!({
        "sides": sides,
        "shape": {"mode": mode, "center": point_json(center), "point": point_json(point)}
    }))
}

fn point_prompt(key: &str, message: Message) -> Prompt {
    Prompt::new(
        key,
        message.message_id,
        Accept::Point {
            dims: PointDims::Two,
        },
    )
}

/// Rejects a point equal to the previous point answer.
fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    let Some(point) = point_of(state, input) else {
        return Ok(());
    };
    let previous = state
        .answers
        .iter()
        .rev()
        .find_map(|a| point_of(state, &a.input));
    if previous.is_some_and(|p| super::geometry::coincident(p, point)) {
        return Err(Error::new(codes::POINTS_COLLINEAR));
    }
    Ok(())
}

fn vertices(shape: PolygonShape, sides: usize) -> Result<Vec<Point2>> {
    let p = |[x, y]: [f64; 2]| Point2::new(x, y);
    match shape {
        PolygonShape::Inscribed { center, point } => {
            polygon_about(p(center), p(point), sides, PolygonMode::Inscribed)
        }
        PolygonShape::Circumscribed { center, point } => {
            polygon_about(p(center), p(point), sides, PolygonMode::Circumscribed)
        }
        PolygonShape::Edge { first, second } => polygon_edge(p(first), p(second), sides),
    }
}

fn run(ctx: &mut Context<'_>, p: PolygonParams) -> Result<Created> {
    let points = vertices(p.shape, p.sides)?;
    let polyline = Polyline2::new(
        points
            .into_iter()
            .map(|v| PolylineVertex::new(v, 0.0))
            .collect(),
        true,
    )?;
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.curve(ctx, &Curve2::Polyline(polyline))?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.draw.polygon",
            LABEL,
            DESCRIPTION,
            remembering(run, |p: &PolygonParams, _| {
                vec![(POLYGON_SIDES, p.sides as f64)]
            }),
        )
        .with_prompt(machine(next, check)),
    ]
}
