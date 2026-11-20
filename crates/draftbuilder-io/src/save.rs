//! Writing the drawing database as DXF: the export of `arc-draft-dxf` with the policy that
//! keeps the source handles, the GUIDs of the database in the header, and the product's loss
//! items.

use arc_base::{ObjectId, Result};
use arc_doc::Snapshot;
use arc_draft_dxf::{HandlePolicy, export_drawing_with};
use arc_io_dxf::loss::LossReport;
use arc_io_dxf::model::HeaderVariable;
use arc_io_dxf::{Drawing, Group};
use draftbuilder_model::database::SOURCE_DOCUMENT;
use draftbuilder_model::{Fingerprint, Handle};

use crate::error::FINGERPRINT_ASSIGNED;
use crate::open::{FINGERPRINT_VARIABLE, VERSION_VARIABLE};
use crate::report::note;

/// What an export needs besides the snapshot.
#[derive(Debug, Clone, Copy)]
pub struct Request {
    /// The fingerprint to write.
    pub fingerprint: Fingerprint,
    /// Whether the fingerprint is new (the drawing had none, or this is an independent copy);
    /// the report then says so.
    pub fingerprint_assigned: bool,
    /// The GUID of this save, which differs on every save.
    pub version_guid: Fingerprint,
    /// The drawing's handle high-water mark: handles the export assigns are above it.
    pub floor: u64,
}

/// The result of an export.
#[derive(Debug, Clone)]
pub struct Exported {
    /// The drawing, ready for [`arc_io_dxf::write`].
    pub drawing: Drawing,
    /// The report of the mapping, with the product's items.
    pub report: LossReport,
    /// The entities written, in the space and in blocks.
    pub entities: u64,
    /// The handle written for each exported object (entities, layers, linetypes, styles and
    /// block records).
    pub handles: Vec<(ObjectId, Handle)>,
}

fn set_variable(drawing: &mut Drawing, name: &str, text: String) {
    let values = vec![Group::text(2, text)];
    match drawing
        .header
        .iter_mut()
        .find(|v| v.name.eq_ignore_ascii_case(name))
    {
        Some(var) => var.values = values,
        None => drawing.header.push(HeaderVariable {
            name: name.to_owned(),
            values,
        }),
    }
}

/// Exports the model space `space` of `snapshot`, keeping the handles bound to the drawing's
/// source document and assigning the others above `request.floor`. The fingerprint and the
/// version GUID go into the header.
///
/// # Errors
/// The errors of [`arc_draft_dxf::export_drawing_with`].
pub fn export(snapshot: &Snapshot, space: ObjectId, request: &Request) -> Result<Exported> {
    let policy = HandlePolicy::KeepSources {
        document: SOURCE_DOCUMENT.to_owned(),
        floor: request.floor,
    };
    let exported = export_drawing_with(snapshot, space, &policy)?;
    let mut report = exported.report;
    if request.fingerprint_assigned {
        note(&mut report, FINGERPRINT_ASSIGNED, "drawing", 1);
    }
    let mut drawing = exported.drawing;
    set_variable(
        &mut drawing,
        FINGERPRINT_VARIABLE,
        request.fingerprint.to_braced(),
    );
    set_variable(
        &mut drawing,
        VERSION_VARIABLE,
        request.version_guid.to_braced(),
    );
    let handles = exported
        .handles
        .into_iter()
        .filter_map(|(id, h)| Handle::new(h.0).map(|h| (id, h)))
        .collect();
    Ok(Exported {
        drawing,
        report,
        entities: exported.entities,
        handles,
    })
}
