//! DIVIDE (`draftbuilder.draw.divide`, typed `DIVIDE`) and MEASURE (`draftbuilder.draw.measure`,
//! typed `MEASURE`): point entities placed along a curve at equal arc length. Only points are
//! placed: a block reference is not built, because the product has no block insertion for these
//! commands yet.
//!
//! DIVIDE splits a bounded curve into `segments` equal parts. An open curve gets the `segments - 1`
//! points between the parts; a closed curve gets `segments` points, one at the start and one at
//! each division. MEASURE places a point every `length` from the end of an open curve nearest the
//! pick (the points at the multiples of the length, the end itself excluded). On a closed curve it
//! has no end, so it starts at the point nearest the pick and runs forward around the curve.
//! Infinite curves (lines, rays, construction lines) and zero-length curves are refused.

use arc_base::math::Point2;
use arc_base::{Error, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_draft::types::{ARC, CIRCLE, ELLIPSE, ELLIPTICAL_ARC, LINE, POLYLINE, SPLINE};
use arc_geom::{Curve2, CurveEval, CurveProject};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{Created, Creator, done, from_meters, machine, point_of};
use crate::error::{Message, codes};

/// The fewest segments of DIVIDE.
const MIN_SEGMENTS: u32 = 2;
/// The most segments of DIVIDE, and the most points of either command in one run.
const MAX_POINTS: u32 = 10_000;

/// Parameters of `draftbuilder.draw.divide`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DivideParams {
    /// The curve to divide.
    pub object: IdParam,
    /// The number of equal parts, from 2 to 10000.
    pub segments: u32,
}

/// Parameters of `draftbuilder.draw.measure`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeasureParams {
    /// The curve to measure.
    pub object: IdParam,
    /// The distance between points, in drawing units, above zero.
    pub length: f64,
    /// The point the measure starts from: on an open curve the end nearer to it, on a closed curve
    /// the point on the curve nearest to it. In drawing units.
    pub pick: [f64; 2],
}

const DIVIDE_LABEL: Message = Message {
    message_id: "db-cmd-draw-divide-label",
};
const DIVIDE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-divide-description",
};
const DIVIDE_OBJECT: Message = Message {
    message_id: "db-cmd-draw-divide-object",
};
const DIVIDE_SEGMENTS: Message = Message {
    message_id: "db-cmd-draw-divide-segments",
};
const MEASURE_LABEL: Message = Message {
    message_id: "db-cmd-draw-measure-label",
};
const MEASURE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-measure-description",
};
const MEASURE_OBJECT: Message = Message {
    message_id: "db-cmd-draw-measure-object",
};
const MEASURE_LENGTH: Message = Message {
    message_id: "db-cmd-draw-measure-length",
};
const MEASURE_PICK: Message = Message {
    message_id: "db-cmd-draw-measure-pick",
};

/// The curve kinds the prompt form picks.
fn curve_types() -> Vec<String> {
    [LINE, ARC, CIRCLE, ELLIPSE, ELLIPTICAL_ARC, POLYLINE, SPLINE]
        .iter()
        .map(|key| (*key).to_owned())
        .collect()
}

/// The length of a curve that has an end to measure from: finite and above the tolerance.
///
/// # Errors
///
/// `db.cmd.geometry_unsupported` (detail `reason`) for an infinite or zero-length curve, and the
/// errors of the length.
fn bounded_length(curve: &Curve2, tolerance: &Tolerance) -> Result<f64> {
    let length = curve.length(tolerance)?;
    if !length.is_finite() {
        return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("reason", "unbounded"));
    }
    if length <= tolerance.linear {
        return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("reason", "empty"));
    }
    Ok(length)
}

/// The points of DIVIDE: `segments` equal parts of the curve's arc length.
///
/// # Errors
///
/// As [`bounded_length`], and the errors of the parameter lookup.
fn divide_points(curve: &Curve2, segments: u32) -> Result<Vec<Point2>> {
    let tolerance = Tolerance::DEFAULT;
    let total = bounded_length(curve, &tolerance)?;
    let first = if curve.is_closed() { 0 } else { 1 };
    let mut points = Vec::new();
    for index in first..segments {
        let distance = total * f64::from(index) / f64::from(segments);
        let t = curve.parameter_at_length(distance, &tolerance)?;
        points.push(curve.point_at(t));
    }
    Ok(points)
}

/// The points of MEASURE: the arc lengths from the start of the curve at which points are placed,
/// in the order they are placed.
///
/// # Errors
///
/// As [`bounded_length`]; `db.cmd.limit_exceeded` when more than [`MAX_POINTS`] points would be
/// placed; `db.cmd.value_invalid` (detail `length`) for a length that is not above zero.
fn measure_positions(curve: &Curve2, step: f64, pick: Point2) -> Result<Vec<f64>> {
    let tolerance = Tolerance::DEFAULT;
    if !(step.is_finite() && step > 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "length"));
    }
    let total = bounded_length(curve, &tolerance)?;
    let closed = curve.is_closed();
    let foot = curve.closest_point(pick, &tolerance)?;
    let from_start = curve.length_between(curve.parameter_range().0, foot.parameter, &tolerance)?;
    // On an open curve the end nearer the pick is the origin; the positions run from that end.
    let from_end = !closed && from_start > total / 2.0;
    let mut positions = Vec::new();
    let mut k: u32 = 1;
    while f64::from(k) * step < total - tolerance.linear {
        if positions.len() >= MAX_POINTS as usize {
            return Err(
                Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", MAX_POINTS.to_string())
            );
        }
        let along = f64::from(k) * step;
        let position = if closed {
            (from_start + along).rem_euclid(total)
        } else if from_end {
            total - along
        } else {
            along
        };
        positions.push(position);
        k += 1;
    }
    // A length at or above the curve's length places nothing: refused, so no empty step is written.
    if positions.is_empty() {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "length"));
    }
    Ok(positions)
}

/// The number of segments of DIVIDE, from 2 to [`MAX_POINTS`].
fn valid_segments(segments: u32) -> Result<()> {
    if (MIN_SEGMENTS..=MAX_POINTS).contains(&segments) {
        Ok(())
    } else {
        Err(Error::new(codes::VALUE_INVALID).with_detail("field", "segments"))
    }
}

fn divide(ctx: &mut Context<'_>, p: DivideParams) -> Result<Created> {
    valid_segments(p.segments)?;
    let curve = crate::modify::required_curve(ctx.snapshot(), p.object.0)?;
    let points = divide_points(&curve, p.segments)?;
    let mut creator = Creator::new(ctx.snapshot())?;
    for point in points {
        creator.point(ctx, point)?;
    }
    Ok(creator.finish(ctx))
}

fn measure(ctx: &mut Context<'_>, p: MeasureParams) -> Result<Created> {
    if !(p.pick.iter().all(|v| v.is_finite())) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "pick"));
    }
    let curve = crate::modify::required_curve(ctx.snapshot(), p.object.0)?;
    let positions = measure_positions(&curve, p.length, Point2::new(p.pick[0], p.pick[1]))?;
    let tolerance = Tolerance::DEFAULT;
    let mut points = Vec::with_capacity(positions.len());
    for position in positions {
        let t = curve.parameter_at_length(position, &tolerance)?;
        points.push(curve.point_at(t));
    }
    let mut creator = Creator::new(ctx.snapshot())?;
    for point in points {
        creator.point(ctx, point)?;
    }
    Ok(creator.finish(ctx))
}

fn divide_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(object) = state.answer("object") else {
        return Ok(Step::Ask(Prompt::new(
            "object",
            DIVIDE_OBJECT.message_id,
            Accept::Pick {
                types: curve_types(),
            },
        )));
    };
    let PromptInput::Pick(id) = object else {
        return Err(Error::new(codes::PICK_INVALID));
    };
    let id = id.to_string();
    let Some(PromptInput::Number(quantity)) = state.answer("segments") else {
        return Ok(Step::Ask(Prompt::new(
            "segments",
            DIVIDE_SEGMENTS.message_id,
            Accept::Number {
                dimension: arc_base::Dimension::NONE,
                min: Some(f64::from(MIN_SEGMENTS)),
                max: Some(f64::from(MAX_POINTS)),
                integer: true,
            },
        )));
    };
    let segments = (MIN_SEGMENTS..=MAX_POINTS)
        .find(|n| f64::from(*n) == quantity.value())
        .ok_or_else(|| Error::new(codes::VALUE_INVALID).with_detail("field", "segments"))?;
    done(serde_json::json!({"object": id, "segments": segments}))
}

fn measure_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(object) = state.answer("object") else {
        return Ok(Step::Ask(Prompt::new(
            "object",
            MEASURE_OBJECT.message_id,
            Accept::Pick {
                types: curve_types(),
            },
        )));
    };
    let PromptInput::Pick(id) = object else {
        return Err(Error::new(codes::PICK_INVALID));
    };
    let id = id.to_string();
    let Some(PromptInput::Distance(meters)) = state.answer("length") else {
        return Ok(Step::Ask(Prompt::new(
            "length",
            MEASURE_LENGTH.message_id,
            Accept::distance(),
        )));
    };
    let length = from_meters(state.snapshot, *meters);
    let Some(pick) = state
        .answer("pick")
        .and_then(|input| point_of(state, input))
    else {
        return Ok(Step::Ask(Prompt::new(
            "pick",
            MEASURE_PICK.message_id,
            Accept::Point {
                dims: PointDims::Two,
            },
        )));
    };
    done(serde_json::json!({"object": id, "length": length, "pick": [pick.x, pick.y]}))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.draw.divide",
            DIVIDE_LABEL,
            DIVIDE_DESCRIPTION,
            divide,
        )
        .with_prompt(machine(divide_next, super::create::accept_all)),
        super::spec(
            "draftbuilder.draw.measure",
            MEASURE_LABEL,
            MEASURE_DESCRIPTION,
            measure,
        )
        .with_prompt(machine(measure_next, super::create::accept_all)),
    ]
}
