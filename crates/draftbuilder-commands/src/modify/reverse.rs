//! REVERSE (`draftbuilder.modify.reverse`, typed `REVERSE`): reverses the direction of lines, polylines and
//! splines, so that linetypes with text and direction-dependent symbols read the other way. A line swaps its
//! ends, a polyline reverses its vertex order (its bulges and widths turn with it), and a spline reverses its
//! parameter; a spline made through fit points has its fit points reversed too. The change is in place and the
//! identifier stays. Other kinds are refused with
//! `db.cmd.geometry_unsupported`, and no object changes when one is refused. Helices are not in the model.

use arc_base::{Error, Result};
use arc_command::{Accept, CommandSpec, Context, IdParam, Prompt, PromptState, Step};
use arc_doc::{FieldPath, Value};
use arc_draft::types::{LINE, POLYLINE, SPLINE, entity};
use arc_geom::Curve2;
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, refuse_locked, replace_geometry, selection_answer};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.reverse`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReverseParams {
    /// The lines, polylines and splines to reverse; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-reverse-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-reverse-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};

fn run(ctx: &mut Context<'_>, p: ReverseParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    // Every object is checked before the first change.
    let mut reversed = Vec::with_capacity(ids.len());
    for id in &ids {
        let curve = super::required_curve(&snapshot, *id)?;
        let reversible = matches!(
            curve,
            Curve2::Segment(_) | Curve2::Polyline(_) | Curve2::Nurbs(_)
        );
        let flipped = if reversible { curve.reversed() } else { None }.ok_or_else(|| {
            Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string())
        })?;
        reversed.push((*id, flipped));
    }
    for (id, curve) in &reversed {
        replace_geometry(ctx, *id, curve)?;
    }
    // A spline made through fit points keeps them stored with it, in the order of the curve: they reverse too.
    for id in &ids {
        let object = snapshot.get(snapshot.resolve(*id)?)?;
        if super::entity_key(&snapshot, *id)? != SPLINE {
            continue;
        }
        if let Some(Some(Value::List(fit))) = object.values.get(entity::SPLINE_FIT_POINTS) {
            let mut reversed_fit = fit.clone();
            reversed_fit.reverse();
            ctx.txn()?.modify(
                *id,
                FieldPath::new("fit_points"),
                Some(Value::List(reversed_fit)),
            )?;
        }
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
            Accept::Selection {
                min: 1,
                max: None,
                types: vec![LINE.to_owned(), POLYLINE.to_owned(), SPLINE.to_owned()],
            },
        ))),
    }
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.reverse", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, crate::draw::create::accept_all)),
    ]
}
