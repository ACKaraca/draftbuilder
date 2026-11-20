//! CHANGE (`draftbuilder.modify.change`, identifier only: the typed name CHANGE belongs to the property change
//! command): moves the endpoint of a line, or sets the radius of a circle, to a change point. For a line, the
//! endpoint nearer the pick point on the line is moved to the change point. For a circle, the radius becomes the
//! distance from the center to the change point. The change is in place and the identifier stays. Other kinds,
//! including text and block references, are refused with `db.cmd.geometry_unsupported`.

use arc_base::math::Point2;
use arc_base::{Error, Result, Tolerance};
use arc_command::{Accept, CommandSpec, Context, IdParam, Prompt, PromptInput, PromptState, Step};
use arc_draft::types::{CIRCLE, LINE};
use arc_geom::{Circle2, Curve2, Segment2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, point_answer, refuse_locked, replace_geometry};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.change`. A resulting line or radius at or below the linear tolerance is refused
/// with `db.cmd.value_invalid`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeParams {
    /// The line or circle to change.
    pub object: IdParam,
    /// A point on the line, in drawing units, that picks the endpoint to move.
    pub pick: [f64; 2],
    /// The change point, in drawing units: the new endpoint of a line, or a point on the new radius of a circle.
    pub point: [f64; 2],
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-change-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-change-description",
};
const OBJECT: Message = Message {
    message_id: "db-cmd-modify-change-object",
};
const PICK: Message = Message {
    message_id: "db-cmd-modify-change-pick",
};
const POINT: Message = Message {
    message_id: "db-cmd-modify-change-point",
};

fn run(ctx: &mut Context<'_>, p: ChangeParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &[p.object])?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    let id = ids[0];
    if !p.pick.iter().chain(&p.point).all(|v| v.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "point"));
    }
    let pick = Point2::new(p.pick[0], p.pick[1]);
    let to = Point2::new(p.point[0], p.point[1]);
    let new_curve = match super::required_curve(&snapshot, id)? {
        Curve2::Segment(s) => {
            let near_end = (s.end() - pick).norm() <= (s.start() - pick).norm();
            // The line keeps its other end: the new length must be above the linear tolerance.
            let other = if near_end { s.start() } else { s.end() };
            if (to - other).norm() <= Tolerance::DEFAULT.linear {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "point"));
            }
            Curve2::Segment(if near_end {
                Segment2::new(s.start(), to)?
            } else {
                Segment2::new(to, s.end())?
            })
        }
        Curve2::Circle(c) => {
            let radius = (to - c.center()).norm();
            if radius <= Tolerance::DEFAULT.linear {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "point"));
            }
            Curve2::Circle(Circle2::new(c.center(), radius)?)
        }
        _ => return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string())),
    };
    replace_geometry(ctx, id, &new_curve)?;
    ctx.set_selection(vec![id])?;
    Ok(Changed {
        objects: vec![IdParam(id)],
    })
}

fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(PromptInput::Pick(object)) = state.answer("object") else {
        return Ok(Step::Ask(Prompt::new(
            "object",
            OBJECT.message_id,
            Accept::Pick {
                types: vec![LINE.to_owned(), CIRCLE.to_owned()],
            },
        )));
    };
    let object = object.to_string();
    let Some(pick) = point_answer(state, "pick") else {
        return Ok(Step::Ask(Prompt::new(
            "pick",
            PICK.message_id,
            super::point(),
        )));
    };
    let Some(point) = point_answer(state, "point") else {
        return Ok(Step::Ask(Prompt::new(
            "point",
            POINT.message_id,
            super::point(),
        )));
    };
    done(serde_json::json!({"object": object, "pick": pick, "point": point}))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.change", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, crate::draw::create::accept_all)),
    ]
}
