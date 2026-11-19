//! The commands and document type set of DraftBuilder, headless.
//!
//! [`types`] is the document type set: the drafting types of `arc-draft`. [`registry`] is the
//! command set: the `core.draft.*` commands plus the product's
//!
//! - `draftbuilder.drawing.open_dxf` ([`open`]): reads a DXF file into the document as a
//!   drawing database (entities with their source handles, the fingerprint);
//! - `draftbuilder.drawing.save_dxf` ([`save`]): writes the drawing as DXF with those handles and
//!   records the handles it assigns;
//!
//! Move is the core command `core.draft.entity.transform` under the alias `move`: it takes
//! `{"objects": [...], "transform": {"translation": [dx, dy]}}`.
//!
//! The product layer wires and names: reading and writing the format is `arc-io-dxf` and
//! `arc-draft-dxf`, the drawing database is `draftbuilder-model`, the mapping between them is
//! `draftbuilder-io`.
//!
//! No function in this crate panics.

pub mod error;
pub mod open;
pub mod save;

use std::path::Path;
use std::sync::Arc;

use arc_base::{ObjectId, Result};
use arc_command::headless::Product;
use arc_command::{Clock, Invocation, Invoker, Permission, Registry, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, DocumentConfig, Snapshot, TypeRegistry};
use arc_draft::types::{SPACE, SpaceKind};
use arc_draft_dxf::fields::Reader;

/// The name the product's headless runner reports.
pub const PRODUCT_NAME: &str = "draftbuilder";

/// The document type set: the drafting types of `arc-draft`.
///
/// # Errors
/// The registry's errors; none occur for these declarations.
pub fn types() -> Result<TypeRegistry> {
    let mut registry = TypeRegistry::new();
    arc_draft::register_types(&mut registry)?;
    Ok(registry)
}

/// The command set: the `core.draft.*` commands and the product's commands.
///
/// # Errors
/// `cmd.duplicate` when a command is registered twice; none occurs for this set.
pub fn registry() -> Result<Registry> {
    let mut registry = Registry::new();
    arc_draft::register_commands(&mut registry)?;
    open::register(&mut registry)?;
    save::register(&mut registry)?;
    registry.add_user_alias("move", "core.draft.entity.transform")?;
    Ok(registry)
}

/// The product of the headless runner: the type set and the command set.
///
/// # Errors
/// The errors of [`types`] and [`registry`].
pub fn product(clock: Arc<dyn Clock>, owner: LockOwner) -> Result<Product> {
    Ok(Product {
        name: PRODUCT_NAME.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        types: Arc::new(types()?),
        evaluators: Arc::new(Evaluators::new()),
        registry: registry()?,
        clock,
        owner,
    })
}

/// Adds the drafting validators (block nesting, names, layout owners) to `document`. A document
/// the shared runner opens has none; the callers that open documents themselves install them.
pub fn install(document: &Document) {
    arc_draft::install(document);
}

/// The first model space of `snapshot`, by identifier; `None` when there is none.
#[must_use]
pub fn model_space(snapshot: &Snapshot) -> Option<ObjectId> {
    let spaces = snapshot.iter_type(SPACE).ok()?;
    spaces
        .filter(|(_, object)| {
            Reader::new(snapshot.registry(), SPACE, &object.values).enum_code("kind")
                == Some(SpaceKind::Model as u32)
        })
        .map(|(_, object)| object.id)
        .min()
}

/// A seed that differs between calls, for the identifiers a command generates.
fn fresh_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// A command session over `dir`: read access, and write access with `write`.
fn file_session(dir: &Path, write: bool) -> Result<Session> {
    let mut granted = vec![Permission::Read, Permission::Edit, Permission::FileRead];
    if write {
        granted.push(Permission::FileWrite);
    }
    let mut session = Session::new(Surface::Automation, granted);
    session.roots = if write {
        Roots::new([dir], [dir])?
    } else {
        Roots::new([dir], std::iter::empty::<&Path>())?
    };
    Ok(session)
}

fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Opens the DXF drawing at `path` as a new document with the product's types and validators:
/// the native open of the application.
///
/// # Errors
/// The errors of the document and of `draftbuilder.drawing.open_dxf`.
pub fn open_document(path: &Path, config: DocumentConfig) -> Result<Document> {
    let document = Document::open(
        Snapshot::empty(Arc::new(types()?)),
        Arc::new(Evaluators::new()),
        config,
    )?;
    install(&document);
    let absolute = std::fs::canonicalize(path)
        .map_err(|e| arc_base::Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    let commands = registry()?;
    let invocation = Invocation {
        seed: fresh_seed(),
        ..Invocation::new(
            open::ID,
            serde_json::json!({"path": absolute.to_string_lossy()}),
        )
    };
    Invoker::new(&commands).invoke(
        &document,
        &file_session(parent_of(&absolute), false)?,
        invocation,
    )?;
    Ok(document)
}

/// Saves `document` to the DXF file at `path` (which need not exist yet) with
/// `draftbuilder.drawing.save_dxf`: the native save of the application.
///
/// # Errors
/// The errors of the command; the directory of `path` must exist.
pub fn save_document(document: &Document, path: &Path) -> Result<()> {
    let dir = std::fs::canonicalize(parent_of(path))
        .map_err(|e| arc_base::Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    let target = dir.join(path.file_name().unwrap_or_default());
    let commands = registry()?;
    let invocation = Invocation {
        seed: fresh_seed(),
        ..Invocation::new(
            save::ID,
            serde_json::json!({"path": target.to_string_lossy()}),
        )
    };
    Invoker::new(&commands).invoke(document, &file_session(&dir, true)?, invocation)?;
    Ok(())
}
