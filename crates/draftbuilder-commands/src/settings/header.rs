//! The header variables of a DXF drawing that carry the drawing-scoped settings: the variables
//! [`header_values`] returns are written on save, and [`load_header`] reads them back on open.
//! A setting with no variable keeps its value in the rows of the document only.

use arc_base::{Error, Result};
use arc_command::Context;
use arc_doc::Snapshot;
use arc_io_dxf::model::HeaderVariable;
use arc_io_dxf::{Drawing, Group, Value as Dxf};

use super::codes;
use super::store::{self, Values};
use super::{CURRENT_LAYER, Kind, Quantity, Scope, Setting, TABLE, Value, parse_json, stored_form};

/// The header variables of the drawing `snapshot` that carry its drawing-scoped settings, in
/// table order, followed by the current coordinate system. The caller replaces the variables of
/// the same names in the drawing's header.
#[must_use]
pub fn header_values(snapshot: &Snapshot) -> Vec<HeaderVariable> {
    let values = Values::of(snapshot, &|_| None);
    // Only the settings with a stored row are written: a default the file never had is not.
    let stored = store::stored_names(snapshot);
    let mut out: Vec<HeaderVariable> = TABLE
        .iter()
        .filter(|setting| setting.scope == Scope::Drawing && stored.contains(setting.name))
        .filter_map(|setting| {
            let header = setting.header?;
            let value = values.get(setting.name)?;
            let shown = match (setting.kind, value) {
                // The current layer is written by name, from the layer the drawing resolves (layer 0
                // when the stored one is gone), never as the stored identifier.
                // The angle settings hold radians; the header holds degrees, as DXF entity angles do.
                (Kind::Real { .. }, Value::Real(radians)) if is_angle(setting) => {
                    Value::Real(radians.to_degrees())
                }
                (Kind::Linetype, _) => Value::Text(
                    super::presented(snapshot, setting, value)
                        .as_str()
                        .unwrap_or("ByLayer")
                        .to_owned(),
                ),
                (Kind::Layer, _) => {
                    let name = super::current_layer(snapshot)
                        .and_then(|id| arc_draft_dxf::fields::name_of(snapshot, Some(id)))
                        .unwrap_or_else(|| "0".to_owned());
                    Value::Text(name)
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
        (Dxf::Double(value), _) if is_angle(setting) => serde_json::json!(value.to_radians()),
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

/// What loading a header did with the file's values.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HeaderLoad {
    /// The values the header holds that their settings cannot take (kept at the default).
    pub ignored: u64,
    /// The numbers outside their setting's range, set to the nearest bound.
    pub clamped: u64,
    /// The current layer of the file is frozen or missing: layer 0 is current.
    pub layer_fallback: u64,
}

/// Whether the setting is an angle in the header: its value is radians, the header's is degrees.
fn is_angle(setting: &Setting) -> bool {
    matches!(setting.name, "draft.angle.base" | "draft.snap.angle")
}

/// The number of the first value of the header variable `name` of `drawing`, when it is a number.
fn raw_number(drawing: &Drawing, setting: &Setting) -> Option<f64> {
    let groups = drawing.header_variable(setting.header?.name)?;
    let value = match groups.first()?.value {
        Dxf::Double(value) => value,
        Dxf::Int(value) => value as f64,
        _ => return None,
    };
    Some(if is_angle(setting) {
        value.to_radians()
    } else {
        value
    })
}

/// The value of a numeric setting for `raw`, clamped to its range; `None` for a setting that is not
/// numeric or a value that is not finite.
fn clamped(setting: &Setting, raw: f64) -> Option<Value> {
    if !raw.is_finite() {
        return None;
    }
    match setting.kind {
        // The value is clamped to the range of the setting first, so the cast cannot truncate.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "clamped to the setting's range first"
        )]
        Kind::Int { min, max } => Some(Value::Int(
            (raw.round().clamp(min as f64, max as f64)) as i64,
        )),
        Kind::Real { min, max } => Some(Value::Real(raw.clamp(min, max))),
        _ => None,
    }
}

/// The value of `setting` in the drawing's unit: a length or a point of the file, scaled by `factor` (the
/// file's unit over the drawing's, see `open`). A positive-only length keeps a value that is not positive.
#[must_use]
pub fn scaled(setting: &Setting, value: Value, factor: f64) -> Value {
    match (setting.quantity, value) {
        (Quantity::Point, Value::Point(p)) => Value::Point([p[0] * factor, p[1] * factor]),
        (Quantity::Length, Value::Real(v)) => Value::Real(v * factor),
        (Quantity::PositiveLength, Value::Real(v)) if v > 0.0 => Value::Real(v * factor),
        (_, value) => value,
    }
}

/// Reads the drawing-scoped settings and the current coordinate system from the header of `drawing`
/// into the document of `ctx`, in its transaction. The lengths and points are scaled by `factor` (the
/// file's unit over the drawing's). A value that does not fit its setting is ignored (the default
/// applies), a number out of range is clamped, and the counts say which. Only the product's settings
/// are read: the other header variables of the file are not kept.
///
/// # Errors
///
/// The transaction's errors.
pub fn load_header(ctx: &mut Context<'_>, drawing: &Drawing, factor: f64) -> Result<HeaderLoad> {
    let mut load = HeaderLoad::default();
    for setting in TABLE.iter().filter(|s| s.scope == Scope::Drawing) {
        let Some(header) = setting.header else {
            continue;
        };
        if drawing.header_variable(header.name).is_none() {
            continue;
        }
        let value = read(drawing, setting).or_else(|| {
            let raw = raw_number(drawing, setting)?;
            let value = clamped(setting, raw)?;
            load.clamped += 1;
            Some(value)
        });
        let stored = match value {
            Some(value) => stored_form(ctx.snapshot(), setting, scaled(setting, value, factor)),
            None => Err(Error::new(codes::SETTING_VALUE)),
        };
        match stored {
            Ok(stored) => store::write_row(ctx.txn()?, setting, Some(stored))?,
            Err(error)
                if setting.name == CURRENT_LAYER || error.code() == codes::LAYER_FROZEN.code =>
            {
                load.layer_fallback += 1;
            }
            Err(_) => load.ignored += 1,
        }
    }
    load.ignored += crate::precision::ucs::load_header(ctx, drawing, factor)?;
    Ok(load)
}
