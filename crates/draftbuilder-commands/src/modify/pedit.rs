//! PEDIT (`draftbuilder.modify.pedit`, typed `PEDIT`): opens or closes polylines. `close` adds the straight
//! segment from the last vertex back to the first, which takes no width, or, when the last vertex already is the
//! first, drops that duplicate vertex and closes with the segment that reached it. Closing a polyline of two
//! vertices is refused (`db.cmd.value_invalid`). `open` removes the closing segment,
//! and the last vertex of an open polyline keeps no bulge or width. A polyline already in the wanted state is
//! left as it is. Other kinds are refused with `db.cmd.geometry_unsupported`, and no object changes when one is
//! refused. Joining, uniform width, vertex editing, curve fitting and line conversion are not built.

use arc_base::{Error, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptState, Step,
};
use arc_draft::types::POLYLINE;
use arc_geom::{Curve2, Polyline2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, keyword_answer, refuse_locked, replace_geometry, selection_answer};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};

/// The state a polyline is put in.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeditAction {
    /// Closes an open polyline.
    Close,
    /// Opens a closed polyline.
    Open,
}

/// Parameters of `draftbuilder.modify.pedit`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PeditParams {
    /// The polylines to change; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// Whether the polylines are closed or opened.
    pub action: PeditAction,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-pedit-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-pedit-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};
const ACTION: Message = Message {
    message_id: "db-cmd-modify-pedit-action",
};
const OPTION_OPEN: Message = Message {
    message_id: "db-cmd-modify-option-open",
};
const OPTION_CLOSE: Message = Message {
    message_id: "db-cmd-modify-option-close-polyline",
};

fn run(ctx: &mut Context<'_>, p: PeditParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    let closed = p.action == PeditAction::Close;
    // Every object is checked before the first change.
    let mut edits: Vec<(arc_base::ObjectId, Polyline2)> = Vec::new();
    for id in &ids {
        let Curve2::Polyline(poly) = super::required_curve(&snapshot, *id)? else {
            return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()));
        };
        if poly.is_closed() == closed {
            continue;
        }
        let mut vertices = poly.vertices().to_vec();
        if closed {
            // A polyline of two vertices has no closing segment to add: it would close onto itself.
            if vertices.len() < 3 {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "action"));
            }
            // A last vertex that is the first one already closes the polyline: it is dropped, and the segment that
            // reached it closes the polyline, keeping its bulge and widths.
            let (Some(first), Some(last)) = (vertices.first(), vertices.last()) else {
                return Err(
                    Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string())
                );
            };
            if (last.point - first.point).norm() <= Tolerance::DEFAULT.linear {
                vertices.pop();
            } else if let Some(last) = vertices.last_mut() {
                // Otherwise the closing segment is added straight.
                last.bulge = 0.0;
                last.start_width = 0.0;
                last.end_width = 0.0;
            }
        } else if let Some(last) = vertices.last_mut() {
            // An open polyline's last vertex starts no segment: it keeps no bulge and no width.
            last.bulge = 0.0;
            last.start_width = 0.0;
            last.end_width = 0.0;
        }
        edits.push((*id, Polyline2::new(vertices, closed)?));
    }
    for (id, poly) in &edits {
        replace_geometry(ctx, *id, &Curve2::Polyline(poly.clone()))?;
    }
    ctx.set_selection(ids.clone())?;
    Ok(Changed {
        objects: ids.into_iter().map(IdParam).collect(),
    })
}

fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(ids) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            Accept::Selection {
                min: 1,
                max: None,
                types: vec![POLYLINE.to_owned()],
            },
        )));
    };
    let Some(action) = keyword_answer(state, "action") else {
        return Ok(Step::Ask(Prompt::new(
            "action",
            ACTION.message_id,
            Accept::keywords([
                KeywordOption::new("close", OPTION_CLOSE.message_id),
                KeywordOption::new("open", OPTION_OPEN.message_id),
            ]),
        )));
    };
    done(serde_json::json!({"objects": ids, "action": action}))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.pedit", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, crate::draw::create::accept_all)),
    ]
}
