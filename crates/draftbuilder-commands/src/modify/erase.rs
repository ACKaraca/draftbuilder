//! ERASE (`draftbuilder.modify.erase`, typed `ERASE`, alias `E`): deletes the selected entities in
//! one undo step. An entity on a locked layer is refused. An entity that another entity refers to
//! (a dimension, a hatch boundary, a path array) is erased only when that dependent is erased too;
//! otherwise the command is refused with `db.cmd.erase_dependent`, so no dependent loses its
//! geometry silently.

use arc_base::Result;
use arc_command::{CommandSpec, Context, IdParam, Prompt, PromptState, Step};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{entities, entity_selection, erase_entities, refuse_locked, selection_answer};
use crate::draw::create::{accept_all, done, machine};
use crate::error::Message;

/// Parameters of `draftbuilder.modify.erase`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EraseParams {
    /// The entities to erase; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

/// What `draftbuilder.modify.erase` returns: the entities it erased.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Erased {
    /// The erased entities, in order.
    pub objects: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-erase-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-erase-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};

fn run(ctx: &mut Context<'_>, p: EraseParams) -> Result<Erased> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    refuse_locked(ctx.snapshot(), &ids)?;
    erase_entities(ctx, &ids)?;
    Ok(Erased {
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
        super::spec("draftbuilder.modify.erase", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, accept_all)),
    ]
}
