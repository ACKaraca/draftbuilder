//! STRETCH (`draftbuilder.modify.stretch`, typed `STRETCH`, alias `S`). The entities that lie entirely
//! inside a window move with the displacement. The entities that the window crosses change only at the
//! points inside the window, which move with the displacement; the rest of the geometry stays:
//!
//! - a segment: each end inside the window moves; a segment with both ends inside is one of the entities
//!   that lie wholly inside, and moves as a whole;
//! - a polyline: each vertex inside the window moves; the bulges of its segments are kept;
//! - an arc: one end inside the window moves, and the arc is the circle through the moved end, the
//!   other end and the midpoint of the original arc; both ends inside move with the displacement, and the
//!   arc moves as a whole.
//!
//! Other kinds that the window crosses (circles, texts, dimensions, hatches and the rest) are refused
//! with `db.cmd.stretch_unsupported`: which of their points move is not defined for them. Entities on a
//! locked layer that the window touches are refused with `draft.layer.locked`.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam};
use arc_draft::types::{ARC, LINE, POLYLINE};
use arc_geom::{Arc2, Curve2, Polyline2, Segment2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::transform::{Changed, transform_entities, translation};
use super::{entity_key, refuse_locked, replace_geometry};
use crate::draw::geometry::arc_through;
use crate::error::{Message, codes};
use crate::select::{candidates::Options, methods};

/// Parameters of `draftbuilder.modify.stretch`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StretchParams {
    /// One corner of the window, in drawing units.
    pub first: [f64; 2],
    /// The opposite corner of the window, in drawing units.
    pub second: [f64; 2],
    /// The displacement, in drawing units. Give it, or `base` and `to`.
    #[serde(default)]
    pub displacement: Option<[f64; 2]>,
    /// The base point, in drawing units.
    #[serde(default)]
    pub base: Option<[f64; 2]>,
    /// The second point, in drawing units; the displacement is `to − base`.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-modify-stretch-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-stretch-description",
};

/// Whether `p` lies in the window with the corners `a` and `b` (on its boundary counts).
fn inside(window: [Point2; 2], p: Point2) -> bool {
    let (lo_x, hi_x) = (window[0].x.min(window[1].x), window[0].x.max(window[1].x));
    let (lo_y, hi_y) = (window[0].y.min(window[1].y), window[0].y.max(window[1].y));
    p.x >= lo_x && p.x <= hi_x && p.y >= lo_y && p.y <= hi_y
}

/// The new geometry of a crossed entity, or `None` when none of its points is inside the window.
fn stretched(key: &str, curve: &Curve2, window: [Point2; 2], d: Point2) -> Result<Option<Curve2>> {
    let moved = |p: Point2| {
        if inside(window, p) {
            Point2::new(p.x + d.x, p.y + d.y)
        } else {
            p
        }
    };
    match (key, curve) {
        (LINE, Curve2::Segment(s)) => {
            let (start, end) = (s.start(), s.end());
            if !inside(window, start) && !inside(window, end) {
                return Ok(None);
            }
            Ok(Some(Curve2::Segment(Segment2::new(
                moved(start),
                moved(end),
            )?)))
        }
        (POLYLINE, Curve2::Polyline(p)) => {
            let mut vertices = p.vertices().to_vec();
            let mut changed = false;
            for vertex in &mut vertices {
                if inside(window, vertex.point) {
                    vertex.point = moved(vertex.point);
                    changed = true;
                }
            }
            if !changed {
                return Ok(None);
            }
            Ok(Some(Curve2::Polyline(Polyline2::new(
                vertices,
                p.is_closed(),
            )?)))
        }
        (ARC, Curve2::Arc(a)) => {
            let start = Point2::new(
                a.center().x + a.radius() * a.start_angle().cos(),
                a.center().y + a.radius() * a.start_angle().sin(),
            );
            let end_angle = a.start_angle() + a.sweep();
            let end = Point2::new(
                a.center().x + a.radius() * end_angle.cos(),
                a.center().y + a.radius() * end_angle.sin(),
            );
            let (in_start, in_end) = (inside(window, start), inside(window, end));
            if !in_start && !in_end {
                return Ok(None);
            }
            if in_start && in_end {
                // Both ends move with the displacement: the arc moves as a whole.
                let center = Point2::new(a.center().x + d.x, a.center().y + d.y);
                return Ok(Some(Curve2::Arc(Arc2::new(
                    center,
                    a.radius(),
                    a.start_angle(),
                    a.sweep(),
                )?)));
            }
            let middle_angle = a.start_angle() + 0.5 * a.sweep();
            let middle = Point2::new(
                a.center().x + a.radius() * middle_angle.cos(),
                a.center().y + a.radius() * middle_angle.sin(),
            );
            let new_start = moved(start);
            let new_end = moved(end);
            let arc = arc_through(new_start, middle, new_end)?;
            Ok(Some(Curve2::Arc(arc)))
        }
        _ => Err(Error::new(codes::STRETCH_UNSUPPORTED).with_detail("kind", key.to_owned())),
    }
}

fn run(ctx: &mut Context<'_>, p: StretchParams) -> Result<Changed> {
    let displacement = match (p.displacement, p.base, p.to) {
        (Some(d), None, None) => d,
        (None, Some(b), Some(t)) => [t[0] - b[0], t[1] - b[1]],
        _ => return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "displacement")),
    };
    let snapshot = ctx.snapshot().clone();
    let space = crate::model_space(&snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let window = [
        Point2::new(p.first[0], p.first[1]),
        Point2::new(p.second[0], p.second[1]),
    ];
    let options = Options {
        include_locked: false,
    };
    let whole = methods::select_window(&snapshot, space, window, &options)?.ids;
    let crossed = methods::select_crossing(&snapshot, space, window, &options)?.ids;
    let partly: Vec<ObjectId> = crossed
        .into_iter()
        .filter(|id| !whole.contains(id))
        .collect();
    refuse_locked(&snapshot, &partly)?;
    let mut planned: Vec<(ObjectId, Curve2)> = Vec::new();
    for id in &partly {
        let key = entity_key(&snapshot, *id)?;
        let curve = super::required_curve(&snapshot, *id)?;
        if let Some(new) = stretched(
            &key,
            &curve,
            window,
            Point2::new(displacement[0], displacement[1]),
        )? {
            planned.push((*id, new));
        }
    }
    refuse_locked(&snapshot, &whole)?;
    transform_entities(ctx, &whole, translation(displacement[0], displacement[1]))?;
    for (id, curve) in &planned {
        replace_geometry(ctx, *id, curve)?;
    }
    let mut out = whole;
    out.extend(partly);
    ctx.set_selection(out.clone())?;
    Ok(Changed {
        objects: out.into_iter().map(IdParam).collect(),
    })
}

const FIRST: Message = Message {
    message_id: "db-cmd-modify-stretch-first",
};
const SECOND: Message = Message {
    message_id: "db-cmd-modify-stretch-second",
};
const BASE: Message = Message {
    message_id: "db-cmd-modify-stretch-base",
};
const TO: Message = Message {
    message_id: "db-cmd-modify-stretch-to",
};

/// The prompt machine: the crossing window, then the base point and the second point.
fn next(state: &arc_command::PromptState<'_>) -> arc_base::Result<arc_command::Step> {
    use arc_command::{Prompt, Step};
    let Some(first) = super::point_answer(state, "first") else {
        return Ok(Step::Ask(Prompt::new(
            "first",
            FIRST.message_id,
            super::point(),
        )));
    };
    let Some(second) = super::point_answer(state, "second") else {
        return Ok(Step::Ask(Prompt::new(
            "second",
            SECOND.message_id,
            super::point(),
        )));
    };
    let Some(base) = super::point_answer(state, "base") else {
        return Ok(Step::Ask(Prompt::new(
            "base",
            BASE.message_id,
            super::point(),
        )));
    };
    let Some(to) = super::point_answer(state, "to") else {
        return Ok(Step::Ask(Prompt::new("to", TO.message_id, super::point())));
    };
    crate::draw::create::done(serde_json::json!({
        "first": first, "second": second, "base": base, "to": to
    }))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.modify.stretch", LABEL, DESCRIPTION, run).with_prompt(
            crate::draw::create::machine(next, crate::draw::create::accept_all),
        ),
    ]
}
