//! ARC (`draftbuilder.draw.arc`, typed `ARC`): a circular arc by three points; by start, center
//! and end (counterclockwise from start); by start, center and included angle; by start, end and
//! radius (the minor arc, counterclockwise for a positive radius); or by start, end and included
//! angle (positive counterclockwise); by start, center and chord length; or tangent to the last
//! line or arc of the model space, continuing from its end to an end point (`continue` in the prompt
//! form).

use arc_base::math::Point2;
use arc_base::{Error, Result};
use arc_command::{
    Accept, CommandSpec, Context, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::Snapshot;
use arc_geom::{Arc2, Curve2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_ANGLE, OPTION_CENTER, OPTION_RADIUS, OPTION_SECOND, done, from_meters,
    keyword_of, machine, option, point_of,
};
use super::geometry::{
    arc_center_angle, arc_center_chord, arc_center_end, arc_start_end_angle, arc_start_end_radius,
    arc_tangent_end, arc_through,
};
use super::line::last_continuation;
use crate::error::{Message, codes};

/// How an arc is placed.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArcShape {
    /// Three points: the arc from `start` through `second` to `end`.
    ThreePoints {
        /// The start point, in drawing units.
        start: [f64; 2],
        /// A point on the arc, in drawing units.
        second: [f64; 2],
        /// The end point, in drawing units.
        end: [f64; 2],
    },
    /// Start, center and end: counterclockwise from `start` to `end`.
    StartCenterEnd {
        /// The start point, in drawing units.
        start: [f64; 2],
        /// The center, in drawing units.
        center: [f64; 2],
        /// The end point, in drawing units.
        end: [f64; 2],
    },
    /// Start, center and included angle.
    StartCenterAngle {
        /// The start point, in drawing units.
        start: [f64; 2],
        /// The center, in drawing units.
        center: [f64; 2],
        /// The included angle in radians; positive counterclockwise.
        angle: f64,
    },
    /// Start, end and radius: the minor arc; counterclockwise for a positive radius.
    StartEndRadius {
        /// The start point, in drawing units.
        start: [f64; 2],
        /// The end point, in drawing units.
        end: [f64; 2],
        /// The signed radius in drawing units; its magnitude at least half the chord.
        radius: f64,
    },
    /// Start, end and included angle; positive counterclockwise.
    StartEndAngle {
        /// The start point, in drawing units.
        start: [f64; 2],
        /// The end point, in drawing units.
        end: [f64; 2],
        /// The included angle in radians, not a full turn.
        angle: f64,
    },
    /// Start, center and chord length: counterclockwise for a positive chord, clockwise for a
    /// negative one, at most the diameter in magnitude.
    StartCenterChord {
        /// The start point, in drawing units.
        start: [f64; 2],
        /// The center, in drawing units.
        center: [f64; 2],
        /// The signed chord length in drawing units.
        chord: f64,
    },
    /// Tangent to the last line or arc of the model space at its end, through `end`.
    Continue {
        /// The end point, in drawing units.
        end: [f64; 2],
    },
}

/// Parameters of `draftbuilder.draw.arc`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArcParams {
    /// The placement of the arc.
    pub shape: ArcShape,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-arc-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-arc-description",
};
const START: Message = Message {
    message_id: "db-cmd-draw-arc-start",
};
const METHOD: Message = Message {
    message_id: "db-cmd-draw-arc-method",
};
const POINT: Message = Message {
    message_id: "db-cmd-draw-arc-point",
};
const END: Message = Message {
    message_id: "db-cmd-draw-arc-end",
};
const VALUE_ANGLE: Message = Message {
    message_id: "db-cmd-draw-arc-value-angle",
};
const VALUE_RADIUS: Message = Message {
    message_id: "db-cmd-draw-arc-value-radius",
};
const START_CONTINUE: Message = Message {
    message_id: "db-cmd-draw-arc-start-continue",
};
const CONTINUE_END: Message = Message {
    message_id: "db-cmd-draw-arc-continue-end",
};
const OPTION_CONTINUE: Message = Message {
    message_id: "db-cmd-draw-arc-option-continue",
};
const OPTION_CHORD: Message = Message {
    message_id: "db-cmd-draw-arc-option-chord",
};
const VALUE_CHORD: Message = Message {
    message_id: "db-cmd-draw-arc-value-chord",
};

fn ask(key: &str, message: Message, accept: Accept) -> Result<Step> {
    Ok(Step::Ask(Prompt::new(key, message.message_id, accept)))
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
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

fn distance(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Distance(meters)) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    if let Some(PromptInput::Keyword(id)) = state.answer("start")
        && id.as_str() == "continue"
    {
        return continue_next(state);
    }
    let Some(start) = point(state, "start") else {
        return match last_continuation(state.snapshot)? {
            Some(_) => {
                let cont = option("continue", OPTION_CONTINUE, None);
                Ok(Step::Ask(
                    Prompt::new("start", START_CONTINUE.message_id, point_accept())
                        .or(Accept::keywords([cont])),
                ))
            }
            None => ask("start", START, point_accept()),
        };
    };
    let Some(method) = state.answer("method") else {
        let second = option("second", OPTION_SECOND, Some('s'));
        let center = option("center", OPTION_CENTER, Some('c'));
        let radius = option("radius", OPTION_RADIUS, Some('r'));
        let angle_option = option("angle", OPTION_ANGLE, Some('a'));
        let methods = Accept::keywords([second, center, radius, angle_option]);
        return Ok(Step::Ask(Prompt::new("method", METHOD.message_id, methods)));
    };
    match keyword_of(method) {
        Some("second") => second_next(state, start),
        Some("center") => center_next(state, start),
        Some("radius") => match point(state, "end") {
            None => ask("end", END, point_accept()),
            Some(end) => match distance(state, "value") {
                None => ask("value", VALUE_RADIUS, Accept::distance()),
                Some(radius) => done(serde_json::json!({
                    "shape": {"method": "start_end_radius", "start": start, "end": end, "radius": radius}
                })),
            },
        },
        _ => match point(state, "end") {
            None => ask("end", END, point_accept()),
            Some(end) => match angle(state, "value") {
                None => ask("value", VALUE_ANGLE, Accept::angle()),
                Some(value) => done(serde_json::json!({
                    "shape": {"method": "start_end_angle", "start": start, "end": end, "angle": value}
                })),
            },
        },
    }
}

fn continue_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(end) = point(state, "end") else {
        return ask("end", CONTINUE_END, point_accept());
    };
    done(serde_json::json!({"shape": {"method": "continue", "end": end}}))
}

fn second_next(state: &PromptState<'_>, start: [f64; 2]) -> Result<Step> {
    let Some(second) = point(state, "second") else {
        return ask("second", POINT, point_accept());
    };
    let Some(end) = point(state, "end") else {
        return ask("end", END, point_accept());
    };
    done(serde_json::json!({
        "shape": {"method": "three_points", "start": start, "second": second, "end": end}
    }))
}

fn center_next(state: &PromptState<'_>, start: [f64; 2]) -> Result<Step> {
    let Some(center) = point(state, "center") else {
        return ask("center", POINT, point_accept());
    };
    match state.answer("end") {
        None => {
            let angle_option = option("angle", OPTION_ANGLE, Some('a'));
            let chord_option = option("length", OPTION_CHORD, Some('l'));
            Ok(Step::Ask(
                Prompt::new("end", END.message_id, point_accept())
                    .or(Accept::keywords([angle_option, chord_option])),
            ))
        }
        Some(PromptInput::Keyword(id)) if id.as_str() == "length" => match distance(state, "chord")
        {
            None => ask("chord", VALUE_CHORD, Accept::distance()),
            Some(chord) => done(serde_json::json!({
                "shape": {"method": "start_center_chord", "start": start, "center": center, "chord": chord}
            })),
        },
        Some(PromptInput::Keyword(_)) => match angle(state, "value") {
            None => ask("value", VALUE_ANGLE, Accept::angle()),
            Some(value) => done(serde_json::json!({
                "shape": {"method": "start_center_angle", "start": start, "center": center, "angle": value}
            })),
        },
        Some(_) => {
            let end = point(state, "end").unwrap_or(center);
            done(serde_json::json!({
                "shape": {"method": "start_center_end", "start": start, "center": center, "end": end}
            }))
        }
    }
}

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if let Some(p) = point_of(state, input) {
        let previous = state
            .answers
            .iter()
            .rev()
            .find_map(|a| point_of(state, &a.input));
        if previous.is_some_and(|q| super::geometry::coincident(p, q)) {
            return Err(Error::new(codes::POINTS_COLLINEAR));
        }
    }
    Ok(())
}

fn arc(snapshot: &Snapshot, shape: ArcShape) -> Result<Arc2> {
    let p = |[x, y]: [f64; 2]| Point2::new(x, y);
    match shape {
        ArcShape::Continue { end } => {
            let (start, tangent) = last_continuation(snapshot)?
                .ok_or_else(|| Error::new(codes::VALUE_INVALID).with_detail("field", "shape"))?;
            arc_tangent_end(start, tangent, p(end))
        }
        ArcShape::StartCenterChord {
            start,
            center,
            chord,
        } => arc_center_chord(p(start), p(center), chord),
        ArcShape::ThreePoints { start, second, end } => arc_through(p(start), p(second), p(end)),
        ArcShape::StartCenterEnd { start, center, end } => {
            arc_center_end(p(start), p(center), p(end))
        }
        ArcShape::StartCenterAngle {
            start,
            center,
            angle,
        } => arc_center_angle(p(start), p(center), angle),
        ArcShape::StartEndRadius { start, end, radius } => {
            arc_start_end_radius(p(start), p(end), radius)
        }
        ArcShape::StartEndAngle { start, end, angle } => {
            arc_start_end_angle(p(start), p(end), angle)
        }
    }
}

fn run(ctx: &mut Context<'_>, p: ArcParams) -> Result<Created> {
    let found = arc(ctx.snapshot(), p.shape)?;
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.curve(ctx, &Curve2::Arc(found))?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.arc", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
