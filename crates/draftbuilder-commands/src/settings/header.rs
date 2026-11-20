//! The header variables of a DXF drawing that carry the drawing-scoped settings: the variables
//! [`header_values`] returns are written on save, and [`load_header`] reads them back on open.
//! A setting with no variable keeps its value in the rows of the document only.

use arc_base::Result;
use arc_command::Context;
use arc_doc::Snapshot;
use arc_io_dxf::model::HeaderVariable;
use arc_io_dxf::{Drawing, Group, Value as Dxf};

use super::store::{self, Values};
use super::{Kind, Scope, Setting, TABLE, Value, parse_json, stored_form};

/// The header variables of the drawing `snapshot` that carry its drawing-scoped settings, in
/// table order, followed by the current coordinate system. The caller replaces the variables of
/// the same names in the drawing's header.
#[must_use]
pub fn header_values(snapshot: &Snapshot) -> Vec<HeaderVariable> {
    let values = Values::of(snapshot);
    let mut out: Vec<HeaderVariable> = TABLE
        .iter()
        .filter(|setting| setting.scope == Scope::Drawing)
        .filter_map(|setting| {
            let header = setting.header?;
            let value = values.get(setting.name)?;
            let shown = match (setting.kind, value) {
                (Kind::Layer, Value::Text(id)) => {
                    let name = super::presented(snapshot, setting, &Value::Text(id.clone()));
                    Value::Text(name.as_str().unwrap_or_default().to_owned())
                }
                _ => value.clone(),
            };
            Some(HeaderVariable {
                name: header.name.to_owned(),
                values: groups(header.code, &shown),
            })
        })
        .collect();
    out.extend(crate::precision::ucs::header_values(snapshot));
    out
}

/// The group values of a header variable with first code `code` that holds `value`.
fn groups(code: i16, value: &Value) -> Vec<Group> {
    match value {
        Value::Bool(flag) => vec![Group::int(code, i64::from(*flag))],
        Value::Int(value) => vec![Group::int(code, *value)],
        Value::Real(value) => vec![Group::double(code, *value)],
        Value::Point([x, y]) => vec![Group::double(code, *x), Group::double(code + 10, *y)],
        Value::Text(text) => vec![Group::text(code, text.clone())],
        Value::List(_) => Vec::new(),
    }
}

/// The value of `setting` in the header of `drawing`, when the header holds one of its type.
fn read(drawing: &Drawing, setting: &Setting) -> Option<Value> {
    let header = setting.header?;
    let groups = drawing.header_variable(header.name)?;
    let first = groups.iter().find(|g| g.code == header.code)?;
    let json = match (&first.value, setting.kind) {
        (Dxf::Int(value), Kind::Bool) => serde_json::Value::Bool(*value != 0),
        (Dxf::Int(value), _) => serde_json::json!(value),
        (Dxf::Double(value), _) => serde_json::json!(value),
        (Dxf::Text(text), _) => serde_json::Value::String(text.clone()),
        _ => return None,
    };
    match setting.kind {
        Kind::Point => {
            let y = groups.iter().find(|g| g.code == header.code + 10)?;
            let Dxf::Double(y) = y.value else { return None };
            let x = match first.value {
                Dxf::Double(x) => x,
                _ => return None,
            };
            parse_json(setting, &serde_json::json!([x, y])).ok()
        }
        _ => parse_json(setting, &json).ok(),
    }
}

/// Reads the drawing-scoped settings and the current coordinate system from the header of
/// `drawing` into the document of `ctx`, in its transaction. A value the header holds that does
/// not fit its setting is ignored, and the default applies.
///
/// # Errors
///
/// The transaction's errors.
pub fn load_header(ctx: &mut Context<'_>, drawing: &Drawing) -> Result<()> {
    for setting in TABLE.iter().filter(|s| s.scope == Scope::Drawing) {
        let Some(value) = read(drawing, setting) else {
            continue;
        };
        let Ok(stored) = stored_form(ctx.snapshot(), setting, value) else {
            continue;
        };
        store::write_row(ctx.txn()?, setting, Some(stored))?;
    }
    crate::precision::ucs::load_header(ctx, drawing)
}
