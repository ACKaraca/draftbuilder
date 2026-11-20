//! TRIM and EXTEND (`draftbuilder.modify.trim`, typed `TRIM`, alias `TR`; `draftbuilder.modify.extend`,
//! typed `EXTEND`, alias `EX`). Each target is an entity with its own pick point. Trim removes the
//! stretch of the target that contains the pick, between the nearest intersections with the edges on
//! either side; an edge-less side runs to the end of an open target. Extend lengthens the end of the
//! target nearest the pick to the nearest edge beyond it.
//!
//! The edges are the given entities. When none is given (quick mode), every other visible entity of the
//! model space with a curve is an edge (the selection rules apply: objects on layers that are off or
//! frozen, and invisible ones, are not edges). With `extend_edges`, each edge is first taken to its natural
//! continuation: a segment or a ray to its line, an arc to its circle, an elliptical arc to its
//! ellipse, a polyline to the lines of its segments (a spline stays as it is).
//!
//! Several targets are processed in order, each in a savepoint, against the edges as they were when the
//! command started. A target that cannot be processed (no intersection, a locked layer) is rolled back and
//! listed in `skipped` with its error code; the command succeeds when at least one target changed and fails
//! with the first error when none did.
//!
//! The first part of the target's own kind keeps its identifier; the other parts are created beside it with
//! the target's layer and stored fields; a target that changes kind is replaced by new entities and erased,
//! refused when another entity refers to it.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::Snapshot;

use arc_draft::types::ENTITIES;
use arc_geom::{Circle2, Curve2, CurveEnd, CurveProject, Line2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{entities, replace_with_pieces, selection_answer};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};
use crate::select::{candidates::Options, methods};

/// A target a trim or an extend could not process, with the code of its error.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Skipped {
    /// The target.
    pub object: IdParam,
    /// The error code that stopped it (its own code, for example `geom.edit.no_intersection`).
    pub reason: String,
}

/// The result of a trim or an extend: the entities that now hold the changed targets, and the targets that
/// were skipped.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Edited {
    /// The entities produced, in target order.
    pub objects: Vec<IdParam>,
    /// The targets that could not be processed, in target order, with their reasons.
    pub skipped: Vec<Skipped>,
}

/// One target of a trim or an extend: the entity and the point on the part to change.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EdgeTarget {
    /// The entity to trim or extend.
    pub object: IdParam,
    /// A point on the part to remove (trim) or on the end to lengthen (extend), in drawing units.
    pub pick: [f64; 2],
}

/// Parameters of `draftbuilder.modify.trim` and `draftbuilder.modify.extend`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EdgeParams {
    /// The cutting or boundary edges. Empty: every other entity of the model space is an edge.
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub edges: Vec<IdParam>,
    /// The entities to change, each with its pick point, in order.
    #[schemars(length(min = 1, max = 10000))]
    pub targets: Vec<EdgeTarget>,
    /// Whether the edges are first taken to their natural continuation.
    #[serde(default)]
    pub extend_edges: bool,
}

const TRIM_LABEL: Message = Message {
    message_id: "db-cmd-modify-trim-label",
};
const TRIM_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-trim-description",
};
const EXTEND_LABEL: Message = Message {
    message_id: "db-cmd-modify-extend-label",
};
const EXTEND_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-extend-description",
};
const EDGES: Message = Message {
    message_id: "db-cmd-modify-edges",
};
const TARGET: Message = Message {
    message_id: "db-cmd-modify-target-object",
};
const PICK: Message = Message {
    message_id: "db-cmd-modify-pick-point",
};
const OPTION_EXTEND: Message = Message {
    message_id: "db-cmd-modify-option-extend-edges",
};
const OPTION_END: Message = Message {
    message_id: "db-cmd-modify-option-end",
};

/// The natural continuation of a curve, as the edges of an `extend_edges` trim or extend.
fn continuation(curve: &Curve2) -> Result<Vec<Curve2>> {
    Ok(match curve {
        Curve2::Segment(s) => vec![Curve2::Line(Line2::new(s.start(), s.end() - s.start())?)],
        Curve2::Ray(r) => vec![Curve2::Line(Line2::new(r.origin(), r.direction())?)],
        Curve2::Arc(a) => vec![Curve2::Circle(Circle2::new(a.center(), a.radius())?)],
        Curve2::EllipticalArc(e) => vec![Curve2::Ellipse(*e.ellipse())],
        Curve2::Polyline(p) => {
            let mut out = Vec::new();
            for segment in p.segments() {
                match segment {
                    arc_geom::PolylineSegment::Line(s) => {
                        out.extend(continuation(&Curve2::Segment(s))?)
                    }
                    arc_geom::PolylineSegment::Arc(a) => out.extend(continuation(&Curve2::Arc(a))?),
                }
            }
            out
        }
        other => vec![other.clone()],
    })
}

/// The edge curves of a command, each with the curves it stands for (its continuation when
/// `extend_edges` is set), read once from `snapshot`. The edges are the given entities; in quick mode
/// they are the visible objects of the model space with a curve.
fn edge_curves(
    snapshot: &Snapshot,
    edges: &[ObjectId],
    quick: bool,
    extend_edges: bool,
) -> Result<Vec<(ObjectId, Vec<Curve2>)>> {
    let ids: Vec<ObjectId> = if quick {
        // The selection rules apply (layers off or frozen, invisible objects are not edges); locked objects
        // still limit a change.
        let space =
            crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
        let options = Options {
            include_locked: true,
        };
        methods::select_all(snapshot, space, &options)?.ids
    } else {
        edges.to_vec()
    };
    let mut out = Vec::new();
    for id in ids {
        let Some(curve) = super::curve_of(snapshot, id)? else {
            continue;
        };
        let curves = if extend_edges {
            continuation(&curve)?
        } else {
            vec![curve]
        };
        out.push((id, curves));
    }
    Ok(out)
}

/// The edges, validated as entities.
fn edge_ids(ctx: &Context<'_>, p: &EdgeParams) -> Result<Vec<ObjectId>> {
    entities(ctx.snapshot(), ctx.limits(), &p.edges)
}

/// The targets and their pick points, validated as entities; every target is a different entity.
fn target_ids(ctx: &Context<'_>, p: &EdgeParams) -> Result<Vec<(ObjectId, Point2)>> {
    let list: Vec<IdParam> = p.targets.iter().map(|t| t.object).collect();
    let ids = entities(ctx.snapshot(), ctx.limits(), &list)?;
    if ids.len() != p.targets.len() {
        return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "targets"));
    }
    Ok(ids
        .into_iter()
        .zip(&p.targets)
        .map(|(id, t)| (id, Point2::new(t.pick[0], t.pick[1])))
        .collect())
}

/// Trims one target: the stretch containing the pick, between the nearest intersections with `limits`.
fn trim_one(
    ctx: &mut Context<'_>,
    id: ObjectId,
    curve: &Curve2,
    pick: Point2,
    limits: &[Curve2],
) -> Result<Vec<ObjectId>> {
    let tolerance = Tolerance::DEFAULT;
    let at = curve.closest_point(pick, &tolerance)?.parameter;
    let parts = arc_geom::trim(curve, limits, at, &tolerance)?;
    replace_with_pieces(ctx, id, &parts)
}

/// Extends one target at the end nearest the pick, to the nearest of `limits` beyond it.
fn extend_one(
    ctx: &mut Context<'_>,
    id: ObjectId,
    curve: &Curve2,
    pick: Point2,
    limits: &[Curve2],
) -> Result<Vec<ObjectId>> {
    let tolerance = Tolerance::DEFAULT;
    let (Some(start), Some(end)) = (curve.start_point(), curve.end_point()) else {
        return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()));
    };
    let end_kind = if (start - pick).norm() <= (end - pick).norm() {
        CurveEnd::Start
    } else {
        CurveEnd::End
    };
    let extended = arc_geom::extend(curve, end_kind, limits, &tolerance)?;
    replace_with_pieces(ctx, id, &[extended])
}

/// Runs trim (or extend) on every target of a command, each in a savepoint. The edges are the curves they
/// were at the start of the command, so the order of the targets does not change what they cut. A target
/// that cannot be processed is rolled back and listed in `skipped` with its error code; the command succeeds
/// when at least one target changed, and fails with the first error when none did.
fn edit_targets(ctx: &mut Context<'_>, p: EdgeParams, extend: bool) -> Result<Edited> {
    let edges = edge_ids(ctx, &p)?;
    let targets = target_ids(ctx, &p)?;
    let quick = p.edges.is_empty();
    let start = ctx.snapshot().clone();
    let edge_sets = edge_curves(&start, &edges, quick, p.extend_edges)?;
    let mut objects = Vec::new();
    let mut skipped = Vec::new();
    let mut first_error: Option<Error> = None;
    for (id, pick) in targets {
        let curve = super::required_curve(&start, id)?;
        let limits: Vec<Curve2> = edge_sets
            .iter()
            .filter(|(edge, _)| *edge != id)
            .flat_map(|(_, curves)| curves.iter().cloned())
            .collect();
        let savepoint = ctx.txn()?.savepoint()?;
        let outcome = if extend {
            extend_one(ctx, id, &curve, pick, &limits)
        } else {
            trim_one(ctx, id, &curve, pick, &limits)
        };
        match outcome {
            Ok(ids) => {
                ctx.txn()?.release(&savepoint)?;
                objects.extend(ids);
            }
            Err(error) => {
                ctx.txn()?.rollback_to(&savepoint)?;
                skipped.push(Skipped {
                    object: IdParam(id),
                    reason: error.code().to_owned(),
                });
                first_error.get_or_insert(error);
            }
        }
    }
    if objects.is_empty() {
        return Err(first_error.unwrap_or_else(|| {
            Error::new(codes::PARAMS_CONFLICT).with_detail("field", "targets")
        }));
    }
    ctx.set_selection(objects.clone())?;
    Ok(Edited {
        objects: objects.into_iter().map(IdParam).collect(),
        skipped,
    })
}

fn trim_run(ctx: &mut Context<'_>, p: EdgeParams) -> Result<Edited> {
    edit_targets(ctx, p, false)
}

fn extend_run(ctx: &mut Context<'_>, p: EdgeParams) -> Result<Edited> {
    edit_targets(ctx, p, true)
}

/// The number of answers of the keyword `extend` to the edges prompt (an odd number means the edges
/// are extended to their continuation).
fn extend_toggled(state: &PromptState<'_>) -> bool {
    state
        .answers
        .iter()
        .filter(|a| {
            a.key == "edges" && matches!(&a.input, PromptInput::Keyword(k) if k == "extend")
        })
        .count()
        % 2
        == 1
}

/// The targets and pick points answered so far, and whether a target still waits for its pick.
fn targets_so_far(state: &PromptState<'_>) -> (Vec<serde_json::Value>, Option<ObjectId>) {
    let mut out = Vec::new();
    let mut pending: Option<ObjectId> = None;
    for answer in state.answers {
        match (answer.key.as_str(), &answer.input) {
            ("target", PromptInput::Pick(id)) => pending = Some(*id),
            ("pick", PromptInput::Point(p)) => {
                if let Some(id) = pending.take() {
                    let q = crate::draw::create::drawing_point(state.snapshot, *p);
                    out.push(serde_json::json!({"object": id.to_string(), "pick": [q.x, q.y]}));
                }
            }
            _ => {}
        }
    }
    (out, pending)
}

/// The prompt machine shared by trim and extend.
fn edge_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(edges) = selection_answer(state, "edges") else {
        let accept = Accept::Selection {
            min: 0,
            max: None,
            types: ENTITIES.iter().map(|k| (*k).to_owned()).collect(),
        };
        return Ok(Step::Ask(
            Prompt::new("edges", EDGES.message_id, accept)
                .or(Accept::keywords([KeywordOption::new(
                    "extend",
                    OPTION_EXTEND.message_id,
                )]))
                .with_default(PromptInput::Selection(Vec::new())),
        ));
    };
    let extend_edges = extend_toggled(state);
    let (targets, pending) = targets_so_far(state);
    if pending.is_some() {
        return Ok(Step::Ask(Prompt::new(
            "pick",
            PICK.message_id,
            super::point(),
        )));
    }
    let ended = state.answers.last().is_some_and(|a| {
        a.key == "target" && matches!(&a.input, PromptInput::Keyword(k) if k == "end")
    });
    if ended && !targets.is_empty() {
        return done(serde_json::json!({
            "edges": edges, "targets": targets, "extend_edges": extend_edges
        }));
    }
    let mut prompt = Prompt::new(
        "target",
        TARGET.message_id,
        Accept::Pick {
            types: ENTITIES.iter().map(|k| (*k).to_owned()).collect(),
        },
    )
    .or(Accept::keywords([KeywordOption::new(
        "end",
        OPTION_END.message_id,
    )]));
    if !targets.is_empty() {
        prompt = prompt.with_default(PromptInput::Keyword("end".to_owned()));
    }
    Ok(Step::Ask(prompt))
}

/// Refuses an `end` before any target, so the prompt never finishes with nothing to change.
fn edge_check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if matches!(input, PromptInput::Keyword(k) if k == "end") && targets_so_far(state).0.is_empty()
    {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "targets"));
    }
    Ok(())
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.modify.trim",
            TRIM_LABEL,
            TRIM_DESCRIPTION,
            trim_run,
        )
        .with_prompt(machine(edge_next, edge_check)),
        super::spec(
            "draftbuilder.modify.extend",
            EXTEND_LABEL,
            EXTEND_DESCRIPTION,
            extend_run,
        )
        .with_prompt(machine(edge_next, edge_check)),
    ]
}
