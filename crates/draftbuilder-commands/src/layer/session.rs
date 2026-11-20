//! Layer state kept in the session (DB-LAYER-013, 014, 021): the previous layer settings
//! (`LAYERP`), the isolation of layers (`LAYISO`, `LAYUNISO`) and the layer walk (`LAYWALK`, and
//! `draftbuilder.layer.walk_end` to leave it).
//!
//! The state is per session and is not written to the drawing, and it does not follow undo. A
//! session is not tied to one drawing: the application shares one session across its open drawings,
//! and the command line gives one per run. So every record stores the fingerprint of the drawing it
//! was made on, and a command refuses a record of another drawing without consuming it.
//!
//! What each record does on restore: a flag is written back only when it still holds the value the
//! command wrote. A flag the user changed since is left as the user set it, and a flag that undo has
//! already returned to its base value is left alone. A restore that finds no layer it can account
//! for fails and keeps the record.
//!
//! Limits: a session value is JSON of at most [`MAX_SETTING_BYTES`] bytes. Isolation and walk records
//! spread over up to [`MAX_CHUNKS`] keys, about a thousand layers. `LAYERP` keeps [`PREVIOUS_SLOTS`]
//! changes. A change too large for one value is not kept: it leaves a marker, `LAYERP` refuses at the
//! marker, and the older history is dropped so `LAYERP` never steps across a change it cannot undo.
//! A marker is passed when a newer change is recorded. A layer command never fails because its
//! history could not be stored.

use std::collections::BTreeSet;

use arc_base::{Color, Error, ObjectId, Result};
use arc_command::settings::MAX_SETTING_BYTES;
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::{ColorSource, ColorValue, FieldPath, Snapshot, Value};
use arc_draft::types::{LAYER, layer};
use arc_draft_dxf::fields::name_of;
use draftbuilder_model::database::Database;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value as Json, json};

use super::{Layers, field_name, layers_of, spec, stored};
use crate::draw::create::{accept_all, done, machine};
use crate::error::{Message, codes};
use crate::modify::{entity_selection, keyword_answer, selection_answer};
use crate::select::candidates::flag;
use crate::settings::wildcard_match;

/// How many layer-setting changes `LAYERP` can go back.
pub const PREVIOUS_SLOTS: usize = 10;
/// The most keys one isolation or walk record spreads over.
pub const MAX_CHUNKS: usize = 16;
/// The bytes one chunk may take: below the session's value limit, with room for its wrapper.
const CHUNK_BYTES: usize = 3800;

/// The session key of the previous change in slot `slot` (0 is the newest).
fn previous_key(slot: usize) -> String {
    format!("draftbuilder.layer.previous.{slot}")
}

/// The session key prefix of the chunks of the isolation record.
const ISOLATE: &str = "draftbuilder.layer.isolate";
/// The session key prefix of the chunks of the walk record.
const WALK: &str = "draftbuilder.layer.walk";

/// The layer fields a change to the layer settings may record: the flags and the appearance.
const RESTORABLE: &[usize] = &[
    layer::ON,
    layer::FROZEN,
    layer::LOCKED,
    layer::PLOTTABLE,
    layer::COLOR,
    layer::LINETYPE,
    layer::LINEWEIGHT,
    layer::TRANSPARENCY,
    layer::DESCRIPTION,
];

/// One layer-setting change: the layer, the field, the value before the change and the value the
/// command wrote. `LAYERP` restores the "before" value only while the stored value still equals
/// the "after" value.
pub(crate) type Change = (ObjectId, usize, Option<Value>, Option<Value>);

/// One flag an isolation or walk changed: the layer, the field, the base value and the value written.
type Entry = (ObjectId, usize, bool, bool);

const LAYERP_LABEL: Message = Message {
    message_id: "db-cmd-layer-previous-label",
};
const LAYERP_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-previous-description",
};
const ISOLATE_LABEL: Message = Message {
    message_id: "db-cmd-layer-isolate-label",
};
const ISOLATE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-isolate-description",
};
const UNISOLATE_LABEL: Message = Message {
    message_id: "db-cmd-layer-unisolate-label",
};
const UNISOLATE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-unisolate-description",
};
const WALK_LABEL: Message = Message {
    message_id: "db-cmd-layer-walk-label",
};
const WALK_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-walk-description",
};
const WALK_END_LABEL: Message = Message {
    message_id: "db-cmd-layer-walk-end-label",
};
const WALK_END_DESCRIPTION: Message = Message {
    message_id: "db-cmd-layer-walk-end-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-layer-isolate-objects",
};
const MODE: Message = Message {
    message_id: "db-cmd-layer-isolate-mode",
};
const OPTION_OFF: Message = Message {
    message_id: "db-cmd-layer-isolate-option-off",
};
const OPTION_LOCK: Message = Message {
    message_id: "db-cmd-layer-isolate-option-lock",
};
const LAYER_PROMPT: Message = Message {
    message_id: "db-cmd-layer-walk-layer",
};
const KEEP: Message = Message {
    message_id: "db-cmd-layer-walk-keep",
};
const OPTION_KEEP: Message = Message {
    message_id: "db-cmd-layer-walk-option-keep",
};
const OPTION_RESTORE: Message = Message {
    message_id: "db-cmd-layer-walk-option-restore",
};

/// Parameters of the commands without parameters: `LAYERP` and `LAYUNISO`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// The layer settings a command changes, by mode.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IsolateMode {
    /// The other layers are turned off.
    #[default]
    Off,
    /// The other layers are locked.
    Lock,
}

/// Parameters of `draftbuilder.layer.isolate` (`LAYISO`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IsolateParams {
    /// The entities whose layers stay as they are; every other layer takes the mode.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// `off` (the default) turns the other layers off; `lock` locks them.
    #[serde(default)]
    pub mode: IsolateMode,
}

/// Parameters of `draftbuilder.layer.walk` (`LAYWALK`): the layers to show, by name or by
/// wildcard pattern (`*` and `?`). Each call shows its group only; the first call remembers the
/// visibility of every layer until `draftbuilder.layer.walk_end`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WalkParams {
    /// Layer names or wildcard patterns, without regard to case; at least one must match a layer.
    #[schemars(length(min = 1, max = 100))]
    pub layers: Vec<String>,
}

/// Parameters of `draftbuilder.layer.walk_end`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WalkEndParams {
    /// Whether the layers shown by the walk stay visible; false (the default) restores the
    /// visibility each layer had before the walk.
    #[serde(default)]
    pub keep: bool,
}

/// The flags of a layer that isolation and the walk change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Flags {
    on: bool,
    locked: bool,
}

fn flags_of(snapshot: &Snapshot, id: ObjectId) -> Flags {
    Flags {
        on: flag(snapshot, id, layer::ON).unwrap_or(true),
        locked: flag(snapshot, id, layer::LOCKED).unwrap_or(false),
    }
}

/// Whether `id` is a layer record.
fn is_layer(snapshot: &Snapshot, id: ObjectId) -> bool {
    snapshot
        .resolve(id)
        .ok()
        .and_then(|handle| snapshot.type_key(handle))
        .is_some_and(|key| key.as_str() == LAYER)
}

/// Every layer with its name, in table order.
fn all_layers(snapshot: &Snapshot) -> Vec<(ObjectId, String)> {
    snapshot
        .iter_type(LAYER)
        .ok()
        .map(|layers| {
            layers
                .filter_map(|(_, object)| {
                    let name = name_of(snapshot, Some(object.id))?;
                    Some((object.id, name))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The fingerprint of the drawing, as the records store it: empty for a drawing without one.
fn drawing_key(snapshot: &Snapshot) -> String {
    Database::of(snapshot)
        .fingerprint()
        .map_or_else(String::new, |fingerprint| {
            format!("{:032x}", fingerprint.bits())
        })
}

fn corrupt() -> Error {
    Error::new(codes::VALUE_INVALID).with_detail("field", "layer_state")
}

fn not_active(command: &str) -> Error {
    Error::new(codes::LAYER_NOT_ACTIVE).with_detail("command", command)
}

fn other_drawing() -> Error {
    Error::new(codes::LAYER_STATE_OTHER_DRAWING)
}

/// Encodes a value of a change, or `None` when the session has no form for it.
fn encode(value: &Option<Value>) -> Option<Json> {
    Some(match value {
        None => Json::Null,
        Some(Value::Bool(flag)) => Json::Bool(*flag),
        Some(Value::F64(number)) if number.is_finite() => json!(number),
        Some(Value::String(text)) => Json::String(text.clone()),
        Some(Value::Reference(id)) => json!({"ref": id.to_string()}),
        Some(Value::Color(color)) => {
            let rgba = [color.rgba.r, color.rgba.g, color.rgba.b, color.rgba.a];
            match color.source {
                None => json!({"rgba": rgba}),
                Some(ColorSource::Indexed(index)) => json!({"rgba": rgba, "index": index}),
                Some(ColorSource::Named(_)) => return None,
            }
        }
        Some(_) => return None,
    })
}

/// A stored color component, as the f32 a color holds.
#[expect(clippy::cast_possible_truncation, reason = "colors are stored as f32")]
fn component(c: &Json) -> Option<f32> {
    c.as_f64().map(|x| x as f32)
}

/// Decodes a value of a change: the outer `None` when the JSON has no such form.
fn decode(json: &Json) -> Option<Option<Value>> {
    Some(match json {
        Json::Null => None,
        Json::Bool(flag) => Some(Value::Bool(*flag)),
        Json::Number(number) => Some(Value::F64(number.as_f64()?)),
        Json::String(text) => Some(Value::String(text.clone())),
        Json::Object(map) => match map.get("ref") {
            Some(id) => Some(Value::Reference(id.as_str()?.parse().ok()?)),
            None => {
                let rgba = map.get("rgba")?.as_array()?;
                let [r, g, b, a] = rgba.as_slice() else {
                    return None;
                };
                let source = match map.get("index") {
                    Some(index) => Some(ColorSource::Indexed(u8::try_from(index.as_u64()?).ok()?)),
                    None => None,
                };
                let rgba = Color::new(component(r)?, component(g)?, component(b)?, component(a)?);
                Some(Value::Color(Box::new(ColorValue { rgba, source })))
            }
        },
        _ => return None,
    })
}

/// The JSON of one change, or `None` when one of its values has no form.
fn encode_change(change: &Change) -> Option<Json> {
    let (id, index, before, after) = change;
    Some(json!({
        "l": id.to_string(),
        "f": index,
        "v": encode(before)?,
        "a": encode(after)?,
    }))
}

/// The JSON of a change set of drawing `fingerprint`, or `None` when it does not fit one session
/// value or has a value with no form.
fn entry_json(fingerprint: &str, changes: &[Change]) -> Option<Json> {
    let items: Vec<Json> = changes.iter().map(encode_change).collect::<Option<_>>()?;
    let json = json!({"fp": fingerprint, "changes": items});
    let fits = serde_json::to_vec(&json).is_ok_and(|bytes| bytes.len() <= MAX_SETTING_BYTES);
    fits.then_some(json)
}

/// The changes of a stored entry. An entry of another drawing is refused, not decoded.
fn decode_entry(json: &Json, fingerprint: &str) -> Result<Vec<Change>> {
    if json.get("fp").and_then(Json::as_str) != Some(fingerprint) {
        return Err(other_drawing());
    }
    let items = json
        .get("changes")
        .and_then(Json::as_array)
        .ok_or_else(corrupt)?;
    items
        .iter()
        .map(|item| {
            let id: ObjectId = item
                .get("l")
                .and_then(Json::as_str)
                .ok_or_else(corrupt)?
                .parse()
                .map_err(|_| corrupt())?;
            let index = item
                .get("f")
                .and_then(Json::as_u64)
                .and_then(|f| usize::try_from(f).ok())
                .filter(|f| RESTORABLE.contains(f))
                .ok_or_else(corrupt)?;
            let before = decode(item.get("v").ok_or_else(corrupt)?).ok_or_else(corrupt)?;
            let after = decode(item.get("a").ok_or_else(corrupt)?).ok_or_else(corrupt)?;
            Ok((id, index, before, after))
        })
        .collect()
}

/// The slots of the previous changes, newest first, as stored.
fn read_slots(ctx: &Context<'_>) -> Vec<Option<Json>> {
    (0..PREVIOUS_SLOTS)
        .map(|slot| ctx.setting(&previous_key(slot)))
        .collect()
}

/// Stores the slots, newest first; an empty slot removes its key.
fn store_slots(ctx: &mut Context<'_>, slots: Vec<Option<Json>>) -> Result<()> {
    for (slot, value) in slots.into_iter().enumerate().take(PREVIOUS_SLOTS) {
        let key = previous_key(slot);
        match value {
            Some(json) => ctx.set_setting(&key, json)?,
            None => {
                ctx.remove_setting(&key);
            }
        }
    }
    Ok(())
}

/// Removes every previous-change slot.
fn clear_slots(ctx: &mut Context<'_>) {
    for slot in 0..PREVIOUS_SLOTS {
        ctx.remove_setting(&previous_key(slot));
    }
}

/// Records one layer-setting change set as the newest previous change, for the drawing it was made
/// on. A set too large for one session value leaves a marker in its place and drops the older
/// history. Best effort: if the session cannot store the history, the history is dropped, so the
/// layer command that made the change still succeeds, and `LAYERP` has nothing wrong to restore.
pub(crate) fn keep(ctx: &mut Context<'_>, changes: &[Change]) {
    if changes.is_empty() {
        return;
    }
    let fingerprint = drawing_key(ctx.snapshot());
    let mut slots = match entry_json(&fingerprint, changes) {
        Some(json) => {
            let mut slots = vec![Some(json)];
            slots.extend(read_slots(ctx));
            slots
        }
        None => vec![Some(Json::Null)],
    };
    slots.truncate(PREVIOUS_SLOTS);
    if store_slots(ctx, slots).is_err() {
        clear_slots(ctx);
    }
}

/// Whether a change set matches the drawing now: every layer it names that still exists holds the
/// value the change wrote, and at least one exists.
fn matches_now(snapshot: &Snapshot, changes: &[Change]) -> bool {
    let mut any = false;
    for (id, index, _, after) in changes {
        if !is_layer(snapshot, *id) {
            continue;
        }
        any = true;
        if stored(snapshot, *id, *index) != *after {
            return false;
        }
    }
    any
}

/// Restores the newest previous change that still matches the drawing. A change that no longer
/// matches (undo took it back, or the user changed it) is removed without effect, and the next one
/// is tried. A marker stops the search with `db.cmd.layer_previous_unkept`. When nothing is left
/// to restore the command fails with `db.cmd.layer_previous_none`. A failed command keeps no session
/// writes, so the removals before a failure come back, and the same search fails the same way.
fn previous(ctx: &mut Context<'_>, _: NoParams) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let fingerprint = drawing_key(snapshot);
    let slots = read_slots(ctx);
    let mut found = None;
    for (slot, stored_entry) in slots.iter().enumerate() {
        let Some(json) = stored_entry else {
            break;
        };
        if json.is_null() {
            return Err(Error::new(codes::LAYER_PREVIOUS_UNKEPT));
        }
        let changes = decode_entry(json, &fingerprint)?;
        if matches_now(snapshot, &changes) {
            found = Some((slot, changes));
            break;
        }
    }
    let Some((slot, changes)) = found else {
        return Err(Error::new(codes::LAYER_PREVIOUS_NONE));
    };
    let mut writes = Vec::new();
    let mut restored = BTreeSet::new();
    for (id, index, before, _) in changes {
        // A layer deleted since the change has nothing left to restore.
        if !is_layer(snapshot, id) {
            continue;
        }
        if index == layer::FROZEN && before == Some(Value::Bool(true)) {
            crate::settings::refuse_current(snapshot, id)?;
        }
        restored.insert(id);
        writes.push((id, index, before));
    }
    let txn = ctx.txn()?;
    for (id, index, before) in writes {
        txn.modify(id, FieldPath::new(field_name(index)), before)?;
    }
    // The restored entry and the ones skipped above it leave the history.
    let mut rest: Vec<Option<Json>> = slots.into_iter().skip(slot + 1).collect();
    rest.resize(PREVIOUS_SLOTS, None);
    store_slots(ctx, rest)?;
    Ok(Layers {
        layers: restored.into_iter().map(IdParam).collect(),
    })
}

/// Writes `items` as the record of `prefix` for drawing `fingerprint`, over at most [`MAX_CHUNKS`]
/// keys. An empty record is one chunk with no items: it still marks the state as active.
fn write_chunks(
    ctx: &mut Context<'_>,
    prefix: &str,
    fingerprint: &str,
    items: Vec<Json>,
) -> Result<()> {
    let mut chunks: Vec<Vec<Json>> = vec![Vec::new()];
    let mut used = 0usize;
    for item in items {
        let size = serde_json::to_vec(&item).map_or(0, |bytes| bytes.len()) + 1;
        if used + size > CHUNK_BYTES && chunks.last().is_some_and(|chunk| !chunk.is_empty()) {
            chunks.push(Vec::new());
            used = 0;
        }
        if let Some(last) = chunks.last_mut() {
            last.push(item);
        }
        used += size;
    }
    if chunks.len() > MAX_CHUNKS {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", "layers"));
    }
    clear_chunks(ctx, prefix);
    for (index, chunk) in chunks.into_iter().enumerate() {
        let key = format!("{prefix}.{index}");
        ctx.set_setting(&key, json!({"fp": fingerprint, "items": chunk}))?;
    }
    Ok(())
}

/// Removes every chunk key of the record of `prefix`.
fn clear_chunks(ctx: &mut Context<'_>, prefix: &str) {
    for index in 0..MAX_CHUNKS {
        ctx.remove_setting(&format!("{prefix}.{index}"));
    }
}

/// The items of the record of `prefix`, or `None` when none is stored. A record of another drawing
/// is refused and left in place.
fn read_chunks(ctx: &Context<'_>, prefix: &str, fingerprint: &str) -> Result<Option<Vec<Json>>> {
    let mut items = Vec::new();
    let mut found = false;
    for index in 0..MAX_CHUNKS {
        let Some(chunk) = ctx.setting(&format!("{prefix}.{index}")) else {
            break;
        };
        found = true;
        if chunk.get("fp").and_then(Json::as_str) != Some(fingerprint) {
            return Err(other_drawing());
        }
        let list = chunk
            .get("items")
            .and_then(Json::as_array)
            .ok_or_else(corrupt)?;
        items.extend(list.iter().cloned());
    }
    Ok(found.then_some(items))
}

/// The flag entries of the record of `prefix`, or `None` when none is stored.
fn read_record(ctx: &Context<'_>, prefix: &str, fingerprint: &str) -> Result<Option<Vec<Entry>>> {
    let Some(items) = read_chunks(ctx, prefix, fingerprint)? else {
        return Ok(None);
    };
    items
        .iter()
        .map(|item| {
            let id: ObjectId = item
                .get(0)
                .and_then(Json::as_str)
                .and_then(|s| s.parse().ok())
                .ok_or_else(corrupt)?;
            let index = item
                .get(1)
                .and_then(Json::as_u64)
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i == layer::ON || *i == layer::LOCKED)
                .ok_or_else(corrupt)?;
            let base = item.get(2).and_then(Json::as_bool).ok_or_else(corrupt)?;
            let written = item.get(3).and_then(Json::as_bool).ok_or_else(corrupt)?;
            Ok((id, index, base, written))
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

/// What a restore writes, the changes it makes for the history, and the layers it accounts for.
struct Restore {
    writes: Vec<(ObjectId, usize, bool)>,
    history: Vec<Change>,
    handled: BTreeSet<ObjectId>,
}

/// Plans the restore of `entries`. A flag still at the value the command wrote goes back to its
/// base; a flag at its base already is accounted for; any other value was set by the user and is
/// left alone. A record with entries and none accounted for is refused with
/// `db.cmd.layer_state_changed`, and the record stays.
fn restore_plan(snapshot: &Snapshot, entries: &[Entry]) -> Result<Restore> {
    let mut plan = Restore {
        writes: Vec::new(),
        history: Vec::new(),
        handled: BTreeSet::new(),
    };
    for (id, index, base, written) in entries {
        if !is_layer(snapshot, *id) || base == written {
            continue;
        }
        let Some(now) = flag(snapshot, *id, *index) else {
            continue;
        };
        if now == *written {
            plan.writes.push((*id, *index, *base));
            plan.history.push((
                *id,
                *index,
                Some(Value::Bool(now)),
                Some(Value::Bool(*base)),
            ));
            plan.handled.insert(*id);
        } else if now == *base {
            plan.handled.insert(*id);
        }
    }
    if !entries.is_empty() && plan.handled.is_empty() {
        return Err(Error::new(codes::LAYER_STATE_CHANGED));
    }
    Ok(plan)
}

/// Turns the other layers off or locks them, keeping the layers of `objects`. The flags the layers
/// had before are remembered in the session until `draftbuilder.layer.unisolate`. An isolation made
/// while another is active starts from the remembered base flags, so unisolating returns to the state
/// before the first.
fn isolate(ctx: &mut Context<'_>, p: IsolateParams) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let fingerprint = drawing_key(snapshot);
    let kept: BTreeSet<ObjectId> = layers_of(snapshot, &p.objects)?.into_iter().collect();
    let saved = read_record(ctx, ISOLATE, &fingerprint)?.unwrap_or_default();
    let mut bases: std::collections::BTreeMap<ObjectId, Flags> = std::collections::BTreeMap::new();
    for (id, index, base, _) in &saved {
        let entry = bases.entry(*id).or_insert_with(|| flags_of(snapshot, *id));
        if *index == layer::ON {
            entry.on = *base;
        } else {
            entry.locked = *base;
        }
    }
    let mut record: Vec<Entry> = Vec::new();
    let mut writes: Vec<(ObjectId, usize, bool)> = Vec::new();
    let mut history: Vec<Change> = Vec::new();
    for (id, _) in all_layers(snapshot) {
        let base = bases
            .get(&id)
            .copied()
            .unwrap_or_else(|| flags_of(snapshot, id));
        let mut target = base;
        if !kept.contains(&id) {
            match p.mode {
                IsolateMode::Off => target.on = false,
                IsolateMode::Lock => target.locked = true,
            }
        }
        let now = flags_of(snapshot, id);
        if target.on != base.on {
            record.push((id, layer::ON, base.on, target.on));
        }
        if target.locked != base.locked {
            record.push((id, layer::LOCKED, base.locked, target.locked));
        }
        if target.on != now.on {
            writes.push((id, layer::ON, target.on));
            history.push((
                id,
                layer::ON,
                Some(Value::Bool(now.on)),
                Some(Value::Bool(target.on)),
            ));
        }
        if target.locked != now.locked {
            writes.push((id, layer::LOCKED, target.locked));
            history.push((
                id,
                layer::LOCKED,
                Some(Value::Bool(now.locked)),
                Some(Value::Bool(target.locked)),
            ));
        }
    }
    let items: Vec<Json> = record
        .iter()
        .map(|(id, index, base, written)| json!([id.to_string(), index, base, written]))
        .collect();
    let shown: Vec<IdParam> = kept.iter().copied().map(IdParam).collect();
    write_chunks(ctx, ISOLATE, &fingerprint, items)?;
    let txn = ctx.txn()?;
    for (id, index, value) in writes {
        txn.modify(
            id,
            FieldPath::new(field_name(index)),
            Some(Value::Bool(value)),
        )?;
    }
    keep(ctx, &history);
    Ok(Layers { layers: shown })
}

/// Restores the flags that `isolate` remembered and ends the isolation. Flags the user changed since
/// stay as they are. Fails, keeping the record, when the record has layers and none of them can be
/// accounted for. Undo of an isolation leaves its base flags, which count as accounted for.
fn unisolate(ctx: &mut Context<'_>, _: NoParams) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let fingerprint = drawing_key(snapshot);
    let Some(entries) = read_record(ctx, ISOLATE, &fingerprint)? else {
        return Err(not_active("layer.unisolate"));
    };
    let plan = restore_plan(snapshot, &entries)?;
    clear_chunks(ctx, ISOLATE);
    let txn = ctx.txn()?;
    for (id, index, value) in plan.writes {
        txn.modify(
            id,
            FieldPath::new(field_name(index)),
            Some(Value::Bool(value)),
        )?;
    }
    keep(ctx, &plan.history);
    Ok(Layers {
        layers: plan.handled.into_iter().map(IdParam).collect(),
    })
}

/// Shows the layers matching the patterns and hides the others. The first walk remembers every
/// layer's visibility; later walks step to the next group and keep that memory.
fn walk(ctx: &mut Context<'_>, p: WalkParams) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let fingerprint = drawing_key(snapshot);
    let names = all_layers(snapshot);
    let mut shown = BTreeSet::new();
    for pattern in &p.layers {
        if pattern.chars().count() > crate::layer::MAX_NAME_CHARS {
            return Err(Error::new(codes::NAME_INVALID).with_detail("name", pattern.clone()));
        }
        let mut matched = false;
        for (id, name) in &names {
            if wildcard_match(pattern, name) {
                shown.insert(*id);
                matched = true;
            }
        }
        if !matched {
            return Err(Error::new(codes::LAYER_UNKNOWN).with_detail("name", pattern.clone()));
        }
    }
    let saved = read_record(ctx, WALK, &fingerprint)?.unwrap_or_default();
    let mut originals = std::collections::BTreeMap::new();
    for (id, _, base, _) in &saved {
        originals.insert(*id, *base);
    }
    let mut record: Vec<Entry> = Vec::new();
    let mut writes = Vec::new();
    for (id, _) in &names {
        let now = flags_of(snapshot, *id).on;
        let original = originals.get(id).copied().unwrap_or(now);
        let desired = shown.contains(id);
        if desired != original {
            record.push((*id, layer::ON, original, desired));
        }
        if desired != now {
            writes.push((*id, desired));
        }
    }
    let items: Vec<Json> = record
        .iter()
        .map(|(id, index, base, written)| json!([id.to_string(), index, base, written]))
        .collect();
    let result: Vec<IdParam> = shown.iter().copied().map(IdParam).collect();
    write_chunks(ctx, WALK, &fingerprint, items)?;
    let txn = ctx.txn()?;
    for (id, on) in writes {
        txn.modify(
            id,
            FieldPath::new(field_name(layer::ON)),
            Some(Value::Bool(on)),
        )?;
    }
    Ok(Layers { layers: result })
}

/// Ends the walk. Unless `keep` is set, each layer the walk changed gets its visibility back, except
/// a layer the user changed since. Ends with `db.cmd.layer_not_active` when no walk is active.
fn walk_end(ctx: &mut Context<'_>, p: WalkEndParams) -> Result<Layers> {
    let snapshot = ctx.snapshot();
    let fingerprint = drawing_key(snapshot);
    let Some(entries) = read_record(ctx, WALK, &fingerprint)? else {
        return Err(not_active("layer.walk_end"));
    };
    if p.keep {
        clear_chunks(ctx, WALK);
        let ids = entries.iter().map(|(id, ..)| IdParam(*id)).collect();
        return Ok(Layers { layers: ids });
    }
    let plan = restore_plan(snapshot, &entries)?;
    clear_chunks(ctx, WALK);
    let txn = ctx.txn()?;
    for (id, index, value) in plan.writes {
        txn.modify(
            id,
            FieldPath::new(field_name(index)),
            Some(Value::Bool(value)),
        )?;
    }
    Ok(Layers {
        layers: plan.handled.into_iter().map(IdParam).collect(),
    })
}

fn text_answer(state: &PromptState<'_>, key: &str) -> Option<String> {
    match state.answer(key)? {
        PromptInput::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn isolate_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(ids) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        )));
    };
    let Some(mode) = keyword_answer(state, "mode") else {
        return Ok(Step::Ask(Prompt::new(
            "mode",
            MODE.message_id,
            Accept::keywords([
                KeywordOption::new("off", OPTION_OFF.message_id),
                KeywordOption::new("lock", OPTION_LOCK.message_id),
            ]),
        )));
    };
    done(json!({"objects": ids, "mode": mode}))
}

fn walk_next(state: &PromptState<'_>) -> Result<Step> {
    match text_answer(state, "layer") {
        Some(name) => done(json!({"layers": [name]})),
        None => Ok(Step::Ask(Prompt::new(
            "layer",
            LAYER_PROMPT.message_id,
            Accept::text(),
        ))),
    }
}

fn walk_end_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(choice) = keyword_answer(state, "keep") else {
        return Ok(Step::Ask(Prompt::new(
            "keep",
            KEEP.message_id,
            Accept::keywords([
                KeywordOption::new("keep", OPTION_KEEP.message_id),
                KeywordOption::new("restore", OPTION_RESTORE.message_id),
            ]),
        )));
    };
    done(json!({"keep": choice == "keep"}))
}

fn no_prompt(_: &PromptState<'_>) -> Result<Step> {
    done(json!({}))
}

/// The commands of the session layer state: `LAYERP`, `LAYISO`, `LAYUNISO`, `LAYWALK` and its end.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "draftbuilder.layer.previous",
            LAYERP_LABEL,
            LAYERP_DESCRIPTION,
            previous,
        )
        .with_prompt(machine(no_prompt, accept_all)),
        spec(
            "draftbuilder.layer.isolate",
            ISOLATE_LABEL,
            ISOLATE_DESCRIPTION,
            isolate,
        )
        .with_prompt(machine(isolate_next, accept_all)),
        spec(
            "draftbuilder.layer.unisolate",
            UNISOLATE_LABEL,
            UNISOLATE_DESCRIPTION,
            unisolate,
        )
        .with_prompt(machine(no_prompt, accept_all)),
        spec(
            "draftbuilder.layer.walk",
            WALK_LABEL,
            WALK_DESCRIPTION,
            walk,
        )
        .with_prompt(machine(walk_next, accept_all)),
        spec(
            "draftbuilder.layer.walk_end",
            WALK_END_LABEL,
            WALK_END_DESCRIPTION,
            walk_end,
        )
        .with_prompt(machine(walk_end_next, accept_all)),
    ]
}
