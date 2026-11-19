//! The commands and document type set of DraftBuilder, headless.
//!
//! [`types`] is the document type set: the drafting types of `arc-draft`. [`registry`] is the
//! command set: the `core.draft.*` commands plus the product's
//!
//! - `draftbuilder.drawing.open_dxf` ([`open`]): reads a DXF file into the document as a
//!   drawing database (entities with their source handles, the fingerprint);
//! - `draftbuilder.drawing.save_dxf` ([`save`]): writes the drawing as DXF with those handles;
//! - `draftbuilder.modify.move` ([`modify`]): moves objects by an offset.
//!
//! The product layer wires and names: reading and writing the format is `arc-io-dxf` and
//! `arc-draft-dxf`, the drawing database is `draftbuilder-model`, the mapping between them is
//! `draftbuilder-io`.
//!
//! No function in this crate panics.

pub mod error;
pub mod modify;
pub mod open;
pub mod save;

use std::sync::Arc;

use arc_base::{ObjectId, Result};
use arc_command::headless::Product;
use arc_command::{Clock, Registry};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, Snapshot, TypeRegistry};
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
    modify::register(&mut registry)?;
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
