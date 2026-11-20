//! Modify commands (area `modify`): transforms that move, copy, rotate, scale, mirror and align
//! entities, erase and explode them, change their geometry (trim, extend, stretch, offset, fillet,
//! chamfer, break, join, lengthen) and array them.
//!
//! Each command has a typed form (its parameters, used by the protocol, scripts and the CLI) and a
//! prompt machine for the interactive form, which ends in the same invocation, so one command is
//! one undo step. The families are one module each: [`transform`] (MOVE, COPY, ROTATE, SCALE,
//! MIRROR, ALIGN), [`erase`] (ERASE), [`explode`] (EXPLODE), [`stretch`] (STRETCH), [`trim`]
//! (TRIM, EXTEND), [`offset`] (OFFSET), [`corner`] (FILLET, CHAMFER), [`breaking`] (BREAK),
//! [`join`] (JOIN), [`lengthen`] (LENGTHEN), [`reverse`] (REVERSE), [`pedit`] (PEDIT, open and close), [`change`]
//! (the endpoint of a line or the radius of a circle), [`flatten`] (FLATTEN), [`order`] (DRAWORDER) and
//! [`overkill`] (OVERKILL, lines).
//!
//! Shared rules, applied by every family:
//!
//! - Geometry changes that keep the kind of an entity replace its geometry in place: the identifier
//!   and the handle stay. A result of another kind, or a count that differs from the input, creates
//!   entities and erases the inputs they replace; the new entities take the layer and every
//!   appearance field of their source; the pieces of a split keep the identifier on the first piece of the
//!   source's kind and take draw orders beside the source (see [`replace_with_pieces`]).
//! - An entity on a locked layer is refused with the foundation's `draft.layer.locked`, which the
//!   foundation's own transform applies; the product's writes apply the same rule
//!   (`arc_draft::lock::check_entity`).
//! - A copy never copies a reference to another entity (dimensions, hatches with boundaries and
//!   path arrays are associated with the entities they name): such a kind is refused with
//!   `db.cmd.copy_associated`.
//! - Coordinates are in the drawing's units, angles in radians counterclockwise; the prompts answer
//!   in meters and degrees (the session's context units).

pub mod array;
pub mod breaking;
pub mod change;
pub mod corner;
pub mod erase;
pub mod explode;
pub mod flatten;
pub mod join;
pub mod lengthen;
pub mod offset;
pub mod order;
pub mod overkill;
pub mod pedit;
pub mod reverse;
pub mod stretch;
pub mod transform;
pub mod trim;

use arc_base::{Error, Limits, ObjectId, Result};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, Permission, PointDims, PromptInput, PromptState,
    Registry, UndoBehavior,
};
use arc_doc::{Snapshot, Value};
use arc_draft::types::ENTITIES;
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::draw::create::{Creator, drawing_point, from_meters};
use crate::error::{Message, codes};
use crate::vocabulary::{Area, register_names};

/// The most entities one modify command takes, as the foundation's transform does.
pub(crate) const MAX_OBJECTS: usize = 10_000;

/// A command of the area: undoable, with the `edit` permission class.
pub(crate) fn spec<P, R>(
    id: &'static str,
    label: Message,
    description: Message,
    run: impl Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
) -> CommandSpec
where
    P: DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
{
    CommandSpec::new(id, label.message_id, description.message_id, run)
        .with_undo(UndoBehavior::Undoable)
        .with_permissions([Permission::Edit])
}

/// Registers the commands of the area and applies its names.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered, and the errors of
/// [`register_names`].
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs = transform::specs()
        .into_iter()
        .chain(array::specs())
        .chain(erase::specs())
        .chain(explode::specs())
        .chain(stretch::specs())
        .chain(trim::specs())
        .chain(offset::specs())
        .chain(corner::specs())
        .chain(breaking::specs())
        .chain(join::specs())
        .chain(lengthen::specs())
        .chain(reverse::specs())
        .chain(pedit::specs())
        .chain(change::specs())
        .chain(flatten::specs())
        .chain(order::specs())
        .chain(overkill::specs());
    for spec in specs {
        registry.register(spec)?;
    }
    register_names(registry, Area::Modify)
}

/// The type key of an entity identifier, which must name an entity.
///
/// # Errors
///
/// `draft.object.wrong_type` for an identifier that names no entity, and the lookup errors.
pub(crate) fn entity_key(snapshot: &Snapshot, id: ObjectId) -> Result<String> {
    let handle = snapshot.resolve(id)?;
    let key = snapshot
        .type_key(handle)
        .map(|k| k.as_str().to_owned())
        .unwrap_or_default();
    if ENTITIES.contains(&key.as_str()) {
        Ok(key)
    } else {
        Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
            .with_detail("id", id.to_string()))
    }
}

/// The entity identifiers of a list: each entity once, in the order given. More than the session's
/// array limit (and never more than [`MAX_OBJECTS`]) is `db.cmd.limit_exceeded`.
///
/// # Errors
///
/// `draft.object.wrong_type` for an identifier that names no entity, and `db.cmd.limit_exceeded`.
pub(crate) fn entities(
    snapshot: &Snapshot,
    limits: &Limits,
    list: &[IdParam],
) -> Result<Vec<ObjectId>> {
    let limit = usize::try_from(limits.array_length)
        .unwrap_or(usize::MAX)
        .min(MAX_OBJECTS);
    if list.len() > limit {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", limit.to_string()));
    }
    let mut out: Vec<ObjectId> = Vec::with_capacity(list.len());
    for IdParam(id) in list {
        entity_key(snapshot, *id)?;
        if !out.contains(id) {
            out.push(*id);
        }
    }
    Ok(out)
}

/// The curve of an entity, decoded from its stored geometry; `None` for an entity without one (a
/// point, a text, a dimension).
///
/// # Errors
///
/// The decoding errors of the geometry.
pub(crate) fn curve_of(snapshot: &Snapshot, id: ObjectId) -> Result<Option<Curve2>> {
    let key = entity_key(snapshot, id)?;
    let object = snapshot.get(snapshot.resolve(id)?)?;
    match Reader::new(snapshot.registry(), &key, &object.values).get("geometry") {
        Some(Value::Geometry(bytes)) => Ok(Some(Curve2::decode(&bytes.0, &Limits::DEFAULT)?)),
        _ => Ok(None),
    }
}

/// The curve of an entity that must have one.
///
/// # Errors
///
/// `db.cmd.geometry_unsupported` (detail `id`) for an entity without a curve, and the errors of
/// [`curve_of`].
pub(crate) fn required_curve(snapshot: &Snapshot, id: ObjectId) -> Result<Curve2> {
    curve_of(snapshot, id)?
        .ok_or_else(|| Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()))
}

/// Whether a stored value refers to an entity: any reference in it names an object of an entity
/// type.
pub(crate) fn refers_to_entity(snapshot: &Snapshot, value: &Value) -> bool {
    match value {
        Value::Reference(id) => entity_key(snapshot, *id).is_ok(),
        Value::List(items) => items.iter().any(|v| refers_to_entity(snapshot, v)),
        Value::Map(entries) => entries.values().any(|v| refers_to_entity(snapshot, v)),
        Value::Record(fields) => fields
            .iter()
            .flatten()
            .any(|v| refers_to_entity(snapshot, v)),
        _ => false,
    }
}

/// Fails with `db.cmd.copy_associated` (detail `id`) when the entity refers to another entity.
pub(crate) fn refuse_associated_copy(snapshot: &Snapshot, id: ObjectId) -> Result<()> {
    let object = snapshot.get(snapshot.resolve(id)?)?;
    let associated = object
        .values
        .iter()
        .flatten()
        .any(|v| refers_to_entity(snapshot, v));
    if associated {
        return Err(Error::new(codes::COPY_ASSOCIATED).with_detail("id", id.to_string()));
    }
    Ok(())
}

/// Fails with the foundation's `draft.layer.locked` when an entity is on a locked layer.
pub(crate) fn refuse_locked(snapshot: &Snapshot, ids: &[ObjectId]) -> Result<()> {
    for id in ids {
        arc_draft::lock::check_entity(snapshot, *id)?;
    }
    Ok(())
}

/// Copies the entities `ids` in the drawing: each copy has a new identifier and the stored values of
/// its source, on the same layer, owned by the same owner, with a new draw order. Returns the new
/// identifiers in order. Refuses a kind that refers to another entity.
///
/// # Errors
///
/// `db.cmd.copy_associated`, the copy errors of the transaction, and the errors of [`Creator`].
pub(crate) fn copy_entities(ctx: &mut Context<'_>, ids: &[ObjectId]) -> Result<Vec<ObjectId>> {
    let snapshot = ctx.snapshot().clone();
    for id in ids {
        refuse_associated_copy(&snapshot, *id)?;
    }
    let mut creator = Creator::new(&snapshot)?;
    for id in ids {
        creator.copy_of(ctx, *id)?;
    }
    Ok(creator.into_ids())
}

/// Fails with `db.cmd.erase_dependent` (detail `id`, the referring entity) when an entity outside `ids`
/// refers to one of `ids`: a dimension, a hatch boundary, a path array. Identifiers erased by the same call
/// are not dependents of each other.
///
/// # Errors
///
/// `db.cmd.erase_dependent`.
pub(crate) fn refuse_dependents(snapshot: &Snapshot, ids: &[ObjectId]) -> Result<()> {
    refuse_referrers(snapshot, ids, codes::ERASE_DEPENDENT)
}

/// Fails with `code` (detail `id`, the referring entity) when an entity outside `ids` refers to one of them.
///
/// # Errors
///
/// `code`.
pub(crate) fn refuse_referrers(
    snapshot: &Snapshot,
    ids: &[ObjectId],
    code: arc_base::ErrorCode,
) -> Result<()> {
    for id in ids {
        for referrer in snapshot.referrers(*id) {
            let source = referrer.source;
            if !ids.contains(&source) && entity_key(snapshot, source).is_ok() {
                return Err(Error::new(code).with_detail("id", source.to_string()));
            }
        }
    }
    Ok(())
}

/// Erases the entities `ids` (already validated as entities and checked for locked layers by the caller).
/// Every path that erases an entity goes through here: an entity that another entity refers to is refused
/// first, so no dependent loses its geometry (a circle a hatch boundary names is not broken silently).
///
/// # Errors
///
/// `db.cmd.erase_dependent`, and the errors of the transaction and of the delete rules.
pub(crate) fn erase_entities(ctx: &mut Context<'_>, ids: &[ObjectId]) -> Result<()> {
    refuse_dependents(ctx.snapshot(), ids)?;
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    for id in ids {
        txn.delete(*id)?;
    }
    txn.apply_delete_rules(&limits)?;
    Ok(())
}

/// Replaces the geometry of an entity of the same kind in place: the identifier and the handle
/// stay, the stored curve changes. The caller has checked the layer.
///
/// # Errors
///
/// The errors of the transaction.
pub(crate) fn replace_geometry(ctx: &mut Context<'_>, id: ObjectId, curve: &Curve2) -> Result<()> {
    let value = Value::Geometry(arc_doc::GeometryBytes(curve.encode()));
    ctx.txn()?
        .modify(id, arc_doc::FieldPath::new("geometry"), Some(value))?;
    Ok(())
}

/// The stored values of a new entity of `key` that takes the layer and appearance of `source` (the
/// common fields of every entity, before the kind's own fields) and holds `curve`. The draw order is
/// left to the creator.
#[must_use]
pub(crate) fn appearance_values(
    snapshot: &Snapshot,
    key: &str,
    source: &[Option<Value>],
    curve: &Curve2,
) -> Vec<Option<Value>> {
    let mut values = arc_draft::types::defaults(snapshot.registry(), key);
    for index in 0..arc_draft::types::entity::KIND {
        if let (Some(slot), Some(value)) = (values.get_mut(index), source.get(index)) {
            *slot = value.clone();
        }
    }
    if let Some(slot) = values.get_mut(arc_draft::types::entity::KIND) {
        *slot = Some(Value::Geometry(arc_doc::GeometryBytes(curve.encode())));
    }
    values
}

/// The entity type key of a curve.
#[must_use]
pub(crate) fn key_of_curve(curve: &Curve2) -> &'static str {
    use arc_draft::types::{
        ARC, CIRCLE, CONSTRUCTION_LINE, ELLIPSE, ELLIPTICAL_ARC, LINE, POLYLINE, RAY, SPLINE,
    };
    match curve {
        Curve2::Segment(_) => LINE,
        Curve2::Ray(_) => RAY,
        Curve2::Line(_) => CONSTRUCTION_LINE,
        Curve2::Circle(_) => CIRCLE,
        Curve2::Arc(_) => ARC,
        Curve2::Ellipse(_) => ELLIPSE,
        Curve2::EllipticalArc(_) => ELLIPTICAL_ARC,
        Curve2::Polyline(_) => POLYLINE,
        Curve2::Nurbs(_) => SPLINE,
    }
}

/// Makes `pieces` the entities that replace `id`. The identifier and the handle stay with the first piece
/// of the same kind as `id`, which replaces its geometry in place; every other piece is created. A created
/// piece of the same kind as `id` copies every stored value of `id` except its geometry (so a polyline's
/// fields survive a split); a piece of another kind takes the common fields only. Created pieces are owned
/// like `id` (its space or block) and take draw orders adjacent to `id`'s, after it. When no piece has the
/// kind of `id`, `id` is erased. Returns the identifiers of the pieces in order.
///
/// # Errors
///
/// `draft.layer.locked` for an entity on a locked layer, `db.cmd.erase_dependent` when `id` is erased and
/// another entity refers to it, and the errors of the transaction and of the creator.
pub(crate) fn replace_with_pieces(
    ctx: &mut Context<'_>,
    id: ObjectId,
    pieces: &[Curve2],
) -> Result<Vec<ObjectId>> {
    let snapshot = ctx.snapshot().clone();
    arc_draft::lock::check_entity(&snapshot, id)?;
    let key = entity_key(&snapshot, id)?;
    let object = snapshot.get(snapshot.resolve(id)?)?;
    let owner = object.header.owner;
    let source = object.values.clone();
    let order = match source.get(arc_draft::types::entity::DRAW_ORDER) {
        Some(Some(Value::I64(order))) => *order,
        _ => 0,
    };
    let keeper = pieces.iter().position(|piece| key_of_curve(piece) == key);
    let mut creator = Creator::new(&snapshot)?;
    let mut out = Vec::with_capacity(pieces.len());
    let mut created = 0_i64;
    for (index, piece) in pieces.iter().enumerate() {
        if Some(index) == keeper {
            replace_geometry(ctx, id, piece)?;
            out.push(id);
            continue;
        }
        let piece_key = key_of_curve(piece);
        let values = if piece_key == key {
            let mut values = source.clone();
            if let Some(slot) = values.get_mut(arc_draft::types::entity::KIND) {
                *slot = Some(Value::Geometry(arc_doc::GeometryBytes(piece.encode())));
            }
            values
        } else {
            appearance_values(&snapshot, piece_key, &source, piece)
        };
        created += 1;
        out.push(creator.add_values_at(ctx, piece_key, owner, values, order + created)?);
    }
    if keeper.is_none() {
        erase_entities(ctx, &[id])?;
    }
    Ok(out)
}

/// The identifiers of an object answer (a selection prompt), as text in order.
pub(crate) fn selection_answer(state: &PromptState<'_>, key: &str) -> Option<Vec<String>> {
    match state.answer(key)? {
        PromptInput::Selection(ids) => Some(ids.iter().map(ToString::to_string).collect()),
        _ => None,
    }
}

/// The point of a point answer, in the drawing's units.
pub(crate) fn point_answer(state: &PromptState<'_>, key: &str) -> Option<[f64; 2]> {
    match state.answer(key)? {
        PromptInput::Point(point) => {
            let p = drawing_point(state.snapshot, *point);
            Some([p.x, p.y])
        }
        _ => None,
    }
}

/// The distance of a distance answer, in the drawing's units.
pub(crate) fn distance_answer(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key)? {
        PromptInput::Distance(meters) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

/// The angle of an angle answer, in radians.
pub(crate) fn angle_answer(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key)? {
        PromptInput::Angle(radians) => Some(*radians),
        _ => None,
    }
}

/// The value of a dimensionless number answer.
pub(crate) fn number_answer(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key)? {
        PromptInput::Number(quantity) => Some(quantity.value()),
        _ => None,
    }
}

/// The keyword of a keyword answer.
pub(crate) fn keyword_answer<'a>(state: &'a PromptState<'_>, key: &str) -> Option<&'a str> {
    match state.answer(key)? {
        PromptInput::Keyword(id) => Some(id.as_str()),
        _ => None,
    }
}

/// The accept of a selection of entities: any entity type, at least one object.
pub(crate) fn entity_selection() -> Accept {
    Accept::Selection {
        min: 1,
        max: None,
        types: ENTITIES.iter().map(|k| (*k).to_owned()).collect(),
    }
}

/// The accept of a whole number of at least one, up to the largest count a command takes (copies, offsets).
pub(crate) fn count_accept() -> Accept {
    Accept::Number {
        dimension: arc_base::Dimension::NONE,
        min: Some(1.0),
        max: Some(f64::from(u32::MAX)),
        integer: true,
    }
}

/// The whole number of a count answer (see [`count_accept`]), when it is one.
pub(crate) fn count_answer(state: &PromptState<'_>, key: &str) -> Option<u32> {
    let value = number_answer(state, key)?;
    format!("{:.0}", value.round()).parse().ok()
}

/// The accept of a point in two dimensions.
pub(crate) fn point() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modify_codes_are_registered() {
        assert_eq!(arc_base::error::registry_problems(codes::ALL), vec![]);
    }
}
