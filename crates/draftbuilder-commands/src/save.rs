//! `draftbuilder.drawing.save` and `draftbuilder.drawing.save_dxf`: writes the drawing database as
//! a DXF file.
//!
//! The command is undoable because it records, in its transaction, the handles the export
//! assigned to objects the file did not have. A save that assigns none changes nothing and
//! leaves no step in the history. The file is written to a temporary file beside the target,
//! which replaces the target only when it is complete, so a failed save never leaves a truncated
//! drawing. DWG writing is not available yet: a DWG target is refused.

use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

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
use crate::open::{Format, fresh_fingerprint};

/// The identifier of the DXF save.
pub const ID: &str = "draftbuilder.drawing.save_dxf";
/// The identifier of the save.
pub const SAVE_ID: &str = "draftbuilder.drawing.save";
const LABEL: Message = Message {
    message_id: "db-cmd-save-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-save-description",
};
const SAVE_LABEL: Message = Message {
    message_id: "db-cmd-save-drawing-label",
};
const SAVE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-save-drawing-description",
};

/// How many drawings the process keeps a handle mark for. The least recently used mark goes first.
const MARKS_CAP: usize = 16;

/// The highest handle each drawing has had written to a file in this process, by its fingerprint.
///
/// The mark protects against one case: a save that is undone removes its handles from the document,
/// so a later object of the same drawing could get a handle that a file of this process already
/// holds. The mark keeps that handle out of the handles the drawing issues. Other drawings have other
/// fingerprints and their marks do not affect this drawing's handles.
static MARKS: Mutex<VecDeque<(String, u64)>> = Mutex::new(VecDeque::new());

/// The marks, behind their lock; a poisoned lock still gives its marks, which are plain data.
fn marks() -> MutexGuard<'static, VecDeque<(String, u64)>> {
    MARKS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The highest handle written to a file for the drawing `key` in this process; 0 when none.
fn mark_of(key: &str) -> u64 {
    let mut marks = marks();
    match marks.iter().position(|(k, _)| k == key) {
        Some(index) => {
            let entry = marks.remove(index);
            let value = entry.as_ref().map_or(0, |(_, value)| *value);
            if let Some(entry) = entry {
                marks.push_front(entry);
            }
            value
        }
        None => 0,
    }
}

/// Records that the drawing `key` has had handles up to `top` written, and keeps the table bounded.
fn raise_mark(key: &str, top: u64) {
    let mut marks = marks();
    let value = match marks.iter().position(|(k, _)| k == key) {
        Some(index) => marks.remove(index).map_or(top, |(_, value)| value.max(top)),
        None => top,
    };
    marks.push_front((key.to_owned(), value));
    marks.truncate(MARKS_CAP);
}

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
    /// Warnings about a save that completed: the file is in place, but a step after the replace (the
    /// directory sync on Unix) failed.
    pub warnings: Vec<String>,
}

/// The unpredictable part of a temporary name: a hash of a fresh random state (the standard
/// library seeds it from the operating system) mixed with the time, so a name cannot be guessed
/// from the process and the time alone.
fn unpredictable() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    hasher.write_u128(nanos);
    hasher.finish()
}

/// Refuses a target that exists and is not a regular file: a directory, a link, a device.
fn check_regular_target(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() => Err(Error::new(codes::TARGET_NOT_FILE)),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::new(arc_command::error::codes::PATH_IO).with_source(e)),
    }
}

/// Refuses a target that exists and is marked read-only: the save would replace it, which its owner
/// did not allow by making it read-only.
fn check_writable_target(path: &Path) -> Result<()> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.permissions().readonly() => Err(Error::new(codes::TARGET_READ_ONLY)),
        _ => Ok(()),
    }
}

/// Writes `bytes` to `path` through a temporary file beside it, then replaces `path` with it.
///
/// The target is checked against the write roots, must be a regular file or absent (a symbolic link
/// is refused, as it was before: the link itself is never replaced nor followed), and must not be
/// read-only. The temporary file is created exclusively with `Context::create_new`, under a short name
/// with a random part. Its permissions are those of the file it replaces, set on the open handle before
/// any byte is written, so a private drawing is never readable under the default permissions. It is
/// written, synced and closed, then `Context::replace_file` renames it over the target. On failure the
/// temporary file is removed and `path` keeps what it held.
///
/// The directory is synced after the rename on Unix. The file is in place by then, so a failed sync is
/// not an error: it is returned as a warning for the result, and the caller still records what the file
/// holds.
///
/// When the command is the native save of the application, `path` is the empty temporary file the
/// application created beside its target; this writes that file, and the application renames it over the
/// target. The application does not carry the target's permissions over, so a native save takes the
/// permissions of its temporary file.
fn write_atomic(ctx: &Context<'_>, path: &Path, bytes: &[u8]) -> Result<Vec<String>> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    check_regular_target(path)?;
    check_writable_target(path)?;
    ctx.session().roots.check_write(path)?;
    let name = format!(
        ".draftbuilder-{:x}-{:016x}-{}.tmp",
        std::process::id(),
        unpredictable(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let temp_path = path.with_file_name(name);
    let result = (|| {
        let mut file = ctx.create_new(&temp_path)?;
        if let Ok(meta) = std::fs::metadata(path) {
            file.set_permissions(meta.permissions())
                .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
        }
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
        drop(file);
        ctx.replace_file(&temp_path, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result?;
    let mut warnings = Vec::new();
    if let Err(error) = sync_parent(path) {
        warnings.push(format!(
            "{}: the directory was not synced after the replace",
            error.code()
        ));
    }
    Ok(warnings)
}

/// Syncs the directory holding `path`, so the rename survives a power loss (Unix only; Windows does not
/// sync directories).
fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let parent = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        std::fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| {
                Error::new(arc_command::error::codes::PATH_IO)
                    .with_detail("reason", "sync")
                    .with_source(e)
            })?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Object kinds the product itself creates: an export that drops one of them would lose work the user
/// made, so the save refuses. Other dropped kinds (preserved or foreign ones) are only reported.
const PRODUCT_KINDS: &[&str] = &[
    arc_draft::types::POINT,
    arc_draft::types::LINE,
    arc_draft::types::RAY,
    arc_draft::types::CONSTRUCTION_LINE,
    arc_draft::types::CIRCLE,
    arc_draft::types::ARC,
    arc_draft::types::ELLIPSE,
    arc_draft::types::ELLIPTICAL_ARC,
    arc_draft::types::POLYLINE,
    arc_draft::types::SPLINE,
    arc_draft::types::BLOCK_REF,
    arc_draft::array::ARRAY,
];

/// Fails with `db.cmd.export_drops_objects` when the export report drops objects of a kind the product
/// creates: the detail `kind` names the first such kind and `count` the number dropped in all.
fn refuse_dropped(report: &arc_io_dxf::loss::LossReport) -> Result<()> {
    let value = serde_json::to_value(report)
        .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?;
    let mut first: Option<String> = None;
    let mut count = 0_u64;
    for item in value["items"].as_array().into_iter().flatten() {
        let kind = item["record_kind"].as_str().unwrap_or_default();
        if item["outcome"] != "dropped" || !PRODUCT_KINDS.contains(&kind) {
            continue;
        }
        count += item["arguments"]["count"].as_u64().unwrap_or(1);
        first.get_or_insert_with(|| kind.to_owned());
    }
    match first {
        Some(kind) => Err(Error::new(codes::EXPORT_DROPS_OBJECTS)
            .with_detail("kind", kind)
            .with_detail("count", count.to_string())),
        None => Ok(()),
    }
}

/// Refuses a DWG target: DWG writing is not available yet. A name ending in `.dwg` counts, even
/// a file named exactly `.dwg`. Any other name is written as DXF; the application's atomic save
/// writes through a temporary name, so the extension is not required to be `dxf`.
fn check_target(path: &str) -> Result<()> {
    if matches!(Format::of_path(Path::new(path)), Ok(Format::Dwg)) {
        return Err(Error::new(codes::FORMAT_UNSUPPORTED));
    }
    Ok(())
}

/// `draftbuilder.drawing.save_dxf`: writes the drawing as DXF.
fn save_dxf(ctx: &mut Context<'_>, p: SaveParams) -> Result<SaveResult> {
    check_target(&p.path)?;
    save_drawing(ctx, p)
}

/// `draftbuilder.drawing.save`: writes the drawing as DXF; a DWG target is refused.
fn save(ctx: &mut Context<'_>, p: SaveParams) -> Result<SaveResult> {
    check_target(&p.path)?;
    save_drawing(ctx, p)
}

fn save_drawing(ctx: &mut Context<'_>, p: SaveParams) -> Result<SaveResult> {
    let snapshot = ctx.snapshot().clone();
    let space = crate::model_space(&snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let database = Database::of(&snapshot);
    let (fingerprint, assigned) = match (database.fingerprint(), p.independent_copy) {
        (Some(f), false) => (f, database.origin() == Some(Origin::Assigned)),
        _ => (fresh_fingerprint(ctx)?, true),
    };
    let key = fingerprint.to_braced();
    let request = Request {
        fingerprint,
        fingerprint_assigned: assigned,
        version_guid: fresh_fingerprint(ctx)?,
        floor: database.top().max(mark_of(&key)),
    };
    let mut exported = export(&snapshot, space, &request)?;
    // The drawing's settings replace the header values the export wrote under the same names.
    for variable in crate::settings::header::header_values(&snapshot) {
        exported.drawing.header.retain(|h| h.name != variable.name);
        exported.drawing.header.push(variable);
    }
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
    refuse_dropped(&exported.report)?;
    let warnings = write_atomic(ctx, Path::new(&p.path), &output.bytes)?;
    // The file is written: record the handles it now holds for objects that had none.
    let new: Vec<_> = exported
        .handles
        .iter()
        .filter(|(id, _)| database.handle_of(*id).is_none())
        .copied()
        .collect();
    if let Some(top) = exported.handles.iter().map(|(_, h)| h.value()).max() {
        raise_mark(&key, top);
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
        warnings,
        report: serde_json::to_value(&report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    })
}

/// Registers `draftbuilder.drawing.save_dxf` and `draftbuilder.drawing.save` (permission classes
/// `read`, `edit` and `file_write`, undoable).
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, save_dxf)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Read, Permission::Edit, Permission::FileWrite]),
    )?;
    registry.register(
        CommandSpec::new(
            SAVE_ID,
            SAVE_LABEL.message_id,
            SAVE_DESCRIPTION.message_id,
            save,
        )
        .with_undo(UndoBehavior::Undoable)
        .with_permissions([Permission::Read, Permission::Edit, Permission::FileWrite]),
    )
}
