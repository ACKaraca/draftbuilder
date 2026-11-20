//! `draftbuilder.drawing.purge`: removes unused layers, linetypes, block definitions and styles.
//!
//! An object is unused when no live object refers to it through a field (`arc_draft::purge`).
//! A run removes the unused objects of the chosen categories in one undoable transaction. With
//! `repeat` it goes on, pass after pass, until a pass finds nothing: an object used only by an
//! object removed earlier becomes unused in a later pass, and a block used only by a removed block
//! is removed the same way. The passes are capped at [`MAX_PASSES`]; a run that reaches the cap
//! says so in `capped`.
//!
//! A preview runs the same passes inside a savepoint of the transaction and rolls them back, so
//! it lists exactly what a run with the same parameters removes.
//!
//! Protected names are kept whether or not they are unused, per category: the layers `0` and
//! `Defpoints`, the linetypes `Continuous`, `ByLayer` and `ByBlock`, and the text style `Standard`.
//! A name that appears in a preserved record of the file (an entity or object the reader kept as
//! data) is kept too: the foundation does not tell which preserved fields name an object, so any
//! occurrence of the name in such a record keeps the object.
//!
//! Not purged: zero-length geometry, because the model refuses a zero-length segment when it is
//! built, so a file's zero-length line is not imported (and the import reports it); empty text,
//! which has no category yet; and registered application names, which the drawing model does not
//! hold.

use std::collections::BTreeSet;

use arc_base::{Limits, ObjectId, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use arc_doc::{Snapshot, Txn};
use arc_draft::style::STYLES;
use arc_draft::types::{BLOCK, LAYER, LINETYPE};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Message;
use crate::fold::fold;

/// The identifier of the command.
pub const ID: &str = "draftbuilder.drawing.purge";
const LABEL: Message = Message {
    message_id: "db-cmd-purge-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-purge-description",
};

/// The most passes one run makes. A chain of references deeper than this is purged up to the cap
/// and reported as capped.
pub const MAX_PASSES: u32 = 64;

/// A kind of object `draftbuilder.drawing.purge` removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Layers no object uses.
    Layers,
    /// Linetypes no object uses.
    Linetypes,
    /// Block definitions no block reference uses; their entities go with them.
    Blocks,
    /// Text, dimension, leader and table styles no object uses.
    Styles,
}

const ALL_CATEGORIES: &[Category] = &[
    Category::Layers,
    Category::Linetypes,
    Category::Blocks,
    Category::Styles,
];

/// Parameters of `draftbuilder.drawing.purge`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PurgeParams {
    /// The categories to purge; absent: all of them.
    #[serde(default)]
    pub categories: Option<Vec<Category>>,
    /// Whether to repeat the passes until nothing more is unused.
    #[serde(default)]
    pub repeat: bool,
    /// Whether to list what a run would remove, without removing it.
    #[serde(default)]
    pub preview: bool,
}

/// One object removed (or, in a preview, to be removed).
#[derive(Debug, Serialize, JsonSchema)]
pub struct Removed {
    /// The category of the object.
    pub category: Category,
    /// The object's identifier.
    pub object: String,
    /// The object's name, when it has one.
    pub name: Option<String>,
}

/// What `draftbuilder.drawing.purge` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PurgeResult {
    /// Whether the result lists a preview: nothing was removed.
    pub preview: bool,
    /// The number of passes that removed objects.
    pub passes: u64,
    /// Whether the passes stopped at [`MAX_PASSES`] while objects were still being removed.
    pub capped: bool,
    /// The objects removed, in the order of the passes.
    pub removed: Vec<Removed>,
}

/// The result of the passes of one run.
struct Sweep {
    removed: Vec<Removed>,
    passes: u64,
    capped: bool,
}

fn keys(category: Category) -> &'static [&'static str] {
    match category {
        Category::Layers => &[LAYER],
        Category::Linetypes => &[LINETYPE],
        Category::Blocks => &[BLOCK],
        Category::Styles => STYLES,
    }
}

/// Whether `name` is protected in `category`.
fn protected(category: Category, name: &str) -> bool {
    let names: &[&str] = match category {
        Category::Layers => &["0", "Defpoints"],
        Category::Linetypes => &["Continuous", "ByLayer", "ByBlock"],
        Category::Styles => &["Standard"],
        Category::Blocks => &[],
    };
    names.iter().any(|n| fold(n) == fold(name))
}

/// The lower-case bytes of every preserved record of the file, which may name an object.
pub(crate) fn preserved_names(snapshot: &Snapshot) -> Vec<Vec<u8>> {
    snapshot
        .foreign_rows()
        .filter(|r| {
            r.source_format == draftbuilder_model::database::FORMAT
                && matches!(r.record_kind.as_str(), "entity" | "object")
        })
        .map(|r| fold(&String::from_utf8_lossy(&r.data)).into_bytes())
        .collect()
}

/// Whether a preserved record may name `name`.
pub(crate) fn named_by_preserved(preserved: &[Vec<u8>], name: &str) -> bool {
    let needle = fold(name).into_bytes();
    !needle.is_empty()
        && preserved
            .iter()
            .any(|record| record.windows(needle.len()).any(|w| w == needle.as_slice()))
}

/// The objects a pass removes, in category order and identifier order.
fn candidates(snapshot: &Snapshot, categories: &[Category]) -> Vec<Removed> {
    let unused: BTreeSet<ObjectId> = arc_draft::purge::unused(snapshot).into_iter().collect();
    let preserved = preserved_names(snapshot);
    let mut out = Vec::new();
    for &category in categories {
        for key in keys(category) {
            let Ok(objects) = snapshot.iter_type(key) else {
                continue;
            };
            let mut ids: Vec<ObjectId> = objects
                .map(|(_, object)| object.id)
                .filter(|id| unused.contains(id))
                .collect();
            ids.sort();
            for id in ids {
                let name = name_of(snapshot, Some(id));
                // The current layer and the current linetype are kept whatever their name (DB-LAYER):
                // the drawing's settings name them.
                let current = crate::settings::is_current_layer(snapshot, id)
                    || crate::settings::current_linetype(snapshot) == Some(id);
                let keep = current
                    || name.as_deref().is_some_and(|n| {
                        protected(category, n) || named_by_preserved(&preserved, n)
                    });
                if !keep {
                    out.push(Removed {
                        category,
                        object: id.to_string(),
                        name,
                    });
                }
            }
        }
    }
    out
}

/// One pass: deletes the candidates of the transaction's state and applies the delete rules.
fn pass(txn: &mut Txn, categories: &[Category], limits: &Limits) -> Result<Vec<Removed>> {
    let found = candidates(txn.snapshot(), categories);
    if found.is_empty() {
        return Ok(found);
    }
    for item in &found {
        let id: ObjectId = item
            .object
            .parse()
            .map_err(|_| arc_base::Error::new(arc_base::error::codes::ID_INVALID))?;
        // An object already removed in this pass (an entity of a block removed with it) is skipped.
        if txn.snapshot().resolve(id).is_ok() {
            txn.delete(id)?;
        }
    }
    txn.apply_delete_rules(limits)?;
    Ok(found)
}

/// The passes of one run: one pass, or with `repeat` every pass up to [`MAX_PASSES`].
fn sweep(ctx: &mut Context<'_>, categories: &[Category], repeat: bool) -> Result<Sweep> {
    let limits = *ctx.limits();
    let mut out = Sweep {
        removed: Vec::new(),
        passes: 0,
        capped: false,
    };
    for number in 1..=MAX_PASSES {
        ctx.checkpoint()?;
        let found = pass(ctx.txn()?, categories, &limits)?;
        if found.is_empty() {
            break;
        }
        out.passes += 1;
        out.removed.extend(found);
        if !repeat {
            break;
        }
        if number == MAX_PASSES {
            out.capped = true;
        }
    }
    Ok(out)
}

fn purge(ctx: &mut Context<'_>, p: PurgeParams) -> Result<PurgeResult> {
    let categories = p.categories.unwrap_or_else(|| ALL_CATEGORIES.to_vec());
    if p.preview {
        // The passes run as in a run, then the savepoint restores the transaction.
        let savepoint = ctx.txn()?.savepoint()?;
        let swept = sweep(ctx, &categories, p.repeat);
        let txn = ctx.txn()?;
        txn.rollback_to(&savepoint)?;
        txn.release(&savepoint)?;
        let swept = swept?;
        return Ok(result(true, swept));
    }
    let swept = sweep(ctx, &categories, p.repeat)?;
    Ok(result(false, swept))
}

fn result(preview: bool, swept: Sweep) -> PurgeResult {
    PurgeResult {
        preview,
        passes: swept.passes,
        capped: swept.capped,
        removed: swept.removed,
    }
}

/// Registers `draftbuilder.drawing.purge` (permission class `edit`, undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, purge)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Edit]),
    )
}
