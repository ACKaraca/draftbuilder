//! Where the values of settings are kept. A drawing-scoped value is a foreign row of the document
//! (`record_kind` `setting`, the product name as `source_id`, the JSON form as `data`), so the
//! transaction that writes it is undoable and the package saves it with the drawing. A
//! session-scoped value is held in memory for the process.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use arc_base::Result;
use arc_doc::{Snapshot, StoredForeign, Txn};
use draftbuilder_model::database::FORMAT;

use super::{Scope, Setting, TABLE, Value, default_value, find, parse_json};

/// The `record_kind` of the setting rows of a drawing.
pub const RECORD_KIND: &str = "setting";

/// The session values, by product name.
static SESSION: Mutex<BTreeMap<&'static str, Value>> = Mutex::new(BTreeMap::new());

/// The stored row of the setting named `name` in `snapshot`: its identifier and its row.
fn row_of(snapshot: &Snapshot, name: &str) -> Option<(i64, StoredForeign)> {
    snapshot
        .foreign_rows()
        .find(|row| {
            row.source_format == FORMAT
                && row.record_kind == RECORD_KIND
                && row.source_id.as_deref() == Some(name)
        })
        .map(|row| (row.id, (**row).clone()))
}

/// The stored value of the drawing-scoped setting named `name`, when the drawing holds one.
/// A row that does not parse is ignored.
#[must_use]
pub fn row_value(snapshot: &Snapshot, name: &str) -> Option<Value> {
    let setting = find(name)?;
    let (_, row) = row_of(snapshot, setting.name)?;
    let json: serde_json::Value = serde_json::from_slice(&row.data).ok()?;
    parse_json(setting, &json).ok()
}

/// The value of `setting` in effect: its stored value in its scope, else its default.
#[must_use]
pub fn current(snapshot: &Snapshot, setting: &Setting) -> Value {
    let stored = match setting.scope {
        Scope::Drawing => row_value(snapshot, setting.name),
        Scope::Session => session_get(setting),
    };
    stored.unwrap_or_else(|| default_value(setting))
}

/// The session value of `setting`, when one is set.
#[must_use]
pub fn session_get(setting: &Setting) -> Option<Value> {
    session().get(setting.name).cloned()
}

/// Sets (`Some`) or clears (`None`) the session value of `setting`.
pub fn session_set(setting: &Setting, value: Option<Value>) {
    let mut map = session();
    match value {
        Some(value) => map.insert(setting.name, value),
        None => map.remove(setting.name),
    };
}

/// The session map. A poisoned lock still gives its map: the values are plain data.
fn session() -> MutexGuard<'static, BTreeMap<&'static str, Value>> {
    SESSION.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes (`Some`) or removes (`None`) the drawing row of `setting` in `txn`.
///
/// # Errors
///
/// The transaction's errors (`doc.foreign.invalid`, limits, a read-only document).
pub fn write_row(txn: &mut Txn, setting: &Setting, value: Option<Value>) -> Result<()> {
    let existing = row_of(txn.snapshot(), setting.name).map(|(id, _)| id);
    match (existing, value) {
        (Some(id), Some(value)) => txn.set_foreign(row(setting, id, &value)),
        (None, Some(value)) => txn.add_foreign(row(setting, 0, &value)).map(drop),
        (Some(id), None) => txn.remove_foreign(id),
        (None, None) => Ok(()),
    }
}

/// The foreign row that holds `value` of `setting` under identifier `id`.
fn row(setting: &Setting, id: i64, value: &Value) -> StoredForeign {
    StoredForeign {
        id,
        source_format: FORMAT.to_owned(),
        record_kind: RECORD_KIND.to_owned(),
        source_id: Some(setting.name.to_owned()),
        data: serde_json::to_vec(&value.to_json()).unwrap_or_default(),
        ..StoredForeign::default()
    }
}

/// The names of the drawing-scoped settings that have a stored row in `snapshot`, in one pass.
#[must_use]
pub fn stored_names(snapshot: &Snapshot) -> std::collections::BTreeSet<String> {
    snapshot
        .foreign_rows()
        .filter(|row| row.source_format == FORMAT && row.record_kind == RECORD_KIND)
        .filter_map(|row| row.source_id.clone())
        .collect()
}

/// The effective values of every setting of the table, for a computation that reads several.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Values {
    values: BTreeMap<&'static str, Value>,
}

impl Values {
    /// The effective values of the drawing `snapshot` and of the session.
    #[must_use]
    pub fn of(snapshot: &Snapshot) -> Self {
        // One pass over the rows of the document, not one per setting: a drawing has many rows.
        let mut rows: BTreeMap<&str, &StoredForeign> = BTreeMap::new();
        for row in snapshot.foreign_rows() {
            if row.source_format == FORMAT
                && row.record_kind == RECORD_KIND
                && let Some(name) = row.source_id.as_deref()
            {
                rows.entry(name).or_insert(row);
            }
        }
        let values = TABLE
            .iter()
            .map(|setting| {
                let stored = match setting.scope {
                    Scope::Drawing => rows.get(setting.name).and_then(|row| {
                        let json: serde_json::Value = serde_json::from_slice(&row.data).ok()?;
                        parse_json(setting, &json).ok()
                    }),
                    Scope::Session => session_get(setting),
                };
                (
                    setting.name,
                    stored.unwrap_or_else(|| default_value(setting)),
                )
            })
            .collect();
        Self { values }
    }

    /// The value of the setting named `name`, when it is in the table.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// The integer of `name`; zero when the setting is not an integer.
    #[must_use]
    pub fn int(&self, name: &str) -> i64 {
        match self.get(name) {
            Some(Value::Int(value)) => *value,
            _ => 0,
        }
    }

    /// The number of `name`; zero when the setting is not a number.
    #[must_use]
    pub fn real(&self, name: &str) -> f64 {
        match self.get(name) {
            Some(Value::Real(value)) => *value,
            _ => 0.0,
        }
    }

    /// The flag of `name`; false when the setting is not a flag.
    #[must_use]
    pub fn flag(&self, name: &str) -> bool {
        matches!(self.get(name), Some(Value::Bool(true)))
    }

    /// The point of `name`; the origin when the setting is not a point.
    #[must_use]
    pub fn point(&self, name: &str) -> [f64; 2] {
        match self.get(name) {
            Some(Value::Point(value)) => *value,
            _ => [0.0, 0.0],
        }
    }

    /// The text of `name`; empty when the setting is not text.
    #[must_use]
    pub fn text(&self, name: &str) -> String {
        match self.get(name) {
            Some(Value::Text(value)) => value.clone(),
            _ => String::new(),
        }
    }
}

impl Values {
    /// The list of `name`; empty when the setting is not a list.
    #[must_use]
    pub fn list(&self, name: &str) -> Vec<f64> {
        match self.get(name) {
            Some(Value::List(values)) => values.clone(),
            _ => Vec::new(),
        }
    }
}
