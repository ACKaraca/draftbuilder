//! Viewport layer overrides (DB-LAYER-020, 022, 023): freezing layers in layout viewports (`LAYVPI`,
//! `VPLAYER`) and replacing or clearing a layer's color, linetype, lineweight or transparency inside
//! one viewport. The overrides are the viewport's `layer_overrides` records of the foundation: each
//! record holds the layer, its frozen flag and its appearance fields. A record with nothing left is
//! removed. Freezing never freezes the current layer, as the layer commands do.
//!
//! Items of the list that are not layer records of six fields are kept as they are: a change to one
//! layer's override never drops them.
//!
//! Not built here: the freeze default for new viewports (the foundation's layer record has no such
//! flag), plot-style overrides (the record has no such field) and the display toggle of overrides.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::{Error, ObjectId, Result};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::{FieldPath, Snapshot, Value};
use arc_draft::layout::{VIEWPORT, viewport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ColorParam, color_value, layer_named, layers_of, linetype_ref, lineweight, spec};
use crate::draw::create::{accept_all, done, machine};
use crate::error::{Message, codes};
use crate::modify::{entity_selection, keyword_answer, selection_answer};

/// The fields of one override record, in the foundation's order.
const FIELDS: usize = 6;
const F_FROZEN: usize = 1;
const F_COLOR: usize = 2;
const F_LINETYPE: usize = 3;
const F_LINEWEIGHT: usize = 4;
const F_TRANSPARENCY: usize = 5;

/// One override record: the layer and its fields.
type Record = Vec<Option<Value>>;

/// The override list of one viewport: its records by layer, and the items that are not records of
/// that shape, kept unchanged.
#[derive(Debug, Clone, Default, PartialEq)]
struct Overrides {
    records: BTreeMap<ObjectId, Record>,
    other: Vec<Value>,
}

/// The viewports of `list`, in order, each checked to be a viewport.
fn viewport_ids(snapshot: &Snapshot, list: &[IdParam]) -> Result<Vec<ObjectId>> {
    let mut out: Vec<ObjectId> = Vec::new();
    for IdParam(id) in list {
        let handle = snapshot.resolve(*id)?;
        let is_viewport = snapshot
            .type_key(handle)
            .is_some_and(|key| key.as_str() == VIEWPORT);
        if !is_viewport {
            return Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
                .with_detail("id", id.to_string()));
        }
        if !out.contains(id) {
            out.push(*id);
        }
    }
    Ok(out)
}

/// Every viewport of the drawing, in any layout.
fn all_viewports(snapshot: &Snapshot) -> Vec<ObjectId> {
    snapshot
        .iter_type(VIEWPORT)
        .ok()
        .map(|viewports| viewports.map(|(_, object)| object.id).collect())
        .unwrap_or_default()
}

/// The override list of viewport `id`.
fn overrides(snapshot: &Snapshot, id: ObjectId) -> Result<Overrides> {
    let handle = snapshot.resolve(id)?;
    let values = &snapshot.get(handle)?.values;
    let mut out = Overrides::default();
    if let Some(Some(Value::List(items))) = values.get(viewport::LAYER_OVERRIDES) {
        for item in items {
            match item {
                Value::Record(fields) if fields.len() == FIELDS => match fields.first() {
                    Some(Some(Value::Reference(layer))) => {
                        out.records.insert(*layer, fields.clone());
                    }
                    _ => out.other.push(item.clone()),
                },
                _ => out.other.push(item.clone()),
            }
        }
    }
    Ok(out)
}

/// Whether a record changes nothing: not frozen and no appearance field.
fn is_empty(record: &Record) -> bool {
    !matches!(record.get(F_FROZEN), Some(Some(Value::Bool(true))))
        && match record.get(F_COLOR..) {
            Some(appearance) => appearance.iter().all(Option::is_none),
            None => true,
        }
}

/// Sets one field of the record of `layer`, creating the record (thawed, no overrides) if needed.
fn set_field(map: &mut Overrides, layer: ObjectId, index: usize, value: Option<Value>) {
    let record = map.records.entry(layer).or_insert_with(|| {
        vec![
            Some(Value::Reference(layer)),
            Some(Value::Bool(false)),
            None,
            None,
            None,
            None,
        ]
    });
    if let Some(slot) = record.get_mut(index) {
        *slot = value;
    }
}

/// Writes `map` as the overrides of viewport `id` when it differs from `before`. Empty records are
/// dropped first, so a viewport keeps no record that changes nothing.
fn commit(
    txn: &mut arc_doc::Txn,
    id: ObjectId,
    mut before: Overrides,
    mut map: Overrides,
) -> Result<()> {
    before.records.retain(|_, record| !is_empty(record));
    map.records.retain(|_, record| !is_empty(record));
    if before == map {
        return Ok(());
    }
    let list = list_of(map);
    txn.modify(
        id,
        FieldPath::new("layer_overrides"),
        Some(Value::List(list)),
    )
}

/// The override list that `map` stands for: its items that are not records first, then its records
/// by layer. Items that are not records are never dropped.
fn list_of(map: Overrides) -> Vec<Value> {
    map.other
        .into_iter()
        .chain(map.records.into_values().map(Value::Record))
        .collect()
}

/// What the viewport commands return: the viewports they changed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Viewports {
    /// The viewports, in order.
    pub viewports: Vec<IdParam>,
}

/// Parameters of `draftbuilder.layer.viewport_isolate` (`LAYVPI`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolateViewportParams {
    /// The entities whose layers are frozen in every other viewport.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The current viewport, which keeps the layers visible.
    pub viewport: IdParam,
}

/// Parameters of `draftbuilder.layer.viewport_freeze` (`VPLAYER`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FreezeParams {
    /// `freeze` or `thaw` the named layers in the viewports; `reset` returns every layer of the
    /// viewports to the global setting (it takes no layer names).
    pub action: FreezeAction,
    /// The viewports to change.
    #[schemars(length(min = 1, max = 10000))]
    pub viewports: Vec<IdParam>,
    /// The layers, by name, for `freeze` and `thaw`.
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub layers: Vec<String>,
}

/// What `draftbuilder.layer.viewport_freeze` does.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FreezeAction {
    /// Freezes the named layers in the viewports.
    Freeze,
    /// Thaws the named layers in the viewports.
    Thaw,
    /// Clears every layer's freeze in the viewports.
    Reset,
}

/// Parameters of `draftbuilder.layer.viewport_override`: replaces properties of layers in viewports.
/// Each property not given keeps its value. The linetype must be a named linetype: continuous
/// cannot be an override.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverrideParams {
    /// The viewports to change.
    #[schemars(length(min = 1, max = 10000))]
    pub viewports: Vec<IdParam>,
    /// The layers, by name.
    #[schemars(length(min = 1, max = 10000))]
    pub layers: Vec<String>,
    /// The index color (1 to 255) or the true color `[red, green, blue]`.
    #[serde(default)]
    pub color: Option<ColorParam>,
    /// The name of an existing linetype.
    #[serde(default)]
    #[schemars(length(max = 255))]
    pub linetype: Option<String>,
    /// The lineweight in millimeters, 0 to 10.
    #[serde(default)]
    pub lineweight: Option<f64>,
    /// The transparency, 0 to 1.
    #[serde(default)]
    pub transparency: Option<f64>,
}

/// Parameters of `draftbuilder.layer.viewport_clear`: removes appearance overrides, keeping the
/// freeze. Without layer names every layer's overrides of the viewports are removed.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClearParams {
    /// The viewports to change.
    #[schemars(length(min = 1, max = 10000))]
    pub viewports: Vec<IdParam>,
    /// The layers, by name; empty for every layer.
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub layers: Vec<String>,
}

fn isolate_viewport(ctx: &mut Context<'_>, p: IsolateViewportParams) -> Result<Viewports> {
    let snapshot = ctx.snapshot();
    let layers = layers_of(snapshot, &p.objects)?;
    let keep = viewport_ids(snapshot, &[p.viewport])?;
    for layer in &layers {
        crate::settings::refuse_current(snapshot, *layer)?;
    }
    let targets: Vec<ObjectId> = all_viewports(snapshot)
        .into_iter()
        .filter(|id| !keep.contains(id))
        .collect();
    let mut edits = Vec::new();
    for id in &targets {
        let before = overrides(snapshot, *id)?;
        let mut map = before.clone();
        for layer in &layers {
            set_field(&mut map, *layer, F_FROZEN, Some(Value::Bool(true)));
        }
        edits.push((*id, before, map));
    }
    let txn = ctx.txn()?;
    for (id, before, map) in edits {
        commit(txn, id, before, map)?;
    }
    Ok(Viewports {
        viewports: targets.into_iter().map(IdParam).collect(),
    })
}

fn freeze(ctx: &mut Context<'_>, p: FreezeParams) -> Result<Viewports> {
    let snapshot = ctx.snapshot();
    let viewports = viewport_ids(snapshot, &p.viewports)?;
    let layers = match p.action {
        FreezeAction::Reset => {
            if !p.layers.is_empty() {
                return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "layers"));
            }
            Vec::new()
        }
        FreezeAction::Freeze | FreezeAction::Thaw => {
            if p.layers.is_empty() {
                return Err(Error::new(codes::NOTHING_TO_CHANGE));
            }
            let mut ids = Vec::new();
            for name in &p.layers {
                ids.push(layer_named(snapshot, name)?);
            }
            ids
        }
    };
    if p.action == FreezeAction::Freeze {
        for layer in &layers {
            crate::settings::refuse_current(snapshot, *layer)?;
        }
    }
    let mut edits = Vec::new();
    for id in &viewports {
        let before = overrides(snapshot, *id)?;
        let mut map = before.clone();
        match p.action {
            FreezeAction::Freeze | FreezeAction::Thaw => {
                let frozen = p.action == FreezeAction::Freeze;
                for layer in &layers {
                    set_field(&mut map, *layer, F_FROZEN, Some(Value::Bool(frozen)));
                }
            }
            FreezeAction::Reset => {
                for record in map.records.values_mut() {
                    if let Some(slot) = record.get_mut(F_FROZEN) {
                        *slot = Some(Value::Bool(false));
                    }
                }
            }
        }
        edits.push((*id, before, map));
    }
    let txn = ctx.txn()?;
    for (id, before, map) in edits {
        commit(txn, id, before, map)?;
    }
    Ok(Viewports {
        viewports: viewports.into_iter().map(IdParam).collect(),
    })
}

/// The field writes of an override: the record index and the value, checked.
fn override_writes(snapshot: &Snapshot, p: &OverrideParams) -> Result<Vec<(usize, Value)>> {
    let mut writes = Vec::new();
    if let Some(color) = p.color {
        writes.push((F_COLOR, color_value(color)?));
    }
    if let Some(name) = &p.linetype {
        match linetype_ref(snapshot, name)? {
            Some(id) => writes.push((F_LINETYPE, Value::Reference(id))),
            None => {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "linetype"));
            }
        }
    }
    if let Some(mm) = p.lineweight {
        writes.push((F_LINEWEIGHT, Value::F64(lineweight(mm)?)));
    }
    if let Some(value) = p.transparency {
        if !(0.0..=1.0).contains(&value) {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "transparency"));
        }
        writes.push((F_TRANSPARENCY, Value::F64(value)));
    }
    if writes.is_empty() {
        return Err(Error::new(codes::NOTHING_TO_CHANGE));
    }
    Ok(writes)
}

fn override_props(ctx: &mut Context<'_>, p: OverrideParams) -> Result<Viewports> {
    let snapshot = ctx.snapshot();
    let writes = override_writes(snapshot, &p)?;
    let viewports = viewport_ids(snapshot, &p.viewports)?;
    let mut layers = Vec::new();
    for name in &p.layers {
        layers.push(layer_named(snapshot, name)?);
    }
    let mut edits = Vec::new();
    for id in &viewports {
        let before = overrides(snapshot, *id)?;
        let mut map = before.clone();
        for layer in &layers {
            for (index, value) in &writes {
                set_field(&mut map, *layer, *index, Some(value.clone()));
            }
        }
        edits.push((*id, before, map));
    }
    let txn = ctx.txn()?;
    for (id, before, map) in edits {
        commit(txn, id, before, map)?;
    }
    Ok(Viewports {
        viewports: viewports.into_iter().map(IdParam).collect(),
    })
}

fn clear_props(ctx: &mut Context<'_>, p: ClearParams) -> Result<Viewports> {
    let snapshot = ctx.snapshot();
    let viewports = viewport_ids(snapshot, &p.viewports)?;
    let mut only = BTreeSet::new();
    for name in &p.layers {
        only.insert(layer_named(snapshot, name)?);
    }
    let mut edits = Vec::new();
    for id in &viewports {
        let before = overrides(snapshot, *id)?;
        let mut map = before.clone();
        for (layer, record) in map.records.iter_mut() {
            if !(only.is_empty() || only.contains(layer)) {
                continue;
            }
            if let Some(appearance) = record.get_mut(F_COLOR..) {
                for slot in appearance {
                    *slot = None;
                }
            }
        }
        edits.push((*id, before, map));
    }
    let txn = ctx.txn()?;
    for (id, before, map) in edits {
        commit(txn, id, before, map)?;
    }
    Ok(Viewports {
        viewports: viewports.into_iter().map(IdParam).collect(),
    })
}

const ISOLATE_VIEWPORT_LABEL: Message = Message {
    message_id: "db-cmd-layer-viewport-isolate-label",
};
const ISOLATE_VIEWPORT_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-viewport-isolate-description",
};
const FREEZE_LABEL: Message = Message {
    message_id: "db-cmd-layer-viewport-freeze-label",
};
const FREEZE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-viewport-freeze-description",
};
const OVERRIDE_LABEL: Message = Message {
    message_id: "db-cmd-layer-viewport-override-label",
};
const OVERRIDE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-viewport-override-description",
};
const CLEAR_LABEL: Message = Message {
    message_id: "db-cmd-layer-viewport-clear-label",
};
const CLEAR_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-viewport-clear-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-layer-isolate-objects",
};
const VIEWPORT_PICK: Message = Message {
    message_id: "db-cmd-layer-viewport-pick",
};
const ACTION: Message = Message {
    message_id: "db-cmd-layer-viewport-action",
};
const ACTION_FREEZE: Message = Message {
    message_id: "db-cmd-layer-viewport-action-freeze",
};
const ACTION_THAW: Message = Message {
    message_id: "db-cmd-layer-viewport-action-thaw",
};
const ACTION_RESET: Message = Message {
    message_id: "db-cmd-layer-viewport-action-reset",
};
const LAYER_PROMPT: Message = Message {
    message_id: "db-cmd-layer-walk-layer",
};
const PROPERTY: Message = Message {
    message_id: "db-cmd-layer-viewport-property",
};
const PROPERTY_VALUE: Message = Message {
    message_id: "db-cmd-layer-viewport-property-value",
};
const PROPERTY_COLOR: Message = Message {
    message_id: "db-cmd-layer-viewport-property-color",
};
const PROPERTY_LINETYPE: Message = Message {
    message_id: "db-cmd-layer-viewport-property-linetype",
};
const PROPERTY_LINEWEIGHT: Message = Message {
    message_id: "db-cmd-layer-viewport-property-lineweight",
};
const PROPERTY_TRANSPARENCY: Message = Message {
    message_id: "db-cmd-layer-viewport-property-transparency",
};

/// The viewport picked for answer `key`, as the text of its identifier in a list.
fn picked(state: &PromptState<'_>, key: &str) -> Option<Vec<String>> {
    match state.answer(key)? {
        PromptInput::Pick(id) => Some(vec![id.to_string()]),
        _ => None,
    }
}

fn text(state: &PromptState<'_>, key: &str) -> Option<String> {
    match state.answer(key)? {
        PromptInput::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn viewport_pick() -> Accept {
    Accept::Pick {
        types: vec![VIEWPORT.to_owned()],
    }
}

fn isolate_viewport_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(ids) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        )));
    };
    let Some(viewport) = picked(state, "viewport") else {
        return Ok(Step::Ask(Prompt::new(
            "viewport",
            VIEWPORT_PICK.message_id,
            viewport_pick(),
        )));
    };
    done(json!({"objects": ids, "viewport": viewport[0]}))
}

fn freeze_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(action) = keyword_answer(state, "action") else {
        return Ok(Step::Ask(Prompt::new(
            "action",
            ACTION.message_id,
            Accept::keywords([
                KeywordOption::new("freeze", ACTION_FREEZE.message_id),
                KeywordOption::new("thaw", ACTION_THAW.message_id),
                KeywordOption::new("reset", ACTION_RESET.message_id),
            ]),
        )));
    };
    let Some(viewport) = picked(state, "viewport") else {
        return Ok(Step::Ask(Prompt::new(
            "viewport",
            VIEWPORT_PICK.message_id,
            viewport_pick(),
        )));
    };
    if action == "reset" {
        return done(json!({"action": action, "viewports": viewport}));
    }
    match text(state, "layer") {
        Some(name) => done(json!({"action": action, "viewports": viewport, "layers": [name]})),
        None => Ok(Step::Ask(Prompt::new(
            "layer",
            LAYER_PROMPT.message_id,
            Accept::text(),
        ))),
    }
}

/// The override prompt: the viewport, the layer, then one property and its value.
fn override_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(viewport) = picked(state, "viewport") else {
        return Ok(Step::Ask(Prompt::new(
            "viewport",
            VIEWPORT_PICK.message_id,
            viewport_pick(),
        )));
    };
    let Some(layer) = text(state, "layer") else {
        return Ok(Step::Ask(Prompt::new(
            "layer",
            LAYER_PROMPT.message_id,
            Accept::text(),
        )));
    };
    let Some(property) = keyword_answer(state, "property") else {
        return Ok(Step::Ask(Prompt::new(
            "property",
            PROPERTY.message_id,
            Accept::keywords([
                KeywordOption::new("color", PROPERTY_COLOR.message_id),
                KeywordOption::new("linetype", PROPERTY_LINETYPE.message_id),
                KeywordOption::new("lineweight", PROPERTY_LINEWEIGHT.message_id),
                KeywordOption::new("transparency", PROPERTY_TRANSPARENCY.message_id),
            ]),
        )));
    };
    let Some(value) = text(state, "value") else {
        return Ok(Step::Ask(Prompt::new(
            "value",
            PROPERTY_VALUE.message_id,
            Accept::text(),
        )));
    };
    let mut params = json!({"viewports": viewport, "layers": [layer]});
    let key = match property {
        "color" => "color",
        "linetype" => "linetype",
        "lineweight" => "lineweight",
        _ => "transparency",
    };
    let parsed = match key {
        "color" => value
            .trim()
            .parse::<u8>()
            .map(|index| json!(index))
            .map_err(|_| Error::new(codes::VALUE_INVALID).with_detail("field", "color"))?,
        "linetype" => json!(value.trim()),
        _ => json!(
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::new(codes::VALUE_INVALID).with_detail("field", key))?
        ),
    };
    params[key] = parsed;
    done(params)
}

fn clear_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(viewport) = picked(state, "viewport") else {
        return Ok(Step::Ask(Prompt::new(
            "viewport",
            VIEWPORT_PICK.message_id,
            viewport_pick(),
        )));
    };
    done(json!({"viewports": viewport}))
}

/// The commands of the viewport layer overrides.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "draftbuilder.layer.viewport_isolate",
            ISOLATE_VIEWPORT_LABEL,
            ISOLATE_VIEWPORT_DESCRIPTION,
            isolate_viewport,
        )
        .with_prompt(machine(isolate_viewport_next, accept_all)),
        spec(
            "draftbuilder.layer.viewport_freeze",
            FREEZE_LABEL,
            FREEZE_DESCRIPTION,
            freeze,
        )
        .with_prompt(machine(freeze_next, accept_all)),
        spec(
            "draftbuilder.layer.viewport_override",
            OVERRIDE_LABEL,
            OVERRIDE_DESCRIPTION,
            override_props,
        )
        .with_prompt(machine(override_next, accept_all)),
        spec(
            "draftbuilder.layer.viewport_clear",
            CLEAR_LABEL,
            CLEAR_DESCRIPTION,
            clear_props,
        )
        .with_prompt(machine(clear_next, accept_all)),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code")]
mod tests {
    use super::*;

    fn some_layer() -> ObjectId {
        "00000000-0000-0000-0000-000000000001".parse().unwrap()
    }

    /// An item the override list keeps although it is not a layer record, and a record of a layer:
    /// a change to the record leaves the item in the list.
    #[test]
    fn an_unparsed_item_survives_an_unrelated_override() {
        let layer = some_layer();
        let mut map = Overrides {
            records: BTreeMap::new(),
            other: vec![Value::Bool(true)],
        };
        set_field(&mut map, layer, F_TRANSPARENCY, Some(Value::F64(0.5)));
        let list = list_of(map);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], Value::Bool(true));
        assert!(matches!(&list[1], Value::Record(fields) if fields.len() == FIELDS));
    }

    /// A record whose appearance is cleared again is empty and is dropped, but an unparsed item stays.
    #[test]
    fn an_emptied_record_is_dropped_and_the_item_stays() {
        let layer = some_layer();
        let mut map = Overrides {
            records: BTreeMap::new(),
            other: vec![Value::Bool(true)],
        };
        set_field(&mut map, layer, F_COLOR, Some(Value::F64(1.0)));
        set_field(&mut map, layer, F_COLOR, None);
        map.records.retain(|_, record| !is_empty(record));
        assert_eq!(list_of(map), vec![Value::Bool(true)]);
    }
}
