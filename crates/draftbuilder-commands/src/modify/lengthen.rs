//! LENGTHEN (`draftbuilder.modify.lengthen`, typed `LENGTHEN`, alias `LEN`): changes the length of a
//! segment or an arc by a delta, by a percentage of its length, or to a total length, moving the end
//! nearest the pick point. An arc keeps its center and radius and changes its sweep. An open polyline is
//! lengthened at its end segment, at the end nearest the pick. Delta changes that segment by the value; percent
//! and total refer to the whole polyline's length, and the change of that length is applied to the end segment,
//! so the change is refused when it would shorten the segment past its start. The entity keeps its identifier.
//! Other kinds, and closed polylines, are refused with
//! `db.cmd.geometry_unsupported`. The angle form, splines, ellipses and the dynamic drag are not built.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{CommandSpec, Context, IdParam};
use arc_geom::{Arc2, Curve2, Polyline2, PolylineSegment, Segment2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, refuse_locked, replace_geometry};
use crate::error::{Message, codes};

/// How the new length is given.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LengthenMode {
    /// The length changes by the value (negative shortens).
    Delta,
    /// The length changes by the value percent of its length.
    Percent,
    /// The length becomes the value.
    Total,
}

/// Parameters of `draftbuilder.modify.lengthen`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LengthenParams {
    /// The segment or arc to change.
    pub object: IdParam,
    /// A point near the end to move, in drawing units.
    pub pick: [f64; 2],
    /// How `value` is read.
    pub mode: LengthenMode,
    /// The delta, the percentage or the total length, in drawing units for the first and the third.
    pub value: f64,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-lengthen-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-lengthen-description",
};

fn run(ctx: &mut Context<'_>, p: LengthenParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &[p.object])?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    let id = ids[0];
    if !p.pick.iter().all(|v| v.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "pick"));
    }
    let curve = super::required_curve(&snapshot, id)?;
    let pick = Point2::new(p.pick[0], p.pick[1]);
    let (Some(start), Some(end)) = (curve.start_point(), curve.end_point()) else {
        return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()));
    };
    let near_end = (end - pick).norm() <= (start - pick).norm();
    let new_curve = match curve {
        Curve2::Polyline(poly) => Curve2::Polyline(lengthen_polyline(&poly, near_end, &p, id)?),
        other => lengthen_curve(&other, near_end, p.mode, p.value, id)?,
    };
    replace_geometry(ctx, id, &new_curve)?;
    ctx.set_selection(vec![id])?;
    Ok(Changed {
        objects: vec![IdParam(id)],
    })
}

/// The segment or arc with its end at `near_end` changed to the length the command asks for; the other end
/// stays. An arc keeps its center and radius. A full turn or more is refused, since an arc cannot be one.
fn lengthen_curve(
    curve: &Curve2,
    near_end: bool,
    mode: LengthenMode,
    value: f64,
    id: ObjectId,
) -> Result<Curve2> {
    let unsupported = || Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string());
    Ok(match curve {
        Curve2::Segment(s) => {
            let length = (s.end() - s.start()).norm();
            let wanted = new_length(length, mode, value)?;
            let unit = (s.end() - s.start()) / length;
            if near_end {
                Curve2::Segment(Segment2::new(s.start(), s.start() + unit * wanted)?)
            } else {
                Curve2::Segment(Segment2::new(s.end() - unit * wanted, s.end())?)
            }
        }
        Curve2::Arc(a) => {
            let length = a.radius() * a.sweep().abs();
            let wanted = new_length(length, mode, value)?;
            let sweep = a.sweep().signum() * wanted / a.radius();
            if sweep.abs() >= std::f64::consts::TAU - Tolerance::DEFAULT.angular {
                // A sweep of a full turn or more is a circle, which an arc cannot be.
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "value"));
            }
            let end_angle = a.start_angle() + a.sweep();
            if near_end {
                Curve2::Arc(Arc2::new(a.center(), a.radius(), a.start_angle(), sweep)?)
            } else {
                Curve2::Arc(Arc2::new(a.center(), a.radius(), end_angle - sweep, sweep)?)
            }
        }
        _ => return Err(unsupported()),
    })
}

/// An open polyline with the end segment at `near_end` lengthened. The measured length is that segment's, the
/// end the command lengthens is the end nearest the pick: its vertex moves and the bulge of its segment follows
/// the new segment. The widths and the other vertices stay.
fn lengthen_polyline(
    poly: &Polyline2,
    near_end: bool,
    p: &LengthenParams,
    id: ObjectId,
) -> Result<Polyline2> {
    let unsupported = || Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string());
    let count = poly.segment_count();
    if poly.is_closed() || count == 0 {
        return Err(unsupported());
    }
    let index = if near_end { count - 1 } else { 0 };
    let segment = match poly.segment(index).ok_or_else(unsupported)? {
        PolylineSegment::Line(s) => Curve2::Segment(s),
        PolylineSegment::Arc(a) => Curve2::Arc(a),
    };
    // Delta changes the end segment by the value. Percent and Total refer to the whole polyline: the change of its
    // total length, taken from the total measured now, is applied to the end segment.
    let total: f64 = poly.segments().map(|seg| segment_length(&seg)).sum();
    let (mode, value) = match p.mode {
        LengthenMode::Delta => (LengthenMode::Delta, p.value),
        LengthenMode::Percent => (LengthenMode::Delta, total * p.value / 100.0),
        LengthenMode::Total => (LengthenMode::Delta, p.value - total),
    };
    let new = lengthen_curve(&segment, near_end, mode, value, id)?;
    let bulge = match new {
        Curve2::Arc(a) => (a.sweep() / 4.0).tan(),
        _ => 0.0,
    };
    let (Some(new_start), Some(new_end)) = (new.start_point(), new.end_point()) else {
        return Err(unsupported());
    };
    let mut vertices = poly.vertices().to_vec();
    if near_end {
        // The segment at `index` starts at vertex `index` and ends at the last vertex.
        if let Some(vertex) = vertices.get_mut(index) {
            vertex.bulge = bulge;
        }
        if let Some(last) = vertices.last_mut() {
            last.point = new_end;
        }
    } else if let Some(first) = vertices.first_mut() {
        first.point = new_start;
        first.bulge = bulge;
    }
    Polyline2::new(vertices, false)
}

/// The length of one segment of a polyline: the chord of a straight one, the arc length of a curved one.
fn segment_length(segment: &PolylineSegment) -> f64 {
    match segment {
        PolylineSegment::Line(s) => (s.end() - s.start()).norm(),
        PolylineSegment::Arc(a) => a.radius() * a.sweep().abs(),
    }
}

/// The length the command asks for, from the current length.
fn new_length(length: f64, mode: LengthenMode, value: f64) -> Result<f64> {
    let wanted = match mode {
        LengthenMode::Delta => length + value,
        LengthenMode::Percent => length * (1.0 + value / 100.0),
        LengthenMode::Total => value,
    };
    if !(wanted.is_finite() && wanted > Tolerance::DEFAULT.linear) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "value"));
    }
    Ok(wanted)
}

const OBJECT: Message = Message {
    message_id: "db-cmd-modify-lengthen-object",
};
const PICK: Message = Message {
    message_id: "db-cmd-modify-lengthen-pick",
};
const MODE: Message = Message {
    message_id: "db-cmd-modify-lengthen-mode",
};
const VALUE: Message = Message {
    message_id: "db-cmd-modify-lengthen-value",
};
const OPTION_DELTA: Message = Message {
    message_id: "db-cmd-modify-option-delta",
};
const OPTION_PERCENT: Message = Message {
    message_id: "db-cmd-modify-option-percent",
};
const OPTION_TOTAL: Message = Message {
    message_id: "db-cmd-modify-option-total",
};

/// The prompt machine: the entity, the point near the end, the mode, then the value (a distance, or a
/// number for a percentage).
fn next(state: &arc_command::PromptState<'_>) -> arc_base::Result<arc_command::Step> {
    use arc_command::{Accept, KeywordOption, Prompt, PromptInput, Step};
    let Some(object) = state.answer("object") else {
        return Ok(Step::Ask(Prompt::new(
            "object",
            OBJECT.message_id,
            Accept::Pick {
                types: vec![
                    arc_draft::types::LINE.to_owned(),
                    arc_draft::types::ARC.to_owned(),
                    arc_draft::types::POLYLINE.to_owned(),
                ],
            },
        )));
    };
    let PromptInput::Pick(id) = object else {
        return Err(Error::new(codes::PICK_INVALID));
    };
    let id = id.to_string();
    let Some(pick) = super::point_answer(state, "pick") else {
        return Ok(Step::Ask(Prompt::new(
            "pick",
            PICK.message_id,
            super::point(),
        )));
    };
    let mode = match state.answer("mode") {
        Some(PromptInput::Keyword(k)) => match k.as_str() {
            "percent" => LengthenMode::Percent,
            "total" => LengthenMode::Total,
            _ => LengthenMode::Delta,
        },
        _ => {
            return Ok(Step::Ask(
                Prompt::new(
                    "mode",
                    MODE.message_id,
                    Accept::keywords([
                        KeywordOption::new("delta", OPTION_DELTA.message_id),
                        KeywordOption::new("percent", OPTION_PERCENT.message_id),
                        KeywordOption::new("total", OPTION_TOTAL.message_id),
                    ]),
                )
                .with_default(PromptInput::Keyword("delta".to_owned())),
            ));
        }
    };
    let value = match mode {
        LengthenMode::Percent => super::number_answer(state, "value"),
        LengthenMode::Delta | LengthenMode::Total => super::distance_answer(state, "value"),
    };
    let Some(value) = value else {
        let accept = match mode {
            LengthenMode::Percent => Accept::number(arc_base::Dimension::NONE),
            LengthenMode::Delta | LengthenMode::Total => Accept::distance(),
        };
        return Ok(Step::Ask(Prompt::new("value", VALUE.message_id, accept)));
    };
    let mode_text = match mode {
        LengthenMode::Delta => "delta",
        LengthenMode::Percent => "percent",
        LengthenMode::Total => "total",
    };
    crate::draw::create::done(serde_json::json!({
        "object": id, "pick": pick, "mode": mode_text, "value": value
    }))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.lengthen", LABEL, DESCRIPTION, run).with_prompt(
            crate::draw::create::machine(next, crate::draw::create::accept_all),
        ),
    ]
}
