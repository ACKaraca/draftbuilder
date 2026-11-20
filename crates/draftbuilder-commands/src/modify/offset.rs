//! OFFSET (`draftbuilder.modify.offset`, typed `OFFSET`, alias `O`): parallel copies of entities at a
//! distance. The side is given by a point: the copy lies on the side of that point (to the left of the
//! direction of travel when the point is on the left), or the distance is the distance of the
//! `through` point from each entity. Polyline corners close by extension (a miter join with a large
//! limit). The copies are new entities on the layer and with the appearance of their source. With
//! `erase_source` the sources are erased after the copies are made.
//!
//! Copies of an object that the offset splits into several curves are one polyline when they join
//! end to end, and otherwise one entity each. The last distance is not remembered between commands,
//! and the repeated offset from the last result is not built.

use arc_base::math::Point2;
use arc_base::{Error, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Composite2, Curve2, CurveEval, CurveProject, JoinStyle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::transform::Changed;
use super::{
    entities, entity_selection, erase_entities, point_answer, refuse_locked, selection_answer,
};
use crate::draw::create::{Creator, done, from_meters, machine};
use crate::error::{Message, codes};

/// The miter limit of a polyline corner: large enough that corners close by extension.
const CORNER_LIMIT: f64 = 1_000.0;

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
    Ok(if left < 0.0 { -distance } else { distance })
}

/// The copies of one entity's curve at the signed distance, as curves.
fn pieces(curve: &Curve2, distance: f64, ctx: &Context<'_>) -> Result<Vec<Curve2>> {
    let composites: Vec<Composite2> = arc_geom::offset(
        curve,
        distance,
        JoinStyle::Miter {
            limit: CORNER_LIMIT,
        },
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
    if p.erase_source {
        refuse_locked(&snapshot, &ids)?;
    }
    let mut creator = Creator::new(&snapshot)?;
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
        for piece in pieces(&curve, signed, ctx)? {
            let key = super::key_of_curve(&piece);
            let values = super::appearance_values(&snapshot, key, &object.values, &piece);
            creator.add_values(ctx, key, owner, values)?;
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

fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let through_chosen = state.answers.iter().any(|a| {
        a.key == "distance" && matches!(&a.input, PromptInput::Keyword(k) if k == "through")
    });
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
        return Ok(Step::Ask(
            Prompt::new("distance", DISTANCE.message_id, Accept::distance()).or(Accept::keywords(
                [KeywordOption::new("through", OPTION_THROUGH.message_id)],
            )),
        ));
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
    if let PromptInput::Distance(m) = input
        && !(m.is_finite() && *m > 0.0)
    {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "distance"));
    }
    Ok(())
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.offset", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
