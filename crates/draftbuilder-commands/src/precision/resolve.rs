//! Resolving a pick (DB-SNAP-007, DB-SNAP-008): the point a pick gives under the running object
//! snap set, the override of one pick, tracking, ortho, polar and the snap step. [`resolve`] is a
//! pure function of the snapshot, the settings and the pick; the interactive cursor calls it for
//! every point and the protocol calls it through `draftbuilder.precision.resolve`.
//!
//! Precedence, as the foundation's constraint rule: an object snap, then a tracking point, then
//! ortho, and only when ortho is off the polar alignment, then the snap step, else the cursor.
//! A typed coordinate bypasses this function; the setting `snap.override.typed_first` tells the
//! host whether a typed coordinate also wins over a running snap at the same place.

use arc_base::math::{Point2, Vector2};
use arc_base::{ObjectId, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, Permission, Point as PromptPoint, PointDims, Prompt, PromptInput,
    PromptState, Registry, Step, UndoBehavior,
};
use arc_doc::Snapshot;
use arc_draft::snap::{
    ConstraintContext, ConstraintSource, Frame, PolarSettings, SnapKind, SnapModes, SnapQuery,
    SnapStep, constrain, snap_candidates,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::snaps::{self, Override, SNAP_CODES};
use crate::draw::create::drawing_point;
use crate::error::Message;
use crate::error::codes as base_codes;
use crate::settings::store::Values;
use crate::settings::{self, Value};

/// The setting names the resolution reads.
const OBJECT_ON: &str = "snap.object.on";
const RUNNING: &str = "snap.object.running";
const ORTHO: &str = "draft.ortho";
const POLAR_ON: &str = "draft.polar.on";
const POLAR_INCREMENT: &str = "draft.polar.increment";
const POLAR_ADDITIONAL: &str = "draft.polar.additional";
const SNAP_ON: &str = "draft.snap.on";
const SNAP_SPACING: &str = "draft.snap.spacing";
const SNAP_BASE: &str = "draft.snap.base";
const SNAP_ANGLE: &str = "draft.snap.angle";
const GEO_SETTING: &str = "snap.object.geometric_center";
const TYPED_FIRST: &str = "snap.override.typed_first";

/// How close, in degrees, the cursor must be to a polar alignment angle to lock to it.
// shortcut: one fixed angular tolerance for polar locking, no setting for it yet; add a setting
// when the cursor UI needs one.
const POLAR_TOLERANCE_DEGREES: f64 = 0.5;

/// What one resolution takes: the drawing, its model space and the pick.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The drawing.
    pub snapshot: &'a Snapshot,
    /// The model space the object snaps read.
    pub space: ObjectId,
    /// The cursor, in drawing units.
    pub cursor: [f64; 2],
    /// The pick aperture, in drawing units: the largest distance to a snap candidate.
    pub aperture: f64,
    /// The previous point of the running command, when there is one.
    pub from: Option<[f64; 2]>,
    /// A tracking point the host acquired, when there is one.
    pub tracked: Option<[f64; 2]>,
    /// The override of this pick.
    pub over: Override,
}

/// The point a pick resolved to, and what decided it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    /// The point, in drawing units.
    pub point: [f64; 2],
    /// What decided it.
    pub source: ConstraintSource,
    /// The object snap kind, when an object snap decided it.
    pub snap: Option<SnapKind>,
}

/// The set of object snap kinds in effect for one pick: the override's kind alone for one pick,
/// none for `NON`, else the running set while object snap is on.
#[must_use]
pub fn running_set(values: &Values, over: Override) -> SnapModes {
    match over {
        Override::Off => SnapModes::NONE,
        Override::Snap(kind) => SnapModes::of(&[kind]),
        Override::None if values.flag(OBJECT_ON) && !snaps::suppressed(values.int(RUNNING)) => {
            let modes = snaps::modes_of(values.int(RUNNING));
            if values.flag(GEO_SETTING) {
                modes.with(SnapKind::GeometricCenter)
            } else {
                modes
            }
        }
        Override::None => SnapModes::NONE,
    }
}

/// The object snap under the cursor, the best ranked candidate of the running set.
fn object_snap(
    request: &Request<'_>,
    modes: SnapModes,
    ucs_x: Vector2,
) -> Result<Option<(SnapKind, Point2)>> {
    if modes == SnapModes::NONE || request.aperture <= 0.0 {
        return Ok(None);
    }
    let mut query = SnapQuery::new(
        Point2::new(request.cursor[0], request.cursor[1]),
        request.aperture,
        modes,
        Tolerance::DEFAULT,
    );
    query.from = request.from.map(|p| Point2::new(p[0], p[1]));
    query.ucs_x = ucs_x;
    let candidates = snap_candidates(request.snapshot, request.space, &query)?;
    Ok(candidates.first().map(|c| (c.kind, c.point)))
}

/// The snap step in effect: the rectangular grid of the snap setting, when snap is on and its
/// spacing is positive. A one-pick `NON` suppresses object snaps only; the step still applies.
fn snap_step(values: &Values) -> Option<SnapStep> {
    let spacing = values.point(SNAP_SPACING);
    if !values.flag(SNAP_ON) || spacing[0] <= 0.0 || spacing[1] <= 0.0 {
        return None;
    }
    let base = values.point(SNAP_BASE);
    Some(SnapStep::Rectangular {
        base: Point2::new(base[0], base[1]),
        spacing,
        rotation: values.real(SNAP_ANGLE),
    })
}

/// The polar alignment in effect, when polar tracking is on.
fn polar(values: &Values) -> Result<Option<PolarSettings>> {
    if !values.flag(POLAR_ON) {
        return Ok(None);
    }
    let additional = values
        .list(POLAR_ADDITIONAL)
        .into_iter()
        .map(f64::to_radians)
        .collect();
    PolarSettings::new(
        values.real(POLAR_INCREMENT).to_radians(),
        additional,
        POLAR_TOLERANCE_DEGREES.to_radians(),
    )
    .map(Some)
}

/// Resolves a pick under the settings `values`.
///
/// # Errors
///
/// The errors of the object snap query and of the constraints (`geom.input.not_finite`,
/// `draft.snap.limit_exceeded`, `draft.geometry.invalid`).
pub fn resolve(request: &Request<'_>, values: &Values) -> Result<Pick> {
    let ucs = crate::precision::ucs::current(request.snapshot);
    let ucs_x = Vector2::new(ucs.x_axis[0], ucs.x_axis[1]);
    let modes = running_set(values, request.over);
    let snapped = object_snap(request, modes, ucs_x)?;
    let polar = polar(values)?;
    let context = ConstraintContext {
        frame: Frame::new(ucs_x)?,
        base: request.from.map(|p| Point2::new(p[0], p[1])),
        snapped: snapped.map(|(_, p)| p),
        tracked: request.tracked.map(|p| Point2::new(p[0], p[1])),
        ortho: values.flag(ORTHO),
        polar: polar.as_ref(),
        step: snap_step(values),
    };
    let constrained = constrain(Point2::new(request.cursor[0], request.cursor[1]), &context)?;
    let snap = match constrained.source {
        ConstraintSource::ObjectSnap => snapped.map(|(kind, _)| kind),
        _ => None,
    };
    Ok(Pick {
        point: [constrained.point.x, constrained.point.y],
        source: constrained.source,
        snap,
    })
}

/// Whether a typed coordinate takes precedence over a running snap at the same place.
#[must_use]
pub fn typed_first(values: &Values) -> bool {
    values.flag(TYPED_FIRST)
}

/// The code of an object snap kind in the table, or of the geometric center.
fn code_of(kind: SnapKind) -> Option<&'static str> {
    if kind == SnapKind::GeometricCenter {
        return Some(snaps::GEOMETRIC_CENTER_CODE);
    }
    SNAP_CODES
        .iter()
        .find(|s| s.kind == Some(kind))
        .map(|s| s.code)
}

/// The text of what decided a pick.
fn source_text(source: ConstraintSource) -> &'static str {
    match source {
        ConstraintSource::Raw => "raw",
        ConstraintSource::ObjectSnap => "object_snap",
        ConstraintSource::Tracking => "tracking",
        ConstraintSource::Ortho => "ortho",
        ConstraintSource::Polar { .. } => "polar",
        ConstraintSource::Step => "step",
    }
}

/// Parameters of `draftbuilder.precision.resolve`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveParams {
    /// The cursor, in drawing units.
    pub point: [f64; 2],
    /// The code of the override of this pick (`END`, `MID`, ... or `NON`), when there is one.
    #[serde(default)]
    pub over: Option<String>,
    /// The previous point of the running command, in drawing units.
    #[serde(default)]
    pub from: Option<[f64; 2]>,
    /// A tracking point, in drawing units.
    #[serde(default)]
    pub tracked: Option<[f64; 2]>,
    /// The pick aperture, in drawing units: the largest distance from the pick to a snap candidate.
    /// Required, finite and positive; the pixel count of the aperture setting is the host's to convert.
    pub aperture: f64,
}

/// The resolved point of a pick.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Resolved {
    /// The point, in drawing units.
    pub point: [f64; 2],
    /// What decided it: `raw`, `object_snap`, `tracking`, `ortho`, `polar` or `step`.
    pub source: String,
    /// The object snap code, when an object snap decided it.
    pub snap: Option<String>,
}

fn resolve_command(ctx: &mut Context<'_>, p: ResolveParams) -> Result<Resolved> {
    let over = match p.over.as_deref() {
        None => Override::None,
        Some(code) => snaps::override_of(code).ok_or_else(|| {
            arc_base::Error::new(super::codes::SNAP_UNKNOWN).with_detail("code", code)
        })?,
    };
    let space = crate::model_space(ctx.snapshot())
        .ok_or_else(|| arc_base::Error::new(base_codes::NO_MODEL_SPACE))?;
    if !(p.aperture.is_finite() && p.aperture > 0.0) {
        return Err(
            arc_base::Error::new(base_codes::VALUE_INVALID).with_detail("field", "aperture")
        );
    }
    let values = Values::of(ctx.snapshot());
    let pick = resolve(
        &Request {
            snapshot: ctx.snapshot(),
            space,
            cursor: p.point,
            aperture: p.aperture,
            from: p.from,
            tracked: p.tracked,
            over,
        },
        &values,
    )?;
    Ok(Resolved {
        point: pick.point,
        source: source_text(pick.source).to_owned(),
        snap: pick.snap.and_then(code_of).map(str::to_owned),
    })
}

/// Parameters of `draftbuilder.precision.osnap` (`OSNAP`): the running set, as codes.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OsnapParams {
    /// `NONE`, `ALL`, or object snap codes separated by commas, such as `END,MID,INT`.
    #[schemars(length(min = 1, max = 256))]
    pub modes: String,
}

fn osnap(ctx: &mut Context<'_>, p: OsnapParams) -> Result<settings::Report> {
    let (bits, geometric) = snaps::bits_of_codes(&p.modes)?;
    let running = settings::lookup(RUNNING)?;
    settings::write(ctx, running, Value::Int(bits))?;
    let geo = settings::lookup(GEO_SETTING)?;
    settings::write(ctx, geo, Value::Bool(geometric))?;
    Ok(settings::report(ctx.snapshot(), running))
}

/// Parameters of `draftbuilder.precision.filter`: X from one point and Y from another.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FilterParams {
    /// The point the X coordinate comes from.
    pub x: [f64; 2],
    /// The point the Y coordinate comes from.
    pub y: [f64; 2],
}

/// A point built by a filter.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Filtered {
    /// The point, in drawing units.
    pub point: [f64; 2],
}

fn filter(_: &mut Context<'_>, p: FilterParams) -> Result<Filtered> {
    Ok(Filtered {
        point: snaps::filter_xy(p.x, p.y),
    })
}

/// The prompt form of the resolution: the cursor point.
fn ask_point(state: &PromptState<'_>) -> Result<Step> {
    point_step(state, "point", RESOLVE_PROMPT.message_id)
}

/// The prompt form of the filter: the two points.
fn ask_filter(state: &PromptState<'_>) -> Result<Step> {
    let points: Vec<[f64; 2]> = state
        .answers
        .iter()
        .filter_map(|answer| match answer.input {
            PromptInput::Point(p) => Some(drawing_point_of(state, p)),
            _ => None,
        })
        .collect();
    let (key, message) = if points.is_empty() {
        ("x", FILTER_X_PROMPT.message_id)
    } else {
        ("y", FILTER_Y_PROMPT.message_id)
    };
    match points.as_slice() {
        [x, y] => crate::settings::finish(serde_json::json!({"x": x, "y": y})),
        _ => Ok(Step::Ask(Prompt::new(key, message, point_accept()))),
    }
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

fn drawing_point_of(state: &PromptState<'_>, p: PromptPoint) -> [f64; 2] {
    let point = drawing_point(state.snapshot, p);
    [point.x, point.y]
}

/// A prompt that asks one point and finishes with it as `point`.
fn point_step(state: &PromptState<'_>, key: &str, message: &str) -> Result<Step> {
    let first = state.answers.iter().find_map(|answer| match answer.input {
        PromptInput::Point(p) => Some(drawing_point_of(state, p)),
        _ => None,
    });
    match first {
        Some(point) => crate::settings::finish(serde_json::json!({"point": point})),
        None => Ok(Step::Ask(Prompt::new(key, message, point_accept()))),
    }
}

fn ask_osnap(state: &PromptState<'_>) -> Result<Step> {
    match crate::settings::texts(state).first() {
        Some(modes) => crate::settings::finish(serde_json::json!({"modes": modes})),
        None => Ok(Step::Ask(Prompt::new(
            "modes",
            OSNAP_PROMPT.message_id,
            Accept::text(),
        ))),
    }
}

/// Registers the resolution, the running set and the filter commands.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs: Vec<CommandSpec> = vec![
        crate::settings::spec(
            "draftbuilder.precision.resolve",
            Message {
                message_id: "db-cmd-precision-resolve-label",
            },
            Message {
                message_id: "db-cmd-precision-resolve-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_point,
            resolve_command,
        ),
        crate::settings::spec(
            "draftbuilder.precision.osnap",
            Message {
                message_id: "db-cmd-precision-osnap-label",
            },
            Message {
                message_id: "db-cmd-precision-osnap-description",
            },
            Permission::Edit,
            UndoBehavior::Undoable,
            ask_osnap,
            osnap,
        ),
        crate::settings::spec(
            "draftbuilder.precision.filter",
            Message {
                message_id: "db-cmd-precision-filter-label",
            },
            Message {
                message_id: "db-cmd-precision-filter-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_filter,
            filter,
        ),
    ];
    for spec in specs {
        registry.register(spec)?;
    }
    Ok(())
}

/// The prompt that asks the cursor point.
const RESOLVE_PROMPT: Message = Message {
    message_id: "db-cmd-precision-resolve-prompt",
};
/// The prompt that asks the point the X coordinate comes from.
const FILTER_X_PROMPT: Message = Message {
    message_id: "db-cmd-precision-filter-x-prompt",
};
/// The prompt that asks the point the Y coordinate comes from.
const FILTER_Y_PROMPT: Message = Message {
    message_id: "db-cmd-precision-filter-y-prompt",
};
/// The prompt that asks for the object snap codes.
const OSNAP_PROMPT: Message = Message {
    message_id: "db-cmd-precision-osnap-prompt",
};
