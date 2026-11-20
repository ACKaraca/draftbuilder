//! RECTANG (`draftbuilder.draw.rectangle`, typed `RECTANG`): a closed polyline rectangle placed by
//! two opposite corners, by a rotation and a corner, by its length and width, or by its area and
//! length. The side point of the size forms picks the quadrant the rectangle extends into. The
//! typed form takes a constant width of the outline and a fillet radius of the corners (the
//! foundation's corner fillet, refused for a rectangle with a width). Chamfers, elevation and
//! thickness are not built.

use arc_base::Result;
use arc_base::math::Point2;
use arc_base::{Dimension, Error, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, Polyline2, PolylineVertex, corner};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, done, from_meters, machine, meters_per_unit, option, point_of,
};
use super::geometry::{rectangle_corners, rectangle_rotated, rectangle_sized};
use crate::error::{Message, codes};

/// How a rectangle is placed.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum RectangleShape {
    /// Two opposite corners.
    Corners {
        /// The first corner, in drawing units.
        first: [f64; 2],
        /// The opposite corner, in drawing units.
        other: [f64; 2],
    },
    /// A corner, the angle of one side in radians, and the opposite corner in that frame.
    Rotation {
        /// The first corner, in drawing units.
        first: [f64; 2],
        /// The angle of the first side, in radians.
        angle: f64,
        /// The opposite corner, in drawing units.
        other: [f64; 2],
    },
    /// A corner, the lengths along x and y, and the quadrant of `side`.
    Dimensions {
        /// The first corner, in drawing units.
        first: [f64; 2],
        /// The length along x, in drawing units, above zero.
        length: f64,
        /// The width along y, in drawing units, above zero.
        width: f64,
        /// A point in the quadrant the rectangle extends into, in drawing units.
        side: [f64; 2],
    },
    /// A corner, the area and the length along x, and the quadrant of `side`.
    Area {
        /// The first corner, in drawing units.
        first: [f64; 2],
        /// The area in square drawing units, above zero.
        area: f64,
        /// The length along x, in drawing units, above zero.
        length: f64,
        /// A point in the quadrant the rectangle extends into, in drawing units.
        side: [f64; 2],
    },
}

/// Parameters of `draftbuilder.draw.rectangle`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RectangleParams {
    /// The placement of the rectangle.
    pub shape: RectangleShape,
    /// The width of the outline in drawing units, zero for none.
    #[serde(default)]
    pub width: f64,
    /// The fillet radius of the corners in drawing units, zero for square corners.
    #[serde(default)]
    pub fillet: f64,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-rectangle-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-rectangle-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-rectangle-first",
};
const OTHER: Message = Message {
    message_id: "db-cmd-draw-rectangle-other",
};
const ANGLE: Message = Message {
    message_id: "db-cmd-draw-rectangle-angle",
};
const ROTATED: Message = Message {
    message_id: "db-cmd-draw-rectangle-rotated",
};
const LENGTH: Message = Message {
    message_id: "db-cmd-draw-rectangle-length",
};
const WIDTH: Message = Message {
    message_id: "db-cmd-draw-rectangle-width",
};
const AREA: Message = Message {
    message_id: "db-cmd-draw-rectangle-area",
};
const SIDE: Message = Message {
    message_id: "db-cmd-draw-rectangle-side",
};
const OPTION_ROTATION: Message = Message {
    message_id: "db-cmd-option-rotation",
};
const OPTION_DIMENSIONS: Message = Message {
    message_id: "db-cmd-option-dimensions",
};
const OPTION_AREA: Message = Message {
    message_id: "db-cmd-option-area",
};

fn ask(key: &str, message: Message, accept: Accept) -> Result<Step> {
    Ok(Step::Ask(Prompt::new(key, message.message_id, accept)))
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

fn distance_accept() -> Accept {
    Accept::distance()
}

fn distance(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Distance(meters)) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

fn point(state: &PromptState<'_>, key: &str) -> Option<[f64; 2]> {
    state
        .answer(key)
        .and_then(|input| point_of(state, input))
        .map(|p| [p.x, p.y])
}

fn angle(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Angle(radians)) => Some(*radians),
        _ => None,
    }
}

/// The area answered, in square drawing units.
fn area(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Number(quantity)) => {
            let scale = meters_per_unit(state.snapshot);
            Some(quantity.value() / (scale * scale))
        }
        _ => None,
    }
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let Some(first) = point(state, "first") else {
        return ask("first", FIRST, point_accept());
    };
    match state.answer("other") {
        None => {
            let rotation = option("rotation", OPTION_ROTATION, Some('r'));
            let dimensions = option("dimensions", OPTION_DIMENSIONS, Some('d'));
            let area_option = option("area", OPTION_AREA, Some('a'));
            Ok(Step::Ask(
                Prompt::new("other", OTHER.message_id, point_accept()).or(Accept::keywords([
                    rotation,
                    dimensions,
                    area_option,
                ])),
            ))
        }
        Some(PromptInput::Keyword(id)) => match id.as_str() {
            "rotation" => rotation_next(state, first),
            "dimensions" => dimensions_next(state, first),
            _ => area_next(state, first),
        },
        Some(_) => done(serde_json::json!({
            "shape": {"mode": "corners", "first": first, "other": point(state, "other")}
        })),
    }
}

fn rotation_next(state: &PromptState<'_>, first: [f64; 2]) -> Result<Step> {
    let Some(angle_value) = angle(state, "angle") else {
        return ask("angle", ANGLE, Accept::angle());
    };
    let Some(other) = point(state, "rotated") else {
        return ask("rotated", ROTATED, point_accept());
    };
    done(serde_json::json!({
        "shape": {"mode": "rotation", "first": first, "angle": angle_value, "other": other}
    }))
}

fn dimensions_next(state: &PromptState<'_>, first: [f64; 2]) -> Result<Step> {
    let Some(length) = distance(state, "length") else {
        return ask("length", LENGTH, distance_accept());
    };
    let Some(width) = distance(state, "width") else {
        return ask("width", WIDTH, distance_accept());
    };
    let Some(side) = point(state, "side") else {
        return ask("side", SIDE, point_accept());
    };
    done(serde_json::json!({
        "shape": {"mode": "dimensions", "first": first, "length": length, "width": width, "side": side}
    }))
}

fn area_next(state: &PromptState<'_>, first: [f64; 2]) -> Result<Step> {
    let Some(total) = area(state, "area") else {
        return ask("area", AREA, Accept::number(Dimension::AREA));
    };
    let Some(length) = distance(state, "length") else {
        return ask("length", LENGTH, distance_accept());
    };
    let Some(side) = point(state, "side") else {
        return ask("side", SIDE, point_accept());
    };
    done(serde_json::json!({
        "shape": {"mode": "area", "first": first, "area": total, "length": length, "side": side}
    }))
}

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if let PromptInput::Distance(meters) = input
        && *meters <= 0.0
        && state.answer("first").is_some()
    {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "length"));
    }
    Ok(())
}

/// The corners of a placement, in order.
fn corners(shape: RectangleShape) -> Result<[Point2; 4]> {
    let p = |[x, y]: [f64; 2]| Point2::new(x, y);
    match shape {
        RectangleShape::Corners { first, other } => rectangle_corners(p(first), p(other)),
        RectangleShape::Rotation {
            first,
            angle,
            other,
        } => rectangle_rotated(p(first), angle, p(other)),
        RectangleShape::Dimensions {
            first,
            length,
            width,
            side,
        } => rectangle_sized(p(first), length, width, p(side)),
        RectangleShape::Area {
            first,
            area,
            length,
            side,
        } => {
            if !(area > 0.0 && length > 0.0) {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "area"));
            }
            rectangle_sized(p(first), length, area / length, p(side))
        }
    }
}

fn run(ctx: &mut Context<'_>, p: RectangleParams) -> Result<Created> {
    if !(p.width.is_finite() && p.width >= 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "width"));
    }
    let outline = corners(p.shape)?;
    let vertices = outline
        .iter()
        .map(|c| PolylineVertex {
            point: *c,
            bulge: 0.0,
            start_width: p.width,
            end_width: p.width,
        })
        .collect();
    let mut polyline = Polyline2::new(vertices, true)?;
    if p.fillet > 0.0 {
        polyline =
            corner::fillet_polyline_vertices(&polyline, p.fillet, &Tolerance::DEFAULT)?.polyline;
    }
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.curve(ctx, &Curve2::Polyline(polyline))?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.rectangle", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
