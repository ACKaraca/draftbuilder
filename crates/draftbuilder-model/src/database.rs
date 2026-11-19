//! The drawing database prototype (ADR-0001, ADR-0002): the state a drawing keeps beside its
//! `arc-draft` objects, stored in the `arc-doc` document so that it is journaled, undone with
//! the commands that change it and saved with the package.
//!
//! The state is three kinds of foreign data rows of the format `dxf` (the drawing database is
//! the DXF database; see `docs/architecture/domain-model.md`):
//!
//! - the **handle table**: one `arc-doc` source-map record per bound object, whose source
//!   identifier is the object's handle in hexadecimal;
//! - the **handle high-water mark**: the highest handle the drawing has ever had, from the file
//!   or from a binding; the next handle is one above it. Rows of deleted objects stay, so a
//!   deleted object's handle is not issued again;
//! - the **fingerprint** and where it came from.
//!
//! The source identifier of these rows is also what the DXF export reads to start its own
//! handles above every handle of the drawing, which keeps structural handles clear of bound
//! ones.

use std::collections::BTreeMap;

use arc_base::id::Identifier;
use arc_base::{Error, ObjectId, Result};
use arc_doc::{Snapshot, SourceMap, StoredForeign, Txn};

use crate::error::codes;
use crate::handle::Handle;
use crate::identity::{Fingerprint, StableId, stable_id};

/// The `source_format` of every row of the drawing database.
pub const FORMAT: &str = "dxf";
/// The document name of the handle table inside the source map.
const TABLE: &str = "handles";
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
                (arc_doc::source_map::RECORD_KIND, Some(h)) if row.data == TABLE.as_bytes() => {
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

/// Binds each object to its handle in `txn` and raises the high-water mark to the highest of
/// them. Nothing is bound twice: a handle or an object that is already bound is an error.
///
/// # Errors
/// `db.model.handle_taken` when a handle is bound to another live object, or an object already
/// has a handle; the transaction's errors.
pub fn bind(txn: &mut Txn, pairs: &[(ObjectId, Handle)]) -> Result<()> {
    let db = Database::of(txn.snapshot());
    let mut map = SourceMap::of(txn.snapshot());
    let mut seen = BTreeMap::new();
    for (object, handle) in pairs {
        if db.object_of(*handle).is_some_and(|o| o != *object)
            || db.handle_of(*object).is_some_and(|h| h != *handle)
            || seen.insert(*handle, *object).is_some_and(|o| o != *object)
        {
            return Err(Error::new(codes::HANDLE_TAKEN).with_arg("handle", handle.to_hex()));
        }
        map.bind(txn, FORMAT, TABLE, &handle.to_hex(), *object)?;
    }
    match pairs.iter().map(|(_, h)| h.value()).max() {
        Some(top) => raise_top(txn, top),
        None => Ok(()),
    }
}
