//! ELLIPSE (`draftbuilder.draw.ellipse`, typed `ELLIPSE`): an ellipse by the two endpoints of one
//! axis and the length of the other semi-axis; by a center, one axis end and the other semi-axis;
//! or by a center, an axis end and a rotation of a circle about that axis (the minor semi-axis is
//! the radius times the cosine of the rotation, in [0, π/2)). With a start and an end parameter
//! it is an elliptical arc, counterclockwise from the start to the end. The parameters are the
//! eccentric angles of the ellipse. Isometric circles are not built.

use arc_base::Error;
use arc_base::Result;
use arc_base::math::Point2;
use arc_command::{
    Accept, CommandSpec, Context, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, EllipticalArc2};
use schemars::JsonSchema;
use serde::Deserialize;
use std::f64::consts::TAU;

use super::create::{
    Created, Creator, OPTION_ARC, done, from_meters, keyword_of, machine, option, point_of,
};
use super::geometry::{ellipse_about, ellipse_on_axis, ellipse_rotated};
use crate::error::{Message, codes};

/// How an ellipse is placed.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum EllipseShape {
    /// Two endpoints of one axis and the semi-length of the other axis.
    Axis {
        /// One endpoint of the first axis, in drawing units.
        first: [f64; 2],
        /// The other endpoint of the first axis, in drawing units.
        second: [f64; 2],
        /// The semi-length of the other axis, in drawing units, above zero.
        minor: f64,
    },
    /// A center, an endpoint of one axis and the semi-length of the other axis.
    Center {
        /// The center, in drawing units.
        center: [f64; 2],
        /// An endpoint of an axis, in drawing units.
        end: [f64; 2],
        /// The semi-length of the other axis, in drawing units, above zero.
        minor: f64,
    },
    /// A center, an endpoint of an axis and a rotation about that axis.
    Rotation {
        /// The center, in drawing units.
        center: [f64; 2],
        /// An endpoint of the major axis, in drawing units.
        end: [f64; 2],
        /// The rotation in radians, from 0 up to but not including π/2.
        rotation: f64,
    },
}

/// Parameters of `draftbuilder.draw.ellipse`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EllipseParams {
    /// The placement of the ellipse.
    pub shape: EllipseShape,
    /// The start parameter in radians of an elliptical arc; with `end_angle`.
    #[serde(default)]
    pub start_angle: Option<f64>,
    /// The end parameter in radians of an elliptical arc; with `start_angle`.
    #[serde(default)]
    pub end_angle: Option<f64>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-ellipse-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-ellipse-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-ellipse-first",
};
const SECOND: Message = Message {
    message_id: "db-cmd-draw-ellipse-second",
};
const CENTER: Message = Message {
    message_id: "db-cmd-draw-ellipse-center",
};
const END: Message = Message {
    message_id: "db-cmd-draw-ellipse-end",
};
const MINOR: Message = Message {
    message_id: "db-cmd-draw-ellipse-minor",
};
const ROTATION: Message = Message {
    message_id: "db-cmd-draw-ellipse-rotation",
};
const ARC: Message = Message {
    message_id: "db-cmd-draw-ellipse-arc",
};
const ARC_START: Message = Message {
    message_id: "db-cmd-draw-ellipse-arc-start",
};
const ARC_END: Message = Message {
    message_id: "db-cmd-draw-ellipse-arc-end",
};
const OPTION_CENTER: Message = Message {
    message_id: "db-cmd-option-ellipse-center",
};
const OPTION_ROTATION: Message = Message {
    message_id: "db-cmd-option-ellipse-rotation",
};
const OPTION_FULL: Message = Message {
    message_id: "db-cmd-option-full",
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

fn distance(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Distance(meters)) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

fn angle(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Angle(radians)) => Some(*radians),
        _ => None,
    }
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let Some(first) = state.answer("first") else {
        let center = option("center", OPTION_CENTER, Some('c'));
        return Ok(Step::Ask(
            Prompt::new("first", FIRST.message_id, point_accept()).or(Accept::keywords([center])),
        ));
    };
    let center_mode = keyword_of(first) == Some("center");
    if center_mode {
        return center_next(state);
    }
    let Some(first_point) = point(state, "first") else {
        return ask("first", FIRST, point_accept());
    };
    let Some(second) = point(state, "second") else {
        return ask("second", SECOND, point_accept());
    };
    let Some(minor) = distance(state, "minor") else {
        return ask("minor", MINOR, Accept::distance());
    };
    let shape =
        serde_json::json!({"mode": "axis", "first": first_point, "second": second, "minor": minor});
    arc_question(state, shape)
}

fn center_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(center) = point(state, "center") else {
        return ask("center", CENTER, point_accept());
    };
    let Some(end) = point(state, "end") else {
        return ask("end", END, point_accept());
    };
    let Some(minor) = state.answer("minor") else {
        let rotation = option("rotation", OPTION_ROTATION, Some('r'));
        return Ok(Step::Ask(
            Prompt::new("minor", MINOR.message_id, Accept::distance())
                .or(Accept::keywords([rotation])),
        ));
    };
    if keyword_of(minor) == Some("rotation") {
        let Some(rotation) = angle(state, "rotation") else {
            return ask("rotation", ROTATION, Accept::angle());
        };
        let shape = serde_json::json!({"mode": "rotation", "center": center, "end": end, "rotation": rotation});
        return arc_question(state, shape);
    }
    let minor = distance(state, "minor").unwrap_or(0.0);
    let shape = serde_json::json!({"mode": "center", "center": center, "end": end, "minor": minor});
    arc_question(state, shape)
}

/// Asks whether the ellipse is a full ellipse or an arc, then the arc parameters.
fn arc_question(state: &PromptState<'_>, shape: serde_json::Value) -> Result<Step> {
    let Some(choice) = state.answer("arc") else {
        let arc_option = option("arc", OPTION_ARC, Some('a'));
        let full = option("full", OPTION_FULL, Some('f'));
        return Ok(Step::Ask(
            Prompt::new("arc", ARC.message_id, Accept::keywords([arc_option, full]))
                .with_default(PromptInput::Keyword("full".to_owned())),
        ));
    };
    if keyword_of(choice) == Some("full") {
        return done(serde_json::json!({"shape": shape}));
    }
    let Some(start) = angle(state, "arc_start") else {
        return ask("arc_start", ARC_START, Accept::angle());
    };
    let Some(end) = angle(state, "arc_end") else {
        return ask("arc_end", ARC_END, Accept::angle());
    };
    done(serde_json::json!({"shape": shape, "start_angle": start, "end_angle": end}))
}

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if let PromptInput::Distance(meters) = input
        && *meters <= 0.0
    {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "minor"));
    }
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

fn run(ctx: &mut Context<'_>, p: EllipseParams) -> Result<Created> {
    let q = |[x, y]: [f64; 2]| Point2::new(x, y);
    let ellipse = match p.shape {
        EllipseShape::Axis {
            first,
            second,
            minor,
        } => ellipse_on_axis(q(first), q(second), minor)?,
        EllipseShape::Center { center, end, minor } => ellipse_about(q(center), q(end), minor)?,
        EllipseShape::Rotation {
            center,
            end,
            rotation,
        } => ellipse_rotated(q(center), q(end), rotation)?,
    };
    let curve = match (p.start_angle, p.end_angle) {
        (None, None) => Curve2::Ellipse(ellipse),
        (Some(start), Some(end)) => {
            let sweep = (end - start).rem_euclid(TAU);
            if sweep <= 0.0 {
                return Err(Error::new(codes::NO_SOLUTION));
            }
            Curve2::EllipticalArc(EllipticalArc2::new(ellipse, start, sweep)?)
        }
        _ => return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "end_angle")),
    };
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.curve(ctx, &curve)?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.ellipse", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
