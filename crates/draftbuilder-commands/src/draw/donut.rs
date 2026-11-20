//! DONUT (`draftbuilder.draw.donut`, typed `DONUT`): rings from an inside and an outside diameter
//! at each center, stored as closed wide polylines. An inside diameter of zero gives a filled disc.
//! The ring is a closed polyline of two half-circle segments on the mid circle, with the width of
//! the ring on both segments, so its edges are the two diameters exactly.

use arc_base::Error;
use arc_base::Result;
use arc_base::math::Point2;
use arc_command::{
    Accept, CommandSpec, Context, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{Curve2, Polyline2, PolylineVertex};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{
    Created, Creator, OPTION_END, answers, done, from_meters, keyword_of, machine, option, point_of,
};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.draw.donut`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DonutParams {
    /// The inside diameter in drawing units, zero or more.
    pub inner_diameter: f64,
    /// The outside diameter in drawing units, above the inside diameter.
    pub outer_diameter: f64,
    /// The centers, in drawing units, one ring each.
    #[schemars(length(min = 1, max = 10000))]
    pub centers: Vec<[f64; 2]>,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-donut-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-donut-description",
};
const INNER: Message = Message {
    message_id: "db-cmd-draw-donut-inner",
};
const OUTER: Message = Message {
    message_id: "db-cmd-draw-donut-outer",
};
const CENTER: Message = Message {
    message_id: "db-cmd-draw-donut-center",
};

/// The ring of a center with the two diameters, as a closed polyline.
fn ring(center: Point2, inner: f64, outer: f64) -> Result<Curve2> {
    let mid = (outer + inner) / 4.0;
    let width = (outer - inner) / 2.0;
    let vertex = |point: Point2| PolylineVertex {
        point,
        bulge: 1.0,
        start_width: width,
        end_width: width,
    };
    let east = center + arc_base::math::Vector2::new(mid, 0.0);
    let west = center - arc_base::math::Vector2::new(mid, 0.0);
    Polyline2::new(vec![vertex(east), vertex(west)], true).map(Curve2::Polyline)
}

fn check_diameters(inner: f64, outer: f64) -> Result<()> {
    if inner.is_finite() && outer.is_finite() && inner >= 0.0 && outer > inner {
        Ok(())
    } else {
        Err(Error::new(codes::VALUE_INVALID).with_detail("field", "outer_diameter"))
    }
}

fn run(ctx: &mut Context<'_>, p: DonutParams) -> Result<Created> {
    check_diameters(p.inner_diameter, p.outer_diameter)?;
    let rings = p
        .centers
        .iter()
        .map(|c| ring(Point2::new(c[0], c[1]), p.inner_diameter, p.outer_diameter))
        .collect::<Result<Vec<_>>>()?;
    let mut creator = Creator::new(ctx.snapshot())?;
    for curve in &rings {
        creator.curve(ctx, curve)?;
    }
    Ok(creator.finish(ctx))
}

/// The diameter answered at `key`, in drawing units, `None` when not answered.
fn diameter(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Distance(meters)) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let mut centers: Vec<[f64; 2]> = Vec::new();
    let mut finished = false;
    for input in answers(state) {
        if let Some(point) = point_of(state, input) {
            centers.push([point.x, point.y]);
        } else if keyword_of(input) == Some("end") {
            finished = true;
        }
    }
    let inner = diameter(state, "inner");
    let outer = diameter(state, "outer");
    let Some(inner) = inner else {
        let prompt = Prompt::new("inner", INNER.message_id, Accept::distance())
            .with_default(PromptInput::Distance(0.0));
        return Ok(Step::Ask(prompt));
    };
    let Some(outer) = outer else {
        return Ok(Step::Ask(Prompt::new(
            "outer",
            OUTER.message_id,
            Accept::distance(),
        )));
    };
    if finished && !centers.is_empty() {
        return done(serde_json::json!({
            "inner_diameter": inner, "outer_diameter": outer, "centers": centers
        }));
    }
    let end = option("end", OPTION_END, None);
    let mut prompt = Prompt::new(
        "center",
        CENTER.message_id,
        Accept::Point {
            dims: PointDims::Two,
        },
    )
    .or(Accept::keywords([end]));
    if !centers.is_empty() {
        prompt = prompt.with_default(PromptInput::Keyword("end".to_owned()));
    }
    Ok(Step::Ask(prompt))
}

fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if let PromptInput::Distance(meters) = input
        && state.answer("inner").is_some()
    {
        let inner = diameter(state, "inner").unwrap_or(0.0);
        let outer = from_meters(state.snapshot, *meters);
        return check_diameters(inner, outer);
    }
    if keyword_of(input) == Some("end") {
        let has_center = answers(state).any(|i| point_of(state, i).is_some());
        if !has_center {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "centers"));
        }
    }
    Ok(())
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec("draftbuilder.draw.donut", LABEL, DESCRIPTION, run)
            .with_prompt(machine(next, check)),
    ]
}
