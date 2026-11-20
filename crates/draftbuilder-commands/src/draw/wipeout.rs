//! WIPEOUT (`draftbuilder.draw.wipeout`, typed `WIPEOUT`): a masking polygon, a wipeout entity whose
//! closed boundary hides what is drawn beneath it in draw order. The boundary is either the corner
//! points given (at least three, closed by a straight segment back to the first) or a closed
//! polyline picked in the drawing, which is kept as it is. The source polyline is not changed.
//! Frames are not shown: the wipeout's own frame stays off, and the drawing-wide frame setting is
//! not built.

use arc_base::math::Point2;
use arc_base::{Error, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::{GeometryBytes, Value};
use arc_draft::types::{self, POLYLINE, entity};
use arc_geom::encoding::GeomCodec;
use arc_geom::{Curve2, Polyline2, PolylineVertex};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_END, answers, done, keyword_of, machine, option, point_of,
};
use super::geometry::coincident;
use crate::error::{Message, codes};

/// The fewest corners of a wipeout boundary.
const MIN_CORNERS: usize = 3;
/// The most corners of a wipeout boundary.
const MAX_CORNERS: usize = 10_000;

/// The boundary of a wipeout.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum WipeoutBoundary {
    /// The corners in drawing units, from 3 to 10000; the boundary closes back to the first.
    Points {
        /// The corners, in drawing units.
        points: Vec<[f64; 2]>,
    },
    /// A closed polyline of the drawing.
    Object {
        /// The closed polyline.
        object: IdParam,
    },
}

/// Parameters of `draftbuilder.draw.wipeout`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WipeoutParams {
    /// The boundary of the mask.
    pub boundary: WipeoutBoundary,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-wipeout-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-wipeout-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-wipeout-first",
};
const NEXT: Message = Message {
    message_id: "db-cmd-draw-wipeout-next",
};
const OBJECT: Message = Message {
    message_id: "db-cmd-draw-wipeout-object",
};
const OPTION_POLYLINE: Message = Message {
    message_id: "db-cmd-draw-wipeout-option-polyline",
};

/// The signed area enclosed by a closed polyline, including the circular segments of its bulged
/// segments (a positive bulge adds the segment to the area on its side).
fn enclosed_area(polyline: &Polyline2) -> f64 {
    let vertices = polyline.vertices();
    let mut area = 0.0;
    for (v, w) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
        area += 0.5 * (v.point.x * w.point.y - w.point.x * v.point.y);
        if v.bulge != 0.0 {
            let chord = (w.point - v.point).norm();
            let sweep = 4.0 * v.bulge.abs().atan();
            let radius = chord / (2.0 * (sweep / 2.0).sin());
            let segment = 0.5 * radius * radius * (sweep - sweep.sin());
            area += segment.copysign(v.bulge);
        }
    }
    area
}

/// The boundary as a closed polyline with straight segments through the corners.
fn corner_boundary(points: &[Point2]) -> Result<Curve2> {
    let vertices = points
        .iter()
        .map(|p| PolylineVertex {
            point: *p,
            bulge: 0.0,
            start_width: 0.0,
            end_width: 0.0,
        })
        .collect();
    Ok(Curve2::Polyline(Polyline2::new(vertices, true)?))
}

fn run(ctx: &mut Context<'_>, p: WipeoutParams) -> Result<Created> {
    let boundary = match p.boundary {
        WipeoutBoundary::Points { points } => {
            if !(MIN_CORNERS..=MAX_CORNERS).contains(&points.len())
                || !points.iter().flatten().all(|v| v.is_finite())
            {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "points"));
            }
            let corners: Vec<Point2> = points.iter().map(|[x, y]| Point2::new(*x, *y)).collect();
            corner_boundary(&corners)?
        }
        WipeoutBoundary::Object {
            object: IdParam(id),
        } => {
            let curve = crate::modify::required_curve(ctx.snapshot(), id)?;
            if !matches!(&curve, Curve2::Polyline(polyline) if polyline.is_closed()) {
                return Err(Error::new(codes::GEOMETRY_UNSUPPORTED)
                    .with_detail("reason", "not_closed_polyline")
                    .with_detail("id", id.to_string()));
            }
            curve
        }
    };
    // A boundary with no area masks nothing and is refused; the tolerance is squared, as an area.
    let Curve2::Polyline(polyline) = &boundary else {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "boundary"));
    };
    let linear = Tolerance::DEFAULT.linear;
    if enclosed_area(polyline).abs() <= linear * linear {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "boundary"));
    }
    let mut creator = Creator::new(ctx.snapshot())?;
    let geometry = Value::Geometry(GeometryBytes(boundary.encode()));
    creator.add(
        ctx,
        types::WIPEOUT,
        vec![
            (entity::KIND, geometry),
            (entity::WIPEOUT_FRAME, Value::Bool(false)),
        ],
    )?;
    Ok(creator.finish(ctx))
}

fn points_of(state: &PromptState<'_>) -> Vec<Point2> {
    answers(state)
        .filter_map(|input| point_of(state, input))
        .collect()
}

/// Refuses Enter before three corners and a corner equal to the previous one, as LINE does.
fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    let points = points_of(state);
    if keyword_of(input) == Some("end") && points.len() < MIN_CORNERS {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "points"));
    }
    if let Some(point) = point_of(state, input)
        && points.last().is_some_and(|last| coincident(*last, point))
    {
        return Err(Error::new(codes::POINTS_COLLINEAR));
    }
    Ok(())
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    if let Some(PromptInput::Pick(id)) = state.answer("object") {
        return done(serde_json::json!({"boundary": {"mode": "object", "object": id.to_string()}}));
    }
    let points = points_of(state);
    let finished = answers(state).any(|input| keyword_of(input) == Some("end"));
    if points.is_empty() {
        if answers(state).any(|input| keyword_of(input) == Some("polyline")) {
            return Ok(Step::Ask(Prompt::new(
                "object",
                OBJECT.message_id,
                Accept::Pick {
                    types: vec![POLYLINE.to_owned()],
                },
            )));
        }
        let polyline: KeywordOption = option("polyline", OPTION_POLYLINE, Some('p'));
        return Ok(Step::Ask(
            Prompt::new(
                "point",
                FIRST.message_id,
                Accept::Point {
                    dims: arc_command::PointDims::Two,
                },
            )
            .or(Accept::keywords([polyline])),
        ));
    }
    if finished && points.len() >= MIN_CORNERS {
        let list: Vec<[f64; 2]> = points.iter().map(|p| [p.x, p.y]).collect();
        return done(serde_json::json!({"boundary": {"mode": "points", "points": list}}));
    }
    let end: KeywordOption = option("end", OPTION_END, None);
    Ok(Step::Ask(
        Prompt::new(
            "point",
            NEXT.message_id,
            Accept::Point {
                dims: arc_command::PointDims::Two,
            },
        )
        .or(Accept::keywords([end]))
        .with_default(PromptInput::Keyword("end".to_owned())),
    ))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.wipeout", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
