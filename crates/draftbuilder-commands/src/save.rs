//! `draftbuilder.drawing.save_dxf`: writes the drawing database as a DXF file.

use std::io::Write;
use std::path::Path;

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use arc_draft_dxf::commands::DxfVersion;
use arc_io_dxf::{Progress, WriteOptions};
use draftbuilder_io::report::merge;
use draftbuilder_io::save::{Request, export};
use draftbuilder_model::database::{Database, Origin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, SAVE_PROGRESS, codes};
use crate::open::fresh_fingerprint;

/// The command identifier.
pub const ID: &str = "draftbuilder.drawing.save_dxf";
const LABEL: Message = Message {
    message_id: "db-cmd-save-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-save-description",
};

/// Parameters of `draftbuilder.drawing.save_dxf`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveParams {
    /// The file to write, inside the session's write roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
    /// The version to write; the default is the newest.
    #[serde(default)]
    pub version: DxfVersion,
    /// Whether to write the binary form instead of ASCII.
    #[serde(default)]
    pub binary: bool,
    /// Whether to save an independent copy: the copy gets a new fingerprint, so its objects
    /// have new stable identifiers. A plain save keeps the fingerprint.
    #[serde(default)]
    pub independent_copy: bool,
}

/// What `draftbuilder.drawing.save_dxf` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SaveResult {
    /// The bytes written.
    pub bytes: u64,
    /// The entities written, in the space and in blocks.
    pub entities: u64,
    /// Model space entities written with their source handle.
    pub handles_carried: u64,
    /// Model space entities written with a handle assigned by DraftBuilder.
    pub handles_assigned: u64,
    /// The fingerprint written, in the form files use.
    pub fingerprint: String,
    /// The loss and preservation report.
    pub report: serde_json::Value,
}

fn save(ctx: &mut Context<'_>, p: SaveParams) -> Result<SaveResult> {
    let snapshot = ctx.snapshot().clone();
    let space = crate::model_space(&snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let database = Database::of(&snapshot);
    let (fingerprint, assigned) = match (database.fingerprint(), p.independent_copy) {
        (Some(f), false) => (f, database.origin() == Some(Origin::Assigned)),
        _ => (fresh_fingerprint(ctx)?, true),
    };
    let request = Request {
        fingerprint,
        fingerprint_assigned: assigned,
        version_guid: fresh_fingerprint(ctx)?,
    };
    let exported = export(&snapshot, space, &database, &request)?;
    ctx.progress(0.5, SAVE_PROGRESS.message_id);
    let mut options = WriteOptions::for_version(p.version.version());
    options.binary = p.binary;
    let cancel = options.cancel.clone();
    let output = {
        let probe: &Context<'_> = ctx;
        let mut tick = |_: Progress| {
            if probe.checkpoint().is_err() {
                cancel.cancel();
            }
        };
        options.progress = Some(&mut tick);
        arc_io_dxf::write(&exported.drawing, &mut options)?
    };
    ctx.checkpoint()?;
    let mut file = ctx.create_write(Path::new(&p.path))?;
    file.write_all(&output.bytes)
        .and_then(|()| file.flush())
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    let mut report = exported.report;
    merge(&mut report, &output.report);
    Ok(SaveResult {
        bytes: u64::try_from(output.bytes.len()).unwrap_or(u64::MAX),
        entities: exported.entities,
        handles_carried: exported.carried,
        handles_assigned: exported.assigned,
        fingerprint: fingerprint.to_braced(),
        report: serde_json::to_value(&report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    })
}

/// Registers `draftbuilder.drawing.save_dxf` (permission classes `read` and `file_write`, not
/// undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, save)
            .with_undo(UndoBehavior::NotUndoable)
            .with_permissions([Permission::Read, Permission::FileWrite]),
    )
}
