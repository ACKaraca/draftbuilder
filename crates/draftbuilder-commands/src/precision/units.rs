//! Unit formats (DB-SNAP-030): `UNITS` sets the length and angle formats, the display precision,
//! the angle base direction and the positive angle direction. The values are drawing settings, so
//! they are saved in the drawing and undone like any setting; a format changes only how values
//! are shown and never a stored coordinate.

use arc_base::Result;
use arc_command::{CommandSpec, Context, Permission, PromptState, Registry, Step, UndoBehavior};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::Message;
use crate::settings::{self, Value};

/// The length format names, as the drawing setting codes them.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LengthFormat {
    /// Scientific notation.
    Scientific,
    /// Decimal.
    Decimal,
    /// Engineering: feet and decimal inches.
    Engineering,
    /// Architectural: feet and fractional inches.
    Architectural,
    /// Fractional.
    Fractional,
}

impl LengthFormat {
    /// The setting code of the format.
    #[must_use]
    pub fn code(self) -> i64 {
        match self {
            Self::Scientific => 1,
            Self::Decimal => 2,
            Self::Engineering => 3,
            Self::Architectural => 4,
            Self::Fractional => 5,
        }
    }
}

/// The angle format names, as the drawing setting codes them.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AngleFormat {
    /// Decimal degrees.
    DecimalDegrees,
    /// Degrees, minutes and seconds.
    DegreesMinutesSeconds,
    /// Grads.
    Grads,
    /// Radians.
    Radians,
    /// Surveyor's bearings.
    SurveyorBearings,
}

impl AngleFormat {
    /// The setting code of the format.
    #[must_use]
    pub fn code(self) -> i64 {
        match self {
            Self::DecimalDegrees => 0,
            Self::DegreesMinutesSeconds => 1,
            Self::Grads => 2,
            Self::Radians => 3,
            Self::SurveyorBearings => 4,
        }
    }
}

/// Parameters of `draftbuilder.precision.units`. Each field that is given changes its setting;
/// the others keep their values.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnitsParams {
    /// The length format.
    #[serde(default)]
    pub length_format: Option<LengthFormat>,
    /// The decimal places of lengths, 0 to 8.
    #[serde(default)]
    pub length_precision: Option<i64>,
    /// The angle format.
    #[serde(default)]
    pub angle_format: Option<AngleFormat>,
    /// The decimal places of angles, 0 to 8.
    #[serde(default)]
    pub angle_precision: Option<i64>,
    /// The direction of angle zero, in radians from the x axis of the user coordinate system.
    #[serde(default)]
    pub angle_base: Option<f64>,
    /// Whether positive angles run clockwise (false: counterclockwise).
    #[serde(default)]
    pub clockwise: Option<bool>,
}

/// The unit settings after the command, as the settings report them.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UnitsOut {
    /// The reports of the six settings, in the order of [`UnitsParams`].
    pub settings: Vec<settings::Report>,
}

/// The names of the six settings, in the order the report lists them.
const NAMES: [&str; 6] = [
    "draft.units.length",
    "draft.units.length_precision",
    "draft.units.angle",
    "draft.units.angle_precision",
    "draft.angle.base",
    "draft.angle.direction",
];

fn units(ctx: &mut Context<'_>, p: UnitsParams) -> Result<UnitsOut> {
    let writes = [
        (
            "draft.units.length",
            p.length_format.map(|f| json!(f.code())),
        ),
        (
            "draft.units.length_precision",
            p.length_precision.map(|v| json!(v)),
        ),
        ("draft.units.angle", p.angle_format.map(|f| json!(f.code()))),
        (
            "draft.units.angle_precision",
            p.angle_precision.map(|v| json!(v)),
        ),
        ("draft.angle.base", p.angle_base.map(|v| json!(v))),
        (
            "draft.angle.direction",
            p.clockwise.map(|c| json!(i64::from(c))),
        ),
    ];
    for (name, given) in writes {
        if let Some(given) = given {
            let setting = settings::lookup(name)?;
            // The setting's own parser checks the range, as the set command does.
            let value: Value = settings::parse_json(setting, &given)?;
            settings::write(ctx, setting, value)?;
        }
    }
    let snapshot = ctx.snapshot();
    let read = |key: &str| ctx.setting(key);
    let settings = NAMES
        .iter()
        .map(|name| {
            settings::lookup(name).map(|setting| settings::report(snapshot, &read, setting))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(UnitsOut { settings })
}

/// The units take their values from the parameters; with none given the command changes nothing.
fn ask_units(_: &PromptState<'_>) -> Result<Step> {
    settings::finish(serde_json::json!({}))
}

/// Registers the unit format command.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let spec: CommandSpec = settings::spec(
        "draftbuilder.precision.units",
        Message {
            message_id: "db-cmd-precision-units-label",
        },
        Message {
            message_id: "db-cmd-precision-units-description",
        },
        Permission::Edit,
        UndoBehavior::Undoable,
        ask_units,
        units,
    );
    registry.register(spec)
}
