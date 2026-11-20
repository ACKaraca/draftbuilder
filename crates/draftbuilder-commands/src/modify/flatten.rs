//! FLATTEN (`draftbuilder.modify.flatten`, typed `FLATTEN`): sets the elevation and the thickness of the
//! selected entities to zero, so they lie in the XY plane at elevation zero. Curves are two-dimensional in the
//! model, so their geometry needs no projection. An entity whose extrusion direction is not the +Z axis is
//! refused with `db.cmd.geometry_unsupported`, since its elevation would not describe the XY plane. Entities on
//! a locked layer are refused. Every entity is checked before any is changed, so a refusal changes nothing.
//! Hidden-line conversion of three-dimensional objects is not built.

use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{CommandSpec, Context, IdParam, Prompt, PromptState, Step};
use arc_doc::{FieldPath, Value};
use arc_draft::types::entity;
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, entity_selection, refuse_locked, selection_answer};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.flatten`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlattenParams {
    /// The entities to flatten; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-flatten-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-flatten-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};

/// Whether an entity's extrusion direction is the +Z axis (within the linear tolerance on X and Y).
fn extrusion_is_up(snapshot: &arc_doc::Snapshot, id: ObjectId) -> Result<bool> {
    let object = snapshot.get(snapshot.resolve(id)?)?;
    Ok(match object.values.get(entity::EXTRUSION) {
        Some(Some(Value::Vector3(v))) => {
            v.x.abs() <= Tolerance::DEFAULT.linear
                && v.y.abs() <= Tolerance::DEFAULT.linear
                && v.z > 0.0
        }
        _ => true,
    })
}

fn run(ctx: &mut Context<'_>, p: FlattenParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    refuse_locked(ctx.snapshot(), &ids)?;
    let snapshot = ctx.snapshot().clone();
    for id in &ids {
        if !extrusion_is_up(&snapshot, *id)? {
            return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()));
        }
    }
    let txn = ctx.txn()?;
    for id in &ids {
        txn.modify(*id, FieldPath::new("elevation"), Some(Value::F64(0.0)))?;
        txn.modify(*id, FieldPath::new("thickness"), Some(Value::F64(0.0)))?;
    }
    ctx.set_selection(ids.clone())?;
    Ok(Changed {
        objects: ids.into_iter().map(IdParam).collect(),
    })
}

fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    match selection_answer(state, "objects") {
        Some(ids) => done(serde_json::json!({"objects": ids})),
        None => Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        ))),
    }
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.flatten", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, crate::draw::create::accept_all)),
    ]
}
