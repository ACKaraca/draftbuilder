//! The drawing database of DraftBuilder, headless (ADR-0001, ADR-0002).
//!
//! The in-memory model is the `arc-doc` document of `arc-draft` objects. This crate adds what
//! the DWG/DXF database has and `arc-draft` does not: handles, the drawing fingerprint and the
//! stable identifiers derived from both.
//!
//! - [`Handle`] is a drawing-unique, never-zero number.
//! - [`stable_id`] derives an object's 128-bit identifier from the [`Fingerprint`] and the
//!   handle; nothing is stored for it.
//! - [`database::Database`] reads the handle table (the source map `arc-draft-dxf` writes), the
//!   handle high-water mark and the fingerprint from a snapshot; [`database::record`],
//!   [`database::raise_top`] and [`database::set_fingerprint`] change them inside a transaction,
//!   so undo and the journal cover them.
//!
//! No function in this crate panics.

pub mod database;
pub mod handle;
pub mod identity;
mod sha1;

pub use database::{Database, Origin};
pub use handle::Handle;
pub use identity::{Fingerprint, StableId, stable_id};
