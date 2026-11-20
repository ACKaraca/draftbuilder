//! LINE (`draftbuilder.draw.line`, typed `LINE`): a chain of independent line segments, each its
//! own entity, created in one undo step. Options: `undo` removes the last segment, `close` joins
//! the last point to the first, and `end` (Enter) finishes the chain.

use arc_base::Result;
use arc_base::math::Point2;
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, Segment2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_CLOSE, OPTION_END, OPTION_UNDO, answers, done, keyword_of, machine,
    option, point_of,
};
use super::geometry::coincident;
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.draw.line`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineParams {
    /// The points in drawing units, two or more; each consecutive pair makes one line.
    #[schemars(length(min = 2, max = 10000))]
    pub points: Vec<[f64; 2]>,
    /// Whether a last line joins the last point back to the first.
    #[serde(default)]
    pub close: bool,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-line-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-line-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-line-first",
};
const NEXT: Message = Message {
    message_id: "db-cmd-draw-line-next",
};

/// The keyword options of a line: undo, close and finish.
fn options(points: usize) -> Vec<KeywordOption> {
    let mut out = vec![option("undo", OPTION_UNDO, Some('u'))];
    if points >= 2 {
        out.push(option("close", OPTION_CLOSE, Some('c')));
    }
    out.push(option("end", OPTION_END, None));
    out
}

/// The points of the chain after the answers, and whether it ended with close or finish.
struct Chain {
    points: Vec<Point2>,
    closed: bool,
    finished: bool,
}

fn chain(state: &PromptState<'_>) -> Chain {
    let mut out = Chain {
        points: Vec::new(),
        closed: false,
        finished: false,
    };
    for input in answers(state) {
        if let Some(point) = point_of(state, input) {
            out.points.push(point);
            continue;
        }
        match keyword_of(input) {
            Some("undo") => {
                out.points.pop();
            }
            Some("close") => {
                out.closed = true;
                out.finished = true;
            }
            Some("end") => out.finished = true,
            _ => {}
        }
    }
    out
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let chain = chain(state);
    if chain.finished {
        let points: Vec<[f64; 2]> = chain.points.iter().map(|p| [p.x, p.y]).collect();
        return done(serde_json::json!({"points": points, "close": chain.closed}));
    }
    let prompt = if chain.points.is_empty() {
        Prompt::new("point", FIRST.message_id, point_accept())
    } else {
        Prompt::new("point", NEXT.message_id, point_accept())
            .or(Accept::keywords(options(chain.points.len())))
            .with_default(PromptInput::Keyword("end".to_owned()))
    };
    Ok(Step::Ask(prompt))
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    let chain = chain(state);
    if let Some(point) = point_of(state, input) {
        if chain
            .points
            .last()
            .is_some_and(|last| coincident(*last, point))
        {
            return Err(arc_base::Error::new(codes::POINTS_COLLINEAR));
        }
        return Ok(());
    }
    match keyword_of(input) {
        Some("undo") if chain.points.is_empty() => {
            Err(arc_base::Error::new(codes::NOTHING_TO_UNDO))
        }
        Some("close" | "end") if chain.points.len() < 2 => {
            Err(arc_base::Error::new(codes::VALUE_INVALID).with_detail("field", "points"))
        }
        _ => Ok(()),
    }
}

fn run(ctx: &mut Context<'_>, p: LineParams) -> Result<Created> {
    let mut points: Vec<Point2> = p.points.iter().map(|q| Point2::new(q[0], q[1])).collect();
    if p.close
        && let Some(first) = points.first().copied()
    {
        points.push(first);
    }
    let segments = points
        .windows(2)
        .map(|pair| Segment2::new(pair[0], pair[1]).map(Curve2::Segment))
        .collect::<Result<Vec<_>>>()?;
    let mut creator = Creator::new(ctx.snapshot())?;
    for segment in &segments {
        creator.curve(ctx, segment)?;
    }
    Ok(creator.finish(ctx))
}

/// The command of the area.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.line", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
