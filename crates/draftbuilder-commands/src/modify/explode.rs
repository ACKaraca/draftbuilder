//! EXPLODE (`draftbuilder.modify.explode`, typed `EXPLODE`, alias `X`): breaks a polyline into its
//! lines and arcs, and a block reference into the entities of its definition, placed by the
//! reference. Other kinds are refused with `db.cmd.explode_unsupported`, naming the kind.
//!
//! Each component keeps the stored values of its source (its appearance and its kind's fields). An entity
//! on layer `0` takes the reference's layer. A `ByBlock` appearance takes the reference's effective value:
//! the reference's own value, or, when the reference is by-layer, the explicit value of the reference's
//! layer (the component keeps its own layer, so the value is written, not inherited). A reference that is
//! itself by-block leaves the component by-block. A block reference is placed by its position, rotation and
//! scale. A uniform scale, or a scale whose axes differ only in sign, is exact; any other
//! non-uniform scale is refused. A block reference with attribute values is refused, because its
//! attribute text is not exploded yet. Polylines with widths are refused, because the components
//! would lose them.
//!
//! The source is erased; the components are new entities, owned by the source's owner, in order.

use std::f64::consts::PI;

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam};
use arc_doc::{Snapshot, Value};
use arc_draft::types::{self, BLOCK, BLOCK_REF, By, ENTITIES, POLYLINE, entity};
use arc_draft_dxf::fields::{Reader, name_of};
use arc_geom::{Curve2, Polyline2, PolylineSegment};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use arc_draft::TransformParam;

use super::transform::transform_entities;
use super::{
    entities, entity_key, entity_selection, erase_entities, refuse_associated_copy, refuse_locked,
    selection_answer,
};
use crate::draw::create::{Creator, accept_all, done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.explode`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplodeParams {
    /// The entities to explode; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

/// What `draftbuilder.modify.explode` returns: the components, in order.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Exploded {
    /// The new components.
    pub objects: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-explode-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-explode-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};

/// The appearance fields that a `ByBlock` component takes from its reference, as (mode, value).
const APPEARANCE: [(usize, usize, usize); 4] = [
    (entity::COLOR_BY, entity::COLOR, types::layer::COLOR),
    (
        entity::LINETYPE_BY,
        entity::LINETYPE,
        types::layer::LINETYPE,
    ),
    (
        entity::LINEWEIGHT_BY,
        entity::LINEWEIGHT,
        types::layer::LINEWEIGHT,
    ),
    (
        entity::TRANSPARENCY_BY,
        entity::TRANSPARENCY,
        types::layer::TRANSPARENCY,
    ),
];

/// The explicit appearance a by-block component takes from its block reference, as the mode and the value
/// to store: the reference's own value when it has one, and the value of the reference's layer when the
/// reference is by-layer (the component keeps its own layer, so the layer's value must be written
/// explicitly). `None` when the reference is itself by-block: the component then stays by-block.
fn inherited(
    snapshot: &Snapshot,
    insert: &[Option<Value>],
    mode: usize,
    value: usize,
    layer_field: usize,
) -> Option<(Value, Option<Value>)> {
    let explicit = Value::Enum(By::Object as u32);
    match By::of(types::enum_at(insert, mode)) {
        By::Object => Some((explicit, insert.get(value).cloned().flatten())),
        By::Layer => {
            let layer = match insert.get(entity::LAYER) {
                Some(Some(Value::Reference(layer))) => *layer,
                _ => return Some((explicit, None)),
            };
            let stored = snapshot
                .get(snapshot.resolve(layer).ok()?)
                .ok()?
                .values
                .get(layer_field)
                .cloned()
                .flatten();
            Some((explicit, stored))
        }
        By::Block => None,
    }
}

fn unsupported(key: &str, reason: &str) -> Error {
    Error::new(codes::EXPLODE_UNSUPPORTED)
        .with_detail("kind", key)
        .with_detail("reason", reason)
}

/// A component to create: its type key and its stored values.
type Component = (&'static str, Vec<Option<Value>>);

/// The components of a polyline: one line or arc per segment, with the polyline's values.
fn polyline_components(
    snapshot: &Snapshot,
    source: &[Option<Value>],
    polyline: &Polyline2,
) -> Result<Vec<Component>> {
    if polyline
        .vertices()
        .iter()
        .any(|v| v.start_width != 0.0 || v.end_width != 0.0)
    {
        return Err(unsupported(POLYLINE, "width"));
    }
    let mut out = Vec::new();
    for segment in polyline.segments() {
        let curve = match segment {
            PolylineSegment::Line(s) => Curve2::Segment(s),
            PolylineSegment::Arc(a) => Curve2::Arc(a),
        };
        let key = super::key_of_curve(&curve);
        out.push((key, super::appearance_values(snapshot, key, source, &curve)));
    }
    Ok(out)
}

/// The linear part of a block reference's placement, applied to `p`: mirror, scale, then rotation.
fn linear(p: [f64; 2], mirror: bool, scale: f64, rotation: f64) -> [f64; 2] {
    let x = p[0];
    let y = if mirror { -p[1] } else { p[1] };
    let (s, c) = rotation.sin_cos();
    [scale * (c * x - s * y), scale * (s * x + c * y)]
}

/// The transform of a block reference's entities: `p ↦ position + A (p − base)` with `A = R diag(sx, sy)`,
/// as the foundation's similarity transform, or an error when the scale is not uniform up to a sign.
fn placement(snapshot: &Snapshot, insert: &[Option<Value>], key: &str) -> Result<TransformParam> {
    let reader = Reader::new(snapshot.registry(), BLOCK_REF, insert);
    let block = reader
        .reference("block")
        .ok_or_else(|| unsupported(key, "block"))?;
    let position = match reader.get("position") {
        Some(Value::Point2(p)) => [p.x, p.y],
        _ => [0.0, 0.0],
    };
    let (sx, sy) = match reader.get("scale") {
        Some(Value::Vector3(v)) => (v.x, v.y),
        _ => (1.0, 1.0),
    };
    let rotation = reader.f64("rotation").unwrap_or(0.0);
    let definition = snapshot.get(snapshot.resolve(block)?)?;
    let base = match Reader::new(snapshot.registry(), BLOCK, &definition.values).get("base_point") {
        Some(Value::Point2(p)) => [p.x, p.y],
        _ => [0.0, 0.0],
    };
    let (mirror, scale) = if (sx - sy).abs() == 0.0 {
        (false, sx.abs())
    } else if (sx + sy).abs() == 0.0 {
        (true, sx.abs())
    } else {
        return Err(unsupported(key, "scale"));
    };
    if !(scale.is_finite() && scale > 0.0) {
        return Err(unsupported(key, "scale"));
    }
    // A negative factor on both axes is a half turn; on the mirrored form it turns the mirror.
    let turn = if sx < 0.0 { PI } else { 0.0 };
    let rotation = rotation + turn;
    let moved = linear(base, mirror, scale, rotation);
    Ok(TransformParam {
        mirror: mirror.then_some([1.0, 0.0]),
        scale,
        rotation,
        translation: [position[0] - moved[0], position[1] - moved[1]],
    })
}

/// The components of a block reference: the entities of its definition, with the reference's layer
/// and appearance where the definition leaves them to the reference. Returns each with the key and
/// the values, and the transform that places them.
fn block_components(snapshot: &Snapshot, id: ObjectId) -> Result<(Vec<Component>, TransformParam)> {
    let key = entity_key(snapshot, id)?;
    let object = snapshot.get(snapshot.resolve(id)?)?;
    let insert = &object.values;
    let reader = Reader::new(snapshot.registry(), BLOCK_REF, insert);
    if let Some(Value::Map(attributes)) = reader.get("attributes")
        && !attributes.is_empty()
    {
        return Err(unsupported(&key, "attributes"));
    }
    let block = reader
        .reference("block")
        .ok_or_else(|| unsupported(&key, "block"))?;
    let placement = placement(snapshot, insert, &key)?;
    let mut out = Vec::new();
    for member_key in ENTITIES {
        let Ok(members) = snapshot.iter_type(member_key) else {
            continue;
        };
        for (_, member) in members {
            if member.header.owner != Some(block) {
                continue;
            }
            refuse_associated_copy(snapshot, member.id)?;
            let mut values = member.values.clone();
            // A layer-0 component takes the layer of the reference.
            if layer_name(snapshot, &member.values).as_deref() == Some("0")
                && let Some(slot) = values.get_mut(entity::LAYER)
            {
                *slot = insert.get(entity::LAYER).cloned().flatten();
            }
            for (mode, value, layer_field) in APPEARANCE {
                if By::of(types::enum_at(&member.values, mode)) != By::Block {
                    continue;
                }
                if let Some((by, stored)) = inherited(snapshot, insert, mode, value, layer_field) {
                    if let Some(slot) = values.get_mut(mode) {
                        *slot = Some(by);
                    }
                    if let Some(slot) = values.get_mut(value) {
                        *slot = stored;
                    }
                }
            }
            out.push((*member_key, values));
        }
    }
    Ok((out, placement))
}

/// The name of the layer of stored `values`, `None` when the entity names no layer.
fn layer_name(snapshot: &Snapshot, values: &[Option<Value>]) -> Option<String> {
    match values.get(entity::LAYER) {
        Some(Some(Value::Reference(layer))) => name_of(snapshot, Some(*layer)),
        _ => None,
    }
}

fn run(ctx: &mut Context<'_>, p: ExplodeParams) -> Result<Exploded> {
    let snapshot = ctx.snapshot().clone();
    let ids = entities(&snapshot, ctx.limits(), &p.objects)?;
    refuse_locked(&snapshot, &ids)?;
    let space = crate::model_space(&snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let mut creator = Creator::new(&snapshot)?;
    let mut out: Vec<ObjectId> = Vec::new();
    for id in &ids {
        let key = entity_key(&snapshot, *id)?;
        let object = snapshot.get(snapshot.resolve(*id)?)?;
        let owner = object.header.owner.or(Some(space));
        match key.as_str() {
            POLYLINE => {
                let curve = super::required_curve(&snapshot, *id)?;
                let Curve2::Polyline(polyline) = curve else {
                    return Err(unsupported(&key, "geometry"));
                };
                for (piece_key, values) in
                    polyline_components(&snapshot, &object.values, &polyline)?
                {
                    out.push(creator.add_values(ctx, piece_key, owner, values)?);
                }
            }
            BLOCK_REF => {
                let (members, placement) = block_components(&snapshot, *id)?;
                let mut created = Vec::with_capacity(members.len());
                for (member_key, values) in members {
                    created.push(creator.add_values(ctx, member_key, owner, values)?);
                }
                transform_entities(ctx, &created, placement)?;
                out.extend(created);
            }
            other => return Err(unsupported(other, "kind")),
        }
    }
    erase_entities(ctx, &ids)?;
    ctx.set_selection(out.clone())?;
    Ok(Exploded {
        objects: out.into_iter().map(IdParam).collect(),
    })
}

fn next(state: &arc_command::PromptState<'_>) -> arc_base::Result<arc_command::Step> {
    match selection_answer(state, "objects") {
        Some(ids) => done(serde_json::json!({"objects": ids})),
        None => Ok(arc_command::Step::Ask(arc_command::Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        ))),
    }
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.explode", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, accept_all)),
    ]
}
