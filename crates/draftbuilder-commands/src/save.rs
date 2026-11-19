//! `draftbuilder.drawing.save_dxf`: writes the drawing database as a DXF file.
//!
//! The command is undoable because it records, in its transaction, the handles the export
//! assigned to objects the file did not have. A save that assigns none changes nothing and
//! leaves no step in the history.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use arc_draft_dxf::commands::DxfVersion;
use arc_io_dxf::{Progress, WriteOptions};
use draftbuilder_io::report::merge;
use draftbuilder_io::save::{Request, export};
use draftbuilder_model::database::{self, Database, Origin};
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

/// The highest handle any save of this process has written. Undoing a save removes its recorded
/// handles from the document, but not from this mark, so a handle written to a file is not
/// issued again to another object in this process.
static WRITTEN: AtomicU64 = AtomicU64::new(0);

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
    /// Objects written with the handle they were bound to.
    pub handles_kept: u64,
    /// Objects that had no handle and were given one, now recorded in the document.
    pub handles_recorded: u64,
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
        floor: database.top().max(WRITTEN.load(Ordering::Relaxed)),
    };
    let exported = export(&snapshot, space, &request)?;
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
    // The file is written: record the handles it now holds for objects that had none.
    let new: Vec<_> = exported
        .handles
        .iter()
        .filter(|(id, _)| database.handle_of(*id).is_none())
        .copied()
        .collect();
    if let Some(top) = exported.handles.iter().map(|(_, h)| h.value()).max() {
        WRITTEN.fetch_max(top, Ordering::Relaxed);
    }
    if !new.is_empty() && !p.independent_copy {
        database::record(ctx.txn()?, &new)?;
    }
    let mut report = exported.report;
    merge(&mut report, &output.report);
    Ok(SaveResult {
        bytes: u64::try_from(output.bytes.len()).unwrap_or(u64::MAX),
        entities: exported.entities,
        handles_kept: (exported.handles.len() - new.len()) as u64,
        handles_recorded: if p.independent_copy {
            0
        } else {
            new.len() as u64
        },
        fingerprint: fingerprint.to_braced(),
        report: serde_json::to_value(&report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    })
}

/// Registers `draftbuilder.drawing.save_dxf` (permission classes `read`, `edit` and
/// `file_write`, undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, save)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Read, Permission::Edit, Permission::FileWrite]),
    )
}
