//! Construction lines and rays: XLINE (`draftbuilder.draw.xline`, typed `XLINE`), infinite lines
//! through a point, and RAY (`draftbuilder.draw.ray`, typed `RAY`), semi-infinite lines from one
//! start point through successive points.
//!
//! XLINE supports lines through two points, horizontal and vertical lines through a point, and
//! lines at an angle through a point. The options bisector and offset need a picked line or angle
//! and are not built. Lines are created in one undo step per command.

use arc_base::Result;
use arc_base::math::Point2;
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, Line2, Ray2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_ANGLE, OPTION_END, OPTION_HORIZONTAL, OPTION_UNDO, OPTION_VERTICAL,
    answers, done, keyword_of, machine, option, point_of,
};
use super::geometry::coincident;
use crate::error::{Message, codes};

/// One construction line of `draftbuilder.draw.xline`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum XlineSpec {
    /// An infinite line through two points.
    Through {
        /// The first point, in drawing units.
        point: [f64; 2],
        /// The second point, in drawing units; not equal to the first.
        through: [f64; 2],
    },
    /// A horizontal line through a point.
    Horizontal {
        /// The point, in drawing units.
        point: [f64; 2],
    },
    /// A vertical line through a point.
    Vertical {
        /// The point, in drawing units.
        point: [f64; 2],
    },
    /// A line through a point at an angle from the positive x axis, in radians.
    Angle {
        /// The point, in drawing units.
        point: [f64; 2],
        /// The angle in radians, counterclockwise.
        angle: f64,
    },
}

/// Parameters of `draftbuilder.draw.xline`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct XlineParams {
    /// The construction lines, in order.
    #[schemars(length(min = 1, max = 10000))]
    pub lines: Vec<XlineSpec>,
}

/// Parameters of `draftbuilder.draw.ray`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RayParams {
    /// The start point, in drawing units.
    pub start: [f64; 2],
    /// The points each ray passes through, in drawing units; one ray per point.
    #[schemars(length(min = 1, max = 10000))]
    pub through: Vec<[f64; 2]>,
}

const XLINE_LABEL: Message = Message {
    message_id: "db-cmd-draw-xline-label",
};
const XLINE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-xline-description",
};
const XLINE_FIRST: Message = Message {
    message_id: "db-cmd-draw-xline-first",
};
const XLINE_NEXT: Message = Message {
    message_id: "db-cmd-draw-xline-next",
};
const XLINE_ANGLE: Message = Message {
    message_id: "db-cmd-draw-xline-angle",
};
const RAY_LABEL: Message = Message {
    message_id: "db-cmd-draw-ray-label",
};
const RAY_DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-ray-description",
};
const RAY_FIRST: Message = Message {
    message_id: "db-cmd-draw-ray-first",
};
const RAY_NEXT: Message = Message {
    message_id: "db-cmd-draw-ray-next",
};

fn json_point(p: Point2) -> serde_json::Value {
    serde_json::json!([p.x, p.y])
}

/// The state of an XLINE prompt path: the base point of the through lines, the sticky mode the
/// point answers take, and the lines so far as their typed form.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sticky {
    Through,
    Horizontal,
    Vertical,
    Angle(f64),
}

struct Xlines {
    base: Option<Point2>,
    sticky: Sticky,
    pending_angle: bool,
    lines: Vec<serde_json::Value>,
    finished: bool,
}

fn xlines(state: &PromptState<'_>) -> Xlines {
    let mut out = Xlines {
        base: None,
        sticky: Sticky::Through,
        pending_angle: false,
        lines: Vec::new(),
        finished: false,
    };
    for input in answers(state) {
        if out.pending_angle {
            if let PromptInput::Angle(angle) = input {
                out.sticky = Sticky::Angle(*angle);
                out.pending_angle = false;
            }
            continue;
        }
        if let Some(point) = point_of(state, input) {
            apply_point(&mut out, point);
            continue;
        }
        match keyword_of(input) {
            Some("hor") => out.sticky = Sticky::Horizontal,
            Some("ver") => out.sticky = Sticky::Vertical,
            Some("ang") => out.pending_angle = true,
            Some("undo") => {
                out.lines.pop();
            }
            Some("end") => out.finished = true,
            _ => {}
        }
    }
    out
}

fn apply_point(out: &mut Xlines, point: Point2) {
    match out.sticky {
        Sticky::Through => match out.base {
            None => out.base = Some(point),
            Some(base) => out.lines.push(serde_json::json!({
                "kind": "through", "point": json_point(base), "through": json_point(point)
            })),
        },
        Sticky::Horizontal => out
            .lines
            .push(serde_json::json!({"kind": "horizontal", "point": json_point(point)})),
        Sticky::Vertical => out
            .lines
            .push(serde_json::json!({"kind": "vertical", "point": json_point(point)})),
        Sticky::Angle(angle) => out.lines.push(serde_json::json!({
            "kind": "angle", "point": json_point(point), "angle": angle
        })),
    }
}

fn xline_next(state: &PromptState<'_>) -> Result<Step> {
    let xl = xlines(state);
    if xl.finished {
        return done(serde_json::json!({"lines": xl.lines}));
    }
    if xl.pending_angle {
        let prompt = Prompt::new("angle", XLINE_ANGLE.message_id, Accept::angle())
            .with_default(PromptInput::Angle(0.0));
        return Ok(Step::Ask(prompt));
    }
    let options = vec![
        option("hor", OPTION_HORIZONTAL, Some('h')),
        option("ver", OPTION_VERTICAL, Some('v')),
        option("ang", OPTION_ANGLE, Some('a')),
    ];
    let mut prompt = if xl.base.is_none() && xl.sticky == Sticky::Through {
        Prompt::new("point", XLINE_FIRST.message_id, point_accept())
    } else {
        Prompt::new("point", XLINE_NEXT.message_id, point_accept())
    };
    prompt = prompt.or(Accept::keywords(with_undo(options, !xl.lines.is_empty())));
    if xl.base.is_some() || xl.sticky != Sticky::Through {
        prompt = prompt.with_default(PromptInput::Keyword("end".to_owned()));
    }
    Ok(Step::Ask(prompt))
}

/// The keyword options with undo when there is something to undo, and finish.
fn with_undo(mut options: Vec<KeywordOption>, undoable: bool) -> Vec<KeywordOption> {
    if undoable {
        options.push(option("undo", OPTION_UNDO, Some('u')));
    }
    options.push(option("end", OPTION_END, None));
    options
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

fn xline_check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    let xl = xlines(state);
    if let Some(point) = point_of(state, input) {
        if let (Sticky::Through, Some(base)) = (xl.sticky, xl.base)
            && coincident(base, point)
        {
            return Err(arc_base::Error::new(codes::POINTS_COLLINEAR));
        }
        return Ok(());
    }
    match keyword_of(input) {
        Some("undo") if xl.lines.is_empty() => Err(arc_base::Error::new(codes::NOTHING_TO_UNDO)),
        _ => Ok(()),
    }
}

fn xline_curve(spec: XlineSpec) -> Result<Curve2> {
    let point = |p: [f64; 2]| Point2::new(p[0], p[1]);
    match spec {
        XlineSpec::Through {
            point: a,
            through: b,
        } => Ok(Curve2::Line(Line2::new(point(a), point(b) - point(a))?)),
        XlineSpec::Horizontal { point: a } => Ok(Curve2::Line(Line2::new(
            point(a),
            arc_base::math::Vector2::new(1.0, 0.0),
        )?)),
        XlineSpec::Vertical { point: a } => Ok(Curve2::Line(Line2::new(
            point(a),
            arc_base::math::Vector2::new(0.0, 1.0),
        )?)),
        XlineSpec::Angle { point: a, angle } => Ok(Curve2::Line(Line2::new(
            point(a),
            arc_base::math::Vector2::new(angle.cos(), angle.sin()),
        )?)),
    }
}

fn xline_run(ctx: &mut Context<'_>, p: XlineParams) -> Result<Created> {
    let curves = p
        .lines
        .into_iter()
        .map(xline_curve)
        .collect::<Result<Vec<_>>>()?;
    let mut creator = Creator::new(ctx.snapshot())?;
    for curve in &curves {
        creator.curve(ctx, curve)?;
    }
    Ok(creator.finish(ctx))
}

/// The rays of a start point and its through points, in order.
struct Rays {
    start: Option<Point2>,
    through: Vec<Point2>,
    finished: bool,
}

fn rays(state: &PromptState<'_>) -> Rays {
    let mut out = Rays {
        start: None,
        through: Vec::new(),
        finished: false,
    };
    for input in answers(state) {
        if let Some(point) = point_of(state, input) {
            if out.start.is_none() {
                out.start = Some(point);
            } else {
                out.through.push(point);
            }
            continue;
        }
        match keyword_of(input) {
            Some("undo") => {
                out.through.pop();
            }
            Some("end") => out.finished = true,
            _ => {}
        }
    }
    out
}

fn ray_next(state: &PromptState<'_>) -> Result<Step> {
    let rays = rays(state);
    if rays.finished {
        let start = rays.start.map_or([0.0, 0.0], |p| [p.x, p.y]);
        let through: Vec<[f64; 2]> = rays.through.iter().map(|p| [p.x, p.y]).collect();
        return done(serde_json::json!({"start": start, "through": through}));
    }
    let prompt = match rays.start {
        None => Prompt::new("point", RAY_FIRST.message_id, point_accept()),
        Some(_) => Prompt::new("point", RAY_NEXT.message_id, point_accept())
            .or(Accept::keywords(with_undo(
                Vec::new(),
                !rays.through.is_empty(),
            )))
            .with_default(PromptInput::Keyword("end".to_owned())),
    };
    Ok(Step::Ask(prompt))
}

fn ray_check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    let rays = rays(state);
    if let Some(point) = point_of(state, input) {
        if rays.start.is_some_and(|start| coincident(start, point)) {
            return Err(arc_base::Error::new(codes::POINTS_COLLINEAR));
        }
        return Ok(());
    }
    match keyword_of(input) {
        Some("undo") if rays.through.is_empty() => {
            Err(arc_base::Error::new(codes::NOTHING_TO_UNDO))
        }
        Some("end") if rays.through.is_empty() => {
            Err(arc_base::Error::new(codes::VALUE_INVALID).with_detail("field", "through"))
        }
        _ => Ok(()),
    }
}

fn ray_run(ctx: &mut Context<'_>, p: RayParams) -> Result<Created> {
    let start = Point2::new(p.start[0], p.start[1]);
    let curves = p
        .through
        .iter()
        .map(|t| {
            let through = Point2::new(t[0], t[1]);
            Ray2::new(start, through - start).map(Curve2::Ray)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut creator = Creator::new(ctx.snapshot())?;
    for curve in &curves {
        creator.curve(ctx, curve)?;
    }
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.draw.xline",
            XLINE_LABEL,
            XLINE_DESCRIPTION,
            xline_run,
        )
        .with_prompt(machine(xline_next, xline_check)),
        super::spec("draftbuilder.draw.ray", RAY_LABEL, RAY_DESCRIPTION, ray_run)
            .with_prompt(machine(ray_next, ray_check)),
    ]
}
