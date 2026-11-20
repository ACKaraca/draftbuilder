//! OFFSET (`draftbuilder.modify.offset`, typed `OFFSET`, alias `O`): parallel copies of entities at a
//! distance. The side is given by a point: the copy lies on the side of that point (to the left of the
//! direction of travel when the point is on the left), or the distance is the distance of the
//! `through` point from each entity. The copies are new entities on the layer and with the appearance of
//! their source, or on the current layer with `layer: current`. With `erase_source` the sources are erased after
//! the copies are made. With `repeat` (multiple mode) each source makes that many offsets, each from the last
//! result at the same distance; not with `through`.
//!
//! Polyline corners are mitered up to [`MITER_LIMIT`] times the distance and beveled beyond it. Copies of
//! an object that the offset splits into several curves are one polyline when they join end to end, and
//! otherwise one entity each. A side point on the entity is refused, since it picks no side. Rays and
//! construction lines offset through their own parallel constructions. The last distance is not remembered
//! between commands.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::Value;
use arc_geom::{Composite2, Curve2, CurveEval, CurveProject, JoinStyle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::transform::Changed;
use super::{
    entities, entity_selection, erase_entities, point_answer, refuse_locked, selection_answer,
};
use crate::draw::create::{Creator, done, from_meters, machine};
use crate::error::{Message, codes};
use crate::settings::remember::{OFFSET_DISTANCE, offered, remembering};

/// The miter limit of a polyline corner, in units of the offset distance: a corner whose miter point lies
/// farther than this from the corner is beveled (its two offset ends are joined by a chord), so a very sharp
/// corner does not throw a long spike.
const MITER_LIMIT: f64 = 4.0;

/// Parameters of `draftbuilder.modify.offset`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OffsetParams {
    /// The entities to offset; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The distance in drawing units, above zero, with `side`.
    #[serde(default)]
    pub distance: Option<f64>,
    /// A point that picks the side of the copies, in drawing units, with `distance`.
    #[serde(default)]
    pub side: Option<[f64; 2]>,
    /// A point through which each copy passes: the distance is the point's distance from the entity.
    #[serde(default)]
    pub through: Option<[f64; 2]>,
    /// Whether the sources are erased after the copies are made.
    #[serde(default)]
    pub erase_source: bool,
    /// The layer of the copies: the source's (the default) or the current layer.
    #[serde(default)]
    pub layer: OffsetLayer,
    /// How many offsets each source makes, each taken from the last result (multiple mode); one by default.
    /// Not with `through`.
    #[serde(default = "one")]
    pub repeat: u32,
}

/// The layer the copies of an offset go to.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OffsetLayer {
    /// The layer of the source.
    #[default]
    Source,
    /// The current layer of the drawing.
    Current,
}

fn one() -> u32 {
    1
}

/// The result of an offset: the copies, in order.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Offset {
    /// The new entities.
    pub objects: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-offset-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-offset-description",
};
const DISTANCE: Message = Message {
    message_id: "db-cmd-modify-offset-distance",
};
const THROUGH: Message = Message {
    message_id: "db-cmd-modify-offset-through",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};
const SIDE: Message = Message {
    message_id: "db-cmd-modify-offset-side",
};
const OPTION_THROUGH: Message = Message {
    message_id: "db-cmd-modify-option-through",
};
const OPTION_LAYER: Message = Message {
    message_id: "db-cmd-modify-option-layer",
};
const OPTION_MULTIPLE: Message = Message {
    message_id: "db-cmd-modify-option-multiple",
};
const LAYER_MODE: Message = Message {
    message_id: "db-cmd-modify-offset-layer",
};
const REPEAT: Message = Message {
    message_id: "db-cmd-modify-offset-repeat",
};
const OPTION_SOURCE: Message = Message {
    message_id: "db-cmd-modify-option-source",
};
const OPTION_CURRENT: Message = Message {
    message_id: "db-cmd-modify-option-current",
};

/// The signed distance of the copy of `curve` that passes the side point `side` at `distance`:
/// positive to the left of the direction of travel.
fn signed_distance(
    curve: &Curve2,
    side: Point2,
    distance: f64,
    tolerance: &Tolerance,
) -> Result<f64> {
    let foot = curve.closest_point(side, tolerance)?;
    let tangent = curve.tangent(foot.parameter);
    let offset = side - foot.point;
    let left = offset.x * -tangent.y + offset.y * tangent.x;
    // A side point on the curve picks no side: it is refused rather than silently choosing one.
    if left.abs() <= tolerance.linear {
        return Err(Error::new(codes::PICK_INVALID)
            .with_detail("reason", "on_curve")
            .with_detail("field", "side"));
    }
    Ok(if left < 0.0 { -distance } else { distance })
}

/// The copies of one entity's curve at the signed distance, as curves.
fn pieces(curve: &Curve2, distance: f64, ctx: &Context<'_>) -> Result<Vec<Curve2>> {
    // Rays and lines are unbounded: their parallel copy is the geometry function of their own kind.
    match curve {
        Curve2::Ray(ray) => return Ok(vec![Curve2::Ray(ray.offset(distance)?)]),
        Curve2::Line(line) => return Ok(vec![Curve2::Line(line.offset(distance)?)]),
        _ => {}
    }
    let composites: Vec<Composite2> = arc_geom::offset(
        curve,
        distance,
        JoinStyle::Miter { limit: MITER_LIMIT },
        &Tolerance::DEFAULT,
        ctx.limits(),
    )?;
    let mut out = Vec::new();
    for composite in composites {
        if composite.curves().len() == 1 {
            out.push(composite.curves()[0].clone());
        } else if let Some(polyline) = composite.to_polyline() {
            out.push(Curve2::Polyline(polyline));
        } else {
            out.extend(composite.curves().iter().cloned());
        }
    }
    Ok(out)
}

fn run(ctx: &mut Context<'_>, p: OffsetParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let snapshot = ctx.snapshot().clone();
    let tolerance = Tolerance::DEFAULT;
    let (side, distance) = match (p.distance, p.side, p.through) {
        (Some(d), Some(side), None) => (Point2::new(side[0], side[1]), d),
        (None, None, Some(through)) => (Point2::new(through[0], through[1]), f64::NAN),
        _ => return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "distance")),
    };
    if p.through.is_none() && !(distance.is_finite() && distance > 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "distance"));
    }
    if p.repeat == 0 || (p.through.is_some() && p.repeat != 1) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "repeat"));
    }
    let limit = usize::try_from(ctx.limits().array_length).unwrap_or(usize::MAX);
    if ids
        .len()
        .checked_mul(usize::try_from(p.repeat).unwrap_or(usize::MAX))
        .is_none_or(|copies| copies > limit)
    {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", "array_length"));
    }
    let current_layer = match p.layer {
        OffsetLayer::Source => None,
        OffsetLayer::Current => Some(
            crate::settings::current_layer(&snapshot)
                .ok_or_else(|| Error::new(codes::PARAMS_CONFLICT).with_detail("field", "layer"))?,
        ),
    };
    if p.erase_source {
        refuse_locked(&snapshot, &ids)?;
    }
    let mut creator = Creator::new(&snapshot)?;
    // Every piece made is counted as it is made: one source can offset into several pieces per offset.
    let mut made: usize = 0;
    for id in &ids {
        let curve = super::required_curve(&snapshot, *id)?;
        let at = if p.through.is_some() {
            curve.closest_point(side, &tolerance)?.distance
        } else {
            distance
        };
        if !(at.is_finite() && at > 0.0) {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "through"));
        }
        let signed = signed_distance(&curve, side, at, &tolerance)?;
        let object = snapshot.get(snapshot.resolve(*id)?)?;
        let owner = object.header.owner;
        // Multiple mode: each offset starts from the pieces of the last one, at the same signed distance.
        let mut last = vec![curve];
        for _ in 0..p.repeat {
            let mut next = Vec::new();
            for source in &last {
                next.extend(pieces(source, signed, ctx)?);
            }
            made = made.saturating_add(next.len());
            if made > limit {
                return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", "array_length"));
            }
            for piece in &next {
                let key = super::key_of_curve(piece);
                let mut values = super::appearance_values(&snapshot, key, &object.values, piece);
                if let Some(layer) = current_layer {
                    set_layer(&mut values, layer);
                }
                creator.add_values(ctx, key, owner, values)?;
            }
            last = next;
        }
    }
    let created = creator.into_ids();
    if p.erase_source {
        erase_entities(ctx, &ids)?;
    }
    ctx.set_selection(created.clone())?;
    Ok(Changed {
        objects: created.into_iter().map(IdParam).collect(),
    })
}

/// Stores the layer reference of an entity's values.
fn set_layer(values: &mut [Option<Value>], layer: ObjectId) {
    if let Some(slot) = values.get_mut(arc_draft::types::entity::LAYER) {
        *slot = Some(Value::Reference(layer));
    }
}

fn keyword_at(state: &PromptState<'_>, key: &str, word: &str) -> bool {
    state
        .answers
        .iter()
        .any(|a| a.key == key && matches!(&a.input, PromptInput::Keyword(k) if k == word))
}

fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    // The keywords of the distance prompt that take a further answer: the layer, then the multiple count.
    if keyword_at(state, "distance", "layer") && state.answer("layer_mode").is_none() {
        return Ok(Step::Ask(
            Prompt::new(
                "layer_mode",
                LAYER_MODE.message_id,
                Accept::keywords([
                    KeywordOption::new("source", OPTION_SOURCE.message_id),
                    KeywordOption::new("current", OPTION_CURRENT.message_id),
                ]),
            )
            .with_default(PromptInput::Keyword("source".to_owned())),
        ));
    }
    if keyword_at(state, "distance", "multiple") && state.answer("repeat").is_none() {
        return Ok(Step::Ask(Prompt::new(
            "repeat",
            REPEAT.message_id,
            super::count_accept(),
        )));
    }
    let through_chosen = keyword_at(state, "distance", "through");
    if through_chosen {
        let Some(through) = point_answer(state, "through") else {
            return Ok(Step::Ask(Prompt::new(
                "through",
                THROUGH.message_id,
                super::point(),
            )));
        };
        return objects_then(state, serde_json::json!({"through": through}));
    }
    let Some(distance) = state
        .answers
        .iter()
        .find_map(|a| match (a.key.as_str(), &a.input) {
            ("distance", PromptInput::Distance(m)) => Some(from_meters(state.snapshot, *m)),
            _ => None,
        })
    else {
        return Ok(Step::Ask(offered(
            state,
            OFFSET_DISTANCE,
            Prompt::new("distance", DISTANCE.message_id, Accept::distance()).or(Accept::keywords(
                [
                    KeywordOption::new("through", OPTION_THROUGH.message_id),
                    KeywordOption::new("layer", OPTION_LAYER.message_id),
                    KeywordOption::new("multiple", OPTION_MULTIPLE.message_id),
                ],
            )),
        )));
    };
    objects_then(state, serde_json::json!({"distance": distance}))
}

/// The objects and, for the distance form, the side point; then the command's parameters.
fn objects_then(state: &PromptState<'_>, mut params: serde_json::Value) -> arc_base::Result<Step> {
    let Some(objects) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        )));
    };
    params["objects"] = serde_json::json!(objects);
    let layer = if keyword_at(state, "layer_mode", "current") {
        "current"
    } else {
        "source"
    };
    params["layer"] = serde_json::json!(layer);
    let repeat = super::count_answer(state, "repeat").unwrap_or(1);
    params["repeat"] = serde_json::json!(repeat);
    if params.get("through").is_some() {
        return done(params);
    }
    let Some(side) = point_answer(state, "side") else {
        return Ok(Step::Ask(Prompt::new(
            "side",
            SIDE.message_id,
            super::point(),
        )));
    };
    params["side"] = serde_json::json!(side);
    done(params)
}

fn check(_: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> arc_base::Result<()> {
    match input {
        PromptInput::Distance(m) if !(m.is_finite() && *m > 0.0) => {
            Err(Error::new(codes::VALUE_INVALID).with_detail("field", "distance"))
        }
        _ => Ok(()),
    }
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.modify.offset",
            LABEL,
            DESCRIPTION,
            remembering(run, |p: &OffsetParams, per_unit| {
                p.distance
                    .map(|d| vec![(OFFSET_DISTANCE, d * per_unit)])
                    .unwrap_or_default()
            }),
        )
        .with_prompt(machine(next, check)),
    ]
}
