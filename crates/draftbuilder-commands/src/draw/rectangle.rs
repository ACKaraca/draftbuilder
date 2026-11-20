//! RECTANG (`draftbuilder.draw.rectangle`, typed `RECTANG`): a closed polyline rectangle placed by
//! two opposite corners, by a rotation and a corner, by its length and width, or by its area and
//! length. The side point of the size forms picks the quadrant the rectangle extends into. The
//! options are a constant width of the outline, a fillet radius or a chamfer distance of the corners
//! (the fillet and the chamfer are exclusive), an elevation and a thickness. The prompt form offers
//! them as keywords on the first-corner prompt; each takes its value, then the prompt comes back.

use arc_base::Result;
use arc_base::math::Point2;
use arc_base::{Dimension, Error, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::{GeometryBytes, Value};
use arc_draft::types::{self, entity};
use arc_geom::encoding::GeomCodec;
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
    /// The chamfer distance of the corners in drawing units, zero for none; exclusive with `fillet`.
    #[serde(default)]
    pub chamfer: f64,
    /// The elevation of the rectangle in drawing units along its extrusion direction.
    #[serde(default)]
    pub elevation: f64,
    /// The extrusion thickness of the rectangle in drawing units.
    #[serde(default)]
    pub thickness: f64,
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
const OPTION_FILLET: Message = Message {
    message_id: "db-cmd-draw-rectangle-option-fillet",
};
const OPTION_CHAMFER: Message = Message {
    message_id: "db-cmd-draw-rectangle-option-chamfer",
};
const OPTION_WIDTH_KEYWORD: Message = Message {
    message_id: "db-cmd-draw-rectangle-option-width",
};
const OPTION_ELEVATION: Message = Message {
    message_id: "db-cmd-draw-rectangle-option-elevation",
};
const OPTION_THICKNESS: Message = Message {
    message_id: "db-cmd-draw-rectangle-option-thickness",
};
const VALUE_FILLET: Message = Message {
    message_id: "db-cmd-draw-rectangle-value-fillet",
};
const VALUE_CHAMFER: Message = Message {
    message_id: "db-cmd-draw-rectangle-value-chamfer",
};
const VALUE_WIDTH: Message = Message {
    message_id: "db-cmd-draw-rectangle-value-width",
};
const VALUE_ELEVATION: Message = Message {
    message_id: "db-cmd-draw-rectangle-value-elevation",
};
const VALUE_THICKNESS: Message = Message {
    message_id: "db-cmd-draw-rectangle-value-thickness",
};

/// The options of the prompt form, by keyword: the keyword, its message and its value message.
const VALUE_OPTIONS: [(&str, Message, Message); 5] = [
    ("fillet", OPTION_FILLET, VALUE_FILLET),
    ("chamfer", OPTION_CHAMFER, VALUE_CHAMFER),
    ("width", OPTION_WIDTH_KEYWORD, VALUE_WIDTH),
    ("elevation", OPTION_ELEVATION, VALUE_ELEVATION),
    ("thickness", OPTION_THICKNESS, VALUE_THICKNESS),
];

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
    if let Some(step) = option_value_step(state) {
        return Ok(step);
    }
    let Some(first) = point(state, "first") else {
        return Ok(first_prompt());
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
        Some(_) => finish(
            state,
            serde_json::json!({"mode": "corners", "first": first, "other": point(state, "other")}),
        ),
    }
}

fn rotation_next(state: &PromptState<'_>, first: [f64; 2]) -> Result<Step> {
    let Some(angle_value) = angle(state, "angle") else {
        return ask("angle", ANGLE, Accept::angle());
    };
    let Some(other) = point(state, "rotated") else {
        return ask("rotated", ROTATED, point_accept());
    };
    finish(
        state,
        serde_json::json!({"mode": "rotation", "first": first, "angle": angle_value, "other": other}),
    )
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
    finish(
        state,
        serde_json::json!({"mode": "dimensions", "first": first, "length": length, "width": width, "side": side}),
    )
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
    finish(
        state,
        serde_json::json!({"mode": "area", "first": first, "area": total, "length": length, "side": side}),
    )
}

/// The value prompt of an option keyword answered on the first-corner prompt, while its value is
/// not given after the keyword.
fn option_value_step(state: &PromptState<'_>) -> Option<Step> {
    let Some(PromptInput::Keyword(id)) = state.answer("first") else {
        return None;
    };
    let (_, _, value) = VALUE_OPTIONS.iter().find(|(k, _, _)| *k == id.as_str())?;
    let key = format!("value_{id}");
    let position = |k: &str| state.answers.iter().rposition(|a| a.key == k);
    let answered = matches!((position(&key), position("first")), (Some(v), Some(f)) if v > f);
    (!answered).then(|| Step::Ask(Prompt::new(key, value.message_id, Accept::distance())))
}

/// The first-corner prompt: its option keywords, which set the outline options before the corners.
fn first_prompt() -> Step {
    let keywords: Vec<KeywordOption> = VALUE_OPTIONS
        .iter()
        .map(|(id, label, _)| {
            option(
                id,
                Message {
                    message_id: label.message_id,
                },
                None,
            )
        })
        .collect();
    Step::Ask(Prompt::new("first", FIRST.message_id, point_accept()).or(Accept::keywords(keywords)))
}

/// The complete parameters: the shape and the options given on the first-corner prompt.
fn finish(state: &PromptState<'_>, shape: serde_json::Value) -> Result<Step> {
    let mut params = serde_json::json!({"shape": shape});
    for (keyword, _, _) in VALUE_OPTIONS {
        if let Some(PromptInput::Distance(meters)) = state.answer(&format!("value_{keyword}")) {
            params[keyword] = serde_json::json!(from_meters(state.snapshot, *meters));
        }
    }
    done(params)
}

fn check(state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
    if let PromptInput::Distance(meters) = input
        && *meters <= 0.0
        && matches!(prompt.key.as_str(), "length" | "width")
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
    if !(p.fillet.is_finite() && p.fillet >= 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "fillet"));
    }
    if !(p.chamfer.is_finite() && p.chamfer >= 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "chamfer"));
    }
    if p.fillet > 0.0 && p.chamfer > 0.0 {
        return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "chamfer"));
    }
    if !(p.elevation.is_finite() && p.thickness.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "elevation"));
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
    if p.chamfer > 0.0 {
        let cut = corner::chamfer_polyline_vertices(&polyline, p.chamfer, &Tolerance::DEFAULT)?;
        if !cut.skipped.is_empty() {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "chamfer"));
        }
        polyline = cut.polyline;
    }
    let geometry = Value::Geometry(GeometryBytes(Curve2::Polyline(polyline).encode()));
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.add(
        ctx,
        types::POLYLINE,
        vec![
            (entity::KIND, geometry),
            (entity::ELEVATION, Value::F64(p.elevation)),
            (entity::THICKNESS, Value::F64(p.thickness)),
        ],
    )?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.rectangle", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
