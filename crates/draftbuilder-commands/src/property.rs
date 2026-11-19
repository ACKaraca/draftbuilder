//! Object property commands (area `property`): changing the layer and the appearance of chosen
//! entities (`draftbuilder.property.change`, typed `CHPROP` and `CHANGE`), and copying the
//! appearance of one entity to others (`draftbuilder.property.match`, typed `MATCHPROP`).
//!
//! An appearance property is written with its source: a value sets it to ByObject, `by_layer` and
//! `by_block` set the source only. Each command writes the entity fields in its transaction and is
//! undone as one step. The foundation's type-specific properties (text style, dimension style,
//! hatch pattern and the like) are not copied; see the architecture overview.

use arc_base::color::indexed_rgb;
use arc_base::{Color, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{ColorSource, ColorValue, FieldPath, Snapshot, Value};
use arc_draft::types::{By, MAX_LINEWEIGHT_MM, entity};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, codes};
use crate::layer::{self, Moved, refuse_locked_entity, refuse_locked_layer};
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
    /// The index color, 1 to 255, or a source.
    #[serde(default)]
    pub color: Option<Setting<u8>>,
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
            objects.map(|(_, object)| object.id).find(|id| {
                name_of(snapshot, Some(*id)).is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
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
        || p.transparency.is_some();
    if !any {
        return Err(Error::new(codes::NOTHING_TO_CHANGE));
    }
    let snapshot = ctx.snapshot();
    let objects = entities(snapshot, &p.objects)?;
    // The writes are computed before the transaction takes the context, so a bad value fails the
    // command before anything is written. An entity on a locked layer is not changed, and an
    // entity is not moved onto a locked layer (the foundation's rule, see `layer::refuse_locked_layer`).
    for id in &objects {
        refuse_locked_entity(snapshot, *id)?;
    }
    let mut writes: Vec<Write> = Vec::new();
    if let Some(name) = &p.layer {
        let target = named(
            snapshot,
            arc_draft::types::LAYER,
            name,
            codes::LAYER_UNKNOWN,
        )?;
        refuse_locked_layer(snapshot, target)?;
        writes.push((entity::LAYER, Some(Value::Reference(target))));
    }
    if let Some(setting) = &p.color {
        writes.extend(setting_writes(setting, entity::COLOR_BY, |index| {
            if *index == 0 {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "color"));
            }
            let [r, g, b] = indexed_rgb(*index);
            let color = Color::new(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            );
            Ok((
                entity::COLOR,
                Some(Value::Color(Box::new(ColorValue {
                    rgba: color,
                    source: Some(ColorSource::Indexed(*index)),
                }))),
            ))
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
    let txn = ctx.txn()?;
    for id in &objects {
        for (index, value) in &writes {
            txn.modify(*id, FieldPath::new(field_name(*index)), value.clone())?;
        }
    }
    Ok(Moved {
        objects: objects.into_iter().map(IdParam).collect(),
    })
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
    for id in &objects {
        refuse_locked_entity(snapshot, *id)?;
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
    register_names(registry, Area::Property)
}
