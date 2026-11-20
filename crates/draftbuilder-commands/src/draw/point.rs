//! POINT (`draftbuilder.draw.point`, typed `POINT`): point entities at the given positions, one
//! undo step. The drawing-wide point style is not built, so points take the default style.

use arc_base::Result;
use arc_base::math::Point2;
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, PointDims, Prompt, PromptInput, PromptState, Step,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_END, answers, done, keyword_of, machine, option, point_of,
};
use crate::error::Message;

/// Parameters of `draftbuilder.draw.point`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointParams {
    /// The positions, in drawing units, one point entity each.
    #[schemars(length(min = 1, max = 10000))]
    pub points: Vec<[f64; 2]>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-point-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-point-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-point-first",
};
const NEXT: Message = Message {
    message_id: "db-cmd-draw-point-next",
};

fn next(state: &PromptState<'_>) -> Result<Step> {
    let mut points: Vec<Point2> = Vec::new();
    let mut finished = false;
    for input in answers(state) {
        if let Some(point) = point_of(state, input) {
            points.push(point);
        } else if keyword_of(input) == Some("end") {
            finished = true;
        }
    }
    if finished {
        let list: Vec<[f64; 2]> = points.iter().map(|p| [p.x, p.y]).collect();
        return done(serde_json::json!({"points": list}));
    }
    let prompt = if points.is_empty() {
        Prompt::new(
            "point",
            FIRST.message_id,
            Accept::Point {
                dims: PointDims::Two,
            },
        )
    } else {
        let end: KeywordOption = option("end", OPTION_END, None);
        Prompt::new(
            "point",
            NEXT.message_id,
            Accept::Point {
                dims: PointDims::Two,
            },
        )
        .or(Accept::keywords([end]))
        .with_default(PromptInput::Keyword("end".to_owned()))
    };
    Ok(Step::Ask(prompt))
}

fn run(ctx: &mut Context<'_>, p: PointParams) -> Result<Created> {
    let mut creator = Creator::new(ctx.snapshot())?;
    for [x, y] in p.points {
        creator.point(ctx, Point2::new(x, y))?;
    }
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.point", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, super::create::accept_all)),
    ]
}
