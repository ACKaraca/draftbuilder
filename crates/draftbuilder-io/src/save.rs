//! Writing the drawing database as DXF: the export of `arc-draft-dxf` with the handles and the
//! GUIDs of the database put back, and the product's loss items.

use arc_base::{ObjectId, Result};
use arc_doc::relationship::ReferenceField;
use arc_doc::{Snapshot, Value};
use arc_draft::order::sorted;
use arc_draft::types::ENTITIES;
use arc_draft_dxf::export_drawing;
use arc_draft_dxf::fields::Reader;
use arc_io_dxf::loss::LossReport;
use arc_io_dxf::model::HeaderVariable;
use arc_io_dxf::{Drawing, EntityKind, Group};
use draftbuilder_model::{Database, Fingerprint, Handle};

use crate::error::{FINGERPRINT_ASSIGNED, HANDLE_ASSIGNED};
use crate::open::{FINGERPRINT_VARIABLE, VERSION_VARIABLE};
use crate::report::note;

/// Handles below this are the export's own: its root dictionary has a fixed handle in this
/// range. An entity whose source handle is lower is written with an assigned handle.
pub const RESERVED_HANDLES: u64 = 0x20;

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
    /// The model space entities written with their source handle.
    pub carried: u64,
    /// The model space entities written with an assigned handle.
    pub assigned: u64,
}

/// The entities of the model space `space` in the order the export writes them: draw order, then
/// identifier.
pub fn space_entities(snapshot: &Snapshot, space: ObjectId) -> Vec<ObjectId> {
    let key_of = |id: ObjectId| -> Option<(String, i64)> {
        let handle = snapshot.resolve(id).ok()?;
        let key = snapshot.type_key(handle)?.as_str();
        if !ENTITIES.contains(&key) {
            return None;
        }
        let object = snapshot.get(handle).ok()?;
        let order = match Reader::new(snapshot.registry(), key, &object.values).get("draw_order") {
            Some(Value::I64(k)) => *k,
            _ => 0,
        };
        Some((key.to_owned(), order))
    };
    let keyed: Vec<(ObjectId, i64)> = snapshot
        .referrers(space)
        .filter(|r| r.field == ReferenceField::Owner)
        .filter_map(|r| key_of(r.source).map(|(_, order)| (r.source, order)))
        .collect();
    sorted(&keyed).into_iter().map(|(id, _)| id).collect()
}

fn set_variable(drawing: &mut Drawing, name: &str, text: String) {
    let values = vec![Group::text(2, text)];
    match drawing.header.iter_mut().find(|v| v.name == name) {
        Some(var) => var.values = values,
        None => drawing.header.push(HeaderVariable {
            name: name.to_owned(),
            values,
        }),
    }
}

/// Exports the model space `space` of `snapshot` with the handles of `database`: an entity
/// bound to a handle at or above [`RESERVED_HANDLES`] is written with it; every other entity,
/// and everything structural, receives a handle above the drawing's high-water mark. The
/// fingerprint and the version GUID of `request` go into the header.
///
/// If the model space entities cannot be matched one to one with the objects (the export left
/// one out), none is matched and all are reported as assigned.
///
/// # Errors
/// The errors of [`arc_draft_dxf::export_drawing`].
pub fn export(
    snapshot: &Snapshot,
    space: ObjectId,
    database: &Database,
    request: &Request,
) -> Result<Exported> {
    let mut exported = export_drawing(snapshot, space)?;
    let ids = space_entities(snapshot, space);
    let typed: Vec<&mut arc_io_dxf::Entity> = exported
        .drawing
        .entities
        .iter_mut()
        .filter(|e| !matches!(e.kind, EntityKind::Preserved(_)))
        .collect();
    let matched = typed.len() == ids.len();
    let count = typed.len() as u64;
    let mut carried = 0_u64;
    if matched {
        for (entity, id) in typed.into_iter().zip(ids) {
            let source = database
                .handle_of(id)
                .filter(|h: &Handle| h.value() >= RESERVED_HANDLES);
            if let Some(h) = source {
                entity.head.handle = Some(arc_io_dxf::Handle(h.value()));
                carried += 1;
            }
        }
    }
    let assigned = count - carried;
    let mut report = exported.report;
    note(&mut report, HANDLE_ASSIGNED, "entity", assigned);
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
    Ok(Exported {
        drawing,
        report,
        entities: exported.entities,
        carried,
        assigned,
    })
}
