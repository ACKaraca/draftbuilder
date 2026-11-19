//! Layer commands (area `layer`): creating a layer with its properties, switching the on, freeze
//! and lock states of the layers of chosen objects, moving objects to another layer, and changing
//! the properties of layers. Each command is a thin product command over the layer record of
//! `arc-draft` (`core.draft.layer`): it validates its typed parameters, writes the fields in the
//! command's transaction and is undone as one step.
//!
//! Rename, delete, delete with contents and merge follow the special-layer rules: layer `0` and the
//! definition-point layer cannot be renamed, deleted or merged away, and names are unique without
//! regard to case. Edits to objects on a locked layer are refused, as the foundation refuses them.
//!
//! Not built here, because the foundation has no record or rule for them: the current layer, layer
//! isolation, layer states, viewport overrides and layer walk.

use std::collections::BTreeSet;

use arc_base::color::indexed_rgb;
use arc_base::{Color, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{ColorSource, ColorValue, FieldPath, Header, Snapshot, Value};
use arc_draft::types::{self, ENTITIES, LAYER, LINETYPE, MAX_LINEWEIGHT_MM, entity, layer};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, codes};
use crate::select::candidates::{default_layer, flag, reference};
use crate::vocabulary::{Area, register_names};

/// The most transparency a layer takes: 90 percent (DB-LAYER-001).
pub const MAX_LAYER_TRANSPARENCY: f64 = 0.9;
/// The longest layer name, in characters.
pub const MAX_NAME_CHARS: usize = 255;
/// The characters a layer name may not contain: the reserved ones of the drawing format.
const RESERVED: &[char] = &['<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', '=', '`'];

/// Whether `name` is a layer name the drawing format accepts: one to 255 characters, none of them
/// reserved or a control character.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    let length = name.chars().count();
    (1..=MAX_NAME_CHARS).contains(&length)
        && !name
            .chars()
            .any(|c| RESERVED.contains(&c) || c.is_control())
}

/// The layer named `name`, compared without regard to case.
fn layer_named(snapshot: &Snapshot, name: &str) -> Result<ObjectId> {
    snapshot
        .iter_type(LAYER)
        .ok()
        .and_then(|layers| {
            layers.map(|(_, object)| object.id).find(|id| {
                name_of(snapshot, Some(*id)).is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
        })
        .ok_or_else(|| Error::new(codes::LAYER_UNKNOWN).with_detail("name", name))
}

/// The reference of the linetype named `name`, compared without regard to case. The continuous
/// linetype is the absent reference, which a drawing without a `Continuous` record also means
/// (the foundation's rule for an absent linetype).
pub(crate) fn linetype_ref(snapshot: &Snapshot, name: &str) -> Result<Option<ObjectId>> {
    let found = snapshot.iter_type(LINETYPE).ok().and_then(|types| {
        types
            .map(|(_, object)| object.id)
            .find(|id| name_of(snapshot, Some(*id)).is_some_and(|n| n.eq_ignore_ascii_case(name)))
    });
    match found {
        Some(id) => Ok(Some(id)),
        None if name.eq_ignore_ascii_case("Continuous") => Ok(None),
        None => Err(Error::new(codes::LINETYPE_UNKNOWN).with_detail("name", name)),
    }
}

/// The entity identifiers of `objects`, failing with `draft.object.wrong_type` for any object that
/// is not an entity.
fn entities(snapshot: &Snapshot, objects: &[IdParam]) -> Result<Vec<ObjectId>> {
    objects
        .iter()
        .map(|IdParam(id)| {
            let handle = snapshot.resolve(*id)?;
            let key = snapshot.type_key(handle).map(|k| k.as_str().to_owned());
            match key {
                Some(key) if ENTITIES.contains(&key.as_str()) => Ok(*id),
                _ => Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
                    .with_detail("id", id.to_string())),
            }
        })
        .collect()
}

/// The layer of an entity: its own, or layer `0` when it names none.
fn layer_of(snapshot: &Snapshot, id: ObjectId) -> Result<ObjectId> {
    reference(snapshot, id, entity::LAYER)
        .or_else(|| default_layer(snapshot))
        .ok_or_else(|| Error::new(codes::LAYER_UNKNOWN).with_detail("name", "0"))
}

/// The distinct layers of the entities `objects`, in their first-seen order.
fn layers_of(snapshot: &Snapshot, objects: &[IdParam]) -> Result<Vec<ObjectId>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in entities(snapshot, objects)? {
        let layer_id = layer_of(snapshot, id)?;
        if seen.insert(layer_id) {
            out.push(layer_id);
        }
    }
    Ok(out)
}

/// The color of an index color: the indexed palette entry, with its source kept.
fn indexed_color(index: u8) -> Value {
    let [r, g, b] = indexed_rgb(index);
    Value::Color(Box::new(ColorValue {
        rgba: Color::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        ),
        source: Some(ColorSource::Indexed(index)),
    }))
}

/// The index color parameter: 1 to 255.
fn index_color(index: u8) -> Result<u8> {
    if index == 0 {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "color"));
    }
    Ok(index)
}

/// Checks a lineweight in millimeters (0 to the foundation's maximum).
fn lineweight(mm: f64) -> Result<f64> {
    if (0.0..=MAX_LINEWEIGHT_MM).contains(&mm) {
        Ok(mm)
    } else {
        Err(Error::new(codes::VALUE_INVALID).with_detail("field", "lineweight"))
    }
}

/// Checks a transparency (0 to 90 percent for a layer).
fn transparency(value: f64) -> Result<f64> {
    if (0.0..=MAX_LAYER_TRANSPARENCY).contains(&value) {
        Ok(value)
    } else {
        Err(Error::new(codes::VALUE_INVALID).with_detail("field", "transparency"))
    }
}

/// The properties of a layer that a command may set. Each `None` leaves the property as it is.
#[derive(Debug, Default)]
struct Props {
    color: Option<u8>,
    linetype: Option<String>,
    lineweight: Option<f64>,
    transparency: Option<f64>,
    description: Option<String>,
    plottable: Option<bool>,
}

impl Props {
    /// The fields to write, as indices of the layer record and values, checked.
    fn fields(&self, snapshot: &Snapshot) -> Result<Vec<(usize, Option<Value>)>> {
        let mut out = Vec::new();
        if let Some(index) = self.color {
            out.push((layer::COLOR, Some(indexed_color(index_color(index)?))));
        }
        if let Some(name) = &self.linetype {
            let target = linetype_ref(snapshot, name)?;
            out.push((layer::LINETYPE, target.map(Value::Reference)));
        }
        if let Some(mm) = self.lineweight {
            out.push((layer::LINEWEIGHT, Some(Value::F64(lineweight(mm)?))));
        }
        if let Some(value) = self.transparency {
            out.push((layer::TRANSPARENCY, Some(Value::F64(transparency(value)?))));
        }
        if let Some(text) = &self.description {
            out.push((layer::DESCRIPTION, Some(Value::String(text.clone()))));
        }
        if let Some(flag) = self.plottable {
            out.push((layer::PLOTTABLE, Some(Value::Bool(flag))));
        }
        Ok(out)
    }

    fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.linetype.is_none()
            && self.lineweight.is_none()
            && self.transparency.is_none()
            && self.description.is_none()
            && self.plottable.is_none()
    }
}

/// The name of a layer field by its index.
fn field_name(index: usize) -> &'static str {
    match index {
        layer::COLOR => "color",
        layer::LINETYPE => "linetype",
        layer::LINEWEIGHT => "lineweight",
        layer::TRANSPARENCY => "transparency",
        layer::DESCRIPTION => "description",
        layer::PLOTTABLE => "plottable",
        layer::ON => "on",
        layer::FROZEN => "frozen",
        layer::LOCKED => "locked",
        _ => "name",
    }
}

/// Parameters of `draftbuilder.layer.create` (`LAYER`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLayer {
    /// The name, unique among layers; no reserved character, at most 255 characters.
    #[schemars(length(max = 255))]
    pub name: String,
    /// The index color, 1 to 255; the default white when absent.
    #[serde(default)]
    pub color: Option<u8>,
    /// The name of an existing linetype; continuous when absent.
    #[serde(default)]
    #[schemars(length(max = 255))]
    pub linetype: Option<String>,
    /// The lineweight in millimeters, 0 to 10; the default lineweight when absent.
    #[serde(default)]
    pub lineweight: Option<f64>,
    /// The transparency, 0 to 0.9.
    #[serde(default)]
    pub transparency: Option<f64>,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the layer plots; true when absent.
    #[serde(default)]
    pub plottable: Option<bool>,
}

/// What a layer command returns: the layers it created or changed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Layers {
    /// The layers, in their order.
    pub layers: Vec<IdParam>,
}

/// Parameters of the commands that set a flag of the layers of chosen objects.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Objects {
    /// The entities whose layers change.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

/// Parameters of `draftbuilder.layer.modify`: the layers and the properties to set.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModifyLayers {
    /// The layers to change.
    #[schemars(length(min = 1, max = 10000))]
    pub layers: Vec<IdParam>,
    /// Whether the layers are on.
    #[serde(default)]
    pub on: Option<bool>,
    /// Whether the layers are frozen.
    #[serde(default)]
    pub frozen: Option<bool>,
    /// Whether the layers are locked.
    #[serde(default)]
    pub locked: Option<bool>,
    /// Whether the layers plot.
    #[serde(default)]
    pub plottable: Option<bool>,
    /// The index color, 1 to 255.
    #[serde(default)]
    pub color: Option<u8>,
    /// The name of an existing linetype.
    #[serde(default)]
    #[schemars(length(max = 255))]
    pub linetype: Option<String>,
    /// The lineweight in millimeters, 0 to 10.
    #[serde(default)]
    pub lineweight: Option<f64>,
    /// The transparency, 0 to 0.9.
    #[serde(default)]
    pub transparency: Option<f64>,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
}

/// Parameters of `draftbuilder.layer.change`: moves the objects to the named layer.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeLayer {
    /// The entities to move.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The name of the layer.
    #[schemars(length(max = 255))]
    pub layer: String,
}

/// Parameters of `draftbuilder.layer.match`: moves the objects to the layer of `source`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatchLayer {
    /// The entities to move.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The entity whose layer the objects take.
    pub source: IdParam,
}

/// What an object command returns: the layers it touched, or the entities it moved.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Moved {
    /// The entities that changed.
    pub objects: Vec<IdParam>,
}

fn create(ctx: &mut Context<'_>, p: CreateLayer) -> Result<Layers> {
    if !valid_name(&p.name) {
        return Err(Error::new(codes::NAME_INVALID).with_detail("name", p.name));
    }
    if name_taken(ctx.snapshot(), &p.name, None) {
        return Err(Error::new(codes::NAME_TAKEN).with_detail("name", p.name));
    }
    let props = Props {
        color: p.color,
        linetype: p.linetype,
        lineweight: p.lineweight,
        transparency: p.transparency,
        description: p.description,
        plottable: p.plottable,
    };
    let fields = props.fields(ctx.snapshot())?;
    let mut values = types::defaults(ctx.snapshot().registry(), LAYER);
    if let Some(slot) = values.get_mut(layer::NAME) {
        *slot = Some(Value::String(p.name));
    }
    for (index, value) in fields {
        if let Some(slot) = values.get_mut(index) {
            *slot = value;
        }
    }
    let id: ObjectId = ctx.new_id();
    let header = arc_doc::Header::default();
    ctx.txn()?.create(LAYER, id, header, values)?;
    Ok(Layers {
        layers: vec![IdParam(id)],
    })
}

/// Sets one flag of the layers of `objects` to `value`.
fn set_flag(
    ctx: &mut Context<'_>,
    objects: &[IdParam],
    index: usize,
    value: bool,
) -> Result<Layers> {
    let layers = layers_of(ctx.snapshot(), objects)?;
    let txn = ctx.txn()?;
    for id in &layers {
        txn.modify(
            *id,
            FieldPath::new(field_name(index)),
            Some(Value::Bool(value)),
        )?;
    }
    Ok(Layers {
        layers: layers.into_iter().map(IdParam).collect(),
    })
}

fn modify(ctx: &mut Context<'_>, p: ModifyLayers) -> Result<Layers> {
    let props = Props {
        color: p.color,
        linetype: p.linetype,
        lineweight: p.lineweight,
        transparency: p.transparency,
        description: p.description,
        plottable: p.plottable,
    };
    let flags: Vec<(usize, bool)> = [
        (layer::ON, p.on),
        (layer::FROZEN, p.frozen),
        (layer::LOCKED, p.locked),
    ]
    .into_iter()
    .filter_map(|(index, flag)| flag.map(|f| (index, f)))
    .collect();
    if props.is_empty() && flags.is_empty() {
        return Err(Error::new(codes::NOTHING_TO_CHANGE));
    }
    let mut writes = props.fields(ctx.snapshot())?;
    writes.extend(flags.into_iter().map(|(i, f)| (i, Some(Value::Bool(f)))));
    let mut targets = Vec::new();
    for IdParam(id) in &p.layers {
        let handle = ctx.snapshot().resolve(*id)?;
        let key = ctx
            .snapshot()
            .type_key(handle)
            .map(|k| k.as_str().to_owned());
        if key.as_deref() != Some(LAYER) {
            return Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
                .with_detail("id", id.to_string()));
        }
        targets.push(*id);
    }
    let txn = ctx.txn()?;
    for id in &targets {
        for (index, value) in &writes {
            txn.modify(*id, FieldPath::new(field_name(*index)), value.clone())?;
        }
    }
    Ok(Layers {
        layers: targets.into_iter().map(IdParam).collect(),
    })
}

fn change(ctx: &mut Context<'_>, p: ChangeLayer) -> Result<Moved> {
    let snapshot = ctx.snapshot();
    let target = layer_named(snapshot, &p.layer)?;
    let objects = entities(snapshot, &p.objects)?;
    refuse_locked_layer(snapshot, target)?;
    for id in &objects {
        refuse_locked_entity(snapshot, *id)?;
    }
    let txn = ctx.txn()?;
    for id in &objects {
        txn.modify(*id, FieldPath::new("layer"), Some(Value::Reference(target)))?;
    }
    Ok(Moved {
        objects: objects.into_iter().map(IdParam).collect(),
    })
}

fn match_layer(ctx: &mut Context<'_>, p: MatchLayer) -> Result<Moved> {
    let source = entities(ctx.snapshot(), &[p.source])?
        .first()
        .copied()
        .ok_or_else(|| Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE))?;
    let snapshot = ctx.snapshot();
    let source_layer = layer_of(snapshot, source)?;
    let objects = entities(snapshot, &p.objects)?;
    refuse_locked_layer(snapshot, source_layer)?;
    for id in &objects {
        refuse_locked_entity(snapshot, *id)?;
    }
    let txn = ctx.txn()?;
    for id in &objects {
        txn.modify(
            *id,
            FieldPath::new("layer"),
            Some(Value::Reference(source_layer)),
        )?;
    }
    Ok(Moved {
        objects: objects.into_iter().map(IdParam).collect(),
    })
}

/// The layer names the drawing treats as special (DB-LAYER-005): layer `0` and the definition-point
/// layer that dimensions use. Such a layer cannot be renamed, deleted or merged away.
const SPECIAL: &[&str] = &["0", "Defpoints"];

/// The form in which two layer names are compared: the standard library's full lowercase mapping,
/// the Unicode case folding the product applies to names. Two names are the same when their forms
/// are equal.
fn fold(name: &str) -> String {
    name.to_lowercase()
}

fn is_special(name: &str) -> bool {
    SPECIAL.iter().any(|special| fold(special) == fold(name))
}

/// Whether a layer other than `except` has the name `name`, compared by [`fold`].
fn name_taken(snapshot: &Snapshot, name: &str, except: Option<ObjectId>) -> bool {
    let Ok(mut layers) = snapshot.iter_type(LAYER) else {
        return false;
    };
    layers.any(|(_, object)| {
        Some(object.id) != except
            && name_of(snapshot, Some(object.id)).is_some_and(|n| fold(&n) == fold(name))
    })
}

/// The layer record `id`, failing with `draft.object.wrong_type` for any other object.
fn layer_record(snapshot: &Snapshot, id: ObjectId) -> Result<ObjectId> {
    let handle = snapshot.resolve(id)?;
    match snapshot.type_key(handle) {
        Some(key) if key.as_str() == LAYER => Ok(id),
        _ => Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
            .with_detail("id", id.to_string())),
    }
}

/// The distinct layer records of `list`, in their order.
fn layer_records(snapshot: &Snapshot, list: &[IdParam]) -> Result<Vec<ObjectId>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for IdParam(id) in list {
        let id = layer_record(snapshot, *id)?;
        if seen.insert(id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Fails with `db.cmd.layer_special` when the layer is layer `0` or the definition-point layer.
fn refuse_special(snapshot: &Snapshot, id: ObjectId) -> Result<()> {
    let name = name_of(snapshot, Some(id)).unwrap_or_default();
    if is_special(&name) {
        return Err(Error::new(codes::LAYER_SPECIAL).with_detail("name", name));
    }
    Ok(())
}

/// Fails with the foundation's `draft.layer.locked`, naming the layer, when the layer is locked.
/// The foundation applies this rule in its own commands; the product applies it to the entities
/// its commands write directly, because the foundation's check is not public.
pub(crate) fn refuse_locked_layer(snapshot: &Snapshot, layer_id: ObjectId) -> Result<()> {
    if flag(snapshot, layer_id, layer::LOCKED) != Some(true) {
        return Ok(());
    }
    let name = name_of(snapshot, Some(layer_id)).unwrap_or_else(|| layer_id.to_string());
    Err(Error::new(arc_draft::error::codes::LAYER_LOCKED).with_arg("layer", name))
}

/// Fails when the entity's own layer is locked (an entity that names no layer passes, as in the
/// foundation).
pub(crate) fn refuse_locked_entity(snapshot: &Snapshot, id: ObjectId) -> Result<()> {
    match reference(snapshot, id, entity::LAYER) {
        Some(layer_id) => refuse_locked_layer(snapshot, layer_id),
        None => Ok(()),
    }
}

/// The entities whose layer is `layer_id`, in any container (block definitions included).
fn contents(snapshot: &Snapshot, layer_id: ObjectId) -> Vec<ObjectId> {
    let mut out = BTreeSet::new();
    for referrer in snapshot.referrers(layer_id) {
        let Ok(handle) = snapshot.resolve(referrer.source) else {
            continue;
        };
        let is_entity = snapshot
            .type_key(handle)
            .is_some_and(|key| ENTITIES.contains(&key.as_str()));
        if is_entity {
            out.insert(referrer.source);
        }
    }
    out.into_iter().collect()
}

/// Creates layer `0` when the drawing has none (the reserved record of DB-LAYER-005) and says so.
/// A drawing that has one keeps it.
///
/// # Errors
///
/// The errors of the transaction.
pub(crate) fn ensure_layer_zero(ctx: &mut Context<'_>) -> Result<bool> {
    if name_taken(ctx.snapshot(), "0", None) {
        return Ok(false);
    }
    let mut values = types::defaults(ctx.snapshot().registry(), LAYER);
    if let Some(slot) = values.get_mut(layer::NAME) {
        *slot = Some(Value::String("0".to_owned()));
    }
    let id: ObjectId = ctx.new_id();
    ctx.txn()?.create(LAYER, id, Header::default(), values)?;
    Ok(true)
}

/// Parameters of `draftbuilder.layer.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameLayer {
    /// The layer to rename; layer `0` and the definition-point layer cannot be renamed.
    pub layer: IdParam,
    /// The new name: 1 to 255 characters, no reserved character, not the name of another layer
    /// (compared without regard to case).
    #[schemars(length(max = 255))]
    pub name: String,
}

/// Parameters of the layer commands that act on whole layers: delete, delete with contents.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerList {
    /// The layers.
    #[schemars(length(min = 1, max = 10000))]
    pub layers: Vec<IdParam>,
}

/// Parameters of `draftbuilder.layer.merge`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeLayers {
    /// The layers whose objects move to `target`; each is removed afterwards.
    #[schemars(length(min = 1, max = 10000))]
    pub sources: Vec<IdParam>,
    /// The layer that takes the objects.
    pub target: IdParam,
}

/// What `draftbuilder.layer.delete_with_contents` returns: the objects and layers it removed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Removed {
    /// The entities removed, including those inside block definitions.
    pub objects: Vec<IdParam>,
    /// The layers removed.
    pub layers: Vec<IdParam>,
}

fn rename(ctx: &mut Context<'_>, p: RenameLayer) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let id = layer_record(snapshot, p.layer.0)?;
    refuse_special(snapshot, id)?;
    if !valid_name(&p.name) {
        return Err(Error::new(codes::NAME_INVALID).with_detail("name", p.name));
    }
    if name_taken(snapshot, &p.name, Some(id)) {
        return Err(Error::new(codes::NAME_TAKEN).with_detail("name", p.name));
    }
    ctx.txn()?
        .modify(id, FieldPath::new("name"), Some(Value::String(p.name)))?;
    Ok(Layers {
        layers: vec![IdParam(id)],
    })
}

fn delete(ctx: &mut Context<'_>, p: LayerList) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let targets = layer_records(snapshot, &p.layers)?;
    for id in &targets {
        refuse_special(snapshot, *id)?;
        if !contents(snapshot, *id).is_empty() {
            return Err(Error::new(codes::LAYER_IN_USE)
                .with_detail("name", name_of(snapshot, Some(*id)).unwrap_or_default()));
        }
    }
    let txn = ctx.txn()?;
    for id in &targets {
        txn.delete(*id)?;
    }
    Ok(Layers {
        layers: targets.into_iter().map(IdParam).collect(),
    })
}

fn delete_with_contents(ctx: &mut Context<'_>, p: LayerList) -> Result<Removed> {
    let snapshot = ctx.snapshot();
    let targets = layer_records(snapshot, &p.layers)?;
    let mut objects = Vec::new();
    for id in &targets {
        refuse_special(snapshot, *id)?;
        let found = contents(snapshot, *id);
        if !found.is_empty() {
            refuse_locked_layer(snapshot, *id)?;
        }
        objects.extend(found);
    }
    let txn = ctx.txn()?;
    for id in &objects {
        txn.delete(*id)?;
    }
    for id in &targets {
        txn.delete(*id)?;
    }
    Ok(Removed {
        objects: objects.into_iter().map(IdParam).collect(),
        layers: targets.into_iter().map(IdParam).collect(),
    })
}

fn merge(ctx: &mut Context<'_>, p: MergeLayers) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let target = layer_record(snapshot, p.target.0)?;
    let sources = layer_records(snapshot, &p.sources)?;
    if sources.contains(&target) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "target"));
    }
    let mut moved = Vec::new();
    for source in &sources {
        refuse_special(snapshot, *source)?;
        let found = contents(snapshot, *source);
        if !found.is_empty() {
            refuse_locked_layer(snapshot, *source)?;
            refuse_locked_layer(snapshot, target)?;
        }
        moved.extend(found);
    }
    let txn = ctx.txn()?;
    for id in &moved {
        txn.modify(*id, FieldPath::new("layer"), Some(Value::Reference(target)))?;
    }
    for source in &sources {
        txn.delete(*source)?;
    }
    Ok(Layers {
        layers: vec![IdParam(target)],
    })
}

fn spec<P, R>(
    id: &'static str,
    label: Message,
    description: Message,
    run: impl Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
) -> CommandSpec
where
    P: serde::de::DeserializeOwned + JsonSchema,
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
    for spec in record_specs().into_iter().chain(flag_specs()) {
        registry.register(spec)?;
    }
    register_names(registry, Area::Layer)
}

/// The commands that create and change layer records and objects.
fn record_specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "draftbuilder.layer.create",
            Message {
                message_id: "db-cmd-layer-create-label",
            },
            Message {
                message_id: "db-cmd-layer-create-description",
            },
            create,
        ),
        spec(
            "draftbuilder.layer.modify",
            Message {
                message_id: "db-cmd-layer-modify-label",
            },
            Message {
                message_id: "db-cmd-layer-modify-description",
            },
            modify,
        ),
        spec(
            "draftbuilder.layer.change",
            Message {
                message_id: "db-cmd-layer-change-label",
            },
            Message {
                message_id: "db-cmd-layer-change-description",
            },
            change,
        ),
        spec(
            "draftbuilder.layer.match",
            Message {
                message_id: "db-cmd-layer-match-label",
            },
            Message {
                message_id: "db-cmd-layer-match-description",
            },
            match_layer,
        ),
        spec(
            "draftbuilder.layer.rename",
            Message {
                message_id: "db-cmd-layer-rename-label",
            },
            Message {
                message_id: "db-cmd-layer-rename-description",
            },
            rename,
        ),
        spec(
            "draftbuilder.layer.delete",
            Message {
                message_id: "db-cmd-layer-delete-label",
            },
            Message {
                message_id: "db-cmd-layer-delete-description",
            },
            delete,
        ),
        spec(
            "draftbuilder.layer.delete_with_contents",
            Message {
                message_id: "db-cmd-layer-delete-with-contents-label",
            },
            Message {
                message_id: "db-cmd-layer-delete-with-contents-description",
            },
            delete_with_contents,
        ),
        spec(
            "draftbuilder.layer.merge",
            Message {
                message_id: "db-cmd-layer-merge-label",
            },
            Message {
                message_id: "db-cmd-layer-merge-description",
            },
            merge,
        ),
    ]
}

/// The commands that turn the flags of the layers of chosen objects on or off.
fn flag_specs() -> Vec<CommandSpec> {
    let flag = |index: usize, value: bool| {
        move |ctx: &mut Context<'_>, p: Objects| set_flag(ctx, &p.objects, index, value)
    };
    vec![
        spec(
            "draftbuilder.layer.on",
            Message {
                message_id: "db-cmd-layer-on-label",
            },
            Message {
                message_id: "db-cmd-layer-on-description",
            },
            flag(layer::ON, true),
        ),
        spec(
            "draftbuilder.layer.off",
            Message {
                message_id: "db-cmd-layer-off-label",
            },
            Message {
                message_id: "db-cmd-layer-off-description",
            },
            flag(layer::ON, false),
        ),
        spec(
            "draftbuilder.layer.freeze",
            Message {
                message_id: "db-cmd-layer-freeze-label",
            },
            Message {
                message_id: "db-cmd-layer-freeze-description",
            },
            flag(layer::FROZEN, true),
        ),
        spec(
            "draftbuilder.layer.thaw",
            Message {
                message_id: "db-cmd-layer-thaw-label",
            },
            Message {
                message_id: "db-cmd-layer-thaw-description",
            },
            flag(layer::FROZEN, false),
        ),
        spec(
            "draftbuilder.layer.lock",
            Message {
                message_id: "db-cmd-layer-lock-label",
            },
            Message {
                message_id: "db-cmd-layer-lock-description",
            },
            flag(layer::LOCKED, true),
        ),
        spec(
            "draftbuilder.layer.unlock",
            Message {
                message_id: "db-cmd-layer-unlock-label",
            },
            Message {
                message_id: "db-cmd-layer-unlock-description",
            },
            flag(layer::LOCKED, false),
        ),
    ]
}
