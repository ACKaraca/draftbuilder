//! BREAK (`draftbuilder.modify.break`, typed `BREAK`, alias `BR`): removes the part of an entity between
//! two points on it, or splits it at one point. Both points are projected onto the curve. Between two
//! points of an open entity, the part between them is removed; on a closed entity the part from the
//! first point counterclockwise (in the direction of increasing parameter) to the second is removed.
//! Splitting at one point keeps both parts.
//!
//! The first part of the entity's own kind keeps the identifier; the other parts are new entities beside
//! it, with the entity's layer and stored fields (see [`super::replace_with_pieces`]). Two points closer
//! than the tolerance are one point and are refused. A point at an end of an open entity splits nothing and
//! is refused as well: nothing is reported as broken.

use arc_base::math::Point2;
use arc_base::{Error, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, CurveProject, split_at_parameters};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, refuse_locked, replace_with_pieces};
use crate::draw::create::{accept_all, done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.break`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BreakParams {
    /// The entity to break.
    pub object: IdParam,
    /// The first point, in drawing units.
    pub first: [f64; 2],
    /// The second point, in drawing units; absent: the entity is split at the first point.
    #[serde(default)]
    pub second: Option<[f64; 2]>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-break-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-break-description",
};

/// Whether the parameter `t` of `curve` lies in the removed interval between the cuts `a` and `b`.
fn removed(closed: bool, a: f64, b: f64, t: f64) -> bool {
    if closed && a > b {
        t > a || t < b
    } else {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        t > lo && t < hi
    }
}

fn run(ctx: &mut Context<'_>, p: BreakParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &[p.object])?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    let id = ids[0];
    let curve = super::required_curve(&snapshot, id)?;
    let tolerance = Tolerance::DEFAULT;
    if !p.first.iter().all(|v| v.is_finite())
        || !p.second.unwrap_or([0.0, 0.0]).iter().all(|v| v.is_finite())
    {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "first"));
    }
    let first_foot = curve.closest_point(Point2::new(p.first[0], p.first[1]), &tolerance)?;
    // A point at an end of an open curve splits nothing: it is refused rather than reported as a break.
    let at_end = |point: Point2| -> bool {
        !curve.is_closed()
            && [curve.start_point(), curve.end_point()]
                .into_iter()
                .flatten()
                .any(|end| (end - point).norm() <= tolerance.linear)
    };
    if at_end(first_foot.point) {
        return Err(Error::new(codes::PICK_INVALID)
            .with_detail("reason", "at_end")
            .with_detail("field", "first"));
    }
    let pieces: Vec<Curve2> = match p.second {
        None => split_at_parameters(&curve, &[first_foot.parameter], &tolerance)?,
        Some(second) => {
            let other_foot = curve.closest_point(Point2::new(second[0], second[1]), &tolerance)?;
            // Two points closer than the tolerance are one point, so there is no part between them.
            if (other_foot.point - first_foot.point).norm() <= tolerance.linear {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "second"));
            }
            if at_end(other_foot.point) {
                return Err(Error::new(codes::PICK_INVALID)
                    .with_detail("reason", "at_end")
                    .with_detail("field", "second"));
            }
            let (first, other) = (first_foot.parameter, other_foot.parameter);
            let (lo, hi) = if first <= other {
                (first, other)
            } else {
                (other, first)
            };
            let closed = curve.is_closed();
            let parts = split_at_parameters(&curve, &[lo, hi], &tolerance)?;
            let mut kept = Vec::new();
            for part in parts {
                let (start, end) = part.parameter_range();
                let middle = curve
                    .closest_point(part.point_at(0.5 * (start + end)), &tolerance)?
                    .parameter;
                if !removed(closed, first, other, middle) {
                    kept.push(part);
                }
            }
            kept
        }
    };
    let objects = replace_with_pieces(ctx, id, &pieces)?;
    ctx.set_selection(objects.clone())?;
    Ok(Changed {
        objects: objects.into_iter().map(IdParam).collect(),
    })
}

const OBJECT: Message = Message {
    message_id: "db-cmd-modify-break-object",
};
const FIRST_POINT: Message = Message {
    message_id: "db-cmd-modify-break-first",
};
const SECOND_POINT: Message = Message {
    message_id: "db-cmd-modify-break-second",
};
const OPTION_AT_POINT: Message = Message {
    message_id: "db-cmd-modify-option-at-point",
};

/// The prompt machine: the entity, the first point, then the second point or the keyword `at_point`
/// that splits the entity at the first point.
fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(object) = state.answer("object") else {
        return Ok(Step::Ask(Prompt::new(
            "object",
            OBJECT.message_id,
            Accept::Pick {
                types: arc_draft::types::ENTITIES
                    .iter()
                    .map(|k| (*k).to_owned())
                    .collect(),
            },
        )));
    };
    let PromptInput::Pick(id) = object else {
        return Err(Error::new(codes::PICK_INVALID));
    };
    let id = id.to_string();
    let Some(first) = super::point_answer(state, "first") else {
        return Ok(Step::Ask(Prompt::new(
            "first",
            FIRST_POINT.message_id,
            super::point(),
        )));
    };
    if let Some(PromptInput::Keyword(_)) = state.answer("second") {
        return done(serde_json::json!({"object": id, "first": first}));
    }
    let Some(second) = super::point_answer(state, "second") else {
        return Ok(Step::Ask(
            Prompt::new("second", SECOND_POINT.message_id, super::point()).or(Accept::keywords([
                KeywordOption::new("at_point", OPTION_AT_POINT.message_id),
            ])),
        ));
    };
    done(serde_json::json!({"object": id, "first": first, "second": second}))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.break", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, accept_all)),
    ]
}
