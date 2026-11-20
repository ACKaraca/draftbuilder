//! Where the values of settings are kept. A drawing-scoped value is a foreign row of the document
//! (`record_kind` `setting`, the product name as `source_id`, the JSON form as `data`), so the
//! transaction that writes it is undoable and the package saves it with the drawing. A
//! session-scoped value is kept in the session's settings of the foundation (`arc_command`), under
//! the key [`session_key`]: each session has its own values, and a write applies when its command
//! succeeds.

use std::collections::BTreeMap;

use arc_base::Result;
use arc_doc::{Snapshot, StoredForeign, Txn};
use draftbuilder_model::database::FORMAT;

use super::{Scope, Setting, TABLE, Value, default_value, find, parse_json};

/// The `record_kind` of the setting rows of a drawing.
pub const RECORD_KIND: &str = "setting";

/// The namespace of the product's keys in the foundation's session settings.
pub const SESSION_NAMESPACE: &str = "draftbuilder";

/// A reader of the session settings: a command context's `setting`, or a session's.
pub type SessionRead<'a> = &'a dyn Fn(&str) -> Option<serde_json::Value>;

/// The key of the session setting `setting` in the foundation's session settings.
#[must_use]
pub fn session_key(setting: &Setting) -> String {
    format!("{SESSION_NAMESPACE}.{}", setting.name)
}

/// The session value of `setting` that `read` gives, when the session has one that parses.
#[must_use]
pub fn session_value(read: SessionRead<'_>, setting: &Setting) -> Option<Value> {
    let json = read(&session_key(setting))?;
    parse_json(setting, &json).ok()
}

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

/// The value of the session setting `setting` in effect: the session's value, else its default.
#[must_use]
pub fn session_current(session: SessionRead<'_>, setting: &Setting) -> Value {
    session_value(session, setting).unwrap_or_else(|| default_value(setting))
}

/// The value of `setting` in effect: its stored value in its scope, else its default.
#[must_use]
pub fn current(snapshot: &Snapshot, session: SessionRead<'_>, setting: &Setting) -> Value {
    match setting.scope {
        Scope::Drawing => {
            row_value(snapshot, setting.name).unwrap_or_else(|| default_value(setting))
        }
        Scope::Session => session_current(session, setting),
    }
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
    /// The effective values of the drawing `snapshot` and of the session `session` reads. A caller
    /// that needs drawing values only passes a reader that gives nothing.
    #[must_use]
    pub fn of(snapshot: &Snapshot, session: SessionRead<'_>) -> Self {
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
        let mut values: BTreeMap<&'static str, Value> = TABLE
            .iter()
            .map(|setting| {
                let stored = match setting.scope {
                    Scope::Drawing => rows.get(setting.name).and_then(|row| {
                        let json: serde_json::Value = serde_json::from_slice(&row.data).ok()?;
                        parse_json(setting, &json).ok()
                    }),
                    Scope::Session => session_value(session, setting),
                };
                (
                    setting.name,
                    stored.unwrap_or_else(|| default_value(setting)),
                )
            })
            .collect();
        // The window's status bar toggles shadow the drawing's values while it is open.
        for (key, name) in crate::view::TOGGLES {
            let on = session(&crate::view::override_key(key)).and_then(|v| v.as_bool());
            let Some(on) = on else { continue };
            if *key == "osnap" {
                let bits = match values.get(*name) {
                    Some(Value::Int(bits)) => *bits,
                    _ => 0,
                };
                let suppressed = crate::precision::snaps::RUNNING_SUPPRESSED;
                let bits = if on {
                    bits & !suppressed
                } else {
                    bits | suppressed
                };
                values.insert(*name, Value::Int(bits));
            } else {
                values.insert(*name, Value::Bool(on));
            }
        }
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
