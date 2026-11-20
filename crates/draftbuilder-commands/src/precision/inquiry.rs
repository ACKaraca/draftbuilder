//! Inquiry commands (DB-SNAP-031, DB-SNAP-032 and the measures that reduce to `arc-geom`):
//! `DIST` between two points, `ID` of a point, `AREA` of a point sequence or of closed objects with
//! running totals, `LENGTH` of curves and `LIST` of the properties of objects. Every length and
//! area is formatted in the drawing's display settings (its length unit and length precision); an
//! angle in decimal degrees with the angle precision. Coordinates are reported in the current user
//! coordinate system, and the angle base and direction of the drawing apply to angles.

use std::f64::consts::TAU;

use arc_base::Limits;
use arc_base::math::{Point2, Vector2};
use arc_base::{Dimension, DisplayFormat, Error, Quantity, Result, Tolerance, Unit};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Permission, Prompt, PromptInput,
    PromptState, Registry, Step, UndoBehavior,
};
use arc_doc::{Snapshot, Value};
use arc_draft::types::{By, ENTITIES, entity, enum_at};
use arc_draft_dxf::fields::{Reader, name_of};
use arc_geom::{Curve2, CurveEval, GeomCodec, Loop2, Segment2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::codes;
use crate::error::Message;
use crate::select::candidates::{default_layer, reference};
use crate::settings::store::Values;
use crate::settings::{self};

/// The unit lengths are displayed in: the drawing's length unit, the meter for a unitless drawing.
fn length_unit(snapshot: &Snapshot) -> Result<&'static Unit> {
    match settings::space_unit(snapshot).unit() {
        Some(unit) => Ok(unit),
        None => Unit::by_name("meter"),
    }
}

/// The precision of a display setting, as the number of digits.
fn precision(values: &Values, name: &str) -> u8 {
    u8::try_from(values.int(name)).unwrap_or(4)
}

/// A length in drawing units, formatted in the drawing's length unit and precision.
///
/// # Errors
///
/// The errors of [`DisplayFormat::format`].
pub fn length_text(snapshot: &Snapshot, values: &Values, units: f64) -> Result<String> {
    let format = DisplayFormat {
        precision: precision(values, settings::LENGTH_PRECISION),
        ..DisplayFormat::new(*length_unit(snapshot)?)
    };
    let meters = units * crate::draw::create::meters_per_unit(snapshot);
    format.format(Quantity::new(meters, Dimension::LENGTH), '.')
}

/// An angle in radians, formatted in decimal degrees with the angle precision.
///
/// # Errors
///
/// The errors of [`DisplayFormat::format`].
pub fn angle_text(values: &Values, radians: f64) -> Result<String> {
    let degree = Unit::by_name("degree")?;
    let format = DisplayFormat {
        precision: precision(values, settings::ANGLE_PRECISION),
        ..DisplayFormat::new(*degree)
    };
    format.format(Quantity::new(radians, Dimension::ANGLE), '.')
}

/// An area in squared drawing units, formatted with the length precision and the squared unit
/// symbol of the drawing.
pub fn area_text(snapshot: &Snapshot, values: &Values, square_units: f64) -> Result<String> {
    let digits = usize::from(precision(values, settings::LENGTH_PRECISION));
    let mut text = format!("{square_units:.digits$}");
    if text.contains('.') {
        text.truncate(text.trim_end_matches('0').trim_end_matches('.').len());
    }
    Ok(format!("{text} {}²", length_unit(snapshot)?.symbol))
}

/// The display angle of a direction `raw` (counterclockwise from the X axis of the coordinate
/// system), with the angle base `base` and the direction: clockwise when `clockwise`.
#[must_use]
pub fn display_angle(raw: f64, base: f64, clockwise: bool) -> f64 {
    let turned = if clockwise { base - raw } else { raw - base };
    turned.rem_euclid(TAU)
}

/// The angle base and direction of the drawing: the base in radians and whether angles run
/// clockwise.
fn angle_settings(values: &Values) -> (f64, bool) {
    (
        values.real(settings::ANGLE_BASE),
        values.int(settings::ANGLE_DIRECTION) == 1,
    )
}

/// A distance between two points.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Distance {
    /// The distance, in drawing units.
    pub distance: f64,
    /// The change in X and in Y of the current coordinate system, in drawing units.
    pub delta: [f64; 2],
    /// The angle of the line in the plane, in radians, with the angle base and direction.
    pub angle: f64,
    /// The values in the display format of the drawing.
    pub texts: DistanceTexts,
}

/// The display forms of a distance.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DistanceTexts {
    /// The distance.
    pub distance: String,
    /// The change in X.
    pub delta_x: String,
    /// The change in Y.
    pub delta_y: String,
    /// The angle.
    pub angle: String,
}

/// The distance between two points (DB-SNAP-031), reported in the current coordinate system.
///
/// # Errors
///
/// The errors of the display formats.
pub fn distance(snapshot: &Snapshot, from: [f64; 2], to: [f64; 2]) -> Result<Distance> {
    let values = Values::of(snapshot);
    let ucs = crate::precision::ucs::current(snapshot);
    let local_from = ucs.local(from);
    let local_to = ucs.local(to);
    let delta = [local_to[0] - local_from[0], local_to[1] - local_from[1]];
    let distance = delta[0].hypot(delta[1]);
    let (base, clockwise) = angle_settings(&values);
    let angle = display_angle(delta[1].atan2(delta[0]), base, clockwise);
    Ok(Distance {
        distance,
        delta,
        angle,
        texts: DistanceTexts {
            distance: length_text(snapshot, &values, distance)?,
            delta_x: length_text(snapshot, &values, delta[0])?,
            delta_y: length_text(snapshot, &values, delta[1])?,
            angle: angle_text(&values, angle)?,
        },
    })
}

/// The coordinates of a point (DB-SNAP-031).
#[derive(Debug, Serialize, JsonSchema)]
pub struct Identity {
    /// The point in the current coordinate system, in drawing units.
    pub point: [f64; 2],
    /// The X coordinate, as text.
    pub x: String,
    /// The Y coordinate, as text.
    pub y: String,
}

/// The coordinates of `point` in the current coordinate system.
///
/// # Errors
///
/// The errors of the display formats.
pub fn identity(snapshot: &Snapshot, point: [f64; 2]) -> Result<Identity> {
    let values = Values::of(snapshot);
    let local = crate::precision::ucs::current(snapshot).local(point);
    Ok(Identity {
        point: local,
        x: length_text(snapshot, &values, local[0])?,
        y: length_text(snapshot, &values, local[1])?,
    })
}

/// The area of a closed figure of the drawing, from its boundary curves.
fn measure(curves: Vec<Curve2>) -> Result<(f64, f64)> {
    let tolerance = Tolerance::DEFAULT;
    let measured = Loop2::new(curves, &tolerance)?.measure(&tolerance)?;
    Ok((measured.area, measured.perimeter))
}

/// The curve of an entity, from its geometry field.
fn curve_of(snapshot: &Snapshot, id: arc_base::ObjectId) -> Result<Option<Curve2>> {
    let handle = snapshot.resolve(id)?;
    let key = snapshot
        .type_key(handle)
        .map(|k| k.as_str().to_owned())
        .unwrap_or_default();
    let object = snapshot.get(handle)?;
    match Reader::new(snapshot.registry(), &key, &object.values).get("geometry") {
        Some(Value::Geometry(bytes)) => Curve2::decode(&bytes.0, &Limits::DEFAULT).map(Some),
        _ => Ok(None),
    }
}

/// The entity `id`, failing for an object that is not an entity.
fn entity_of(snapshot: &Snapshot, id: IdParam) -> Result<arc_base::ObjectId> {
    let handle = snapshot.resolve(id.0)?;
    match snapshot.type_key(handle) {
        Some(key) if ENTITIES.contains(&key.as_str()) => Ok(id.0),
        _ => Err(Error::new(arc_draft::error::codes::OBJECT_WRONG_TYPE)
            .with_detail("id", id.0.to_string())),
    }
}

/// The most points or objects an inquiry takes in one call.
pub const MAX_LIST: usize = 10_000;

/// Refuses a list longer than [`MAX_LIST`].
fn check_count(count: usize) -> Result<()> {
    if count > MAX_LIST {
        return Err(Error::new(crate::error::codes::LIMIT_EXCEEDED)
            .with_detail("limit", MAX_LIST.to_string()));
    }
    Ok(())
}

/// The area and perimeter of a point sequence, closed by its last segment.
///
/// A self-intersecting sequence is measured as its boundary runs: the area of the loops it encloses
/// with their orientation, not refused.
fn polygon_measure(points: &[[f64; 2]]) -> Result<(f64, f64)> {
    if points.len() < 3 {
        return Err(Error::new(codes::POINTS_TOO_FEW));
    }
    check_count(points.len())?;
    let curves = (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            Segment2::new(Point2::new(a[0], a[1]), Point2::new(b[0], b[1])).map(Curve2::Segment)
        })
        .collect::<Result<Vec<Curve2>>>()?;
    measure(curves)
}

/// The area and perimeter of a closed object.
fn object_measure(snapshot: &Snapshot, id: IdParam) -> Result<(f64, f64)> {
    let id = entity_of(snapshot, id)?;
    let curve = curve_of(snapshot, id)?.ok_or_else(|| Error::new(codes::NO_CURVE))?;
    if !curve.is_closed() {
        return Err(Error::new(codes::NOT_CLOSED).with_detail("id", id.to_string()));
    }
    measure(vec![curve])
}

/// How the area of a figure joins the running total.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Running {
    /// The area alone; the running total is unchanged.
    Single,
    /// The area is added to the running total.
    Add,
    /// The area is subtracted from the running total.
    Subtract,
}

/// Parameters of `draftbuilder.inquiry.area` (`AREA`): points, or closed objects, or both.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AreaParams {
    /// The points of a closed polygon, in drawing units, at least three.
    #[serde(default)]
    pub points: Option<Vec<[f64; 2]>>,
    /// Closed objects whose areas are measured one by one.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// How the area joins the running total; `single` when absent.
    #[serde(default)]
    pub running: Option<Running>,
    /// The running total before this figure, in squared drawing units; zero when absent.
    #[serde(default)]
    pub total: Option<f64>,
}

/// The area inquiry result.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AreaResult {
    /// The area, in squared drawing units.
    pub area: f64,
    /// The perimeter, in drawing units.
    pub perimeter: f64,
    /// The running total after this figure, in squared drawing units.
    pub total: f64,
    /// The display forms: area, perimeter and total.
    pub texts: AreaTexts,
}

/// The display forms of an area result.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AreaTexts {
    /// The area.
    pub area: String,
    /// The perimeter.
    pub perimeter: String,
    /// The running total.
    pub total: String,
}

/// The area and perimeter of the figures of `params`, joined to the running total.
fn area_of(ctx: &Context<'_>, params: AreaParams) -> Result<AreaResult> {
    check_count(params.objects.as_ref().map_or(0, Vec::len))?;
    let snapshot = ctx.snapshot();
    let mut measured = Vec::new();
    if let Some(points) = &params.points {
        measured.push(polygon_measure(points)?);
    }
    for id in params.objects.iter().flatten() {
        measured.push(object_measure(snapshot, *id)?);
    }
    if measured.is_empty() {
        return Err(Error::new(codes::POINTS_TOO_FEW));
    }
    let area: f64 = measured.iter().map(|(a, _)| a).sum();
    let perimeter: f64 = measured.iter().map(|(_, p)| p).sum();
    let base = params.total.unwrap_or(0.0);
    let total = match params.running.unwrap_or(Running::Single) {
        Running::Single => area,
        Running::Add => base + area,
        Running::Subtract => base - area,
    };
    let values = Values::of(snapshot);
    let length = |v: f64| length_text(snapshot, &values, v);
    Ok(AreaResult {
        area,
        perimeter,
        total,
        texts: AreaTexts {
            area: area_text(snapshot, &values, area)?,
            perimeter: length(perimeter)?,
            total: area_text(snapshot, &values, total)?,
        },
    })
}

fn area(ctx: &mut Context<'_>, params: AreaParams) -> Result<AreaResult> {
    area_of(ctx, params)
}

/// Parameters of `draftbuilder.inquiry.distance` (`DIST`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DistanceParams {
    /// The first point, in drawing units.
    pub from: [f64; 2],
    /// The second point, in drawing units.
    pub to: [f64; 2],
}

fn dist(ctx: &mut Context<'_>, p: DistanceParams) -> Result<Distance> {
    distance(ctx.snapshot(), p.from, p.to)
}

/// Parameters of `draftbuilder.inquiry.id` (`ID`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdParams {
    /// The point, in drawing units.
    pub point: [f64; 2],
}

fn id(ctx: &mut Context<'_>, p: IdParams) -> Result<Identity> {
    identity(ctx.snapshot(), p.point)
}

/// Parameters of `draftbuilder.inquiry.length`: the objects whose curve lengths are reported.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObjectsParams {
    /// The objects.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

/// The length of one object.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Length {
    /// The object.
    pub id: IdParam,
    /// Its length in drawing units, as text.
    pub text: String,
    /// Its length in drawing units.
    pub length: f64,
}

/// The lengths of the objects and their total.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Lengths {
    /// The objects with a curve, in order.
    pub items: Vec<Length>,
    /// The total length, in drawing units.
    pub total: f64,
    /// The total as text.
    pub total_text: String,
}

fn length(ctx: &mut Context<'_>, p: ObjectsParams) -> Result<Lengths> {
    check_count(p.objects.len())?;
    let snapshot = ctx.snapshot();
    let values = Values::of(snapshot);
    let tolerance = Tolerance::DEFAULT;
    let mut items = Vec::new();
    for id in p.objects {
        let object = entity_of(snapshot, id)?;
        let curve = curve_of(snapshot, object)?.ok_or_else(|| Error::new(codes::NO_CURVE))?;
        let value = curve.length(&tolerance)?;
        items.push(Length {
            id,
            text: length_text(snapshot, &values, value)?,
            length: value,
        });
    }
    let total: f64 = items.iter().map(|i| i.length).sum();
    Ok(Lengths {
        total_text: length_text(snapshot, &values, total)?,
        items,
        total,
    })
}

/// The properties of one object.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Properties {
    /// The object.
    pub id: IdParam,
    /// Its type key.
    pub kind: String,
    /// The name of its layer.
    pub layer: Option<String>,
    /// The source of its color: `layer`, `block` or `object`.
    pub color: Option<String>,
    /// The source of its linetype.
    pub linetype: Option<String>,
    /// The source of its lineweight.
    pub lineweight: Option<String>,
    /// Whether its curve is closed, when it has one.
    pub closed: Option<bool>,
    /// The length of its curve in drawing units, when it has one.
    pub length: Option<f64>,
}

fn by_text(values: &[Option<Value>], index: usize) -> Option<String> {
    let mode = By::of(enum_at(values, index));
    Some(
        match mode {
            By::Layer => "layer",
            By::Block => "block",
            By::Object => "object",
        }
        .to_owned(),
    )
}

/// The properties of the entity `id`.
fn properties(snapshot: &Snapshot, id: IdParam) -> Result<Properties> {
    let object = entity_of(snapshot, id)?;
    let handle = snapshot.resolve(object)?;
    let key = snapshot
        .type_key(handle)
        .map(|k| k.as_str().to_owned())
        .unwrap_or_default();
    let stored = snapshot.get(handle)?;
    let layer = reference(snapshot, object, entity::LAYER)
        .or_else(|| default_layer(snapshot))
        .and_then(|l| name_of(snapshot, Some(l)));
    let curve = curve_of(snapshot, object)?;
    let tolerance = Tolerance::DEFAULT;
    Ok(Properties {
        id,
        kind: key,
        layer,
        color: by_text(&stored.values, entity::COLOR_BY),
        linetype: by_text(&stored.values, entity::LINETYPE_BY),
        lineweight: by_text(&stored.values, entity::LINEWEIGHT_BY),
        closed: curve.as_ref().map(Curve2::is_closed),
        length: match &curve {
            Some(c) => Some(c.length(&tolerance)?),
            None => None,
        },
    })
}

/// Parameters of `draftbuilder.inquiry.list` (`LIST`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// The objects whose properties are listed.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
}

/// The properties of the objects, in order.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Listing {
    /// One entry per object.
    pub items: Vec<Properties>,
}

fn list(ctx: &mut Context<'_>, p: ListParams) -> Result<Listing> {
    let snapshot = ctx.snapshot();
    let items = p
        .objects
        .into_iter()
        .map(|id| properties(snapshot, id))
        .collect::<Result<Vec<Properties>>>()?;
    Ok(Listing { items })
}

/// The prompt forms: points for `DIST` and `ID`, a point list for `AREA`, a selection for the rest.
fn ask_points(state: &PromptState<'_>, count: usize) -> Result<Step> {
    let points: Vec<[f64; 2]> = state
        .answers
        .iter()
        .filter_map(|a| match a.input {
            PromptInput::Point(p) => {
                let d = crate::draw::create::drawing_point(state.snapshot, p);
                Some([d.x, d.y])
            }
            _ => None,
        })
        .collect();
    if points.len() >= count {
        let value = match points.as_slice() {
            [from, to, ..] if count == 2 => serde_json::json!({"from": from, "to": to}),
            [point, ..] => serde_json::json!({"point": point}),
            [] => serde_json::json!({}),
        };
        return settings::finish(value);
    }
    let key = match (count, points.len()) {
        (2, 0) => "from",
        (2, _) => "to",
        _ => "point",
    };
    Ok(Step::Ask(Prompt::new(
        key,
        POINT_PROMPT.message_id,
        Accept::Point {
            dims: arc_command::PointDims::Two,
        },
    )))
}

fn ask_dist(state: &PromptState<'_>) -> Result<Step> {
    ask_points(state, 2)
}

fn ask_id(state: &PromptState<'_>) -> Result<Step> {
    ask_points(state, 1)
}

fn ask_area(state: &PromptState<'_>) -> Result<Step> {
    let mut points: Vec<serde_json::Value> = Vec::new();
    let mut finished = false;
    for answer in state.answers.iter() {
        match &answer.input {
            PromptInput::Point(p) => {
                let d = crate::draw::create::drawing_point(state.snapshot, *p);
                points.push(serde_json::json!([d.x, d.y]));
            }
            PromptInput::Keyword(k) if k.as_str() == "end" => finished = true,
            _ => {}
        }
    }
    if finished && points.len() >= 3 {
        return settings::finish(serde_json::json!({"points": points}));
    }
    let end = KeywordOption::new("end", crate::draw::create::OPTION_END.message_id);
    Ok(Step::Ask(
        Prompt::new(
            "point",
            AREA_PROMPT.message_id,
            Accept::Point {
                dims: arc_command::PointDims::Two,
            },
        )
        .or(Accept::keywords([end]))
        .with_default(PromptInput::Keyword("end".to_owned())),
    ))
}

fn ask_objects(state: &PromptState<'_>) -> Result<Step> {
    let selected: Vec<arc_base::ObjectId> = state
        .answers
        .iter()
        .flat_map(|answer| match &answer.input {
            PromptInput::Selection(ids) => ids.clone(),
            PromptInput::Pick(id) => vec![*id],
            _ => Vec::new(),
        })
        .collect();
    if selected.is_empty() {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS_PROMPT.message_id,
            Accept::Selection {
                min: 1,
                max: None,
                types: Vec::new(),
            },
        )));
    }
    let ids: Vec<String> = selected.iter().map(ToString::to_string).collect();
    settings::finish(serde_json::json!({"objects": ids}))
}

/// The commands of the inquiry family.
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs: Vec<CommandSpec> = vec![
        settings::spec(
            "draftbuilder.inquiry.distance",
            Message {
                message_id: "db-cmd-inquiry-distance-label",
            },
            Message {
                message_id: "db-cmd-inquiry-distance-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_dist,
            dist,
        ),
        settings::spec(
            "draftbuilder.inquiry.id",
            Message {
                message_id: "db-cmd-inquiry-id-label",
            },
            Message {
                message_id: "db-cmd-inquiry-id-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_id,
            id,
        ),
        settings::spec(
            "draftbuilder.inquiry.area",
            Message {
                message_id: "db-cmd-inquiry-area-label",
            },
            Message {
                message_id: "db-cmd-inquiry-area-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_area,
            area,
        ),
        settings::spec(
            "draftbuilder.inquiry.length",
            Message {
                message_id: "db-cmd-inquiry-length-label",
            },
            Message {
                message_id: "db-cmd-inquiry-length-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_objects,
            length,
        ),
        settings::spec(
            "draftbuilder.inquiry.mass",
            Message {
                message_id: "db-cmd-inquiry-mass-label",
            },
            Message {
                message_id: "db-cmd-inquiry-mass-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_object,
            mass,
        ),
        settings::spec(
            "draftbuilder.inquiry.list",
            Message {
                message_id: "db-cmd-inquiry-list-label",
            },
            Message {
                message_id: "db-cmd-inquiry-list-description",
            },
            Permission::Read,
            UndoBehavior::NotUndoable,
            ask_objects,
            list,
        ),
    ];
    // Inquiry commands read the drawing and may run beside a waiting prompt (DB-CMD-012).
    for spec in specs {
        registry.register(spec.transparent())?;
    }
    Ok(())
}

/// The prompt that asks a point.
const POINT_PROMPT: Message = Message {
    message_id: "db-cmd-inquiry-point-prompt",
};
/// The prompt that asks the next point of an area.
const AREA_PROMPT: Message = Message {
    message_id: "db-cmd-inquiry-area-prompt",
};
/// The prompt that asks for objects.
const OBJECTS_PROMPT: Message = Message {
    message_id: "db-cmd-inquiry-objects-prompt",
};

/// Parameters of `draftbuilder.inquiry.mass` (`MASSPROP`): one closed object.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MassParams {
    /// The closed object whose mass properties are reported.
    pub object: IdParam,
}

/// The mass properties of a closed object, in the current coordinate system.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MassProperties {
    /// The area, in squared drawing units.
    pub area: f64,
    /// The perimeter, in drawing units.
    pub perimeter: f64,
    /// The centroid of the area, in the current coordinate system, in drawing units.
    pub centroid: [f64; 2],
    /// The second moment of the area about the centroidal axis parallel to the current X axis:
    /// the integral of the squared distance from that axis, `∫ y² dA` in the centroid's frame, in
    /// drawing units to the fourth power.
    pub second_moment_about_x: f64,
    /// The second moment of the area about the centroidal axis parallel to the current Y axis:
    /// `∫ x² dA` in the centroid's frame, in drawing units to the fourth power.
    pub second_moment_about_y: f64,
    /// The product of the area about the centroidal axes: `∫ x·y dA` in the centroid's frame, in
    /// drawing units to the fourth power.
    pub product_of_area: f64,
    /// The display forms of the area, the perimeter and the centroid.
    pub texts: MassTexts,
}

/// The display forms of mass properties.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MassTexts {
    /// The area.
    pub area: String,
    /// The perimeter.
    pub perimeter: String,
    /// The centroid X.
    pub centroid_x: String,
    /// The centroid Y.
    pub centroid_y: String,
}

fn mass(ctx: &mut Context<'_>, p: MassParams) -> Result<MassProperties> {
    let snapshot = ctx.snapshot();
    let values = Values::of(snapshot);
    let system = crate::precision::ucs::current(snapshot);
    let id = entity_of(snapshot, p.object)?;
    let curve = curve_of(snapshot, id)?.ok_or_else(|| Error::new(codes::NO_CURVE))?;
    if !curve.is_closed() {
        return Err(Error::new(codes::NOT_CLOSED).with_detail("id", id.to_string()));
    }
    let tolerance = Tolerance::DEFAULT;
    let measured = Loop2::new(vec![curve], &tolerance)?.measure(&tolerance)?;
    let centroid = system.local([measured.centroid.x, measured.centroid.y]);
    let moments = measured.about_axes(
        measured.centroid,
        Vector2::new(system.x_axis[0], system.x_axis[1]),
    )?;
    Ok(MassProperties {
        area: measured.area,
        perimeter: measured.perimeter,
        centroid,
        // The foundation names its moments by the integrand over the x and y of its own frame:
        // `xx` is ∫x² dA, which is the second moment about the y axis; `yy` is ∫y² dA, about x.
        second_moment_about_x: moments.yy,
        second_moment_about_y: moments.xx,
        product_of_area: moments.xy,
        texts: MassTexts {
            area: area_text(snapshot, &values, measured.area)?,
            perimeter: length_text(snapshot, &values, measured.perimeter)?,
            centroid_x: length_text(snapshot, &values, centroid[0])?,
            centroid_y: length_text(snapshot, &values, centroid[1])?,
        },
    })
}

/// The prompt form of `MASSPROP`: one object, answered by a pick or a selection of one.
fn ask_object(state: &PromptState<'_>) -> Result<Step> {
    let first = state.answers.iter().find_map(|answer| match &answer.input {
        PromptInput::Selection(ids) => ids.first().copied(),
        PromptInput::Pick(id) => Some(*id),
        _ => None,
    });
    match first {
        Some(id) => settings::finish(serde_json::json!({"object": id.to_string()})),
        None => Ok(Step::Ask(Prompt::new(
            "object",
            OBJECTS_PROMPT.message_id,
            Accept::Selection {
                min: 1,
                max: Some(1),
                types: Vec::new(),
            },
        ))),
    }
}
