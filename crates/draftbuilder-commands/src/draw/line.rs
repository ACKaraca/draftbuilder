//! LINE (`draftbuilder.draw.line`, typed `LINE`): a chain of independent line segments, each its
//! own entity, created in one undo step. Options: `undo` removes the last segment, `close` joins
//! the last point to the first, and `end` (Enter) finishes the chain. `continue` (the first prompt,
//! or `continue_last` in the typed form) starts the chain at the end of the last line or arc of the
//! model space, the one with the highest draw order.

use arc_base::math::{Point2, Vector2};
use arc_base::{Error, ObjectId, Result};
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::{Snapshot, Value};
use arc_draft::types::{self, entity};
use arc_geom::{Curve2, CurveEval, Segment2};
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
    #[schemars(length(min = 1, max = 10000))]
    pub points: Vec<[f64; 2]>,
    /// Whether a last line joins the last point back to the first.
    #[serde(default)]
    pub close: bool,
    /// Whether the chain starts at the end of the last line or arc of the drawing; the points are
    /// the ones after that start.
    #[serde(default)]
    pub continue_last: bool,
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
const FIRST_CONTINUE: Message = Message {
    message_id: "db-cmd-draw-line-first-continue",
};
const OPTION_CONTINUE: Message = Message {
    message_id: "db-cmd-draw-line-option-continue",
};

/// The identifier at the top of the draw order: the largest draw-order key, and on a tie the largest
/// identifier. Identifiers are time-ordered (UUIDv7 from one generator), so the tie goes to the
/// newest entity.
pub(crate) fn top_of_draw_order(
    candidates: impl IntoIterator<Item = (i64, ObjectId)>,
) -> Option<ObjectId> {
    candidates.into_iter().max().map(|(_, id)| id)
}

/// The most points of one LINE run, the limit the other commands of the area use for lists.
const MAX_POINTS: usize = 10_000;

/// The end point and the direction of travel there of the last line or arc of the model space: the
/// one at the top of the draw order, the newest on a tie (see [`top_of_draw_order`]). `None` when
/// the space has neither.
///
/// # Errors
///
/// The decoding errors of the stored geometry.
pub(crate) fn last_continuation(snapshot: &Snapshot) -> Result<Option<(Point2, Vector2)>> {
    let Some(space) = crate::model_space(snapshot) else {
        return Ok(None);
    };
    let mut candidates: Vec<(i64, ObjectId)> = Vec::new();
    for key in [types::LINE, types::ARC] {
        let Ok(iter) = snapshot.iter_type(key) else {
            continue;
        };
        for (_, object) in iter {
            if object.header.owner != Some(space) {
                continue;
            }
            if let Some(Some(Value::I64(order))) = object.values.get(entity::DRAW_ORDER) {
                candidates.push((*order, object.id));
            }
        }
    }
    let Some(id) = top_of_draw_order(candidates) else {
        return Ok(None);
    };
    let curve = crate::modify::required_curve(snapshot, id)?;
    let end = curve.parameter_range().1;
    Ok(Some((curve.point_at(end), curve.tangent(end))))
}

/// The keyword options of a line: undo, close and finish.
fn options(points: usize) -> Vec<KeywordOption> {
    let mut out = vec![option("undo", OPTION_UNDO, Some('u'))];
    if points >= 2 {
        out.push(option("close", OPTION_CLOSE, Some('c')));
    }
    out.push(option("end", OPTION_END, None));
    out
}

/// The points of the chain after the answers, and whether it ended with close or finish. A
/// continued chain starts with the end of the last line or arc, which `undo` never removes.
struct Chain {
    points: Vec<Point2>,
    continued: bool,
    closed: bool,
    finished: bool,
}

impl Chain {
    /// The number of points that are not the continuation start.
    fn base(&self) -> usize {
        usize::from(self.continued)
    }
}

fn chain(state: &PromptState<'_>) -> Result<Chain> {
    let mut out = Chain {
        points: Vec::new(),
        continued: false,
        closed: false,
        finished: false,
    };
    for input in answers(state) {
        if let Some(point) = point_of(state, input) {
            out.points.push(point);
            continue;
        }
        match keyword_of(input) {
            Some("continue") if out.points.is_empty() => {
                if let Some((start, _)) = last_continuation(state.snapshot)? {
                    out.points.push(start);
                    out.continued = true;
                }
            }
            Some("undo") if out.points.len() > out.base() => {
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
    Ok(out)
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let chain = chain(state)?;
    if chain.finished {
        let points: Vec<[f64; 2]> = chain
            .points
            .get(chain.base()..)
            .unwrap_or_default()
            .iter()
            .map(|p| [p.x, p.y])
            .collect();
        return done(serde_json::json!({
            "points": points,
            "close": chain.closed,
            "continue_last": chain.continued,
        }));
    }
    let prompt = if chain.points.is_empty() {
        match last_continuation(state.snapshot)? {
            Some(_) => {
                let cont = KeywordOption::new("continue", OPTION_CONTINUE.message_id);
                Prompt::new("point", FIRST_CONTINUE.message_id, point_accept())
                    .or(Accept::keywords([cont]))
            }
            None => Prompt::new("point", FIRST.message_id, point_accept()),
        }
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
    let chain = chain(state)?;
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
        Some("undo") if chain.points.len() <= chain.base() => {
            Err(arc_base::Error::new(codes::NOTHING_TO_UNDO))
        }
        Some("close" | "end") if chain.points.len() < 2 => {
            Err(arc_base::Error::new(codes::VALUE_INVALID).with_detail("field", "points"))
        }
        Some("continue") => {
            let refused = !chain.points.is_empty() || last_continuation(state.snapshot)?.is_none();
            if refused {
                Err(arc_base::Error::new(codes::VALUE_INVALID).with_detail("field", "continue"))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn run(ctx: &mut Context<'_>, p: LineParams) -> Result<Created> {
    if p.points.len() > MAX_POINTS {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", MAX_POINTS.to_string()));
    }
    let mut points: Vec<Point2> = Vec::with_capacity(p.points.len() + 1);
    if p.continue_last {
        let (start, _) = last_continuation(ctx.snapshot())?.ok_or_else(|| {
            Error::new(codes::VALUE_INVALID).with_detail("field", "continue_last")
        })?;
        points.push(start);
    }
    points.extend(p.points.iter().map(|q| Point2::new(q[0], q[1])));
    if points.len() < 2 {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "points"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use arc_base::id::Identifier;

    #[test]
    fn top_of_draw_order_takes_the_highest_key_and_the_newest_on_a_tie() {
        let (old, new) = (ObjectId::from_u128(1), ObjectId::from_u128(2));
        assert_eq!(top_of_draw_order([(5, old), (5, new)]), Some(new));
        assert_eq!(top_of_draw_order([(7, old), (5, new)]), Some(old));
        assert_eq!(top_of_draw_order(Vec::new()), None);
    }
}
