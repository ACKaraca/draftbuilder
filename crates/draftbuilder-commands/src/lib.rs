//! The commands and document type set of DraftBuilder, headless.
//!
//! [`types`] is the document type set: the drafting types of `arc-draft`. [`registry`] is the
//! command set: the `core.draft.*` commands of `arc-draft` plus the product's
//!
//! - `draftbuilder.drawing.open` ([`open`]) and `draftbuilder.drawing.open_dxf`: read a DWG or
//!   DXF file into the document as a drawing database (entities with their source handles, the
//!   fingerprint, the units and the loss report);
//! - `draftbuilder.drawing.new` ([`new_drawing`]): an empty drawing from a unit system or a template;
//! - `draftbuilder.drawing.save` and `draftbuilder.drawing.save_dxf` ([`save`]): write the drawing
//!   as DXF through a temporary file, with the handles and fingerprint it was opened with;
//! - `draftbuilder.drawing.audit` ([`audit`]): reports the checks that fail, repairs nothing;
//! - `draftbuilder.drawing.purge` ([`purge`]): removes unused objects, with a preview.
//!
//! The names of these commands and of the commands the product uses (`MOVE`, alias `M`) are
//! the rows of [`vocabulary`]. The per-area modules ([`draw`], [`modify`], [`select`], [`layer`],
//! [`settings`], [`precision`]) register the commands of their area and apply their rows; their
//! registration is called from [`registry`], the one registration point. Name prefixes
//! (`_`, `-` and `.`) are read by [`names`].
//!
//! The product layer wires and names: reading and writing the formats is `arc-io-dxf`,
//! `arc-io-dwg` and `arc-draft-dxf`, the drawing database is `draftbuilder-model`, the mapping
//! between them is `draftbuilder-io`.
//!
//! No function in this crate panics.

pub mod audit;
pub mod draw;
pub mod error;
pub mod fold;
pub mod interaction;
pub mod layer;
pub mod modify;
pub mod names;
pub mod new_drawing;
pub mod open;
pub mod precision;
pub mod property;
pub mod purge;
pub mod save;
pub mod script;
pub mod select;
pub mod settings;
pub mod undo;
pub mod view;
pub mod vocabulary;

use std::path::Path;
use std::sync::Arc;

use arc_base::{ObjectId, Result};
use arc_command::headless::Product;
use arc_command::{Clock, Invocation, Invoker, Registry, Session};
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

/// The command set: the `core.draft.*` commands and the product's commands, and the vocabulary
/// names of all of them.
///
/// # Errors
/// `cmd.duplicate` when a command or name is registered twice, and `cmd.unknown` when a vocabulary
/// row names a command that is not registered; neither occurs for this set.
pub fn registry() -> Result<Registry> {
    let mut registry = Registry::new();
    arc_draft::register_commands(&mut registry)?;
    open::register(&mut registry)?;
    save::register(&mut registry)?;
    new_drawing::register(&mut registry)?;
    audit::register(&mut registry)?;
    purge::register(&mut registry)?;
    draw::register(&mut registry)?;
    modify::register(&mut registry)?;
    select::register(&mut registry)?;
    layer::register(&mut registry)?;
    property::register(&mut registry)?;
    settings::register(&mut registry)?;
    precision::register(&mut registry)?;
    view::register(&mut registry)?;
    vocabulary::register_names(&mut registry, vocabulary::Area::Drawing)?;
    names::install(&mut registry)?;
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
        last: Some(Arc::new(last_objects)),
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

/// The product's answer for the keyword `last`: the topmost object of the model space in draw order.
/// Every session the product serves has it: the headless runner and the protocol server take it from
/// [`product`], and the application gives it to its sessions with `ProductBuilder::last_objects`.
#[must_use]
pub fn last_objects(snapshot: &Snapshot) -> Vec<ObjectId> {
    select::last_objects(snapshot)
}

/// Opens the DXF drawing at `path` as a new document with the product's types and validators:
/// the native open of the application. `session` is the application's session with the read
/// root of the file's directory, as the application passes it.
///
/// # Errors
/// The errors of the document and of `draftbuilder.drawing.open_dxf`.
pub fn open_document(path: &Path, config: DocumentConfig, session: &Session) -> Result<Document> {
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
    Invoker::new(&commands).invoke(&document, session, invocation)?;
    Ok(document)
}

/// Saves `document` to `path` with `draftbuilder.drawing.save_dxf`: the native save of the
/// application. The application gives an existing, empty temporary file beside the target; the
/// command writes it and the application renames it over the target.
///
/// # Errors
/// The errors of the command.
pub fn save_document(document: &Document, path: &Path, session: &Session) -> Result<()> {
    let commands = registry()?;
    let invocation = Invocation {
        seed: fresh_seed(),
        ..Invocation::new(
            save::ID,
            serde_json::json!({"path": path.to_string_lossy()}),
        )
    };
    Invoker::new(&commands).invoke(document, session, invocation)?;
    Ok(())
}
