//! Settings (area `settings`): the settings table with its product names, compatibility names,
//! types, ranges, scopes and defaults; typed get and set with validation; the listing with a
//! wildcard; and the current layer and the appearance that new entities take from the settings.
//!
//! Drawing-scoped values are rows of the document (`record_kind` `setting`), so a change is an
//! undoable transaction, the values save with the drawing in the package, and they are written to
//! and read from the header of a DXF file ([`header`]). Session-scoped values live in memory for
//! the process ([`store`]). Profiles are not built: the settings store of the application shell is
//! not a dependency of this crate.
//!
//! Compatibility names are accepted wherever a setting name is, compared without regard to case.
//! A setting with no DXF header variable keeps its value in the rows only.

pub mod header;
pub mod store;

use arc_base::color::indexed_rgb;
use arc_base::{Color, Error, ObjectId, Result};
use arc_command::{
    Accept, CommandSpec, Context, Permission, Prompt, PromptInput, PromptState, Registry, Step,
    UndoBehavior,
};
use arc_doc::{ColorSource, ColorValue, Snapshot, Value as Doc};
use arc_draft::types::{By, entity, layer};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Message;
use crate::fold::fold;
use crate::layer::linetype_ref;
use crate::select::candidates::{default_layer, flag};
use crate::vocabulary::{Area, register_names};

use arc_draft::types::LAYER;

/// Where a setting's value is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Kept in the drawing and saved with it.
    Drawing,
    /// Kept in memory for the process; never saved with a drawing.
    Session,
}

impl Scope {
    /// The name of the scope in results.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Drawing => "drawing",
            Self::Session => "session",
        }
    }
}

/// The type and range of a setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// A flag.
    Bool,
    /// An integer in `min..=max`.
    Int {
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
    },
    /// A finite number in `min..=max`.
    Real {
        /// The smallest value.
        min: f64,
        /// The largest value.
        max: f64,
    },
    /// A point in the plane, two finite numbers written `x,y`.
    Point,
    /// Angles in degrees, each in `(0, 360)`, written comma-separated; empty for none.
    Angles,
    /// The name of an existing layer that is not frozen; stored as the layer's identifier, so it
    /// follows a rename.
    Layer,
    /// `ByLayer`, `ByBlock` or the name of a linetype of the drawing.
    Linetype,
    /// The length unit of the drawing's model space. Read only: the units command changes it.
    Unit,
}

/// The DXF header variable a setting is written to: its name and the group code of its first
/// value. A point takes the code and the code plus ten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderVar {
    /// The variable name, including `$`.
    pub name: &'static str,
    /// The group code of the first value.
    pub code: i16,
}

/// A message identifier of a setting description, in the catalog of this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text {
    /// The identifier in `locales/en-US/draftbuilder-commands.ftl`.
    pub message_id: &'static str,
}

/// What the value of a setting measures. A value read from a file is scaled into the drawing's unit when
/// the file's unit differs from it (`settings::header::load_header`): a length or a point is scaled, and
/// anything else is kept as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantity {
    /// Not a length: a count, a ratio, an angle, a flag, a layer or a name.
    Neither,
    /// A length.
    Length,
    /// A point: both coordinates are lengths.
    Point,
    /// A length that is scaled only when it is positive: a negative size is a percentage of the view.
    PositiveLength,
}

/// One setting of the table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Setting {
    /// The product name: dotted, lower case.
    pub name: &'static str,
    /// The conventional name existing drawings and scripts use, in upper case.
    pub compat: Option<&'static str>,
    /// The type and range.
    pub kind: Kind,
    /// Where the value is kept.
    pub scope: Scope,
    /// The default, as text the kind parses.
    pub default: &'static str,
    /// The header variable the drawing writes it to, when there is one.
    pub header: Option<HeaderVar>,
    /// What the value measures.
    pub quantity: Quantity,
    /// The message identifier of the one-line description.
    pub description: Text,
}

/// The fixed group codes of the header variables the settings use.
const fn hv(name: &'static str, code: i16) -> Option<HeaderVar> {
    Some(HeaderVar { name, code })
}

/// Every setting of the product, in the order a listing shows them.
pub const TABLE: &[Setting] = &[
    Setting {
        name: "draft.layer.current",
        compat: Some("CLAYER"),
        kind: Kind::Layer,
        scope: Scope::Drawing,
        default: "0",
        header: hv("$CLAYER", 8),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-layer-current",
        },
    },
    Setting {
        name: "draft.color.current",
        compat: Some("CECOLOR"),
        kind: Kind::Int { min: 0, max: 256 },
        scope: Scope::Drawing,
        default: "256",
        header: hv("$CECOLOR", 62),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-color-current",
        },
    },
    Setting {
        name: "draft.linetype.current",
        compat: Some("CELTYPE"),
        kind: Kind::Linetype,
        scope: Scope::Drawing,
        default: "ByLayer",
        header: hv("$CELTYPE", 6),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-linetype-current",
        },
    },
    Setting {
        name: "draft.lineweight.current",
        compat: Some("CELWEIGHT"),
        kind: Kind::Int { min: -3, max: 211 },
        scope: Scope::Drawing,
        default: "-1",
        header: hv("$CELWEIGHT", 370),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-lineweight-current",
        },
    },
    Setting {
        name: "draft.transparency.current",
        compat: None,
        kind: Kind::Int { min: -2, max: 90 },
        scope: Scope::Drawing,
        default: "-1",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-transparency-current",
        },
    },
    Setting {
        name: "draft.linetype.global_scale",
        compat: Some("LTSCALE"),
        kind: Kind::Real {
            min: 1e-6,
            max: 1e6,
        },
        scope: Scope::Drawing,
        default: "1",
        header: hv("$LTSCALE", 40),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-linetype-global-scale",
        },
    },
    Setting {
        name: "draft.linetype.object_scale",
        compat: Some("CELTSCALE"),
        kind: Kind::Real {
            min: 1e-6,
            max: 1e6,
        },
        scope: Scope::Drawing,
        default: "1",
        header: hv("$CELTSCALE", 40),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-linetype-object-scale",
        },
    },
    Setting {
        name: "draft.units.length",
        compat: Some("LUNITS"),
        kind: Kind::Int { min: 1, max: 5 },
        scope: Scope::Drawing,
        default: "2",
        header: hv("$LUNITS", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-units-length",
        },
    },
    Setting {
        name: "draft.units.length_precision",
        compat: Some("LUPREC"),
        kind: Kind::Int { min: 0, max: 8 },
        scope: Scope::Drawing,
        default: "4",
        header: hv("$LUPREC", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-units-length-precision",
        },
    },
    Setting {
        name: "draft.units.angle",
        compat: Some("AUNITS"),
        kind: Kind::Int { min: 0, max: 4 },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$AUNITS", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-units-angle",
        },
    },
    Setting {
        name: "draft.units.angle_precision",
        compat: Some("AUPREC"),
        kind: Kind::Int { min: 0, max: 8 },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$AUPREC", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-units-angle-precision",
        },
    },
    Setting {
        name: "draft.units.insertion",
        compat: Some("INSUNITS"),
        kind: Kind::Unit,
        scope: Scope::Drawing,
        default: "unitless",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-units-insertion",
        },
    },
    Setting {
        name: "draft.angle.base",
        compat: Some("ANGBASE"),
        kind: Kind::Real {
            min: -std::f64::consts::TAU,
            max: std::f64::consts::TAU,
        },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$ANGBASE", 50),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-angle-base",
        },
    },
    Setting {
        name: "draft.angle.direction",
        compat: Some("ANGDIR"),
        kind: Kind::Int { min: 0, max: 1 },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$ANGDIR", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-angle-direction",
        },
    },
    Setting {
        name: "draft.limits.check",
        compat: Some("LIMCHECK"),
        kind: Kind::Bool,
        scope: Scope::Drawing,
        default: "false",
        header: hv("$LIMCHECK", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-limits-check",
        },
    },
    Setting {
        name: "draft.limits.min",
        compat: Some("LIMMIN"),
        kind: Kind::Point,
        scope: Scope::Drawing,
        default: "0,0",
        header: hv("$LIMMIN", 10),
        quantity: Quantity::Point,
        description: Text {
            message_id: "db-setting-draft-limits-min",
        },
    },
    Setting {
        name: "draft.limits.max",
        compat: Some("LIMMAX"),
        kind: Kind::Point,
        scope: Scope::Drawing,
        default: "420,297",
        header: hv("$LIMMAX", 10),
        quantity: Quantity::Point,
        description: Text {
            message_id: "db-setting-draft-limits-max",
        },
    },
    Setting {
        name: "draft.point.style",
        compat: Some("PDMODE"),
        kind: Kind::Int { min: 0, max: 127 },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$PDMODE", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-point-style",
        },
    },
    Setting {
        name: "draft.point.size",
        compat: Some("PDSIZE"),
        kind: Kind::Real {
            min: -1e6,
            max: 1e6,
        },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$PDSIZE", 40),
        quantity: Quantity::PositiveLength,
        description: Text {
            message_id: "db-setting-draft-point-size",
        },
    },
    Setting {
        name: "snap.object.running",
        compat: Some("OSMODE"),
        kind: Kind::Int { min: 0, max: 32767 },
        scope: Scope::Drawing,
        default: "63",
        header: hv("$OSMODE", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-snap-object-running",
        },
    },
    Setting {
        name: "snap.object.geometric_center",
        compat: None,
        kind: Kind::Bool,
        scope: Scope::Drawing,
        default: "false",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-snap-object-geometric-center",
        },
    },
    Setting {
        name: "draft.ortho",
        compat: Some("ORTHOMODE"),
        kind: Kind::Bool,
        scope: Scope::Drawing,
        default: "false",
        header: hv("$ORTHOMODE", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-ortho",
        },
    },
    Setting {
        name: "draft.polar.on",
        compat: Some("POLARMODE"),
        kind: Kind::Bool,
        scope: Scope::Drawing,
        default: "false",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-polar-on",
        },
    },
    Setting {
        name: "draft.polar.increment",
        compat: Some("POLARANG"),
        kind: Kind::Real {
            min: 1e-6,
            max: 360.0,
        },
        scope: Scope::Drawing,
        default: "90",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-polar-increment",
        },
    },
    Setting {
        name: "draft.polar.additional",
        compat: Some("POLARADD"),
        kind: Kind::Angles,
        scope: Scope::Drawing,
        default: "",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-polar-additional",
        },
    },
    Setting {
        name: "draft.snap.on",
        compat: Some("SNAPMODE"),
        kind: Kind::Bool,
        scope: Scope::Drawing,
        default: "false",
        header: hv("$SNAPMODE", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-snap-on",
        },
    },
    Setting {
        name: "draft.snap.spacing",
        compat: Some("SNAPUNIT"),
        kind: Kind::Point,
        scope: Scope::Drawing,
        default: "10,10",
        header: hv("$SNAPUNIT", 10),
        quantity: Quantity::Point,
        description: Text {
            message_id: "db-setting-draft-snap-spacing",
        },
    },
    Setting {
        name: "draft.snap.base",
        compat: Some("SNAPBASE"),
        kind: Kind::Point,
        scope: Scope::Drawing,
        default: "0,0",
        header: hv("$SNAPBASE", 10),
        quantity: Quantity::Point,
        description: Text {
            message_id: "db-setting-draft-snap-base",
        },
    },
    Setting {
        name: "draft.snap.angle",
        compat: Some("SNAPANG"),
        kind: Kind::Real {
            min: -std::f64::consts::TAU,
            max: std::f64::consts::TAU,
        },
        scope: Scope::Drawing,
        default: "0",
        header: hv("$SNAPANG", 50),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-snap-angle",
        },
    },
    Setting {
        name: "draft.grid.on",
        compat: Some("GRIDMODE"),
        kind: Kind::Bool,
        scope: Scope::Drawing,
        default: "false",
        header: hv("$GRIDMODE", 70),
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-draft-grid-on",
        },
    },
    Setting {
        name: "draft.grid.spacing",
        compat: Some("GRIDUNIT"),
        kind: Kind::Point,
        scope: Scope::Drawing,
        default: "10,10",
        header: hv("$GRIDUNIT", 10),
        quantity: Quantity::Point,
        description: Text {
            message_id: "db-setting-draft-grid-spacing",
        },
    },
    Setting {
        name: "command.echo",
        compat: Some("CMDECHO"),
        kind: Kind::Bool,
        scope: Scope::Session,
        default: "true",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-command-echo",
        },
    },
    Setting {
        name: "command.autocomplete.on",
        compat: None,
        kind: Kind::Bool,
        scope: Scope::Session,
        default: "true",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-command-autocomplete-on",
        },
    },
    Setting {
        name: "command.autocorrect.on",
        compat: None,
        kind: Kind::Bool,
        scope: Scope::Session,
        default: "true",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-command-autocorrect-on",
        },
    },
    Setting {
        name: "command.history.length",
        compat: None,
        kind: Kind::Int { min: 10, max: 1000 },
        scope: Scope::Session,
        default: "200",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-command-history-length",
        },
    },
    Setting {
        name: "snap.object.on",
        compat: None,
        kind: Kind::Bool,
        scope: Scope::Session,
        default: "true",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-snap-object-on",
        },
    },
    Setting {
        name: "snap.override.typed_first",
        compat: None,
        kind: Kind::Bool,
        scope: Scope::Session,
        default: "false",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-snap-override-typed-first",
        },
    },
    Setting {
        name: "snap.aperture",
        compat: Some("APERTURE"),
        kind: Kind::Int { min: 1, max: 50 },
        scope: Scope::Session,
        default: "10",
        header: None,
        quantity: Quantity::Neither,
        description: Text {
            message_id: "db-setting-snap-aperture",
        },
    },
];

/// The settings a script may set while another command's prompt waits (DB-CMD-012): their values
/// cannot change the meaning of an answer already given. Other settings (units, angle base and
/// direction, limits, the coordinate system and the rest) are refused while a prompt waits.
pub const TRANSPARENT_SETTINGS: &[&str] = &[
    "draft.layer.current",
    "draft.color.current",
    "draft.linetype.current",
    "draft.lineweight.current",
    "snap.object.running",
    "draft.ortho",
    "draft.polar.on",
    "draft.polar.increment",
    "draft.polar.additional",
    "draft.snap.on",
    "draft.snap.spacing",
    "draft.snap.base",
    "draft.snap.angle",
    "draft.grid.on",
    "draft.grid.spacing",
];

/// The error codes of the settings area.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// No setting has the name given, by product name or compatibility name.
    pub const SETTING_UNKNOWN: ErrorCode = ErrorCode {
        code: "db.cmd.setting_unknown",
        category: Category::NotFound,
        message_id: "db-cmd-setting-unknown",
    };
    /// A value does not fit the type or range of its setting.
    pub const SETTING_VALUE: ErrorCode = ErrorCode {
        code: "db.cmd.setting_value",
        category: Category::InvalidInput,
        message_id: "db-cmd-setting-value",
    };
    /// The setting is read only.
    pub const SETTING_READ_ONLY: ErrorCode = ErrorCode {
        code: "db.cmd.setting_read_only",
        category: Category::Conflict,
        message_id: "db-cmd-setting-read-only",
    };
    /// The current layer cannot be frozen, deleted or merged away.
    pub const CURRENT_LAYER: ErrorCode = ErrorCode {
        code: "db.cmd.current_layer",
        category: Category::Conflict,
        message_id: "db-cmd-current-layer",
    };
    /// The layer is frozen, so it cannot be made current.
    pub const LAYER_FROZEN: ErrorCode = ErrorCode {
        code: "db.cmd.layer_frozen",
        category: Category::Conflict,
        message_id: "db-cmd-layer-frozen",
    };
    /// Every code of this module, for the registry check.
    pub const ALL: &[ErrorCode] = &[
        SETTING_UNKNOWN,
        SETTING_VALUE,
        SETTING_READ_ONLY,
        CURRENT_LAYER,
        LAYER_FROZEN,
    ];
}

/// The setting named `name`, by product name or compatibility name, without regard to case.
#[must_use]
pub fn find(name: &str) -> Option<&'static Setting> {
    let key = fold(name.trim());
    TABLE
        .iter()
        .find(|s| fold(s.name) == key || s.compat.is_some_and(|c| fold(c) == key))
}

/// A value of a setting.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A flag.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A finite number.
    Real(f64),
    /// A point in the plane.
    Point([f64; 2]),
    /// A list of angles in degrees.
    List(Vec<f64>),
    /// Text: a linetype name or `ByLayer`/`ByBlock`, or a layer identifier.
    Text(String),
}

impl Value {
    /// The JSON form of the value, as results and the rows of the drawing show it.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Bool(b) => serde_json::Value::Bool(*b),
            Self::Int(i) => serde_json::json!(i),
            Self::Real(x) => serde_json::json!(x),
            Self::Point(p) => serde_json::json!(p),
            Self::List(list) => serde_json::json!(list),
            Self::Text(t) => serde_json::Value::String(t.clone()),
        }
    }
}

fn invalid(setting: &Setting) -> Error {
    Error::new(codes::SETTING_VALUE).with_detail("name", setting.name)
}

fn in_range_int(setting: &Setting, value: i64) -> Result<i64> {
    match setting.kind {
        Kind::Int { min, max } if (min..=max).contains(&value) => Ok(value),
        _ => Err(invalid(setting)),
    }
}

fn in_range_real(setting: &Setting, value: f64) -> Result<f64> {
    match setting.kind {
        Kind::Real { min, max } if value.is_finite() && (min..=max).contains(&value) => Ok(value),
        _ => Err(invalid(setting)),
    }
}

fn angle(setting: &Setting, value: f64) -> Result<f64> {
    if value.is_finite() && value > 0.0 && value < 360.0 {
        Ok(value)
    } else {
        Err(invalid(setting))
    }
}

/// Parses the text of a value of `setting`, checked against its type and range.
///
/// # Errors
///
/// `db.cmd.setting_value` (detail `name`) for text that does not fit the setting.
pub fn parse_text(setting: &Setting, text: &str) -> Result<Value> {
    let t = text.trim();
    match setting.kind {
        Kind::Bool => match t.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Ok(Value::Bool(true)),
            "false" | "off" | "no" | "0" => Ok(Value::Bool(false)),
            _ => Err(invalid(setting)),
        },
        Kind::Int { .. } => {
            let value = t.parse::<i64>().map_err(|_| invalid(setting))?;
            in_range_int(setting, value).map(Value::Int)
        }
        Kind::Real { .. } => {
            let value = t.parse::<f64>().map_err(|_| invalid(setting))?;
            in_range_real(setting, value).map(Value::Real)
        }
        Kind::Point => parse_point(setting, t).map(Value::Point),
        Kind::Angles => {
            if t.is_empty() {
                return Ok(Value::List(Vec::new()));
            }
            t.split(',')
                .map(|part| {
                    let value = part.trim().parse::<f64>().map_err(|_| invalid(setting))?;
                    angle(setting, value)
                })
                .collect::<Result<Vec<f64>>>()
                .map(Value::List)
        }
        Kind::Layer | Kind::Linetype => {
            if t.is_empty() {
                Err(invalid(setting))
            } else {
                Ok(Value::Text(t.to_owned()))
            }
        }
        Kind::Unit => Ok(Value::Text(t.to_owned())),
    }
}

fn parse_point(setting: &Setting, text: &str) -> Result<[f64; 2]> {
    let parts: Vec<&str> = text.split(',').collect();
    match parts.as_slice() {
        [x, y] => {
            let x = x.trim().parse::<f64>().map_err(|_| invalid(setting))?;
            let y = y.trim().parse::<f64>().map_err(|_| invalid(setting))?;
            if x.is_finite() && y.is_finite() {
                Ok([x, y])
            } else {
                Err(invalid(setting))
            }
        }
        _ => Err(invalid(setting)),
    }
}

/// Reads a value of `setting` from JSON, checked against its type and range.
///
/// # Errors
///
/// `db.cmd.setting_value` (detail `name`) for a value that does not fit the setting.
pub fn parse_json(setting: &Setting, json: &serde_json::Value) -> Result<Value> {
    use serde_json::Value as Json;
    match (setting.kind, json) {
        (Kind::Bool, Json::Bool(b)) => Ok(Value::Bool(*b)),
        (Kind::Int { .. }, Json::Number(n)) => {
            let value = n.as_i64().ok_or_else(|| invalid(setting))?;
            in_range_int(setting, value).map(Value::Int)
        }
        (Kind::Real { .. }, Json::Number(n)) => {
            let value = n.as_f64().ok_or_else(|| invalid(setting))?;
            in_range_real(setting, value).map(Value::Real)
        }
        (Kind::Point, Json::Array(items)) => match items.as_slice() {
            [Json::Number(x), Json::Number(y)] => {
                let x = x.as_f64().ok_or_else(|| invalid(setting))?;
                let y = y.as_f64().ok_or_else(|| invalid(setting))?;
                if x.is_finite() && y.is_finite() {
                    Ok(Value::Point([x, y]))
                } else {
                    Err(invalid(setting))
                }
            }
            _ => Err(invalid(setting)),
        },
        (Kind::Angles, Json::Array(items)) => items
            .iter()
            .map(|item| {
                let value = item.as_f64().ok_or_else(|| invalid(setting))?;
                angle(setting, value)
            })
            .collect::<Result<Vec<f64>>>()
            .map(Value::List),
        // A typed value arrives as text (`SETVAR POLARANG 45`): it is read as the setting's text.
        (_, Json::String(text)) => parse_text(setting, text),
        _ => Err(invalid(setting)),
    }
}

/// The value a setting has when no value is stored.
#[must_use]
pub fn default_value(setting: &Setting) -> Value {
    parse_text(setting, setting.default).unwrap_or_else(|_| Value::Text(setting.default.to_owned()))
}

/// Whether `id` is a layer record of `snapshot`.
fn is_layer(snapshot: &Snapshot, id: ObjectId) -> bool {
    snapshot.resolve(id).ok().is_some_and(|handle| {
        snapshot
            .type_key(handle)
            .is_some_and(|key| key.as_str() == LAYER)
    })
}

/// The identifier of the layer named `name`, without regard to case.
fn layer_id(snapshot: &Snapshot, name: &str) -> Option<ObjectId> {
    snapshot
        .iter_type(LAYER)
        .ok()?
        .map(|(_, object)| object.id)
        .find(|id| name_of(snapshot, Some(*id)).is_some_and(|n| fold(&n) == fold(name)))
}

/// The value to store for `value` of `setting`, checked against the snapshot: a layer must exist
/// and not be frozen, and a linetype must exist. A layer is stored by its identifier.
///
/// # Errors
///
/// `db.cmd.setting_read_only` for the unit setting; `db.cmd.setting_value` for a layer or linetype
/// the drawing does not have; `db.cmd.layer_frozen` for a frozen layer.
pub fn stored_form(snapshot: &Snapshot, setting: &Setting, value: Value) -> Result<Value> {
    match (setting.kind, value) {
        (Kind::Unit, _) => {
            Err(Error::new(codes::SETTING_READ_ONLY).with_detail("name", setting.name))
        }
        (Kind::Layer, Value::Text(name)) => {
            let id = layer_id(snapshot, &name).ok_or_else(|| invalid(setting))?;
            if flag(snapshot, id, layer::FROZEN) == Some(true) {
                return Err(Error::new(codes::LAYER_FROZEN).with_detail("name", name));
            }
            Ok(Value::Text(id.to_string()))
        }
        (Kind::Linetype, Value::Text(name)) => match fold(&name).as_str() {
            "BYLAYER" => Ok(Value::Text("ByLayer".to_owned())),
            "BYBLOCK" => Ok(Value::Text("ByBlock".to_owned())),
            "CONTINUOUS" => Ok(Value::Text("Continuous".to_owned())),
            _ => match linetype_ref(snapshot, &name) {
                // The linetype is stored by identifier, like the layer: a rename does not stale it.
                Ok(Some(id)) => Ok(Value::Text(id.to_string())),
                _ => Err(invalid(setting)),
            },
        },
        (Kind::Angles, Value::List(angles)) if angles.len() > arc_draft::snap::MAX_POLAR_ANGLES => {
            Err(invalid(setting))
        }
        (Kind::Int { .. }, Value::Int(value)) if setting.name == CURRENT_LINEWEIGHT => {
            if STANDARD_LINEWEIGHTS.contains(&value) {
                Ok(Value::Int(value))
            } else {
                Err(invalid(setting))
            }
        }
        (_, value) => Ok(value),
    }
}

/// The standard lineweights in hundredths of a millimeter, with the three codes of by layer, by block
/// and the default weight (-1, -2 and -3). A current lineweight is one of these.
pub const STANDARD_LINEWEIGHTS: &[i64] = &[
    -3, -2, -1, 0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120,
    140, 158, 200, 211,
];

/// The identifier of the current linetype when it names a linetype object of `snapshot`.
#[must_use]
pub fn current_linetype(snapshot: &Snapshot) -> Option<ObjectId> {
    let text = store::row_value(snapshot, CURRENT_LINETYPE).and_then(|value| match value {
        Value::Text(text) => Some(text),
        _ => None,
    })?;
    let id = text.parse::<ObjectId>().ok()?;
    let handle = snapshot.resolve(id).ok()?;
    (snapshot.type_key(handle).map(|k| k.as_str()) == Some(arc_draft::types::LINETYPE))
        .then_some(id)
}

/// The product name of the length precision.
pub const LENGTH_PRECISION: &str = "draft.units.length_precision";
/// The product name of the angle precision.
pub const ANGLE_PRECISION: &str = "draft.units.angle_precision";
/// The product name of the angle base.
pub const ANGLE_BASE: &str = "draft.angle.base";
/// The product name of the angle direction.
pub const ANGLE_DIRECTION: &str = "draft.angle.direction";

/// The product name of the drawing's current layer.
pub const CURRENT_LAYER: &str = "draft.layer.current";
/// The product name of the current color.
pub const CURRENT_COLOR: &str = "draft.color.current";
/// The product name of the current linetype.
pub const CURRENT_LINETYPE: &str = "draft.linetype.current";

/// The product name of the current lineweight.
pub const CURRENT_LINEWEIGHT: &str = "draft.lineweight.current";
/// The product name of the current transparency.
pub const CURRENT_TRANSPARENCY: &str = "draft.transparency.current";
/// The product name of the object linetype scale.
pub const OBJECT_SCALE: &str = "draft.linetype.object_scale";
/// The product name of the global linetype scale.
pub const GLOBAL_SCALE: &str = "draft.linetype.global_scale";

/// The space unit of `snapshot`: the model space's unit, unitless when it declares none.
#[must_use]
pub fn space_unit(snapshot: &Snapshot) -> arc_draft::types::SpaceUnit {
    crate::model_space(snapshot)
        .and_then(|space| snapshot.resolve(space).ok())
        .and_then(|handle| snapshot.get(handle).ok())
        .and_then(|object| {
            arc_draft_dxf::fields::Reader::new(
                snapshot.registry(),
                arc_draft::types::SPACE,
                &object.values,
            )
            .enum_code("unit")
        })
        .and_then(arc_draft::types::SpaceUnit::of)
        .unwrap_or(arc_draft::types::SpaceUnit::Unitless)
}

/// The JSON form of the value of `setting` that a result shows: a layer by its name, the
/// insertion unit by its name, other values as they are.
#[must_use]
pub fn presented(snapshot: &Snapshot, setting: &Setting, value: &Value) -> serde_json::Value {
    match (setting.kind, value) {
        (Kind::Unit, _) => {
            let name = space_unit(snapshot)
                .unit()
                .map_or("unitless", |unit| unit.name);
            serde_json::Value::String(name.to_owned())
        }
        (Kind::Linetype, Value::Text(text)) => {
            let name = text
                .parse::<ObjectId>()
                .ok()
                .and_then(|id| name_of(snapshot, Some(id)))
                .unwrap_or_else(|| text.clone());
            serde_json::Value::String(name)
        }
        (Kind::Layer, Value::Text(text)) => {
            let name = text
                .parse::<ObjectId>()
                .ok()
                .and_then(|id| name_of(snapshot, Some(id)))
                .unwrap_or_else(|| text.clone());
            serde_json::Value::String(name)
        }
        _ => value.to_json(),
    }
}

/// The current layer: the layer the setting names, when it still exists, else layer `0`.
#[must_use]
pub fn current_layer(snapshot: &Snapshot) -> Option<ObjectId> {
    let stored = store::row_value(snapshot, CURRENT_LAYER).and_then(|value| match value {
        Value::Text(text) => text.parse::<ObjectId>().ok(),
        _ => None,
    });
    stored
        .filter(|id| is_layer(snapshot, *id))
        .or_else(|| layer_id(snapshot, "0"))
}

/// Whether `id` is the current layer of the drawing.
#[must_use]
pub fn is_current_layer(snapshot: &Snapshot, id: ObjectId) -> bool {
    current_layer(snapshot) == Some(id)
}

/// Fails with `db.cmd.current_layer`, naming the layer, when `id` is the current layer. The layer
/// commands call it before they delete, merge away or freeze a layer.
///
/// # Errors
///
/// `db.cmd.current_layer` when the layer is the current layer.
pub fn refuse_current(snapshot: &Snapshot, id: ObjectId) -> Result<()> {
    if is_current_layer(snapshot, id) {
        let name = name_of(snapshot, Some(id)).unwrap_or_default();
        return Err(Error::new(codes::CURRENT_LAYER).with_detail("name", name));
    }
    Ok(())
}

/// The indexed color value of palette entry `index`, with its source kept.
fn indexed_color(index: u8) -> Doc {
    let [r, g, b] = indexed_rgb(index);
    Doc::Color(Box::new(ColorValue {
        rgba: Color::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        ),
        source: Some(ColorSource::Indexed(index)),
    }))
}

/// The appearance mode of an entity field: from its layer, from its block, or its own value.
fn by(mode: By) -> Doc {
    Doc::Enum(mode as u32)
}

/// The fields a new entity takes from the settings: its layer, its color, linetype, lineweight and
/// transparency (each by layer, by block or its own value) and its linetype scale.
#[must_use]
pub fn entity_fields(snapshot: &Snapshot) -> Vec<(usize, Doc)> {
    // Entity fields come from drawing settings only: no session setting takes part.
    let values = store::Values::of(snapshot, &|_| None);
    let mut out: Vec<(usize, Doc)> = Vec::new();
    if let Some(id) = current_layer(snapshot).or_else(|| default_layer(snapshot)) {
        out.push((entity::LAYER, Doc::Reference(id)));
    }
    color_fields(&values, &mut out);
    linetype_fields(snapshot, &values, &mut out);
    lineweight_fields(&values, &mut out);
    transparency_fields(&values, &mut out);
    out.push((entity::LINETYPE_SCALE, Doc::F64(values.real(OBJECT_SCALE))));
    out
}

/// The color fields of [`entity_fields`].
fn color_fields(values: &store::Values, out: &mut Vec<(usize, Doc)>) {
    match values.int(CURRENT_COLOR) {
        256 => out.push((entity::COLOR_BY, by(By::Layer))),
        0 => out.push((entity::COLOR_BY, by(By::Block))),
        index => {
            out.push((entity::COLOR_BY, by(By::Object)));
            if let Ok(index) = u8::try_from(index) {
                out.push((entity::COLOR, indexed_color(index)));
            }
        }
    }
}

/// The linetype fields of [`entity_fields`]. A stored identifier that no longer names a linetype
/// gives by layer, as a removed linetype does on the entities.
fn linetype_fields(snapshot: &Snapshot, values: &store::Values, out: &mut Vec<(usize, Doc)>) {
    let name = values.text(CURRENT_LINETYPE);
    if let Some(id) = current_linetype(snapshot) {
        out.push((entity::LINETYPE_BY, by(By::Object)));
        out.push((entity::LINETYPE, Doc::Reference(id)));
        return;
    }
    match fold(&name).as_str() {
        "BYBLOCK" => out.push((entity::LINETYPE_BY, by(By::Block))),
        "BYLAYER" => out.push((entity::LINETYPE_BY, by(By::Layer))),
        _ if name.parse::<ObjectId>().is_ok() => out.push((entity::LINETYPE_BY, by(By::Layer))),
        _ => {
            out.push((entity::LINETYPE_BY, by(By::Object)));
            if let Ok(Some(id)) = linetype_ref(snapshot, &name) {
                out.push((entity::LINETYPE, Doc::Reference(id)));
            }
        }
    }
}

/// The lineweight fields of [`entity_fields`]: -1 by layer, -2 by block, -3 the default weight.
fn lineweight_fields(values: &store::Values, out: &mut Vec<(usize, Doc)>) {
    match values.int(CURRENT_LINEWEIGHT) {
        -1 => out.push((entity::LINEWEIGHT_BY, by(By::Layer))),
        -2 => out.push((entity::LINEWEIGHT_BY, by(By::Block))),
        -3 => out.push((entity::LINEWEIGHT_BY, by(By::Object))),
        hundredths => {
            out.push((entity::LINEWEIGHT_BY, by(By::Object)));
            out.push((entity::LINEWEIGHT, Doc::F64(hundredths as f64 / 100.0)));
        }
    }
}

/// The transparency fields of [`entity_fields`]: -1 by layer, -2 by block, else a percentage.
fn transparency_fields(values: &store::Values, out: &mut Vec<(usize, Doc)>) {
    match values.int(CURRENT_TRANSPARENCY) {
        -1 => out.push((entity::TRANSPARENCY_BY, by(By::Layer))),
        -2 => out.push((entity::TRANSPARENCY_BY, by(By::Block))),
        percent => {
            out.push((entity::TRANSPARENCY_BY, by(By::Object)));
            out.push((entity::TRANSPARENCY, Doc::F64(percent as f64 / 100.0)));
        }
    }
}

/// Whether `text` matches `pattern`: `*` matches any run of characters and `?` one character,
/// both compared without regard to case.
#[must_use]
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = fold(pattern).chars().collect();
    let t: Vec<char> = fold(text).chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// A setting in a result: its names, scope, value and default.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Report {
    /// The product name.
    pub name: String,
    /// The compatibility name, when there is one.
    pub compat: Option<String>,
    /// The scope: `drawing` or `session`.
    pub scope: String,
    /// The current value.
    pub value: serde_json::Value,
    /// The default value.
    pub default: serde_json::Value,
}

/// The report of `setting` as the drawing `snapshot` shows it.
#[must_use]
pub fn report(snapshot: &Snapshot, session: store::SessionRead<'_>, setting: &Setting) -> Report {
    let value = store::current(snapshot, session, setting);
    let default = default_value(setting);
    Report {
        name: setting.name.to_owned(),
        compat: setting.compat.map(str::to_owned),
        scope: setting.scope.text().to_owned(),
        value: presented(snapshot, setting, &value),
        default: presented(snapshot, setting, &default),
    }
}

/// Parameters of `draftbuilder.settings.get`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameParams {
    /// The product name or compatibility name of the setting.
    #[schemars(length(min = 1, max = 128))]
    pub name: String,
}

/// Parameters of `draftbuilder.settings.set`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    /// The product name or compatibility name of the setting.
    #[schemars(length(min = 1, max = 128))]
    pub name: String,
    /// The new value: a flag, a number, a point as `[x, y]`, a list of angles, or a name.
    pub value: serde_json::Value,
}

/// Parameters of `draftbuilder.settings.setvar` (`SETVAR`): a read without a value, a write with one.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetVarParams {
    /// The product name or compatibility name of the setting.
    #[schemars(length(min = 1, max = 128))]
    pub name: String,
    /// The new value; the setting is read when absent.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

/// Parameters of `draftbuilder.settings.list`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// A wildcard over the product and compatibility names, `*` and `?`; all settings when absent.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub filter: Option<String>,
}

/// The settings a listing returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Listing {
    /// The settings that match, in table order.
    pub settings: Vec<Report>,
}

/// Parameters of `draftbuilder.settings.current_layer` (`CLAYER`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerParams {
    /// The name of an existing layer that is not frozen.
    #[schemars(length(min = 1, max = 255))]
    pub layer: String,
}

/// The setting named `name`, or `db.cmd.setting_unknown`.
pub(crate) fn lookup(name: &str) -> Result<&'static Setting> {
    find(name).ok_or_else(|| Error::new(codes::SETTING_UNKNOWN).with_detail("name", name))
}

/// Stores `value` in the scope of `setting`, validated against the snapshot.
pub(crate) fn write(ctx: &mut Context<'_>, setting: &Setting, value: Value) -> Result<()> {
    let stored = stored_form(ctx.snapshot(), setting, value)?;
    match setting.scope {
        Scope::Drawing => store::write_row(ctx.txn()?, setting, Some(stored)),
        Scope::Session => ctx.set_setting(&store::session_key(setting), stored.to_json()),
    }
}

fn get(ctx: &mut Context<'_>, p: NameParams) -> Result<Report> {
    let setting = lookup(&p.name)?;
    Ok(report(ctx.snapshot(), &|key| ctx.setting(key), setting))
}

fn set(ctx: &mut Context<'_>, p: SetParams) -> Result<Report> {
    let setting = lookup(&p.name)?;
    if setting.kind == Kind::Unit {
        return Err(Error::new(codes::SETTING_READ_ONLY).with_detail("name", setting.name));
    }
    let value = parse_json(setting, &p.value)?;
    write(ctx, setting, value)?;
    Ok(report(ctx.snapshot(), &|key| ctx.setting(key), setting))
}

fn setvar(ctx: &mut Context<'_>, p: SetVarParams) -> Result<Report> {
    match p.value {
        None => get(ctx, NameParams { name: p.name }),
        Some(value) => set(
            ctx,
            SetParams {
                name: p.name,
                value,
            },
        ),
    }
}

fn list(ctx: &mut Context<'_>, p: ListParams) -> Result<Listing> {
    let pattern = p.filter.unwrap_or_else(|| "*".to_owned());
    let snapshot = ctx.snapshot();
    let read = |key: &str| ctx.setting(key);
    let settings = TABLE
        .iter()
        .filter(|s| {
            wildcard_match(&pattern, s.name)
                || s.compat.is_some_and(|c| wildcard_match(&pattern, c))
        })
        .map(|s| report(snapshot, &read, s))
        .collect();
    Ok(Listing { settings })
}

fn set_current_layer(ctx: &mut Context<'_>, p: LayerParams) -> Result<Report> {
    let setting = lookup(CURRENT_LAYER)?;
    write(ctx, setting, Value::Text(p.layer))?;
    Ok(report(ctx.snapshot(), &|key| ctx.setting(key), setting))
}

/// A prompt machine written as one function over the answers so far.
#[derive(Debug)]
pub(crate) struct Form {
    pub(crate) next: fn(&PromptState<'_>) -> Result<Step>,
}

impl arc_command::PromptMachine for Form {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        (self.next)(state)
    }

    fn check(&self, _: &PromptState<'_>, _: &Prompt, _: &PromptInput) -> Result<()> {
        Ok(())
    }
}

/// The text answers so far, in order.
pub(crate) fn texts(state: &PromptState<'_>) -> Vec<String> {
    state
        .answers
        .iter()
        .filter_map(|answer| match &answer.input {
            PromptInput::Text(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Ends a prompt machine with its parameters.
pub(crate) fn finish(value: serde_json::Value) -> Result<Step> {
    match value {
        serde_json::Value::Object(map) => Ok(Step::Done(map)),
        _ => Err(Error::new(codes::SETTING_VALUE)),
    }
}

const NAME_PROMPT: Message = Message {
    message_id: "db-cmd-setting-name-prompt",
};
const VALUE_PROMPT: Message = Message {
    message_id: "db-cmd-setting-value-prompt",
};
const LAYER_PROMPT: Message = Message {
    message_id: "db-cmd-current-layer-prompt",
};

fn ask_name(state: &PromptState<'_>) -> Result<Step> {
    match texts(state).first() {
        Some(name) => finish(serde_json::json!({"name": name})),
        None => Ok(Step::Ask(Prompt::new(
            "name",
            NAME_PROMPT.message_id,
            Accept::text(),
        ))),
    }
}

fn ask_value_for(state: &PromptState<'_>, optional: bool) -> Result<Step> {
    let answers = texts(state);
    let Some(name) = answers.first() else {
        return ask_name(state);
    };
    if answers.len() < 2 {
        return Ok(Step::Ask(Prompt::new(
            "value",
            VALUE_PROMPT.message_id,
            Accept::text(),
        )));
    }
    let setting = lookup(name)?;
    let value = match answers.get(1) {
        Some(text) if !text.trim().is_empty() => parse_text(setting, text)?.to_json(),
        _ if optional => return finish(serde_json::json!({"name": name})),
        _ => return Err(Error::new(codes::SETTING_VALUE).with_detail("name", setting.name)),
    };
    finish(serde_json::json!({"name": name, "value": value}))
}

fn ask_set(state: &PromptState<'_>) -> Result<Step> {
    ask_value_for(state, false)
}

fn ask_setvar(state: &PromptState<'_>) -> Result<Step> {
    ask_value_for(state, true)
}

fn ask_layer(state: &PromptState<'_>) -> Result<Step> {
    match texts(state).first() {
        Some(layer) => finish(serde_json::json!({"layer": layer})),
        None => Ok(Step::Ask(Prompt::new(
            "layer",
            LAYER_PROMPT.message_id,
            Accept::text(),
        ))),
    }
}

pub(crate) fn spec<P, R>(
    id: &'static str,
    label: Message,
    description: Message,
    permission: Permission,
    undo: UndoBehavior,
    prompt: fn(&PromptState<'_>) -> Result<Step>,
    run: impl Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
) -> CommandSpec
where
    P: serde::de::DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
{
    CommandSpec::new(id, label.message_id, description.message_id, run)
        .with_undo(undo)
        .with_permissions([permission])
        .with_prompt(std::sync::Arc::new(Form { next: prompt }))
}

/// The commands of the area: get, set, the typed `SETVAR`, the listing and the typed `CLAYER`.
fn specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "draftbuilder.settings.get",
            Message {
                message_id: "db-cmd-settings-get-label",
            },
            Message {
                message_id: "db-cmd-settings-get-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_name,
            get,
        ),
        spec(
            "draftbuilder.settings.set",
            Message {
                message_id: "db-cmd-settings-set-label",
            },
            Message {
                message_id: "db-cmd-settings-set-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_set,
            set,
        ),
        spec(
            "draftbuilder.settings.setvar",
            Message {
                message_id: "db-cmd-settings-setvar-label",
            },
            Message {
                message_id: "db-cmd-settings-setvar-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_setvar,
            setvar,
        ),
        spec(
            "draftbuilder.settings.list",
            Message {
                message_id: "db-cmd-settings-list-label",
            },
            Message {
                message_id: "db-cmd-settings-list-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_name,
            list,
        ),
        spec(
            "draftbuilder.settings.current_layer",
            Message {
                message_id: "db-cmd-settings-current-layer-label",
            },
            Message {
                message_id: "db-cmd-settings-current-layer-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_layer,
            set_current_layer,
        ),
    ]
}

/// Registers the commands of the area and applies its names.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered, and the errors of
/// [`register_names`].
pub fn register(registry: &mut Registry) -> Result<()> {
    // Settings commands may run beside a waiting prompt (DB-CMD-012).
    for spec in specs() {
        registry.register(spec.transparent())?;
    }
    register_names(registry, Area::Settings)
}

#[cfg(test)]
mod tests {
    use super::{TABLE, codes, default_value, find, parse_text, wildcard_match};

    const CATALOG: &str = include_str!("../../../../locales/en-US/draftbuilder-commands.ftl");

    #[test]
    fn the_error_registry_is_well_formed_and_every_message_is_defined() {
        assert_eq!(arc_base::error::registry_problems(codes::ALL), vec![]);
        for code in codes::ALL {
            assert!(
                CATALOG.contains(&format!("{} =", code.message_id)),
                "{}",
                code.message_id
            );
        }
    }

    #[test]
    fn every_name_is_unique_and_every_description_is_in_the_catalog() {
        let mut names = std::collections::BTreeSet::new();
        for setting in TABLE {
            assert!(names.insert(setting.name.to_owned()), "{}", setting.name);
            if let Some(compat) = setting.compat {
                assert!(names.insert(compat.to_ascii_lowercase()), "{compat}");
            }
            assert!(
                CATALOG.contains(&format!("{} =", setting.description.message_id)),
                "{}",
                setting.description.message_id
            );
        }
    }

    #[test]
    fn every_default_is_a_valid_value_of_its_setting() {
        for setting in TABLE {
            let text = parse_text(setting, setting.default);
            assert!(
                text.is_ok() || setting.default == "unitless",
                "{}",
                setting.name
            );
            assert_ne!(default_value(setting).to_json(), serde_json::Value::Null);
        }
    }

    #[test]
    fn compatibility_names_find_the_same_setting() {
        assert_eq!(find("lunits").map(|s| s.name), Some("draft.units.length"));
        assert_eq!(
            find("Draft.Units.Length").map(|s| s.name),
            Some("draft.units.length")
        );
        assert!(find("no.such.setting").is_none());
    }

    #[test]
    fn wildcards_match_runs_and_single_characters() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("draft.*.length", "draft.units.length"));
        assert!(wildcard_match("l?nes", "LINES"));
        assert!(wildcard_match("snap*", "snap.object.on"));
        assert!(!wildcard_match("snap?", "snap"));
        assert!(!wildcard_match("ltscale", "ltscal"));
        assert!(wildcard_match("*scale*", "draft.linetype.global_scale"));
    }
}
