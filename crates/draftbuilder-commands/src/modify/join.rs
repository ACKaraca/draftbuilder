//! JOIN (`draftbuilder.modify.join`, typed `JOIN`, alias `J`): joins entities that touch end to end
//! into one entity of the simplest kind that represents them. Collinear segments join into a segment;
//! coaxial arcs join into an arc, and an arc chain that closes into a circle; any other chain of lines,
//! arcs and polylines joins into a polyline. Entities that join with nothing are reported in
//! `unjoined`. The fuzz distance is the largest gap that still counts as touching (default: the
//! drawing tolerance).
//!
//! The joined entity is built from the chain in its order. A segment results only when the pieces run the
//! same way (no fold, no overlap); an arc only when the pieces lie on one circle and turn the same way; a
//! chain that does neither becomes its exact polyline, so no length is lost. A single open polyline whose
//! ends touch is closed. The entity of the chain with the lowest position in the selection keeps the
//! identifier when the result has its kind (otherwise the result is a new entity), takes the layer and
//! appearance of that entity, and the other members of the chain are erased.

use arc_base::math::Vector2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{CommandSpec, Context, IdParam};
use arc_geom::{Arc2, Circle2, Composite2, Curve2, Segment2, join};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{entities, erase_entities, refuse_locked, replace_with_pieces};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.join`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinParams {
    /// The entities to join; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The largest gap in drawing units that still joins two ends; default the drawing tolerance.
    #[serde(default)]
    pub fuzz: Option<f64>,
}

/// The result of a join: the entities it produced, and the entities that joined with nothing.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Joined {
    /// The joined entities, in order.
    pub objects: Vec<IdParam>,
    /// The entities that were not joined with another.
    pub unjoined: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-join-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-join-description",
};

/// The curve of a joined chain, built from its pieces in chain order (the composite holds them oriented):
/// a segment only when the pieces run the same way, so the result is their span with no overlap; an arc
/// only when the pieces lie on one circle and turn the same way, closing into a circle when they make a
/// full turn; otherwise the exact polyline of the chain, which keeps every fold and overlap. `None` when a
/// piece has no polyline form (a spline).
fn chain_curve(composite: &Composite2, tolerance: &Tolerance) -> Option<Curve2> {
    let pieces = composite.curves();
    if pieces.iter().all(|c| matches!(c, Curve2::Segment(_))) && !composite.is_closed() {
        let directions: Vec<Vector2> = pieces
            .iter()
            .filter_map(|c| match c {
                Curve2::Segment(s) => Some(s.end() - s.start()),
                _ => None,
            })
            .collect();
        let same_way = directions
            .windows(2)
            .all(|pair| pair[0].x * pair[1].x + pair[0].y * pair[1].y > 0.0);
        if same_way
            && let Ok(segment) = Segment2::new(composite.start_point(), composite.end_point())
        {
            return Some(Curve2::Segment(segment));
        }
    }
    let arcs: Vec<Arc2> = pieces
        .iter()
        .filter_map(|c| match c {
            Curve2::Arc(a) => Some(*a),
            _ => None,
        })
        .collect();
    if arcs.len() == pieces.len() && !arcs.is_empty() {
        let first = arcs[0];
        let coaxial = arcs.iter().all(|a| {
            (a.center() - first.center()).norm() <= tolerance.linear
                && (a.radius() - first.radius()).abs() <= tolerance.linear
        });
        let turns_alike = arcs
            .windows(2)
            .all(|pair| pair[0].sweep().signum() == pair[1].sweep().signum());
        let total: f64 = arcs.iter().map(Arc2::sweep).sum();
        if coaxial && turns_alike {
            if composite.is_closed()
                && (total.abs() - std::f64::consts::TAU).abs() <= tolerance.angular
            {
                return Circle2::new(first.center(), first.radius())
                    .ok()
                    .map(Curve2::Circle);
            }
            if !composite.is_closed() && total.abs() < std::f64::consts::TAU - tolerance.angular {
                return Arc2::new(first.center(), first.radius(), first.start_angle(), total)
                    .ok()
                    .map(Curve2::Arc);
            }
        }
    }
    composite.to_polyline().map(Curve2::Polyline)
}

/// The polyline an open polyline becomes when its ends touch: the closing segment is the segment from
/// its last vertex (kept bulge) to the first, so the duplicate last vertex is dropped. `None` for any
/// other curve, or an open polyline whose ends do not touch.
fn closed_polyline(curve: &Curve2, tolerance: &Tolerance) -> Option<Curve2> {
    let Curve2::Polyline(polyline) = curve else {
        return None;
    };
    if polyline.is_closed() {
        return None;
    }
    let vertices = polyline.vertices();
    let (first, last) = (vertices.first()?, vertices.last()?);
    if (first.point - last.point).norm() > tolerance.linear || vertices.len() < 3 {
        return None;
    }
    let kept = vertices[..vertices.len() - 1].to_vec();
    arc_geom::Polyline2::new(kept, true)
        .ok()
        .map(Curve2::Polyline)
}

fn run(ctx: &mut Context<'_>, p: JoinParams) -> Result<Joined> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let snapshot = ctx.snapshot().clone();
    refuse_locked(&snapshot, &ids)?;
    let mut unjoined = Vec::new();
    let mut with_curves = Vec::new();
    let mut curves = Vec::new();
    for id in &ids {
        match super::curve_of(&snapshot, *id)? {
            Some(curve) => {
                with_curves.push(*id);
                curves.push(curve);
            }
            None => unjoined.push(*id),
        }
    }
    let tolerance = Tolerance {
        linear: p.fuzz.unwrap_or(Tolerance::DEFAULT.linear),
        ..Tolerance::DEFAULT
    };
    if !(tolerance.linear.is_finite() && tolerance.linear > 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "fuzz"));
    }
    let joined = join(&curves, &tolerance)?;
    let mut out: Vec<ObjectId> = Vec::new();
    for (composite, indices) in &joined.chains {
        let members: Vec<usize> = indices.clone();
        if members.len() < 2 {
            // A single open polyline whose ends touch is closed in place.
            match members
                .first()
                .and_then(|i| closed_polyline(&curves[*i], &tolerance))
            {
                Some(closed) => {
                    out.extend(replace_with_pieces(
                        ctx,
                        with_curves[members[0]],
                        &[closed],
                    )?);
                }
                None => unjoined.extend(members.iter().map(|i| with_curves[*i])),
            }
            continue;
        }
        let Some(result) = chain_curve(composite, &tolerance) else {
            unjoined.extend(members.iter().map(|i| with_curves[*i]));
            continue;
        };
        let target = members.iter().copied().min().unwrap_or(0);
        let target_id = with_curves[target];
        let others: Vec<ObjectId> = members
            .iter()
            .map(|i| with_curves[*i])
            .filter(|id| *id != target_id)
            .collect();
        out.extend(replace_with_pieces(ctx, target_id, &[result])?);
        erase_entities(ctx, &others)?;
    }
    ctx.set_selection(out.clone())?;
    Ok(Joined {
        objects: out.into_iter().map(IdParam).collect(),
        unjoined: unjoined.into_iter().map(IdParam).collect(),
    })
}

/// The prompt machine: the entities, then the join.
fn next(state: &arc_command::PromptState<'_>) -> arc_base::Result<arc_command::Step> {
    use arc_command::{Prompt, Step};
    match super::selection_answer(state, "objects") {
        Some(objects) => crate::draw::create::done(serde_json::json!({"objects": objects})),
        None => Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            super::entity_selection(),
        ))),
    }
}

const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.join", LABEL, DESCRIPTION, run).with_prompt(
            crate::draw::create::machine(next, crate::draw::create::accept_all),
        ),
    ]
}
