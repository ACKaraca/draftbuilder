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
//! A target that keeps one part keeps its identifier when the part has its kind; a target that
//! splits into two (a pick between two edges of a line) keeps the first part and creates the second
//! with the target's layer and appearance; a target that changes kind is replaced by new entities
//! and erased. A target on a locked layer is refused with `draft.layer.locked`.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::Snapshot;
use arc_draft::types::ENTITIES;
use arc_geom::{Circle2, Curve2, CurveEnd, CurveProject, Line2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::Changed;
use super::{entities, replace_with_pieces, selection_answer};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};
use crate::select::{candidates::Options, methods};

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

/// The curves that limit a change of `target`: the edges given, or in quick mode every other visible
/// object of the model space with a curve; each taken to its continuation when `extend_edges` is set.
fn boundaries(
    snapshot: &Snapshot,
    edges: &[ObjectId],
    target: ObjectId,
    quick: bool,
    extend_edges: bool,
) -> Result<Vec<Curve2>> {
    let ids: Vec<ObjectId> = if quick {
        // The visible objects of the model space: the selection rules (layers off or frozen, invisible
        // entities) apply; locked objects still limit a change.
        let space =
            crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
        let options = Options {
            include_locked: true,
        };
        methods::select_all(snapshot, space, &options)?.ids
    } else {
        edges.to_vec()
    };
    let mut curves = Vec::new();
    for id in ids.into_iter().filter(|id| *id != target) {
        let Some(curve) = super::curve_of(snapshot, id)? else {
            continue;
        };
        if extend_edges {
            curves.extend(continuation(&curve)?);
        } else {
            curves.push(curve);
        }
    }
    Ok(curves)
}

/// The edges, validated as entities.
fn edge_ids(ctx: &Context<'_>, p: &EdgeParams) -> Result<Vec<ObjectId>> {
    entities(ctx.snapshot(), ctx.limits(), &p.edges)
}

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

fn trim_run(ctx: &mut Context<'_>, p: EdgeParams) -> Result<Changed> {
    let edges = edge_ids(ctx, &p)?;
    let targets = target_ids(ctx, &p)?;
    let quick = p.edges.is_empty();
    let mut out = Vec::new();
    for (id, pick) in targets {
        let snapshot = ctx.snapshot().clone();
        let curve = super::required_curve(&snapshot, id)?;
        let limits = boundaries(&snapshot, &edges, id, quick, p.extend_edges)?;
        let tolerance = Tolerance::DEFAULT;
        let at = curve.closest_point(pick, &tolerance)?.parameter;
        let parts = arc_geom::trim(&curve, &limits, at, &tolerance)?;
        out.extend(replace_with_pieces(ctx, id, &parts)?);
    }
    ctx.set_selection(out.clone())?;
    Ok(Changed {
        objects: out.into_iter().map(IdParam).collect(),
    })
}

fn extend_run(ctx: &mut Context<'_>, p: EdgeParams) -> Result<Changed> {
    let edges = edge_ids(ctx, &p)?;
    let targets = target_ids(ctx, &p)?;
    let quick = p.edges.is_empty();
    let mut out = Vec::new();
    for (id, pick) in targets {
        let snapshot = ctx.snapshot().clone();
        let curve = super::required_curve(&snapshot, id)?;
        let limits = boundaries(&snapshot, &edges, id, quick, p.extend_edges)?;
        let tolerance = Tolerance::DEFAULT;
        let (Some(start), Some(end)) = (curve.start_point(), curve.end_point()) else {
            return Err(Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", id.to_string()));
        };
        let end_kind = if (start - pick).norm() <= (end - pick).norm() {
            CurveEnd::Start
        } else {
            CurveEnd::End
        };
        let extended = arc_geom::extend(&curve, end_kind, &limits, &tolerance)?;
        out.extend(replace_with_pieces(ctx, id, &[extended])?);
    }
    ctx.set_selection(out.clone())?;
    Ok(Changed {
        objects: out.into_iter().map(IdParam).collect(),
    })
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
