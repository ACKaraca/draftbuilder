//! Object property commands (area `property`): changing the layer and the appearance of chosen
//! entities (`draftbuilder.property.change`, typed `CHPROP` and `CHANGE`), and copying the
//! appearance of one entity to others (`draftbuilder.property.match`, typed `MATCHPROP`).
//!
//! An appearance property is written with its source: a value sets it to ByObject, `by_layer` and
//! `by_block` set the source only. Each command writes the entity fields in its transaction and is
//! undone as one step. The foundation's type-specific properties (text style, dimension style,
//! hatch pattern and the like) are not copied; see the architecture overview.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{FieldPath, Snapshot, Value};
use arc_draft::types::{By, MAX_LINEWEIGHT_MM, POLYLINE, entity};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::draw::create::{accept_all, done, machine};
use crate::error::{Message, codes};
use crate::fold::fold;
use crate::layer::{self, ColorParam, Moved};
use crate::modify::{entity_selection, selection_answer};
use crate::select::candidates::{default_layer, reference};
use crate::vocabulary::{Area, register_names};

/// A source a property takes from its layer or its block, or a value of its own.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BySource {
    /// Takes the value of the layer.
    ByLayer,
    /// Takes the value of the block reference that inserts the entity.
    ByBlock,
}

/// An appearance setting: a source, or a value that the entity holds itself.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Setting<T> {
    /// ByLayer or ByBlock.
    By(BySource),
    /// The value, held by the entity (ByObject).
    Value(T),
}

/// Parameters of `draftbuilder.property.change` (`CHPROP`, `CHANGE`). Each `None` leaves the
/// property as it is; at least one property is required.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeProps {
    /// The entities to change.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The name of the layer the entities move to.
    #[serde(default)]
    #[schemars(length(max = 255))]
    pub layer: Option<String>,
    /// The index color, 1 to 255, a true color `[red, green, blue]`, or a source.
    #[serde(default)]
    pub color: Option<Setting<ColorParam>>,
    /// The name of an existing linetype, or a source.
    #[serde(default)]
    pub linetype: Option<Setting<String>>,
    /// The linetype scale, above zero.
    #[serde(default)]
    pub linetype_scale: Option<f64>,
    /// The lineweight in millimeters, 0 to 10, or a source.
    #[serde(default)]
    pub lineweight: Option<Setting<f64>>,
    /// The transparency, 0 to 1, or a source.
    #[serde(default)]
    pub transparency: Option<Setting<f64>>,
    /// Whether a polyline's dash pattern flows continuously along it (true) or restarts at each
    /// vertex (false). Only polylines take it; other entities keep their value.
    #[serde(default)]
    pub continuous_pattern: Option<bool>,
}

/// Parameters of `draftbuilder.property.match` (`MATCHPROP`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatchProps {
    /// The entity whose appearance is copied.
    pub source: IdParam,
    /// The entities that take it.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

/// The entity fields that hold the appearance and the layer, by name, in copy order.
const APPEARANCE: &[(usize, &str)] = &[
    (entity::LAYER, "layer"),
    (entity::COLOR_BY, "color_by"),
    (entity::COLOR, "color"),
    (entity::LINETYPE_BY, "linetype_by"),
    (entity::LINETYPE, "linetype"),
    (entity::LINETYPE_SCALE, "linetype_scale"),
    (entity::LINEWEIGHT_BY, "lineweight_by"),
    (entity::LINEWEIGHT, "lineweight"),
    (entity::TRANSPARENCY_BY, "transparency_by"),
    (entity::TRANSPARENCY, "transparency"),
    (entity::PLOT_STYLE, "plot_style"),
];

fn entities(snapshot: &Snapshot, objects: &[IdParam]) -> Result<Vec<ObjectId>> {
    objects
        .iter()
        .map(|IdParam(id)| {
            let handle = snapshot.resolve(*id)?;
            let is_entity = snapshot
                .type_key(handle)
                .is_some_and(|k| arc_draft::types::ENTITIES.contains(&k.as_str()));
            if is_entity {
                Ok(*id)
            } else {
                Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
                    .with_detail("id", id.to_string()))
            }
        })
        .collect()
}

fn named(
    snapshot: &Snapshot,
    key: &str,
    name: &str,
    code: arc_base::ErrorCode,
) -> Result<ObjectId> {
    snapshot
        .iter_type(key)
        .ok()
        .and_then(|objects| {
            objects
                .map(|(_, object)| object.id)
                .find(|id| name_of(snapshot, Some(*id)).is_some_and(|n| fold(&n) == fold(name)))
        })
        .ok_or_else(|| Error::new(code).with_detail("name", name))
}

/// The enum value of a source code.
fn by_value(by: By) -> Value {
    Value::Enum(by as u32)
}

/// A field write: the index of the entity field and the value, `None` for absent.
type Write = (usize, Option<Value>);

/// The writes of an appearance setting: its source, and the value when the entity holds one.
fn setting_writes<T>(
    setting: &Setting<T>,
    by_index: usize,
    own: impl FnOnce(&T) -> Result<Write>,
) -> Result<Vec<Write>> {
    match setting {
        Setting::By(BySource::ByLayer) => Ok(vec![(by_index, Some(by_value(By::Layer)))]),
        Setting::By(BySource::ByBlock) => Ok(vec![(by_index, Some(by_value(By::Block)))]),
        Setting::Value(value) => {
            let (index, field) = own(value)?;
            Ok(vec![(by_index, Some(by_value(By::Object))), (index, field)])
        }
    }
}

fn change(ctx: &mut Context<'_>, p: ChangeProps) -> Result<Moved> {
    let any = p.layer.is_some()
        || p.color.is_some()
        || p.linetype.is_some()
        || p.linetype_scale.is_some()
        || p.lineweight.is_some()
        || p.transparency.is_some()
        || p.continuous_pattern.is_some();
    if !any {
        return Err(Error::new(codes::NOTHING_TO_CHANGE));
    }
    let snapshot = ctx.snapshot();
    let objects = entities(snapshot, &p.objects)?;
    // The writes are computed before the transaction takes the context, so a bad value fails the
    // command before anything is written. An entity on a locked layer is not changed, and an
    // entity is not moved onto a locked layer (the foundation's rule, `arc_draft::lock::check_layer`).
    for id in &objects {
        arc_draft::lock::check_entity(snapshot, *id)?;
    }
    let mut writes: Vec<Write> = Vec::new();
    if let Some(name) = &p.layer {
        let target = named(
            snapshot,
            arc_draft::types::LAYER,
            name,
            codes::LAYER_UNKNOWN,
        )?;
        arc_draft::lock::check_layer(snapshot, target)?;
        writes.push((entity::LAYER, Some(Value::Reference(target))));
    }
    if let Some(setting) = &p.color {
        writes.extend(setting_writes(setting, entity::COLOR_BY, |color| {
            Ok((entity::COLOR, Some(layer::color_value(*color)?)))
        })?);
    }
    if let Some(setting) = &p.linetype {
        writes.extend(setting_writes(setting, entity::LINETYPE_BY, |name| {
            let target = layer::linetype_ref(snapshot, name)?;
            Ok((entity::LINETYPE, target.map(Value::Reference)))
        })?);
    }
    if let Some(scale) = p.linetype_scale {
        if !(scale.is_finite() && scale > 0.0) {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "linetype_scale"));
        }
        writes.push((entity::LINETYPE_SCALE, Some(Value::F64(scale))));
    }
    if let Some(setting) = &p.lineweight {
        writes.extend(setting_writes(setting, entity::LINEWEIGHT_BY, |mm| {
            if !(0.0..=MAX_LINEWEIGHT_MM).contains(mm) {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "lineweight"));
            }
            Ok((entity::LINEWEIGHT, Some(Value::F64(*mm))))
        })?);
    }
    if let Some(setting) = &p.transparency {
        writes.extend(setting_writes(setting, entity::TRANSPARENCY_BY, |value| {
            if !(0.0..=1.0).contains(value) {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "transparency"));
            }
            Ok((entity::TRANSPARENCY, Some(Value::F64(*value))))
        })?);
    }
    // The pattern flag applies to the polylines of the selection; the others are left as they are.
    let polylines: Vec<ObjectId> = match p.continuous_pattern {
        Some(_) => objects
            .iter()
            .copied()
            .filter(|id| type_key_of(snapshot, *id).as_deref() == Some(POLYLINE))
            .collect(),
        None => Vec::new(),
    };
    if p.continuous_pattern.is_some() && polylines.is_empty() {
        let id = objects
            .first()
            .map_or_else(String::new, ToString::to_string);
        return Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE).with_detail("id", id));
    }
    let txn = ctx.txn()?;
    for id in &objects {
        for (index, value) in &writes {
            txn.modify(*id, FieldPath::new(field_name(*index)), value.clone())?;
        }
    }
    if let Some(flag) = p.continuous_pattern {
        for id in &polylines {
            txn.modify(
                *id,
                FieldPath::new("continuous_pattern"),
                Some(Value::Bool(flag)),
            )?;
        }
    }
    Ok(Moved {
        objects: objects.into_iter().map(IdParam).collect(),
    })
}

/// The type key of object `id`, when it has one.
fn type_key_of(snapshot: &Snapshot, id: ObjectId) -> Option<String> {
    let handle = snapshot.resolve(id).ok()?;
    snapshot.type_key(handle).map(|key| key.as_str().to_owned())
}

/// Parameters of `draftbuilder.property.summary` (`PROPERTIES`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SummaryParams {
    /// The entities to summarize.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// Only the entities of one type: its type key (`core.draft.line`) or its last part (`line`),
    /// without regard to case.
    #[serde(default)]
    #[schemars(length(max = 255))]
    pub kind: Option<String>,
}

/// The properties of the summarized entities: those they all share, with their value, and those
/// that differ among them.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Summary {
    /// The entities summarized, in order, after the type filter.
    pub objects: Vec<IdParam>,
    /// The shared properties, by `category.name`, with their value as text.
    pub shared: BTreeMap<String, String>,
    /// The properties the entities do not share, by `category.name`.
    pub mixed: Vec<String>,
}

/// The value of a stored field as text: the name of a reference, the value of a number, the source
/// of a color, and the RGB components of a true color.
fn shown(snapshot: &Snapshot, value: Option<Value>) -> String {
    match value {
        None => "none".to_owned(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::F64(number)) => number.to_string(),
        Some(Value::String(text)) => text,
        Some(Value::Reference(id)) => name_of(snapshot, Some(id)).unwrap_or_else(|| id.to_string()),
        Some(Value::Enum(code)) => code.to_string(),
        Some(Value::Color(color)) => match color.source {
            Some(arc_doc::ColorSource::Indexed(index)) => format!("index {index}"),
            _ => format!(
                "rgb({:.3}, {:.3}, {:.3})",
                color.rgba.r, color.rgba.g, color.rgba.b
            ),
        },
        Some(_) => "set".to_owned(),
    }
}

/// An appearance property as text: its source when it is not held by the entity, else its value.
fn appearance(snapshot: &Snapshot, values: &[Option<Value>], by: usize, field: usize) -> String {
    let mode = match values.get(by) {
        Some(Some(Value::Enum(code))) => By::of(Some(*code)),
        _ => By::Layer,
    };
    match mode {
        By::Layer => "by_layer".to_owned(),
        By::Block => "by_block".to_owned(),
        By::Object => shown(snapshot, values.get(field).cloned().flatten()),
    }
}

/// The properties of entity `id` as `(category.name, text)` pairs, and its type key.
fn properties_of(
    snapshot: &Snapshot,
    id: ObjectId,
) -> Result<(String, Vec<(&'static str, String)>)> {
    let handle = snapshot.resolve(id)?;
    let stored = snapshot.get(handle)?;
    let kind = type_key_of(snapshot, id).unwrap_or_default();
    let layer_name = reference(snapshot, id, entity::LAYER)
        .or_else(|| default_layer(snapshot))
        .and_then(|l| name_of(snapshot, Some(l)))
        .unwrap_or_default();
    let v = &stored.values;
    let rows = vec![
        ("general.kind", kind.clone()),
        ("general.layer", layer_name),
        (
            "appearance.color",
            appearance(snapshot, v, entity::COLOR_BY, entity::COLOR),
        ),
        (
            "appearance.linetype",
            appearance(snapshot, v, entity::LINETYPE_BY, entity::LINETYPE),
        ),
        (
            "appearance.linetype_scale",
            shown(snapshot, v.get(entity::LINETYPE_SCALE).cloned().flatten()),
        ),
        (
            "appearance.lineweight",
            appearance(snapshot, v, entity::LINEWEIGHT_BY, entity::LINEWEIGHT),
        ),
        (
            "appearance.transparency",
            appearance(snapshot, v, entity::TRANSPARENCY_BY, entity::TRANSPARENCY),
        ),
        (
            "appearance.plot_style",
            shown(snapshot, v.get(entity::PLOT_STYLE).cloned().flatten()),
        ),
    ];
    Ok((kind, rows))
}

fn summary(ctx: &mut Context<'_>, p: SummaryParams) -> Result<Summary> {
    let snapshot = ctx.snapshot();
    let ids = entities(snapshot, &p.objects)?;
    let wanted = p.kind.as_deref().map(fold);
    let mut kept = Vec::new();
    let mut rows = Vec::new();
    for id in ids {
        let (kind, props) = properties_of(snapshot, id)?;
        if let Some(wanted) = &wanted {
            let short = kind.strip_prefix("core.draft.").unwrap_or(&kind);
            if fold(&kind) != *wanted && fold(short) != *wanted {
                continue;
            }
        }
        kept.push(IdParam(id));
        rows.push(props);
    }
    let mut shared = BTreeMap::new();
    let mut mixed = Vec::new();
    if let Some(first) = rows.first() {
        for (name, value) in first {
            let same = rows
                .iter()
                .all(|row| row.iter().any(|(n, v)| n == name && v == value));
            if same {
                shared.insert((*name).to_owned(), value.clone());
            } else {
                mixed.push((*name).to_owned());
            }
        }
    }
    Ok(Summary {
        objects: kept,
        shared,
        mixed,
    })
}

const SUMMARY_LABEL: Message = Message {
    message_id: "db-cmd-property-summary-label",
};
const SUMMARY_DESCRIPTION: Message = Message {
    message_id: "db-cmd-property-summary-description",
};

const SUMMARY_OBJECTS: Message = Message {
    message_id: "db-cmd-property-summary-objects",
};

fn summary_next(state: &arc_command::PromptState<'_>) -> Result<arc_command::Step> {
    match selection_answer(state, "objects") {
        Some(ids) => done(serde_json::json!({"objects": ids})),
        None => Ok(arc_command::Step::Ask(arc_command::Prompt::new(
            "objects",
            SUMMARY_OBJECTS.message_id,
            entity_selection(),
        ))),
    }
}

/// The name of an entity field, by index.
fn field_name(index: usize) -> &'static str {
    APPEARANCE
        .iter()
        .find(|(i, _)| *i == index)
        .map_or("layer", |(_, name)| name)
}

fn match_props(ctx: &mut Context<'_>, p: MatchProps) -> Result<Moved> {
    let snapshot = ctx.snapshot();
    let source = entities(snapshot, &[p.source])?
        .first()
        .copied()
        .ok_or_else(|| Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE))?;
    let objects = entities(snapshot, &p.objects)?;
    // The copy also copies the source's layer, which moves every destination onto it: a locked
    // source layer refuses the copy, as a move onto a locked layer is refused.
    if let Some(source_layer) = reference(snapshot, source, entity::LAYER) {
        arc_draft::lock::check_layer(snapshot, source_layer)?;
    }
    for id in &objects {
        arc_draft::lock::check_entity(snapshot, *id)?;
    }
    let handle = snapshot.resolve(source)?;
    let values = snapshot.get(handle)?.values.clone();
    let txn = ctx.txn()?;
    for id in &objects {
        for (index, name) in APPEARANCE {
            let value = values.get(*index).cloned().flatten();
            txn.modify(*id, FieldPath::new(*name), value)?;
        }
    }
    Ok(Moved {
        objects: objects.into_iter().map(IdParam).collect(),
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
    registry.register(spec(
        "draftbuilder.property.change",
        Message {
            message_id: "db-cmd-property-change-label",
        },
        Message {
            message_id: "db-cmd-property-change-description",
        },
        change,
    ))?;
    registry.register(spec(
        "draftbuilder.property.match",
        Message {
            message_id: "db-cmd-property-match-label",
        },
        Message {
            message_id: "db-cmd-property-match-description",
        },
        match_props,
    ))?;
    registry.register(
        CommandSpec::new(
            "draftbuilder.property.summary",
            SUMMARY_LABEL.message_id,
            SUMMARY_DESCRIPTION.message_id,
            summary,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read])
        .with_prompt(machine(summary_next, accept_all)),
    )?;
    register_names(registry, Area::Property)
}
