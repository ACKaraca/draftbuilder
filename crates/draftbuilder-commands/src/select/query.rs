//! Selection by properties (DB-SEL-018 to DB-SEL-020): a filter of tests on properties, combined
//! with AND, OR, XOR and NOT, and selection of the objects that share chosen properties with
//! given objects. Both are pure functions over a document snapshot.
//!
//! Properties are the entity type, the layer name and the effective color, linetype, lineweight,
//! linetype scale and transparency. An object whose appearance is ByObject reads its own value; any
//! other source (ByLayer, and ByBlock for a top-level object, which has no block) reads the layer.
//! A color is written `#rrggbb`. Names compare without regard to case, and a wildcard pattern uses
//! `*` for any run of characters and `?` for one character.

use arc_base::{Error, ObjectId, Result};
use arc_doc::{Snapshot, Value};
use arc_draft::types::{By, entity, layer};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::candidates::{Options, default_layer, gather, reference};
use crate::error::codes;

/// The most nested filter nodes one filter may have.
pub const MAX_FILTER_NODES: usize = 256;
/// The deepest nesting of one filter.
pub const MAX_FILTER_DEPTH: usize = 16;
/// The longest text of one filter test, in characters.
pub const MAX_FILTER_TEXT: usize = 255;

/// A property a test or a similarity compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    /// The entity type, such as `line` or `circle`.
    Type,
    /// The layer name.
    Layer,
    /// The effective color, as `#rrggbb`.
    Color,
    /// The effective linetype name.
    Linetype,
    /// The effective lineweight in millimeters; 0 for the default.
    Lineweight,
    /// The linetype scale of the object.
    LinetypeScale,
    /// The effective transparency, from 0 (opaque) to 1.
    Transparency,
}

/// The comparison of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// The property equals the value.
    Equal,
    /// The property differs from the value.
    NotEqual,
    /// The property is greater than the value (text: later in alphabetical order).
    Greater,
    /// The property is less than the value.
    Less,
    /// The text property matches the wildcard pattern in the value.
    Wildcard,
}

/// A filter over the properties of objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    /// One comparison of a property with a value.
    Test {
        /// The property.
        property: Property,
        /// The comparison.
        op: Op,
        /// The value, as text; numbers are read as decimal.
        #[schemars(length(max = 255))]
        value: String,
    },
    /// Every filter holds (true for none).
    And(Vec<Filter>),
    /// Some filter holds (false for none).
    Or(Vec<Filter>),
    /// An odd number of the filters hold (false for none).
    Xor(Vec<Filter>),
    /// The filter does not hold.
    Not(Box<Filter>),
}

/// A property value read from an object.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Read {
    /// A name or a color.
    Text(String),
    /// A number.
    Number(f64),
}

/// Checks a filter against the limits of one filter: its node count, depth and text lengths.
///
/// # Errors
///
/// `db.cmd.selection_invalid` (reason `filter`) when a limit is exceeded or a number is not finite.
pub fn check(filter: &Filter) -> Result<()> {
    let mut nodes = 0;
    check_node(filter, 1, &mut nodes)
}

fn check_node(filter: &Filter, depth: usize, nodes: &mut usize) -> Result<()> {
    let invalid = || Error::new(codes::SELECTION_INVALID).with_detail("reason", "filter");
    *nodes += 1;
    if depth > MAX_FILTER_DEPTH || *nodes > MAX_FILTER_NODES {
        return Err(invalid());
    }
    match filter {
        Filter::Test { value, .. } => {
            if value.chars().count() > MAX_FILTER_TEXT {
                return Err(invalid());
            }
            Ok(())
        }
        Filter::And(items) | Filter::Or(items) | Filter::Xor(items) => items
            .iter()
            .try_for_each(|item| check_node(item, depth + 1, nodes)),
        Filter::Not(item) => check_node(item, depth + 1, nodes),
    }
}

/// The value of a property of an entity, read from the snapshot.
pub(crate) fn read(snapshot: &Snapshot, id: ObjectId, property: Property) -> Read {
    match property {
        Property::Type => Read::Text(type_name(snapshot, id)),
        Property::Layer => Read::Text(
            layer_of(snapshot, id)
                .and_then(|l| name_of(snapshot, Some(l)))
                .unwrap_or_else(|| "0".to_owned()),
        ),
        Property::Color => Read::Text(color_text(snapshot, id)),
        Property::Linetype => Read::Text(linetype_name(snapshot, id)),
        Property::Lineweight => Read::Number(number(snapshot, id, Appearance::Lineweight)),
        Property::LinetypeScale => Read::Number(number(snapshot, id, Appearance::LinetypeScale)),
        Property::Transparency => Read::Number(number(snapshot, id, Appearance::Transparency)),
    }
}

/// The lowercase type name of an entity: its registered key without the `core.draft.` prefix.
fn type_name(snapshot: &Snapshot, id: ObjectId) -> String {
    snapshot
        .resolve(id)
        .ok()
        .and_then(|handle| snapshot.type_key(handle))
        .map(|key| {
            key.as_str()
                .strip_prefix("core.draft.")
                .unwrap_or(key.as_str())
                .to_owned()
        })
        .unwrap_or_default()
}

/// The layer of an entity: its reference, or layer `0` when it names none.
fn layer_of(snapshot: &Snapshot, id: ObjectId) -> Option<ObjectId> {
    reference(snapshot, id, entity::LAYER).or_else(|| default_layer(snapshot))
}

/// The values of an object's fields, `None` when the object is gone.
fn fields(snapshot: &Snapshot, id: ObjectId) -> Option<&[Option<Value>]> {
    let handle = snapshot.resolve(id).ok()?;
    Some(snapshot.get(handle).ok()?.values.as_slice())
}

fn layer_fields(snapshot: &Snapshot, id: ObjectId) -> Option<&[Option<Value>]> {
    fields(snapshot, layer_of(snapshot, id)?)
}

fn by_of(values: &[Option<Value>], index: usize) -> By {
    match values.get(index) {
        Some(Some(Value::Enum(code))) => By::of(Some(*code)),
        _ => By::Layer,
    }
}

/// Which appearance value a number property reads.
#[derive(Debug, Clone, Copy)]
enum Appearance {
    Lineweight,
    LinetypeScale,
    Transparency,
}

fn number(snapshot: &Snapshot, id: ObjectId, appearance: Appearance) -> f64 {
    let Some(values) = fields(snapshot, id) else {
        return 0.0;
    };
    match appearance {
        Appearance::Lineweight => by_value(
            snapshot,
            id,
            values,
            (entity::LINEWEIGHT_BY, entity::LINEWEIGHT, layer::LINEWEIGHT),
            0.0,
        ),
        Appearance::Transparency => by_value(
            snapshot,
            id,
            values,
            (
                entity::TRANSPARENCY_BY,
                entity::TRANSPARENCY,
                layer::TRANSPARENCY,
            ),
            0.0,
        ),
        Appearance::LinetypeScale => match values.get(entity::LINETYPE_SCALE) {
            Some(Some(Value::F64(scale))) => *scale,
            _ => 1.0,
        },
    }
}

/// The number of an appearance field read from the object when its source is ByObject, and from
/// its layer otherwise. The indices are the source, the object's field and the layer's field.
fn by_value(
    snapshot: &Snapshot,
    id: ObjectId,
    values: &[Option<Value>],
    (by_index, own_index, layer_index): (usize, usize, usize),
    default: f64,
) -> f64 {
    let own = by_of(values, by_index) == By::Object;
    let source = if own {
        Some(values)
    } else {
        layer_fields(snapshot, id)
    };
    let index = if own { own_index } else { layer_index };
    match source.and_then(|s| s.get(index)) {
        Some(Some(Value::F64(v))) => *v,
        _ => default,
    }
}

fn linetype_name(snapshot: &Snapshot, id: ObjectId) -> String {
    let Some(values) = fields(snapshot, id) else {
        return "Continuous".to_owned();
    };
    let target = if by_of(values, entity::LINETYPE_BY) == By::Object {
        reference(snapshot, id, entity::LINETYPE)
    } else {
        layer_of(snapshot, id).and_then(|l| reference(snapshot, l, layer::LINETYPE))
    };
    target
        .and_then(|t| name_of(snapshot, Some(t)))
        .unwrap_or_else(|| "Continuous".to_owned())
}

fn color_text(snapshot: &Snapshot, id: ObjectId) -> String {
    let Some(values) = fields(snapshot, id) else {
        return String::new();
    };
    let color_at = |values: &[Option<Value>], index: usize| match values.get(index) {
        Some(Some(Value::Color(color))) => Some(color.rgba),
        _ => None,
    };
    let color = if by_of(values, entity::COLOR_BY) == By::Object {
        color_at(values, entity::COLOR)
    } else {
        layer_fields(snapshot, id).and_then(|lv| color_at(lv, layer::COLOR))
    };
    color.map_or_else(String::new, |c| {
        format!("#{}{}{}", hex(c.r), hex(c.g), hex(c.b))
    })
}

/// Two hexadecimal digits of a linear channel in `0..=1`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the channel is clamped to 0..=255 before the cast"
)]
fn hex(channel: f32) -> String {
    let byte = (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{byte:02x}")
}

/// Whether a text matches a wildcard pattern, case-insensitively.
#[must_use]
pub fn wildcard_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star = None::<usize>;
    let mut mark = 0;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// Compares a property value with a test's value.
fn compare(read: &Read, op: Op, value: &str) -> Result<bool> {
    let invalid = |reason: &str| {
        Error::new(codes::SELECTION_INVALID).with_detail("reason", reason.to_owned())
    };
    match read {
        Read::Text(text) => Ok(match op {
            Op::Equal => text.eq_ignore_ascii_case(value),
            Op::NotEqual => !text.eq_ignore_ascii_case(value),
            Op::Greater => text.to_lowercase() > value.to_lowercase(),
            Op::Less => text.to_lowercase() < value.to_lowercase(),
            Op::Wildcard => wildcard_matches(value, text),
        }),
        Read::Number(number) => {
            if op == Op::Wildcard {
                return Err(invalid("wildcard"));
            }
            let target: f64 = value.trim().parse().map_err(|_| invalid("number"))?;
            if !target.is_finite() {
                return Err(invalid("number"));
            }
            let close = (number - target).abs() <= 1e-9;
            Ok(match op {
                Op::Equal => close,
                Op::NotEqual => !close,
                Op::Greater => *number > target,
                Op::Less => *number < target,
                Op::Wildcard => false,
            })
        }
    }
}

/// Whether `id` satisfies `filter`.
///
/// # Errors
///
/// `db.cmd.selection_invalid` for a number test whose value is not a finite decimal, or a
/// wildcard on a number.
pub fn matches(snapshot: &Snapshot, id: ObjectId, filter: &Filter) -> Result<bool> {
    match filter {
        Filter::Test {
            property,
            op,
            value,
        } => compare(&read(snapshot, id, *property), *op, value),
        Filter::And(items) => {
            for item in items {
                if !matches(snapshot, id, item)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Filter::Or(items) => {
            for item in items {
                if matches(snapshot, id, item)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Filter::Xor(items) => {
            let mut holds = 0_usize;
            for item in items {
                holds += usize::from(matches(snapshot, id, item)?);
            }
            Ok(holds % 2 == 1)
        }
        Filter::Not(item) => Ok(!matches(snapshot, id, item)?),
    }
}

/// The objects of `space` that satisfy `filter` (`invert` false) or that do not (`invert` true),
/// restricted to `within` when it is given, in draw order.
///
/// # Errors
///
/// `db.cmd.selection_invalid` for a filter over the limits or with a bad test (see [`check`] and
/// [`matches`]), and the errors of gathering.
pub fn select_query(
    snapshot: &Snapshot,
    space: ObjectId,
    filter: &Filter,
    invert: bool,
    within: Option<&[ObjectId]>,
    options: &Options,
) -> Result<Vec<ObjectId>> {
    check(filter)?;
    let candidates = gather(snapshot, space, *options)?;
    let mut out = Vec::new();
    for item in &candidates.items {
        if within.is_some_and(|scope| !scope.contains(&item.id)) {
            continue;
        }
        if matches(snapshot, item.id, filter)? != invert {
            out.push(item.id);
        }
    }
    Ok(out)
}

/// The objects of `space` whose listed `properties` equal those of at least one of `reference`,
/// in draw order. References that no longer exist are ignored.
///
/// # Errors
///
/// The errors of gathering.
pub fn select_similar(
    snapshot: &Snapshot,
    space: ObjectId,
    reference: &[ObjectId],
    properties: &[Property],
    options: &Options,
) -> Result<Vec<ObjectId>> {
    let profiles: Vec<Vec<Read>> = reference
        .iter()
        .filter(|id| snapshot.resolve(**id).is_ok())
        .map(|id| properties.iter().map(|p| read(snapshot, *id, *p)).collect())
        .collect();
    let candidates = gather(snapshot, space, *options)?;
    Ok(candidates
        .items
        .iter()
        .filter(|item| {
            let own: Vec<Read> = properties
                .iter()
                .map(|p| read(snapshot, item.id, *p))
                .collect();
            profiles.contains(&own)
        })
        .map(|item| item.id)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::wildcard_matches;

    #[test]
    fn wildcards_match_case_insensitively_with_star_and_question_mark() {
        assert!(wildcard_matches("LINE*", "line"));
        assert!(wildcard_matches("*line", "polyline"));
        assert!(!wildcard_matches("line*", "polyline"));
        assert!(wildcard_matches("w?ll", "Wall"));
        assert!(!wildcard_matches("w?ll", "wall2"));
        assert!(wildcard_matches("*", ""));
        assert!(!wildcard_matches("a*b", "acd"));
    }
}
