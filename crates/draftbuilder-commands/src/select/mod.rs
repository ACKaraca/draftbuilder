//! Selection commands (area `select`): the window, crossing, polygon and fence methods, all, last,
//! previous, filters and similar selection. Each command computes the set of objects of a space
//! (the model space by default, or any space or block definition named by `space`) with a pure
//! function of [`methods`] or [`query`], and returns it combined with the `current` set the caller
//! passes, by `mode` ([`combine`]).
//!
//! Selection changes no document state: the commands are not undoable, create no undo step and
//! leave the session selection as it is. The session keeps no current or previous set yet, so the
//! caller passes the current set explicitly for `add`, `remove` and `within_current`, and the
//! previous set explicitly to `previous`. Previous across commands is not available until the
//! foundation keeps session selection state (see the architecture overview). The pick, highlight,
//! pointer gestures, grips and the prompt machine are not part of this module.

pub mod candidates;
pub mod cycle;
pub mod methods;
pub mod query;

use std::collections::{BTreeMap, BTreeSet};

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::Snapshot;
use arc_draft::types::{BLOCK, SPACE};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, codes};
use crate::vocabulary::{Area, register_names};
use candidates::Options;
use query::{Filter, Property, Read};

/// How a selection combines with the current set the caller passes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The picked objects replace the current set.
    #[default]
    Replace,
    /// The picked objects are added to the current set.
    Add,
    /// The picked objects are removed from the current set.
    Remove,
}

/// Parameters of a window or a crossing: two opposite corners.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rectangle {
    /// One corner.
    pub first: [f64; 2],
    /// The opposite corner.
    pub second: [f64; 2],
    /// The space or block definition searched; the model space when absent.
    #[serde(default)]
    pub space: Option<IdParam>,
    /// The current set that `mode` combines with.
    #[serde(default)]
    pub current: Vec<IdParam>,
    /// How the picked objects combine with the current set.
    #[serde(default)]
    pub mode: Mode,
    /// Whether objects on locked layers are picked.
    #[serde(default)]
    pub include_locked: bool,
}

/// Parameters of a polygon method or a fence: the vertices in order.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Vertices {
    /// The vertices: at least three for a polygon, at least two for a fence.
    #[schemars(length(max = 4096))]
    pub points: Vec<[f64; 2]>,
    /// The space or block definition searched; the model space when absent.
    #[serde(default)]
    pub space: Option<IdParam>,
    /// The current set that `mode` combines with.
    #[serde(default)]
    pub current: Vec<IdParam>,
    /// How the picked objects combine with the current set.
    #[serde(default)]
    pub mode: Mode,
    /// Whether objects on locked layers are picked.
    #[serde(default)]
    pub include_locked: bool,
}

/// Parameters of the selections that take no geometry: all and last.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Everything {
    /// The space or block definition searched; the model space when absent.
    #[serde(default)]
    pub space: Option<IdParam>,
    /// The current set that `mode` combines with.
    #[serde(default)]
    pub current: Vec<IdParam>,
    /// How the picked objects combine with the current set.
    #[serde(default)]
    pub mode: Mode,
    /// Whether objects on locked layers are picked.
    #[serde(default)]
    pub include_locked: bool,
}

/// Parameters of the previous selection: the set the caller recorded as previous.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Previous {
    /// The previous set; the objects that no longer exist are dropped.
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub previous: Vec<IdParam>,
    /// The current set that `mode` combines with.
    #[serde(default)]
    pub current: Vec<IdParam>,
    /// How the picked objects combine with the current set.
    #[serde(default)]
    pub mode: Mode,
}

/// Parameters of a filter selection.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FilterParams {
    /// The filter; see the module documentation of `query`.
    pub filter: Filter,
    /// Selects the objects that do not satisfy the filter.
    #[serde(default)]
    pub invert: bool,
    /// Tests only the objects of `current`.
    #[serde(default)]
    pub within_current: bool,
    /// The space or block definition searched; the model space when absent.
    #[serde(default)]
    pub space: Option<IdParam>,
    /// The current set that `mode` combines with, and that `within_current` restricts to.
    #[serde(default)]
    pub current: Vec<IdParam>,
    /// How the picked objects combine with the current set.
    #[serde(default)]
    pub mode: Mode,
    /// Whether objects on locked layers are picked.
    #[serde(default)]
    pub include_locked: bool,
}

/// Parameters of a similar selection.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Similar {
    /// The objects whose properties are matched.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The properties that must be equal.
    #[schemars(length(min = 1, max = 7))]
    pub properties: Vec<Property>,
    /// The space or block definition searched; the model space when absent.
    #[serde(default)]
    pub space: Option<IdParam>,
    /// The current set that `mode` combines with.
    #[serde(default)]
    pub current: Vec<IdParam>,
    /// How the picked objects combine with the current set.
    #[serde(default)]
    pub mode: Mode,
    /// Whether objects on locked layers are picked.
    #[serde(default)]
    pub include_locked: bool,
}

/// What a selection command returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Picked {
    /// The selection after the mode combined the picked set with the current set, in order.
    pub objects: Vec<IdParam>,
    /// The number of objects the method picked, before the mode combined them.
    pub picked: u64,
    /// The count of each entity type in the selection (`line`, `circle`, ...).
    pub types: BTreeMap<String, u64>,
}

/// The selection of `mode`: the picked objects replace `current`, are added to it, or are removed
/// from it. Added objects keep their order after the current ones, and no object appears twice.
#[must_use]
pub fn combine(current: &[ObjectId], picked: &[ObjectId], mode: Mode) -> Vec<ObjectId> {
    match mode {
        Mode::Replace => {
            let mut seen = BTreeSet::new();
            picked
                .iter()
                .copied()
                .filter(|id| seen.insert(*id))
                .collect()
        }
        Mode::Add => {
            let mut seen: BTreeSet<ObjectId> = current.iter().copied().collect();
            let mut out = current.to_vec();
            out.extend(picked.iter().copied().filter(|id| seen.insert(*id)));
            out
        }
        Mode::Remove => {
            let removed: BTreeSet<ObjectId> = picked.iter().copied().collect();
            current
                .iter()
                .copied()
                .filter(|id| !removed.contains(id))
                .collect()
        }
    }
}

fn options(include_locked: bool) -> Options {
    Options { include_locked }
}

fn points(values: &[[f64; 2]]) -> Vec<Point2> {
    values.iter().map(|[x, y]| Point2::new(*x, *y)).collect()
}

fn ids(list: &[IdParam]) -> Vec<ObjectId> {
    list.iter().map(|IdParam(id)| *id).collect()
}

/// The identifiers of `list` that name an object of `snapshot`, in their order.
fn live(snapshot: &Snapshot, list: &[IdParam]) -> Vec<ObjectId> {
    ids(list)
        .into_iter()
        .filter(|id| snapshot.resolve(*id).is_ok())
        .collect()
}

/// The space a command searches: the model space, or the space or block definition named.
fn space_of(snapshot: &Snapshot, space: Option<IdParam>) -> Result<ObjectId> {
    let Some(IdParam(id)) = space else {
        return crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE));
    };
    let handle = snapshot.resolve(id)?;
    let key = snapshot.type_key(handle).map(|k| k.as_str().to_owned());
    match key.as_deref() {
        Some(SPACE) | Some(BLOCK) => Ok(id),
        _ => Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
            .with_detail("id", id.to_string())),
    }
}

/// The result of a method: the picked objects combined with the current set by `mode`, and the
/// count of each type in the result.
fn finish(snapshot: &Snapshot, current: &[IdParam], picked: Vec<ObjectId>, mode: Mode) -> Picked {
    let objects = combine(&live(snapshot, current), &picked, mode);
    let mut types = BTreeMap::new();
    for id in &objects {
        if let Read::Text(name) = query::read(snapshot, *id, Property::Type) {
            *types.entry(name).or_insert(0) += 1;
        }
    }
    Picked {
        objects: objects.into_iter().map(IdParam).collect(),
        picked: u64::try_from(picked.len()).unwrap_or(u64::MAX),
        types,
    }
}

fn rectangle(ctx: &mut Context<'_>, p: Rectangle) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let corners = [
        Point2::new(p.first[0], p.first[1]),
        Point2::new(p.second[0], p.second[1]),
    ];
    let picked = methods::select_window(snapshot, space, corners, &options(p.include_locked))?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn crossing(ctx: &mut Context<'_>, p: Rectangle) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let corners = [
        Point2::new(p.first[0], p.first[1]),
        Point2::new(p.second[0], p.second[1]),
    ];
    let picked = methods::select_crossing(snapshot, space, corners, &options(p.include_locked))?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn window_polygon(ctx: &mut Context<'_>, p: Vertices) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let picked = methods::select_window_polygon(
        snapshot,
        space,
        &points(&p.points),
        &options(p.include_locked),
    )?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn crossing_polygon(ctx: &mut Context<'_>, p: Vertices) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let picked = methods::select_crossing_polygon(
        snapshot,
        space,
        &points(&p.points),
        &options(p.include_locked),
    )?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn fence(ctx: &mut Context<'_>, p: Vertices) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let picked = methods::select_fence(
        snapshot,
        space,
        &points(&p.points),
        &options(p.include_locked),
    )?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn all(ctx: &mut Context<'_>, p: Everything) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let picked = methods::select_all(snapshot, space, &options(p.include_locked))?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn last(ctx: &mut Context<'_>, p: Everything) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let picked = methods::select_last(snapshot, space, &options(p.include_locked))?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn previous(ctx: &mut Context<'_>, p: Previous) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let picked = methods::select_previous(snapshot, &ids(&p.previous));
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn filter(ctx: &mut Context<'_>, p: FilterParams) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let within = p.within_current.then(|| live(snapshot, &p.current));
    let picked = query::select_query(
        snapshot,
        space,
        &p.filter,
        p.invert,
        within.as_deref(),
        &options(p.include_locked),
    )?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

fn similar(ctx: &mut Context<'_>, p: Similar) -> Result<Picked> {
    let snapshot = ctx.snapshot();
    let space = space_of(snapshot, p.space)?;
    let picked = query::select_similar(
        snapshot,
        space,
        &ids(&p.objects),
        &p.properties,
        &options(p.include_locked),
    )?;
    Ok(finish(snapshot, &p.current, picked, p.mode))
}

/// Registers one selection command: read-only, so it creates no undo step and changes no
/// document state.
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
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read])
}

/// Registers the commands of the area and applies its names.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered, and the errors of
/// [`register_names`].
pub fn register(registry: &mut Registry) -> Result<()> {
    for spec in geometry_specs().into_iter().chain(query_specs()) {
        registry.register(spec)?;
    }
    register_names(registry, Area::Select)
}

/// The commands that pick by geometry.
fn geometry_specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "draftbuilder.select.window",
            Message {
                message_id: "db-cmd-select-window-label",
            },
            Message {
                message_id: "db-cmd-select-window-description",
            },
            rectangle,
        ),
        spec(
            "draftbuilder.select.crossing",
            Message {
                message_id: "db-cmd-select-crossing-label",
            },
            Message {
                message_id: "db-cmd-select-crossing-description",
            },
            crossing,
        ),
        spec(
            "draftbuilder.select.window_polygon",
            Message {
                message_id: "db-cmd-select-window-polygon-label",
            },
            Message {
                message_id: "db-cmd-select-window-polygon-description",
            },
            window_polygon,
        ),
        spec(
            "draftbuilder.select.crossing_polygon",
            Message {
                message_id: "db-cmd-select-crossing-polygon-label",
            },
            Message {
                message_id: "db-cmd-select-crossing-polygon-description",
            },
            crossing_polygon,
        ),
        spec(
            "draftbuilder.select.fence",
            Message {
                message_id: "db-cmd-select-fence-label",
            },
            Message {
                message_id: "db-cmd-select-fence-description",
            },
            fence,
        ),
    ]
}

/// The commands that pick by their names, properties and the previous set.
fn query_specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "draftbuilder.select.all",
            Message {
                message_id: "db-cmd-select-all-label",
            },
            Message {
                message_id: "db-cmd-select-all-description",
            },
            all,
        ),
        spec(
            "draftbuilder.select.last",
            Message {
                message_id: "db-cmd-select-last-label",
            },
            Message {
                message_id: "db-cmd-select-last-description",
            },
            last,
        ),
        spec(
            "draftbuilder.select.previous",
            Message {
                message_id: "db-cmd-select-previous-label",
            },
            Message {
                message_id: "db-cmd-select-previous-description",
            },
            previous,
        ),
        spec(
            "draftbuilder.select.filter",
            Message {
                message_id: "db-cmd-select-filter-label",
            },
            Message {
                message_id: "db-cmd-select-filter-description",
            },
            filter,
        ),
        spec(
            "draftbuilder.select.similar",
            Message {
                message_id: "db-cmd-select-similar-label",
            },
            Message {
                message_id: "db-cmd-select-similar-description",
            },
            similar,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use arc_base::id::Identifier;

    use super::*;

    fn id(n: u128) -> ObjectId {
        ObjectId::from_u128(n)
    }

    #[test]
    fn replace_takes_the_picked_set_once_each() {
        let (a, b) = (id(1), id(2));
        assert_eq!(combine(&[a], &[b, b], Mode::Replace), vec![b]);
    }

    #[test]
    fn add_appends_the_new_objects_after_the_current_ones() {
        let (a, b, c) = (id(1), id(2), id(3));
        assert_eq!(combine(&[a, b], &[b, c], Mode::Add), vec![a, b, c]);
    }

    #[test]
    fn remove_keeps_the_current_order_without_the_picked_objects() {
        let (a, b, c) = (id(1), id(2), id(3));
        assert_eq!(combine(&[a, b, c], &[b], Mode::Remove), vec![a, c]);
        assert_eq!(combine(&[a], &[], Mode::Remove), vec![a]);
    }
}
