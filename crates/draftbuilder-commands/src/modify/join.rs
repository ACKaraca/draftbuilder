//! JOIN (`draftbuilder.modify.join`, typed `JOIN`, alias `J`): joins entities that touch end to end
//! into one entity of the simplest kind that represents them. Collinear segments join into a segment;
//! coaxial arcs join into an arc, and an arc chain that closes into a circle; any other chain of lines,
//! arcs and polylines joins into a polyline. Entities that join with nothing are reported in
//! `unjoined`. The fuzz distance is the largest gap that still counts as touching (default: the
//! drawing tolerance).
//!
//! The joined entity keeps the identifier of the first entity of its chain when it keeps the kind, and
//! takes the layer and appearance of that entity; the other entities of the chain are erased.

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

/// The curve of a chain of curves, in the chain's direction, in the simplest kind that represents it;
/// `None` when the chain has no single representation.
fn chain_curve(
    composite: &Composite2,
    curves: &[Curve2],
    tolerance: &Tolerance,
) -> Result<Option<Curve2>> {
    let all = |f: fn(&Curve2) -> bool| curves.iter().all(f);
    if all(|c| matches!(c, Curve2::Segment(_))) {
        let segments: Vec<Segment2> = curves
            .iter()
            .filter_map(|c| match c {
                Curve2::Segment(s) => Some(*s),
                _ => None,
            })
            .collect();
        let first = segments[0];
        let direction = first.end() - first.start();
        let straight = segments.iter().all(|s| {
            let d = s.end() - s.start();
            let cross = direction.x * d.y - direction.y * d.x;
            cross.abs() <= tolerance.angular * direction.norm() * d.norm()
        });
        if straight && !composite.is_closed() {
            return Ok(Some(Curve2::Segment(Segment2::new(
                composite.start_point(),
                composite.end_point(),
            )?)));
        }
    }
    if all(|c| matches!(c, Curve2::Arc(_))) {
        let arcs: Vec<Arc2> = curves
            .iter()
            .filter_map(|c| match c {
                Curve2::Arc(a) => Some(*a),
                _ => None,
            })
            .collect();
        let first = arcs[0];
        let coaxial = arcs.iter().all(|a| {
            (a.center() - first.center()).norm() <= tolerance.linear
                && (a.radius() - first.radius()).abs() <= tolerance.linear
        });
        if coaxial {
            let sweep: f64 = arcs.iter().map(Arc2::sweep).sum();
            if composite.is_closed()
                && (sweep.abs() - std::f64::consts::TAU).abs() <= tolerance.angular
            {
                return Ok(Some(Curve2::Circle(Circle2::new(
                    first.center(),
                    first.radius(),
                )?)));
            }
            if !composite.is_closed() {
                let arc = Arc2::new(first.center(), first.radius(), arcs[0].start_angle(), sweep)?;
                return Ok(Some(Curve2::Arc(arc)));
            }
        }
    }
    Ok(composite.to_polyline().map(Curve2::Polyline))
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
            unjoined.extend(members.iter().map(|i| with_curves[*i]));
            continue;
        }
        let chain: Vec<Curve2> = members.iter().map(|i| curves[*i].clone()).collect();
        let Some(result) = chain_curve(composite, &chain, &tolerance)? else {
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
