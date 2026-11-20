//! SPLINE (`draftbuilder.draw.spline`, typed `SPLINE`): a NURBS curve through fit points, or with
//! `control` set, a curve of the given degree with the points as control vertices. A fit spline
//! keeps its fit points, as the foundation's spline entity does. The prompt form takes fit points
//! of degree 3 until Enter; control vertices, fit tolerance and end tangents are typed-form only
//! where stated in the overview.

use arc_base::Result;
use arc_base::Tolerance;
use arc_base::math::Point2;
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::Value;
use arc_draft::types::{self, entity};
use arc_geom::{Curve2, Nurbs2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_END, answers, done, keyword_of, machine, option, point_of,
};
use crate::error::{Message, codes};
use arc_base::Error;

/// The degree of a prompt-form spline.
const PROMPT_DEGREE: usize = 3;

/// Parameters of `draftbuilder.draw.spline`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SplineParams {
    /// The fit points, or the control vertices when `control` is true, in drawing units.
    #[schemars(length(min = 2, max = 10000))]
    pub points: Vec<[f64; 2]>,
    /// The degree of the curve, from 1 up to the number of points minus one; 3 when absent.
    #[serde(default)]
    pub degree: Option<usize>,
    /// Whether the points are control vertices instead of fit points.
    #[serde(default)]
    pub control: bool,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-spline-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-spline-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-spline-first",
};
const NEXT: Message = Message {
    message_id: "db-cmd-draw-spline-next",
};

fn next(state: &PromptState<'_>) -> Result<Step> {
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut finished = false;
    for input in answers(state) {
        if let Some(point) = point_of(state, input) {
            points.push([point.x, point.y]);
        } else if keyword_of(input) == Some("end") {
            finished = true;
        }
    }
    if finished {
        return done(
            serde_json::json!({"points": points, "degree": PROMPT_DEGREE, "control": false}),
        );
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

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if keyword_of(input) == Some("end") {
        let count = answers(state)
            .filter(|i| point_of(state, i).is_some())
            .count();
        if count <= PROMPT_DEGREE {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "points"));
        }
    }
    Ok(())
}

/// The curve of the points, and its fit points when the points are fit points.
fn curve(points: &[Point2], degree: usize, control: bool) -> Result<(Nurbs2, Vec<Point2>)> {
    if control {
        let weights = vec![1.0; points.len()];
        Ok((
            Nurbs2::from_control_points(degree, points.to_vec(), weights)?,
            Vec::new(),
        ))
    } else {
        Ok((
            Nurbs2::interpolate(points, degree, &Tolerance::DEFAULT)?,
            points.to_vec(),
        ))
    }
}

fn run(ctx: &mut Context<'_>, p: SplineParams) -> Result<Created> {
    let degree = p.degree.unwrap_or(PROMPT_DEGREE);
    let points: Vec<Point2> = p.points.iter().map(|q| Point2::new(q[0], q[1])).collect();
    let (nurbs, fit) = curve(&points, degree, p.control)?;
    let mut creator = Creator::new(ctx.snapshot())?;
    let fields = vec![
        (
            entity::KIND,
            Value::Geometry(arc_doc::GeometryBytes(
                arc_geom::encoding::GeomCodec::encode(&Curve2::Nurbs(nurbs)),
            )),
        ),
        (
            entity::SPLINE_FIT_POINTS,
            Value::List(fit.into_iter().map(Value::Point2).collect()),
        ),
    ];
    creator.add(ctx, types::SPLINE, fields)?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.spline", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
