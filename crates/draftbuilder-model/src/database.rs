//! The drawing database prototype (ADR-0001, ADR-0002): the state a drawing keeps beside its
//! `arc-draft` objects, stored in the `arc-doc` document so that it is journaled, undone with
//! the commands that change it and saved with the package.
//!
//! The handles themselves are the **source map** of `arc-doc` that `arc-draft-dxf` writes on
//! import and reads on export: one record per bound object, format `dxf`, document
//! [`SOURCE_DOCUMENT`], identifier the handle in hexadecimal. [`Database`] is a view over it. What
//! DraftBuilder adds, as foreign data rows of the same format, is what the source map lacks:
//!
//! - the **handle high-water mark**: the highest handle the drawing has ever had, from the file
//!   or from a recorded binding. It is the `floor` of the export. Rows of deleted objects stay,
//!   so a deleted object's handle is not issued again;
//! - the **fingerprint** and where it came from.

use std::collections::BTreeMap;

use arc_base::id::Identifier;
use arc_base::{ObjectId, Result};
use arc_doc::{Snapshot, SourceMap, StoredForeign, Txn};

use crate::handle::Handle;
use crate::identity::{Fingerprint, StableId, stable_id};

/// The `source_format` of every row of the drawing database.
pub const FORMAT: &str = "dxf";
/// The label of the source document of the drawing's bindings: the `source_document` of the
/// import and the `document` of the export. A drawing has one.
pub const SOURCE_DOCUMENT: &str = "drawing";
/// The `record_kind` of the high-water mark row.
const TOP_KIND: &str = "handle_top";
/// The `record_kind` of the fingerprint row.
const FINGERPRINT_KIND: &str = "fingerprint";

/// Where a drawing's fingerprint came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The file carried it.
    File,
    /// The file had none (or the null GUID) and DraftBuilder assigned one; the next save writes
    /// it and says so in its report.
    Assigned,
}

impl Origin {
    fn text(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Assigned => "assigned",
        }
    }
}

/// The drawing database of one snapshot: its fingerprint, its handle high-water mark and the
/// handle of each bound object.
#[derive(Debug, Clone, Default)]
pub struct Database {
    fingerprint: Option<(Fingerprint, Origin)>,
    top: u64,
    by_object: BTreeMap<ObjectId, Handle>,
    by_handle: BTreeMap<Handle, ObjectId>,
}

impl Database {
    /// The database of `snapshot`, built in time proportional to its foreign data rows. A row
    /// that is not well formed is ignored; of two objects bound to one handle the one bound
    /// first (lowest row) keeps it.
    #[must_use]
    pub fn of(snapshot: &Snapshot) -> Self {
        let mut db = Self::default();
        for row in snapshot
            .foreign_rows()
            .filter(|r| r.source_format == FORMAT)
        {
            let handle = row.source_id.as_deref().and_then(Handle::from_hex);
            if let Some(h) = handle {
                db.top = db.top.max(h.value());
            }
            match (row.record_kind.as_str(), handle) {
                (arc_doc::source_map::RECORD_KIND, Some(h))
                    if row.data == SOURCE_DOCUMENT.as_bytes() =>
                {
                    let object = row
                        .object
                        .as_deref()
                        .and_then(|b| <[u8; 16]>::try_from(b).ok())
                        .map(|b| ObjectId::from_u128(u128::from_be_bytes(b)));
                    if let Some(object) = object.filter(|o| snapshot.resolve(*o).is_ok())
                        && !db.by_handle.contains_key(&h)
                        && !db.by_object.contains_key(&object)
                    {
                        db.by_handle.insert(h, object);
                        db.by_object.insert(object, h);
                    }
                }
                (FINGERPRINT_KIND, _) => db.fingerprint = read_fingerprint(row),
                _ => {}
            }
        }
        db
    }

    /// The fingerprint, if the drawing has one.
    #[must_use]
    pub fn fingerprint(&self) -> Option<Fingerprint> {
        self.fingerprint.map(|(f, _)| f)
    }

    /// Where the fingerprint came from, if the drawing has one.
    #[must_use]
    pub fn origin(&self) -> Option<Origin> {
        self.fingerprint.map(|(_, o)| o)
    }

    /// The highest handle the drawing has ever had; 0 for a drawing that was never opened from
    /// a file.
    #[must_use]
    pub fn top(&self) -> u64 {
        self.top
    }

    /// The handle the next new object receives: one above the high-water mark.
    #[must_use]
    pub fn next_handle(&self) -> u64 {
        self.top.saturating_add(1)
    }

    /// The handle of a live bound object.
    #[must_use]
    pub fn handle_of(&self, object: ObjectId) -> Option<Handle> {
        self.by_object.get(&object).copied()
    }

    /// The live object bound to `handle`.
    #[must_use]
    pub fn object_of(&self, handle: Handle) -> Option<ObjectId> {
        self.by_handle.get(&handle).copied()
    }

    /// The number of live bound objects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_object.len()
    }

    /// Whether no live object is bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_object.is_empty()
    }

    /// The stable identifier of a live bound object: needs the object's handle and the
    /// drawing's fingerprint.
    #[must_use]
    pub fn stable_id_of(&self, object: ObjectId) -> Option<StableId> {
        Some(stable_id(self.fingerprint()?, self.handle_of(object)?))
    }
}

fn read_fingerprint(row: &StoredForeign) -> Option<(Fingerprint, Origin)> {
    let bits = u128::from_be_bytes(<[u8; 16]>::try_from(row.data.as_slice()).ok()?);
    let origin = match row.source_version.as_deref() {
        Some("file") => Origin::File,
        _ => Origin::Assigned,
    };
    Fingerprint::new(bits).map(|f| (f, origin))
}

/// The key of the existing row of `kind`, if any.
fn row_of(snapshot: &Snapshot, kind: &str) -> Option<i64> {
    snapshot
        .foreign_rows()
        .find(|r| r.source_format == FORMAT && r.record_kind == kind)
        .map(|r| r.id)
}

fn put(txn: &mut Txn, kind: &str, row: StoredForeign) -> Result<()> {
    let existing = row_of(txn.snapshot(), kind);
    match existing {
        Some(id) => txn.set_foreign(StoredForeign { id, ..row }),
        None => txn.add_foreign(row).map(drop),
    }
}

/// Sets the fingerprint of the drawing in `txn`.
///
/// # Errors
/// The transaction's errors (`doc.foreign.invalid`, limits, a read-only document).
pub fn set_fingerprint(txn: &mut Txn, fingerprint: Fingerprint, origin: Origin) -> Result<()> {
    put(
        txn,
        FINGERPRINT_KIND,
        StoredForeign {
            source_format: FORMAT.to_owned(),
            source_version: Some(origin.text().to_owned()),
            record_kind: FINGERPRINT_KIND.to_owned(),
            data: fingerprint.bits().to_be_bytes().to_vec(),
            ..StoredForeign::default()
        },
    )
}

/// Raises the handle high-water mark to `top` in `txn`; a lower value changes nothing, so the
/// mark never falls.
///
/// # Errors
/// The transaction's errors.
pub fn raise_top(txn: &mut Txn, top: u64) -> Result<()> {
    if Database::of(txn.snapshot()).top() >= top {
        return Ok(());
    }
    let Some(handle) = Handle::new(top) else {
        return Ok(());
    };
    put(
        txn,
        TOP_KIND,
        StoredForeign {
            source_format: FORMAT.to_owned(),
            source_id: Some(handle.to_hex()),
            record_kind: TOP_KIND.to_owned(),
            ..StoredForeign::default()
        },
    )
}

/// Records `pairs` (object, handle) as bindings in `txn` and returns how many it recorded. An object that has a binding keeps it,
/// and a handle at or below the high-water mark is never bound again, so a handle once issued
/// stays with its object. Used for the handles an export assigned to objects the file did not
/// have.
///
/// It only adds rows (each new binding is a row of its own, which also raises the high-water
/// mark, as the mark is the highest handle of any row), never rewrites one. A transaction that
/// commits as bookkeeping therefore touches no row an earlier undo step wrote, and that step
/// still undoes. The rows of an object that undo removes stay, and bind it again when redo
/// brings it back with the same identifier.
///
/// # Errors
/// The transaction's errors.
pub fn record(txn: &mut Txn, pairs: &[(ObjectId, Handle)]) -> Result<usize> {
    let db = Database::of(txn.snapshot());
    let mut map = SourceMap::of(txn.snapshot());
    let mut recorded = 0;
    for (object, handle) in pairs {
        if handle.value() > db.top() && db.handle_of(*object).is_none() {
            map.bind(txn, FORMAT, SOURCE_DOCUMENT, &handle.to_hex(), *object)?;
            recorded += 1;
        }
    }
    Ok(recorded)
}
