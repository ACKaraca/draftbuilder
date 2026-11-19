//! DraftBuilder's mapping between the drawing database and DXF, headless.
//!
//! Reading and writing the format is `arc-io-dxf` (neutral model) and `arc-draft-dxf` (mapping
//! to the drafting model); this crate adds what the drawing database needs beyond them:
//!
//! - [`open`]: the fingerprint, the unit and the handle high-water mark of a file, and the
//!   source handle of every entity the import plan creates (the plan holds them, the import
//!   does not apply them);
//! - [`save`]: the export with the source handles of the database and the GUIDs written back,
//!   and the loss items of the product ([`error`]).
//!
//! No function in this crate panics.

pub mod error;
pub mod open;
pub mod report;
pub mod save;
