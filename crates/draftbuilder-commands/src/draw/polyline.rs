//! PLINE (`draftbuilder.draw.polyline`, typed `PLINE`): one lightweight polyline of line and arc
//! segments, with a start and an end width per segment.
//!
//! Arc segments are given by bulges in the typed form. The prompt form draws an arc from the
//! previous point to the next one by one of the methods second point (`second`), center
//! (`center`), included angle (`angle`), radius (`radius`) and direction (`direction`); a point in
//! arc mode without a method continues tangentially from the previous segment. `arc` and `line`
//! switch the mode, `width` sets the widths of the segments that follow, `undo` removes the last
//! vertex, `close` closes the polyline with a straight segment, and `end` (Enter) finishes it.
//! The length continuation option is not built.

use arc_base::Error;
use arc_base::Result;
use arc_base::math::{Point2, Vector2};
use arc_command::{
    Accept, CommandSpec, Context, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, Polyline2, PolylineVertex};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_ANGLE, OPTION_ARC, OPTION_CENTER, OPTION_CLOSE, OPTION_DIRECTION,
    OPTION_END, OPTION_LINE, OPTION_RADIUS, OPTION_SECOND, OPTION_UNDO, OPTION_WIDTH, answers,
    done, from_meters, keyword_of, machine, option, point_of,
};
use super::geometry::{
    bulge_angle, bulge_center, bulge_direction, bulge_radius, bulge_through, coincident,
    end_tangent,
};
use crate::error::{Message, codes};

/// One vertex of a polyline: its point, the bulge and widths of the segment that starts there.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolylineVertexParams {
    /// The point, in drawing units.
    pub point: [f64; 2],
    /// The bulge of the segment that starts here: 0 is straight, the tangent of a quarter of the
    /// included angle otherwise (positive counterclockwise).
    #[serde(default)]
    pub bulge: f64,
    /// The width at the start of the segment that starts here, in drawing units.
    #[serde(default)]
    pub start_width: f64,
    /// The width at the end of the segment that starts here, in drawing units.
    #[serde(default)]
    pub end_width: f64,
}

/// Parameters of `draftbuilder.draw.polyline`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolylineParams {
    /// The vertices in order; the last one starts no segment unless `close` is set.
    #[schemars(length(min = 2, max = 10000))]
    pub vertices: Vec<PolylineVertexParams>,
    /// Whether the last vertex is joined back to the first.
    #[serde(default)]
    pub close: bool,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-polyline-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-polyline-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-polyline-first",
};
const NEXT: Message = Message {
    message_id: "db-cmd-draw-polyline-next",
};
const END: Message = Message {
    message_id: "db-cmd-draw-polyline-end",
};
const SECOND: Message = Message {
    message_id: "db-cmd-draw-polyline-second",
};
const CENTER: Message = Message {
    message_id: "db-cmd-draw-polyline-center",
};
const DIRECTION: Message = Message {
    message_id: "db-cmd-draw-polyline-direction",
};
const RADIUS: Message = Message {
    message_id: "db-cmd-draw-polyline-radius",
};
const INCLUDED: Message = Message {
    message_id: "db-cmd-draw-polyline-included",
};
const START_WIDTH: Message = Message {
    message_id: "db-cmd-draw-polyline-start-width",
};
const END_WIDTH: Message = Message {
    message_id: "db-cmd-draw-polyline-end-width",
};

/// A vertex of the path being drawn.
#[derive(Debug, Clone, Copy)]
struct Vertex {
    point: Point2,
    bulge: f64,
    start_width: f64,
    end_width: f64,
}

/// The arc method whose parameter is asked next.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Second,
    Center,
    Angle,
    Radius,
    Direction,
}

/// The parameter of an arc method, once answered.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Param {
    Second(Point2),
    Center(Point2),
    Angle(f64),
    Radius(f64),
    Direction(f64),
}

/// What the next answer is for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    Idle,
    Param(Kind),
    End(Param),
    WidthStart,
    WidthEnd(f64),
}

/// The path after the answers so far.
struct Path {
    vertices: Vec<Vertex>,
    arc: bool,
    width: (f64, f64),
    stage: Stage,
    closed: bool,
    finished: bool,
}

fn bad_value(field: &str) -> Error {
    Error::new(codes::VALUE_INVALID).with_detail("field", field.to_owned())
}

fn kind_of(keyword: &str) -> Option<Kind> {
    match keyword {
        "second" => Some(Kind::Second),
        "center" => Some(Kind::Center),
        "angle" => Some(Kind::Angle),
        "radius" => Some(Kind::Radius),
        "direction" => Some(Kind::Direction),
        _ => None,
    }
}

fn distance_of(state: &PromptState<'_>, input: &PromptInput) -> Option<f64> {
    match input {
        PromptInput::Distance(meters) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

fn angle_of(input: &PromptInput) -> Option<f64> {
    match input {
        PromptInput::Angle(radians) => Some(*radians),
        _ => None,
    }
}

fn param_of(kind: Kind, state: &PromptState<'_>, input: &PromptInput) -> Result<Param> {
    let parsed = match kind {
        Kind::Second => point_of(state, input).map(Param::Second),
        Kind::Center => point_of(state, input).map(Param::Center),
        Kind::Angle => angle_of(input).map(Param::Angle),
        Kind::Radius => distance_of(state, input).map(Param::Radius),
        Kind::Direction => angle_of(input).map(Param::Direction),
    };
    parsed.ok_or_else(|| bad_value("parameter"))
}

/// The bulge of the arc from `from` to `to` by an arc method.
fn bulge_for(param: Param, from: Point2, to: Point2) -> Result<f64> {
    match param {
        Param::Second(second) => bulge_through(from, second, to),
        Param::Center(center) => bulge_center(from, center, to),
        Param::Angle(angle) => bulge_angle(from, to, angle),
        Param::Radius(radius) => bulge_radius(from, to, radius),
        Param::Direction(angle) => {
            bulge_direction(from, Vector2::new(angle.cos(), angle.sin()), to)
        }
    }
}

/// The bulge of the arc that continues tangentially from the last segment to `to`.
fn tangent_bulge(vertices: &[Vertex], to: Point2) -> Result<f64> {
    match vertices {
        [.., before, last] => {
            let direction = end_tangent(before.point, before.bulge, last.point);
            bulge_direction(last.point, direction, to)
        }
        _ => Err(bad_value("tangent")),
    }
}

/// Adds a vertex at `to`; the segment from the last vertex takes `param`'s arc, or the tangent
/// arc in arc mode without a parameter, or a line.
fn push_point(path: &mut Path, to: Point2, param: Option<Param>) -> Result<()> {
    let Some(last) = path.vertices.last().copied() else {
        path.vertices.push(Vertex {
            point: to,
            bulge: 0.0,
            start_width: 0.0,
            end_width: 0.0,
        });
        return Ok(());
    };
    let bulge = match param {
        Some(param) => bulge_for(param, last.point, to)?,
        None if path.arc => tangent_bulge(&path.vertices, to)?,
        None => 0.0,
    };
    let index = path.vertices.len() - 1;
    path.vertices[index].bulge = bulge;
    path.vertices[index].start_width = path.width.0;
    path.vertices[index].end_width = path.width.1;
    path.vertices.push(Vertex {
        point: to,
        bulge: 0.0,
        start_width: 0.0,
        end_width: 0.0,
    });
    Ok(())
}

/// Removes the last vertex; the new last vertex starts no segment.
fn undo(path: &mut Path) {
    path.vertices.pop();
    if let Some(last) = path.vertices.last_mut() {
        last.bulge = 0.0;
        last.start_width = 0.0;
        last.end_width = 0.0;
    }
}

fn idle(path: &mut Path, state: &PromptState<'_>, input: &PromptInput) -> Result<()> {
    if let Some(to) = point_of(state, input) {
        return push_point(path, to, None);
    }
    let Some(id) = keyword_of(input) else {
        return Ok(());
    };
    if let Some(kind) = kind_of(id) {
        path.arc = true;
        path.stage = Stage::Param(kind);
        return Ok(());
    }
    match id {
        "arc" => path.arc = true,
        "line" => path.arc = false,
        "width" => path.stage = Stage::WidthStart,
        "undo" => undo(path),
        "close" => {
            path.closed = true;
            path.finished = true;
        }
        "end" => path.finished = true,
        _ => {}
    }
    Ok(())
}

fn apply(path: &mut Path, state: &PromptState<'_>, input: &PromptInput) -> Result<()> {
    match path.stage {
        Stage::Param(kind) => path.stage = Stage::End(param_of(kind, state, input)?),
        Stage::End(param) => {
            let to = point_of(state, input).ok_or_else(|| bad_value("point"))?;
            push_point(path, to, Some(param))?;
            path.stage = Stage::Idle;
        }
        Stage::WidthStart => {
            let start = distance_of(state, input).ok_or_else(|| bad_value("width"))?;
            path.stage = Stage::WidthEnd(start);
        }
        Stage::WidthEnd(start) => {
            let end = distance_of(state, input).ok_or_else(|| bad_value("width"))?;
            path.width = (start, end);
            path.stage = Stage::Idle;
        }
        Stage::Idle => idle(path, state, input)?,
    }
    Ok(())
}

/// The path after the answers, replayed in order.
fn replay(state: &PromptState<'_>) -> Result<Path> {
    let mut path = Path {
        vertices: Vec::new(),
        arc: false,
        width: (0.0, 0.0),
        stage: Stage::Idle,
        closed: false,
        finished: false,
    };
    for input in answers(state) {
        apply(&mut path, state, input)?;
    }
    Ok(path)
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

fn param_prompt(kind: Kind) -> (Message, Accept) {
    match kind {
        Kind::Second => (SECOND, point_accept()),
        Kind::Center => (CENTER, point_accept()),
        Kind::Angle => (INCLUDED, Accept::angle()),
        Kind::Radius => (RADIUS, Accept::distance()),
        Kind::Direction => (DIRECTION, Accept::angle()),
    }
}

fn idle_prompt(path: &Path, key: String) -> Step {
    if path.vertices.is_empty() {
        return Step::Ask(Prompt::new(key, FIRST.message_id, point_accept()));
    }
    let mut options = Vec::new();
    if path.arc {
        options.push(option("line", OPTION_LINE, Some('l')));
        options.push(option("angle", OPTION_ANGLE, Some('a')));
        options.push(option("center", OPTION_CENTER, Some('e')));
        options.push(option("direction", OPTION_DIRECTION, Some('d')));
        options.push(option("radius", OPTION_RADIUS, Some('r')));
        options.push(option("second", OPTION_SECOND, Some('s')));
    } else {
        options.push(option("arc", OPTION_ARC, Some('a')));
    }
    options.push(option("width", OPTION_WIDTH, Some('w')));
    options.push(option("undo", OPTION_UNDO, Some('u')));
    if path.vertices.len() >= 2 {
        options.push(option("close", OPTION_CLOSE, Some('c')));
    }
    options.push(option("end", OPTION_END, None));
    let mut prompt =
        Prompt::new(key, NEXT.message_id, point_accept()).or(Accept::keywords(options));
    if path.vertices.len() >= 2 {
        prompt = prompt.with_default(PromptInput::Keyword("end".to_owned()));
    }
    Step::Ask(prompt)
}

fn vertex_json(vertex: &Vertex) -> serde_json::Value {
    serde_json::json!({
        "point": [vertex.point.x, vertex.point.y],
        "bulge": vertex.bulge,
        "start_width": vertex.start_width,
        "end_width": vertex.end_width,
    })
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let path = replay(state)?;
    if path.finished {
        let vertices: Vec<serde_json::Value> = path.vertices.iter().map(vertex_json).collect();
        return done(serde_json::json!({"vertices": vertices, "close": path.closed}));
    }
    let key = format!("answer{}", state.answers.len());
    Ok(match path.stage {
        Stage::Param(kind) => {
            let (message, accept) = param_prompt(kind);
            Step::Ask(Prompt::new(key, message.message_id, accept))
        }
        Stage::End(_) => Step::Ask(Prompt::new(key, END.message_id, point_accept())),
        Stage::WidthStart => {
            Step::Ask(Prompt::new(key, START_WIDTH.message_id, Accept::distance()))
        }
        Stage::WidthEnd(_) => Step::Ask(Prompt::new(key, END_WIDTH.message_id, Accept::distance())),
        Stage::Idle => idle_prompt(&path, key),
    })
}

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    let path = replay(state)?;
    match path.stage {
        Stage::Param(kind) => param_of(kind, state, input).map(|_| ()),
        Stage::End(param) => {
            let to = point_of(state, input).ok_or_else(|| bad_value("point"))?;
            let Some(last) = path.vertices.last() else {
                return Ok(());
            };
            if coincident(last.point, to) {
                return Err(Error::new(codes::POINTS_COLLINEAR));
            }
            bulge_for(param, last.point, to).map(|_| ())
        }
        Stage::WidthStart | Stage::WidthEnd(_) => match distance_of(state, input) {
            Some(width) if width >= 0.0 => Ok(()),
            _ => Err(bad_value("width")),
        },
        Stage::Idle => idle_check(&path, state, input),
    }
}

fn idle_check(path: &Path, state: &PromptState<'_>, input: &PromptInput) -> Result<()> {
    if let Some(to) = point_of(state, input) {
        if path
            .vertices
            .last()
            .is_some_and(|last| coincident(last.point, to))
        {
            return Err(Error::new(codes::POINTS_COLLINEAR));
        }
        if path.arc {
            tangent_bulge(&path.vertices, to)?;
        }
        return Ok(());
    }
    match keyword_of(input) {
        Some("undo") if path.vertices.is_empty() => Err(Error::new(codes::NOTHING_TO_UNDO)),
        Some("close" | "end") if path.vertices.len() < 2 => Err(bad_value("vertices")),
        _ => Ok(()),
    }
}

fn run(ctx: &mut Context<'_>, p: PolylineParams) -> Result<Created> {
    let vertices = p
        .vertices
        .iter()
        .map(|v| PolylineVertex {
            point: Point2::new(v.point[0], v.point[1]),
            bulge: v.bulge,
            start_width: v.start_width,
            end_width: v.end_width,
        })
        .collect();
    let polyline = Polyline2::new(vertices, p.close)?;
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.curve(ctx, &Curve2::Polyline(polyline))?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.polyline", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
