//! LENGTHEN (`draftbuilder.modify.lengthen`, typed `LENGTHEN`, alias `LEN`): changes the length of a
//! segment or an arc by a delta, by a percentage of its length, or to a total length, moving the end
//! nearest the pick point. An arc keeps its center and radius and changes its sweep. The entity keeps
//! its identifier. Other kinds are refused with `db.cmd.geometry_unsupported`. The angle form and the
//! dynamic drag are not built.

use arc_base::math::Point2;
use arc_base::{Error, Result, Tolerance};
use arc_command::{CommandSpec, Context, IdParam};
use arc_geom::{Arc2, Curve2, Segment2};
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
    let curve = super::required_curve(&snapshot, id)?;
    let pick = Point2::new(p.pick[0], p.pick[1]);
    let (Some(start), Some(end)) = (curve.start_point(), curve.end_point()) else {
        return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()));
    };
    let new_curve = match curve {
        Curve2::Segment(s) => {
            let length = (s.end() - s.start()).norm();
            let wanted = new_length(length, p)?;
            let unit = (s.end() - s.start()) / length;
            let near_end = (end - pick).norm() <= (start - pick).norm();
            if near_end {
                Curve2::Segment(Segment2::new(s.start(), s.start() + unit * wanted)?)
            } else {
                Curve2::Segment(Segment2::new(s.end() - unit * wanted, s.end())?)
            }
        }
        Curve2::Arc(a) => {
            let length = a.radius() * a.sweep().abs();
            let wanted = new_length(length, p)?;
            let sweep = a.sweep().signum() * wanted / a.radius();
            let end_angle = a.start_angle() + a.sweep();
            let near_end = (end - pick).norm() <= (start - pick).norm();
            if near_end {
                Curve2::Arc(Arc2::new(a.center(), a.radius(), a.start_angle(), sweep)?)
            } else {
                Curve2::Arc(Arc2::new(a.center(), a.radius(), end_angle - sweep, sweep)?)
            }
        }
        _ => return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string())),
    };
    replace_geometry(ctx, id, &new_curve)?;
    ctx.set_selection(vec![id])?;
    Ok(Changed {
        objects: vec![IdParam(id)],
    })
}

/// The length the command asks for, from the current length.
fn new_length(length: f64, p: LengthenParams) -> Result<f64> {
    let wanted = match p.mode {
        LengthenMode::Delta => length + p.value,
        LengthenMode::Percent => length * (1.0 + p.value / 100.0),
        LengthenMode::Total => p.value,
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
