//! `draftbuilder.drawing.save` and `draftbuilder.drawing.save_dxf`: writes the drawing database as
//! a DXF file.
//!
//! The handles the export assigned to objects the file did not have are recorded in the document
//! as bookkeeping (`Context::commit_silently`) before the file is written (`Context::after_commit`
//! writes it once the record is committed): a file never carries a handle the document does not
//! know, and a failed write only burns handles. The record is journaled, but neither an undo step
//! nor a change to the redo stack, so a save is never undone and never shows in the step list. A
//! recorded handle stays with its object for the life of the document, also while undo removes the
//! object and redo brings it back. A save that assigns none changes nothing. The file is written
//! to a temporary file beside the target, which replaces the target only when it is complete, so
//! a failed save never leaves a truncated drawing. DWG writing is not available yet: a DWG target
//! is refused.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, Session, UndoBehavior};
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
    /// The arrays written as block references: each item is a reference, and the counts, spacings,
    /// angles and paths of the array are not written.
    pub arrays_as_references: u64,
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

/// Refuses a target the write is refused for: outside the write roots, not a regular file, or
/// read-only.
fn check_target_writable(session: &Session, path: &Path) -> Result<()> {
    check_regular_target(path)?;
    check_writable_target(path)?;
    session.roots.check_write(path)
}

/// Writes `bytes` to `path` through a temporary file beside it, then replaces `path` with it.
///
/// The target is checked against the write roots, must be a regular file or absent (a symbolic link
/// is refused, as it was before: the link itself is never replaced nor followed), and must not be
/// read-only. The temporary file is created exclusively with `Roots::create_new`, under a short name
/// with a random part. Its permissions are those of the file it replaces, set on the open handle before
/// any byte is written, so a private drawing is never readable under the default permissions. It is
/// written, synced and closed, then `Roots::replace_file` renames it over the target. On failure the
/// temporary file is removed and `path` keeps what it held.
///
/// The directory is synced after the rename on Unix. The file is in place by then, so a failed sync is
/// not an error: it is returned as a warning for the result.
///
/// When the command is the native save of the application, `path` is the empty temporary file the
/// application created beside its target; this writes that file, and the application renames it over the
/// target. The application does not carry the target's permissions over, so a native save takes the
/// permissions of its temporary file.
fn write_atomic(session: &Session, path: &Path, bytes: &[u8]) -> Result<Vec<String>> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    check_target_writable(session, path)?;
    let name = format!(
        ".draftbuilder-{:x}-{:016x}-{}.tmp",
        std::process::id(),
        unpredictable(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let temp_path = path.with_file_name(name);
    let result = (|| {
        let mut file = session.roots.create_new(&temp_path)?;
        if let Ok(meta) = std::fs::metadata(path) {
            file.set_permissions(meta.permissions())
                .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
        }
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
        drop(file);
        session.roots.replace_file(&temp_path, path)
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
    // An array is written as block references and reported as approximated, which the check
    // skips. The export drops an array it cannot place (a path it cannot read, or more items than an
    // array may hold); that drop refuses the save.
    arc_draft::array::ARRAY,
];

/// The number of arrays the report says were written as block references.
fn report_arrays(value: &serde_json::Value) -> u64 {
    let count = value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| {
            item["record_kind"] == arc_draft::array::ARRAY && item["outcome"] == "approximated"
        })
        .count();
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// Fails with `db.cmd.export_drops_objects` when the export report drops objects of a kind the product
/// creates: the detail `kind` names the first such kind and `count` the number dropped in all. The
/// report is the serialized loss report of the export.
fn refuse_dropped(report: &serde_json::Value) -> Result<()> {
    let mut first: Option<String> = None;
    let mut count = 0_u64;
    for item in report["items"].as_array().into_iter().flatten() {
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
    // A target the write would refuse fails here, before anything is recorded; the write checks
    // again, since the target can change in between.
    check_target_writable(ctx.session(), Path::new(&p.path))?;
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
        floor: database.top(),
    };
    let mut exported = export(&snapshot, space, &request)?;
    // The drawing's settings replace the header values the export wrote under the same names.
    for variable in crate::settings::header::header_values(&snapshot) {
        exported
            .drawing
            .header
            .retain(|h| !h.name.eq_ignore_ascii_case(&variable.name));
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
    refuse_dropped(
        &serde_json::to_value(&exported.report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    )?;
    // The handles are recorded in the document before the file is written: the invoker commits the
    // record and only then runs the write, so a file never carries a handle the document does not
    // know. A write that fails afterwards only burns handles, which are never issued again.
    let path = Path::new(&p.path);
    let new: Vec<_> = exported
        .handles
        .iter()
        .filter(|(id, _)| database.handle_of(*id).is_none())
        .copied()
        .collect();
    let recorded = if !new.is_empty() && !p.independent_copy {
        ctx.commit_silently()?;
        database::record(ctx.txn()?, &new)?
    } else {
        0
    };
    let (target, bytes) = (path.to_owned(), output.bytes);
    let written = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    ctx.after_commit(move |session, value| {
        let warnings = write_atomic(session, &target, &bytes)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("warnings".to_owned(), serde_json::json!(warnings));
        }
        Ok(())
    })?;
    let mut report = exported.report;
    merge(&mut report, &output.report);
    let arrays_as_references = report_arrays(
        &serde_json::to_value(&report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    );
    Ok(SaveResult {
        bytes: written,
        entities: exported.entities,
        handles_kept: (exported.handles.len() - new.len()) as u64,
        handles_recorded: recorded as u64,
        fingerprint: fingerprint.to_braced(),
        arrays_as_references,
        warnings: Vec::new(),
        report: serde_json::to_value(&report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    })
}

/// Registers `draftbuilder.drawing.save_dxf` and `draftbuilder.drawing.save` (permission classes
/// `read`, `edit` and `file_write`; the recorded handles commit silently).
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

#[cfg(test)]
mod tests {
    use super::refuse_dropped;
    use arc_base::error::Value;
    use serde_json::json;

    #[test]
    fn a_dropped_product_kind_is_refused_with_its_count() {
        let report = json!({"items": [
            {"record_kind": "core.draft.array", "outcome": "approximated", "arguments": {"count": 4}},
            {"record_kind": "core.draft.line", "outcome": "dropped", "arguments": {"count": 2}},
            {"record_kind": "core.draft.circle", "outcome": "dropped"},
            {"record_kind": "leader", "outcome": "dropped", "arguments": {"count": 9}},
        ]});
        let error = refuse_dropped(&report).unwrap_err();
        assert_eq!(error.code(), "db.cmd.export_drops_objects");
        assert_eq!(
            error.details().get("kind"),
            Some(&Value::from("core.draft.line"))
        );
        assert_eq!(error.details().get("count"), Some(&Value::from("3")));
    }

    #[test]
    fn a_dropped_array_is_refused_and_an_approximated_one_is_not() {
        let dropped = json!({"items": [
            {"record_kind": "core.draft.array", "outcome": "dropped", "arguments": {"count": 3}},
        ]});
        let error = refuse_dropped(&dropped).unwrap_err();
        assert_eq!(
            error.details().get("kind"),
            Some(&Value::from("core.draft.array"))
        );
        let approximated = json!({"items": [
            {"record_kind": "core.draft.array", "outcome": "approximated", "arguments": {"count": 3}},
        ]});
        assert!(refuse_dropped(&approximated).is_ok());
    }

    #[test]
    fn approximated_arrays_and_other_dropped_kinds_are_not_refused() {
        let report = json!({"items": [
            {"record_kind": "core.draft.array", "outcome": "approximated", "arguments": {"count": 4}},
            {"record_kind": "leader", "outcome": "dropped", "arguments": {"count": 9}},
        ]});
        assert!(refuse_dropped(&report).is_ok());
    }
}
