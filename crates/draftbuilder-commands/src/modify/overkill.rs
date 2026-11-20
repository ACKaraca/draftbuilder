//! OVERKILL (`draftbuilder.modify.overkill`, typed `OVERKILL`): merges overlapping, touching and collinear lines
//! within a tolerance, so that duplicates disappear. Lines merge only when they share an owner (space or block)
//! and their layer, color, linetype, linetype scale, lineweight, transparency, plot style, visibility, elevation,
//! thickness and extrusion. Among the lines that share these, the ones lying on one line form a group; each group
//! is swept once along its line (sorted by the start of each extent), and a run of lines that overlap or touch
//! within the tolerance becomes one line. The first line of a run in the selection keeps its identifier and takes
//! the run's geometry; the others are erased. Cost is O(n log n) for the sorting, and grouping compares each line
//! with one first line per group. Other kinds in the selection are left unchanged; arcs, polyline segments and the
//! option to ignore properties or to break polylines are not built.
//!
//! The result reports the lines that kept their identifier with their new geometry (`changed`) and the lines
//! erased (`erased`). A line another entity refers to is refused (`db.cmd.erase_dependent`), and no line changes
//! when a line of the selection is on a locked layer.

use arc_base::math::{Point2, Vector2};
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{CommandSpec, Context, IdParam, Prompt, PromptState, Step};
use arc_doc::Value;
use arc_draft::types::{LINE, entity};
use arc_geom::{Curve2, Segment2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    entities, entity_selection, erase_entities, refuse_locked, replace_geometry, selection_answer,
};
use crate::draw::create::{done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.overkill`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverkillParams {
    /// The objects to clean; only the lines among them are merged.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The distance, in drawing units, within which lines are collinear; the default is the drawing's linear tolerance.
    #[serde(default)]
    pub tolerance: Option<f64>,
}

/// What `draftbuilder.modify.overkill` changed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Overkill {
    /// The lines that kept their identifier and took merged geometry, in selection order.
    pub changed: Vec<IdParam>,
    /// The lines erased because they merged into another line, in selection order.
    pub erased: Vec<IdParam>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-overkill-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-overkill-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};

/// The stored fields of a LINE that lines must share to merge, besides the owner and the header: the common entity
/// fields of arc-draft's entity kinds, in order, except `draw_order` (which orders drawing and differs between any two
/// lines drawn one after another, so comparing it would merge nothing) and the geometry (which is what merges).
/// A LINE has no other stored field: its kind is the common fields and `geometry`. Extended data, hyperlinks and
/// materials are not fields of the model, so there is nothing else to compare.
const SHARED_FIELDS: [usize; 15] = [
    entity::LAYER,
    entity::COLOR_BY,
    entity::COLOR,
    entity::LINETYPE_BY,
    entity::LINETYPE,
    entity::LINETYPE_SCALE,
    entity::LINEWEIGHT_BY,
    entity::LINEWEIGHT,
    entity::TRANSPARENCY_BY,
    entity::TRANSPARENCY,
    entity::PLOT_STYLE,
    entity::VISIBLE,
    entity::ELEVATION,
    entity::THICKNESS,
    entity::EXTRUSION,
];

/// What two lines must share to merge: the owner, the header (name, partition, flags) and the shared fields.
#[derive(PartialEq)]
struct Properties {
    owner: Option<ObjectId>,
    name: Option<String>,
    partition: Option<String>,
    flags: arc_doc::Flags,
    shared: Vec<Option<Value>>,
}

/// A LINE of the selection: its position in the selection, its endpoints, its unit direction (none for a line of
/// no length) and its properties.
struct Line {
    id: ObjectId,
    start: Point2,
    end: Point2,
    unit: Option<Vector2>,
    properties: Properties,
}

/// The perpendicular distance of `p` from the line through `a` along the unit direction `u`.
fn distance_from(a: Point2, u: Vector2, p: Point2) -> f64 {
    let v = p - a;
    (u.x * v.y - u.y * v.x).abs()
}

/// The position of `p` along the unit direction `u` from `a`.
fn position_along(a: Point2, u: Vector2, p: Point2) -> f64 {
    (p - a).dot(&u)
}

/// Whether both ends of `other` lie within `tol` of the line of `base`.
fn collinear(base: &Line, other: &Line, tol: f64) -> bool {
    let Some(u) = base.unit else {
        return false;
    };
    distance_from(base.start, u, other.start) <= tol
        && distance_from(base.start, u, other.end) <= tol
}

/// One run of lines on one line: its extent along the group's direction and its members.
struct Run {
    lo: f64,
    hi: f64,
    members: Vec<usize>,
}

/// The merges of the selection: for each run of lines on one line, the keeper (first in the selection) with the
/// run's extent, written in the keeper's own direction, and the lines of the run to erase. Lines of different
/// owner, header or shared fields never meet.
fn plan(lines: &[Line], tol: f64) -> (Vec<(usize, Point2, Point2)>, Vec<usize>) {
    // Group the lines on one line with the same properties: each line joins the first group whose first line it
    // lies on. A group is a set of lines on one line, so the sweep below is per group.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let found = groups.iter_mut().find(|group| {
            let first = &lines[group[0]];
            first.properties == line.properties && collinear(first, line, tol)
        });
        match found {
            Some(group) => group.push(i),
            None => groups.push(vec![i]),
        }
    }

    // One sweep per group, along the line of its first member: sorted by the start of each line's extent, a
    // line joins the current run while it starts within the tolerance of the run's end.
    let mut keep: Vec<(usize, Point2, Point2)> = Vec::new();
    let mut erase: Vec<usize> = Vec::new();
    for group in &groups {
        let first = &lines[group[0]];
        let Some(u) = first.unit else {
            continue;
        };
        let mut extents: Vec<(f64, f64, usize)> = group
            .iter()
            .map(|&i| {
                let a = position_along(first.start, u, lines[i].start);
                let b = position_along(first.start, u, lines[i].end);
                (a.min(b), a.max(b), i)
            })
            .collect();
        extents.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut runs: Vec<Run> = Vec::new();
        for (lo, hi, i) in extents {
            match runs.last_mut() {
                Some(run) if lo <= run.hi + tol => {
                    run.hi = run.hi.max(hi);
                    run.members.push(i);
                }
                _ => runs.push(Run {
                    lo,
                    hi,
                    members: vec![i],
                }),
            }
        }
        for run in runs {
            let Some(&keeper) = run.members.iter().min() else {
                continue;
            };
            if run.members.len() == 1 {
                continue;
            }
            // The extent runs along the group's direction; the keeper keeps the direction it was drawn in.
            let from = first.start + u * run.lo;
            let to = first.start + u * run.hi;
            let own = lines[keeper].end - lines[keeper].start;
            let (start, end) = if own.dot(&u) < 0.0 {
                (to, from)
            } else {
                (from, to)
            };
            keep.push((keeper, start, end));
            erase.extend(run.members.iter().copied().filter(|&i| i != keeper));
        }
    }
    (keep, erase)
}

fn run_overkill(ctx: &mut Context<'_>, p: OverkillParams) -> Result<Overkill> {
    let tol = p.tolerance.unwrap_or(Tolerance::DEFAULT.linear);
    if !(tol.is_finite() && tol >= 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "tolerance"));
    }
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let snapshot = ctx.snapshot().clone();
    let mut lines: Vec<Line> = Vec::new();
    for id in &ids {
        // Only LINE entities take part; every other kind in the selection is left unchanged.
        if super::entity_key(&snapshot, *id)? != LINE {
            continue;
        }
        let Some(Curve2::Segment(s)) = super::curve_of(&snapshot, *id)? else {
            continue;
        };
        let object = snapshot.get(snapshot.resolve(*id)?)?;
        let shared = SHARED_FIELDS
            .iter()
            .map(|index| object.values.get(*index).cloned().flatten())
            .collect();
        let line = Line {
            id: *id,
            start: s.start(),
            end: s.end(),
            unit: None,
            properties: Properties {
                owner: object.header.owner,
                name: object.header.name.clone(),
                partition: object.header.partition.clone(),
                flags: object.header.flags,
                shared,
            },
        };
        let d = line.end - line.start;
        let length = d.norm();
        if length > Tolerance::DEFAULT.linear {
            lines.push(Line {
                unit: Some(d / length),
                ..line
            });
        }
    }
    refuse_locked(&snapshot, &lines.iter().map(|l| l.id).collect::<Vec<_>>())?;

    let (mut keep, mut erase) = plan(&lines, tol);
    keep.sort_by_key(|(i, _, _)| *i);
    erase.sort_unstable();
    // The changes and the erasures are planned in full, and every refusal is decided, before the first write.
    let mut rewrites: Vec<(ObjectId, Curve2)> = Vec::new();
    let mut changed_ids = Vec::new();
    for (i, start, end) in &keep {
        let line = &lines[*i];
        if (line.start - *start).norm() > Tolerance::DEFAULT.linear
            || (line.end - *end).norm() > Tolerance::DEFAULT.linear
        {
            rewrites.push((line.id, Curve2::Segment(Segment2::new(*start, *end)?)));
            changed_ids.push(line.id);
        }
    }
    let erased: Vec<ObjectId> = erase.iter().map(|i| lines[*i].id).collect();
    super::refuse_dependents(&snapshot, &erased)?;
    for (id, curve) in &rewrites {
        replace_geometry(ctx, *id, curve)?;
    }
    if !erased.is_empty() {
        erase_entities(ctx, &erased)?;
    }
    let mut selection: Vec<ObjectId> = keep.iter().map(|(i, _, _)| lines[*i].id).collect();
    selection.sort_by_key(|id| ids.iter().position(|x| x == id));
    ctx.set_selection(selection)?;
    Ok(Overkill {
        changed: changed_ids.into_iter().map(IdParam).collect(),
        erased: erased.into_iter().map(IdParam).collect(),
    })
}

fn next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    match selection_answer(state, "objects") {
        Some(ids) => done(serde_json::json!({"objects": ids})),
        None => Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        ))),
    }
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.modify.overkill",
            LABEL,
            DESCRIPTION,
            run_overkill,
        )
        .with_prompt(machine(next, crate::draw::create::accept_all)),
    ]
}
