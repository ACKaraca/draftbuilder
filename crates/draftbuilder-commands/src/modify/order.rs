//! DRAWORDER (`draftbuilder.modify.draw_order`, typed `DRAWORDER`): moves entities to the front or the back of
//! their space's draw order, or directly above or below a reference entity of the same space. The foundation's
//! placement rule (`arc_draft::order::reorder`) decides the new keys, so the moved entities keep their relative
//! order and a changed key is stored with the drawing. The entities must share one space, and the reference
//! must be in it and not moved. Bulk placement of kinds (all text, all hatches) is not built.
//!
//! Reordering may renumber the draw-order keys of other entities of the space, including entities on a locked
//! layer: the foundation's rule renumbers the keys of the whole space when the gap between two keys runs out. Only
//! the key of such an entity changes; its geometry and its other fields do not. Only the named entities are
//! checked against locked layers.

use arc_base::{Error, ObjectId, Result};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::relationship::ReferenceField;
use arc_doc::{FieldPath, Snapshot, Value};
use arc_draft::order::{Place, reorder};
use arc_draft::types::{ENTITIES, entity};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, entity_selection, keyword_answer, refuse_locked, selection_answer};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};

/// Where the entities go in the draw order.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DrawPlace {
    /// On top of every other entity of the space.
    Front,
    /// Underneath every other entity of the space.
    Back,
    /// Directly above the reference.
    Above,
    /// Directly below the reference.
    Below,
}

/// Parameters of `draftbuilder.modify.draw_order`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawOrderParams {
    /// The entities to move; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// Where they go.
    pub place: DrawPlace,
    /// The entity to place them next to; required for `above` and `below`, refused otherwise.
    #[serde(default)]
    pub reference: Option<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-draw-order-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-draw-order-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};
const PLACE: Message = Message {
    message_id: "db-cmd-modify-draw-order-place",
};
const REFERENCE: Message = Message {
    message_id: "db-cmd-modify-draw-order-reference",
};
const OPTION_FRONT: Message = Message {
    message_id: "db-cmd-modify-option-front",
};
const OPTION_BACK: Message = Message {
    message_id: "db-cmd-modify-option-back",
};
const OPTION_ABOVE: Message = Message {
    message_id: "db-cmd-modify-option-above",
};
const OPTION_BELOW: Message = Message {
    message_id: "db-cmd-modify-option-below",
};

/// The entities of `space` with their draw-order keys, as the foundation reads them.
fn members(snapshot: &Snapshot, space: ObjectId) -> Vec<(ObjectId, i64)> {
    snapshot
        .referrers(space)
        .filter(|r| r.field == ReferenceField::Owner)
        .filter_map(|r| {
            let handle = snapshot.resolve(r.source).ok()?;
            let key = snapshot.type_key(handle)?;
            if !ENTITIES.contains(&key.as_str()) {
                return None;
            }
            match snapshot.get(handle).ok()?.values.get(entity::DRAW_ORDER) {
                Some(Some(Value::I64(k))) => Some((r.source, *k)),
                _ => Some((r.source, 0)),
            }
        })
        .collect()
}

fn run(ctx: &mut Context<'_>, p: DrawOrderParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    let place = match (p.place, p.reference) {
        (DrawPlace::Front, None) => Place::Front,
        (DrawPlace::Back, None) => Place::Back,
        (DrawPlace::Above, Some(r)) => Place::Above(entities(&snapshot, ctx.limits(), &[r])?[0]),
        (DrawPlace::Below, Some(r)) => Place::Below(entities(&snapshot, ctx.limits(), &[r])?[0]),
        (DrawPlace::Above | DrawPlace::Below, None) => {
            return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "reference"));
        }
        (DrawPlace::Front | DrawPlace::Back, Some(_)) => {
            return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "reference"));
        }
    };
    let Some(&first) = ids.first() else {
        return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "objects"));
    };
    let space = snapshot
        .get(snapshot.resolve(first)?)?
        .header
        .owner
        .ok_or_else(|| Error::new(codes::PARAMS_CONFLICT).with_detail("field", "objects"))?;
    let moved: Vec<ObjectId> = ids.clone();
    let changes = reorder(&members(&snapshot, space), &moved, place)?;
    let txn = ctx.txn()?;
    for (id, key) in changes {
        txn.modify(id, FieldPath::new("draw_order"), Some(Value::I64(key)))?;
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
            entity_selection(),
        )));
    };
    let Some(place) = keyword_answer(state, "place") else {
        return Ok(Step::Ask(Prompt::new(
            "place",
            PLACE.message_id,
            Accept::keywords([
                KeywordOption::new("front", OPTION_FRONT.message_id),
                KeywordOption::new("back", OPTION_BACK.message_id),
                KeywordOption::new("above", OPTION_ABOVE.message_id),
                KeywordOption::new("below", OPTION_BELOW.message_id),
            ]),
        )));
    };
    if place == "above" || place == "below" {
        let Some(PromptInput::Pick(reference)) = state.answer("reference") else {
            return Ok(Step::Ask(Prompt::new(
                "reference",
                REFERENCE.message_id,
                Accept::Pick {
                    types: ENTITIES.iter().map(|k| (*k).to_owned()).collect(),
                },
            )));
        };
        return done(serde_json::json!({
            "objects": ids, "place": place, "reference": reference.to_string()
        }));
    }
    done(serde_json::json!({"objects": ids, "place": place}))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.draw_order", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, crate::draw::create::accept_all)),
    ]
}
