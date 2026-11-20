//! User coordinate systems (DB-SNAP-025, DB-SNAP-026), planar: an origin and an X direction in the
//! plane; the Y direction is a quarter turn counterclockwise from it. The current system and the
//! previous one, and the named systems, are rows of the drawing (`record_kind` `ucs`), so they
//! save with the drawing and changes are undoable. The current system is written to the header
//! of a DXF file. The typed coordinates of the point entry are in the current system
//! ([`plane_transform`]); the inquiry results are reported in it.

use arc_base::{Error, Result};
use arc_command::prompt::PlaneTransform;
use arc_command::{
    Accept, Context, KeywordOption, Permission, Prompt, PromptInput, PromptState, Registry, Step,
    UndoBehavior,
};
use arc_doc::{Snapshot, StoredForeign, Txn};
use arc_io_dxf::model::HeaderVariable;
use arc_io_dxf::{Drawing, Group, Value as Dxf};
use draftbuilder_model::database::FORMAT;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::codes;
use crate::draw::create::drawing_point;
use crate::error::Message;
use crate::error::codes as base_codes;

/// The `record_kind` of the coordinate system rows.
pub const RECORD_KIND: &str = "ucs";
/// The row of the current system.
const CURRENT: &str = "*current*";
/// The row of the previous system.
const PREVIOUS: &str = "*previous*";
/// The longest name of a named system, in characters.
const MAX_NAME_CHARS: usize = 255;

/// A planar coordinate system: an origin and a unit X direction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ucs {
    /// The origin, in drawing units.
    pub origin: [f64; 2],
    /// The unit X direction.
    pub x_axis: [f64; 2],
}

impl Ucs {
    /// The world coordinate system.
    pub const WORLD: Self = Self {
        origin: [0.0, 0.0],
        x_axis: [1.0, 0.0],
    };

    /// The unit Y direction: a quarter turn counterclockwise from X.
    #[must_use]
    pub fn y_axis(&self) -> [f64; 2] {
        [-self.x_axis[1], self.x_axis[0]]
    }

    /// The system with `origin` and the X direction from `origin` toward `toward`.
    ///
    /// # Errors
    ///
    /// `db.cmd.points_collinear` when the two points coincide or a value is not finite.
    pub fn from_points(origin: [f64; 2], toward: [f64; 2]) -> Result<Self> {
        Self::from_direction(origin, [toward[0] - origin[0], toward[1] - origin[1]])
    }

    /// The system with `origin` and the X direction `direction`, normalized directly (no point is
    /// formed from the origin and the direction, so a large origin keeps the direction).
    ///
    /// # Errors
    ///
    /// `db.cmd.points_collinear` when the direction has no length or a value is not finite.
    pub fn from_direction(origin: [f64; 2], direction: [f64; 2]) -> Result<Self> {
        let length = direction[0].hypot(direction[1]);
        if !(length.is_finite() && length > 0.0 && origin.iter().all(|v| v.is_finite())) {
            return Err(Error::new(base_codes::POINTS_COLLINEAR));
        }
        Ok(Self {
            origin,
            x_axis: [direction[0] / length, direction[1] / length],
        })
    }

    /// The coordinates of the world point `point` in this system.
    #[must_use]
    pub fn local(&self, point: [f64; 2]) -> [f64; 2] {
        let d = [point[0] - self.origin[0], point[1] - self.origin[1]];
        let y = self.y_axis();
        [
            d[0] * self.x_axis[0] + d[1] * self.x_axis[1],
            d[0] * y[0] + d[1] * y[1],
        ]
    }

    /// The transform from this system's coordinates to world coordinates.
    ///
    /// # Errors
    ///
    /// The errors of [`PlaneTransform::new`] for a value that is not finite.
    pub fn transform(&self) -> Result<PlaneTransform> {
        let [x0, x1] = self.x_axis;
        let [y0, y1] = self.y_axis();
        PlaneTransform::new([[x0, y0], [x1, y1]], self.origin)
    }
}

/// The current system of `snapshot`; the world system when none is stored.
#[must_use]
pub fn current(snapshot: &Snapshot) -> Ucs {
    read(snapshot, CURRENT).unwrap_or(Ucs::WORLD)
}

/// The transform of the current system, for the point entry of a session: typed coordinates are
/// read in it.
///
/// # Errors
///
/// The errors of [`Ucs::transform`].
pub fn plane_transform(snapshot: &Snapshot) -> Result<PlaneTransform> {
    current(snapshot).transform()
}

/// The system stored under `name`, when there is one.
fn read(snapshot: &Snapshot, name: &str) -> Option<Ucs> {
    let row = snapshot.foreign_rows().find(|row| {
        row.source_format == FORMAT
            && row.record_kind == RECORD_KIND
            && row.source_id.as_deref() == Some(name)
    })?;
    serde_json::from_slice(&row.data).ok()
}

/// The row of `name` in `txn`: written with `value`, or removed when it is `None`.
fn write(txn: &mut Txn, name: &str, value: Option<Ucs>) -> Result<()> {
    let existing = txn.snapshot().foreign_rows().find(|row| {
        row.source_format == FORMAT
            && row.record_kind == RECORD_KIND
            && row.source_id.as_deref() == Some(name)
    });
    let existing = existing.map(|row| row.id);
    match (existing, value) {
        (Some(id), Some(ucs)) => txn.set_foreign(row(id, name, &ucs)),
        (None, Some(ucs)) => txn.add_foreign(row(0, name, &ucs)).map(drop),
        (Some(id), None) => txn.remove_foreign(id),
        (None, None) => Ok(()),
    }
}

/// The foreign row of the system `ucs` under identifier `id`.
fn row(id: i64, name: &str, ucs: &Ucs) -> StoredForeign {
    StoredForeign {
        id,
        source_format: FORMAT.to_owned(),
        record_kind: RECORD_KIND.to_owned(),
        source_id: Some(name.to_owned()),
        data: serde_json::to_vec(ucs).unwrap_or_default(),
        ..StoredForeign::default()
    }
}

/// Checks the name of a saved system: one to 255 characters, none of them a wildcard or a control
/// character.
fn check_name(name: &str) -> Result<()> {
    let length = name.trim().chars().count();
    let bad = name
        .chars()
        .any(|c| matches!(c, '*' | '?') || c.is_control());
    if length == 0 || length > MAX_NAME_CHARS || bad {
        return Err(Error::new(codes::UCS_NAME).with_detail("name", name));
    }
    Ok(())
}

/// Makes `next` the current system, keeping the old one as the previous system.
fn set_current(txn: &mut Txn, current_now: Ucs, next: Ucs) -> Result<()> {
    write(txn, PREVIOUS, Some(current_now))?;
    write(txn, CURRENT, Some(next))
}

/// The header variables of the current system: its origin and X and Y directions, with Z zero.
#[must_use]
pub fn header_values(snapshot: &Snapshot) -> Vec<HeaderVariable> {
    let ucs = current(snapshot);
    let y = ucs.y_axis();
    let vector = |name: &str, v: [f64; 2]| HeaderVariable {
        name: name.to_owned(),
        values: vec![
            Group::double(10, v[0]),
            Group::double(20, v[1]),
            Group::double(30, 0.0),
        ],
    };
    vec![
        vector("$UCSORG", ucs.origin),
        vector("$UCSXDIR", ucs.x_axis),
        vector("$UCSYDIR", y),
    ]
}

/// Reads the current system from the header of `drawing` into `ctx`, when the header holds
/// `$UCSORG` and `$UCSXDIR` with nonzero X. The world system applies otherwise.
///
/// # Errors
///
/// The transaction's errors.
pub fn load_header(ctx: &mut Context<'_>, drawing: &Drawing) -> Result<u64> {
    // The three components of a header vector (codes 10, 20 and 30).
    let vector = |name: &str| -> Option<[f64; 3]> {
        let groups = drawing.header_variable(name)?;
        let get = |code: i16| {
            groups
                .iter()
                .find(|g| g.code == code)
                .and_then(|g| match g.value {
                    Dxf::Double(v) => Some(v),
                    _ => None,
                })
        };
        Some([get(10)?, get(20)?, get(30).unwrap_or(0.0)])
    };
    let (Some(origin), Some(x)) = (vector("$UCSORG"), vector("$UCSXDIR")) else {
        return Ok(0);
    };
    // A system is accepted only when it is planar and right-handed, the form this module keeps: the
    // origin and the X direction have no Z, and a stated Y direction is the X direction turned a
    // quarter turn counterclockwise. Anything else (a 3D or mirrored system) is ignored and counted.
    let planar = origin[2].abs() <= 1e-9 && x[2].abs() <= 1e-9;
    let y_ok = match vector("$UCSYDIR") {
        None => true,
        Some(y) => {
            let turned = [-x[1], x[0]];
            let scale = x[0].hypot(x[1]).max(1.0);
            y[2].abs() <= 1e-9 * scale
                && (y[0] - turned[0]).abs() <= 1e-9 * scale
                && (y[1] - turned[1]).abs() <= 1e-9 * scale
        }
    };
    let accepted = match (
        planar && y_ok,
        Ucs::from_direction([origin[0], origin[1]], [x[0], x[1]]),
    ) {
        (true, Ok(ucs)) => Some(ucs),
        _ => None,
    };
    match accepted {
        Some(ucs) => {
            write(ctx.txn()?, CURRENT, Some(ucs))?;
            Ok(0)
        }
        None => Ok(1),
    }
}

/// The result of a coordinate system command: the system and its name, when it has one.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UcsReport {
    /// The name of the system, when it is a saved one.
    pub name: Option<String>,
    /// The origin, in drawing units.
    pub origin: [f64; 2],
    /// The X direction.
    pub x_axis: [f64; 2],
    /// The Y direction.
    pub y_axis: [f64; 2],
}

fn report(name: Option<String>, ucs: Ucs) -> UcsReport {
    UcsReport {
        name,
        origin: ucs.origin,
        x_axis: ucs.x_axis,
        y_axis: ucs.y_axis(),
    }
}

/// How the current system is defined. `mode` is `origin` (an origin and an optional X point),
/// `three_points` (an origin, an X point and a Y point), `world` or `previous`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UcsParams {
    /// The mode: `origin`, `three_points`, `world` or `previous`.
    #[schemars(length(min = 1, max = 16))]
    pub mode: String,
    /// The origin, for `origin` and `three_points`.
    #[serde(default)]
    pub origin: Option<[f64; 2]>,
    /// A point on the X axis, from the origin: optional for `origin`, required for `three_points`.
    #[serde(default)]
    pub x_point: Option<[f64; 2]>,
    /// A point off the X axis, required for `three_points`.
    #[serde(default)]
    pub y_point: Option<[f64; 2]>,
}

fn define(ctx: &mut Context<'_>, p: UcsParams) -> Result<UcsReport> {
    let before = current(ctx.snapshot());
    let invalid = || Error::new(base_codes::VALUE_INVALID).with_detail("field", "mode");
    let next = match p.mode.as_str() {
        "origin" => {
            let origin = p.origin.ok_or_else(invalid)?;
            match p.x_point {
                Some(toward) => Ucs::from_points(origin, toward)?,
                None => Ucs {
                    origin,
                    x_axis: Ucs::WORLD.x_axis,
                },
            }
        }
        "three_points" => {
            let (origin, x_point, y_point) = match (p.origin, p.x_point, p.y_point) {
                (Some(o), Some(x), Some(y)) => (o, x, y),
                _ => return Err(invalid()),
            };
            let next = Ucs::from_points(origin, x_point)?;
            let side = next.local(y_point)[1];
            if side.abs() <= 1e-9 {
                return Err(Error::new(base_codes::POINTS_COLLINEAR));
            }
            // The system is right-handed: the third point is on its counterclockwise side. A point on
            // the clockwise side would make a mirrored system, which this module does not represent.
            if side < 0.0 {
                return Err(Error::new(codes::UCS_HANDEDNESS));
            }
            next
        }
        "world" => Ucs::WORLD,
        "previous" => read(ctx.snapshot(), PREVIOUS).unwrap_or(Ucs::WORLD),
        _ => return Err(invalid()),
    };
    set_current(ctx.txn()?, before, next)?;
    Ok(report(None, next))
}

/// Parameters of a command that names a saved system.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedParams {
    /// The name of the system.
    #[schemars(length(min = 1, max = 255))]
    pub name: String,
}

fn save(ctx: &mut Context<'_>, p: NamedParams) -> Result<UcsReport> {
    check_name(&p.name)?;
    let ucs = current(ctx.snapshot());
    write(ctx.txn()?, &p.name, Some(ucs))?;
    Ok(report(Some(p.name), ucs))
}

fn restore(ctx: &mut Context<'_>, p: NamedParams) -> Result<UcsReport> {
    check_name(&p.name)?;
    let next = read(ctx.snapshot(), &p.name)
        .ok_or_else(|| Error::new(codes::UCS_UNKNOWN).with_detail("name", p.name.clone()))?;
    let before = current(ctx.snapshot());
    set_current(ctx.txn()?, before, next)?;
    Ok(report(Some(p.name), next))
}

fn delete(ctx: &mut Context<'_>, p: NamedParams) -> Result<UcsReport> {
    check_name(&p.name)?;
    let ucs = read(ctx.snapshot(), &p.name)
        .ok_or_else(|| Error::new(codes::UCS_UNKNOWN).with_detail("name", p.name.clone()))?;
    write(ctx.txn()?, &p.name, None)?;
    Ok(report(Some(p.name), ucs))
}

/// The saved systems of the drawing, by name, in name order.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Saved {
    /// The saved systems.
    pub systems: Vec<UcsReport>,
}

fn list(ctx: &mut Context<'_>, _: NoParams) -> Result<Saved> {
    let mut systems: Vec<UcsReport> = ctx
        .snapshot()
        .foreign_rows()
        .filter(|row| row.source_format == FORMAT && row.record_kind == RECORD_KIND)
        .filter_map(|row| {
            let name = row.source_id.clone()?;
            if name.starts_with('*') {
                return None;
            }
            let ucs: Ucs = serde_json::from_slice(&row.data).ok()?;
            Some(report(Some(name), ucs))
        })
        .collect();
    systems.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Saved { systems })
}

/// No parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// The prompt form of the definition: the mode, then the points the mode takes.
fn ask_mode(state: &PromptState<'_>) -> Result<Step> {
    let answers: Vec<&PromptInput> = state.answers.iter().map(|a| &a.input).collect();
    let Some(PromptInput::Keyword(mode)) = answers.first() else {
        return Ok(Step::Ask(Prompt::new(
            "mode",
            MODE_PROMPT.message_id,
            Accept::keywords([
                KeywordOption::new("origin", OPTION_ORIGIN.message_id),
                KeywordOption::new("three", OPTION_THREE_POINTS.message_id),
                KeywordOption::new("world", OPTION_WORLD.message_id),
                KeywordOption::new("previous", OPTION_PREVIOUS.message_id),
            ]),
        )));
    };
    let points: Vec<[f64; 2]> = answers
        .iter()
        .skip(1)
        .filter_map(|input| match input {
            PromptInput::Point(p) => {
                let d = drawing_point(state.snapshot, *p);
                Some([d.x, d.y])
            }
            _ => None,
        })
        .collect();
    let (need, value) = match mode.as_str() {
        "origin" => (2, origin_json(&points)),
        "three" => (3, three_json(&points)),
        "world" => return crate::settings::finish(serde_json::json!({"mode": "world"})),
        _ => return crate::settings::finish(serde_json::json!({"mode": "previous"})),
    };
    if points.len() < need {
        let key = match points.len() {
            0 => "origin",
            1 => "x_point",
            _ => "y_point",
        };
        return Ok(Step::Ask(Prompt::new(
            key,
            POINT_PROMPT.message_id,
            Accept::Point {
                dims: arc_command::PointDims::Two,
            },
        )));
    }
    crate::settings::finish(value)
}

fn origin_json(points: &[[f64; 2]]) -> serde_json::Value {
    match points {
        [origin, toward, ..] => {
            serde_json::json!({"mode": "origin", "origin": origin, "x_point": toward})
        }
        [origin] => serde_json::json!({"mode": "origin", "origin": origin}),
        [] => serde_json::json!({"mode": "origin"}),
    }
}

fn three_json(points: &[[f64; 2]]) -> serde_json::Value {
    match points {
        [origin, x, y] => {
            serde_json::json!({"mode": "three_points", "origin": origin, "x_point": x, "y_point": y})
        }
        _ => serde_json::json!({"mode": "three_points"}),
    }
}

fn ask_name(state: &PromptState<'_>) -> Result<Step> {
    match crate::settings::texts(state).first() {
        Some(name) => crate::settings::finish(serde_json::json!({"name": name})),
        None => Ok(Step::Ask(Prompt::new(
            "name",
            NAME_PROMPT.message_id,
            Accept::text(),
        ))),
    }
}

fn no_prompt(_: &PromptState<'_>) -> Result<Step> {
    crate::settings::finish(serde_json::json!({}))
}

/// Registers the coordinate system commands.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs = vec![
        crate::settings::spec(
            "draftbuilder.precision.ucs",
            Message {
                message_id: "db-cmd-ucs-label",
            },
            Message {
                message_id: "db-cmd-ucs-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_mode,
            define,
        ),
        crate::settings::spec(
            "draftbuilder.precision.ucs_save",
            Message {
                message_id: "db-cmd-ucs-save-label",
            },
            Message {
                message_id: "db-cmd-ucs-save-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_name,
            save,
        ),
        crate::settings::spec(
            "draftbuilder.precision.ucs_restore",
            Message {
                message_id: "db-cmd-ucs-restore-label",
            },
            Message {
                message_id: "db-cmd-ucs-restore-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_name,
            restore,
        ),
        crate::settings::spec(
            "draftbuilder.precision.ucs_delete",
            Message {
                message_id: "db-cmd-ucs-delete-label",
            },
            Message {
                message_id: "db-cmd-ucs-delete-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_name,
            delete,
        ),
        crate::settings::spec(
            "draftbuilder.precision.ucs_list",
            Message {
                message_id: "db-cmd-ucs-list-label",
            },
            Message {
                message_id: "db-cmd-ucs-list-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            no_prompt,
            list,
        ),
    ];
    for spec in specs {
        registry.register(spec)?;
    }
    Ok(())
}

/// The prompt that asks the mode of a definition.
const MODE_PROMPT: Message = Message {
    message_id: "db-cmd-ucs-mode-prompt",
};
/// The prompt that asks a point of a definition.
const POINT_PROMPT: Message = Message {
    message_id: "db-cmd-ucs-point-prompt",
};
/// The prompt that asks the name of a saved system.
const NAME_PROMPT: Message = Message {
    message_id: "db-cmd-ucs-name-prompt",
};
/// The keyword option of the origin mode.
const OPTION_ORIGIN: Message = Message {
    message_id: "db-cmd-option-origin",
};
/// The keyword option of the three-point mode.
const OPTION_THREE_POINTS: Message = Message {
    message_id: "db-cmd-option-three-points",
};
/// The keyword option of the world mode.
const OPTION_WORLD: Message = Message {
    message_id: "db-cmd-option-world",
};
/// The keyword option of the previous mode.
const OPTION_PREVIOUS: Message = Message {
    message_id: "db-cmd-option-previous",
};
