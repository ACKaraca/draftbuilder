//! CIRCLE (`draftbuilder.draw.circle`, typed `CIRCLE`): a circle by center and radius or diameter,
//! by two points of a diameter, by three points, or tangent to two objects with a radius. The
//! tangent form takes the identifiers of two lines, segments or circles and a pick point on each;
//! the solution whose tangent points are nearest the picks is drawn. The prompt form has the
//! center, two-point and three-point forms; the tangent form needs object picks and is typed only.
//! Tangent to three objects is not built.

use arc_base::Limits;
use arc_base::math::Point2;
use arc_base::{Error, Result};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, PointDims, Prompt, PromptInput, PromptState, Step,
};
use arc_doc::{Snapshot, Value};
use arc_draft_dxf::fields::Reader;
use arc_geom::encoding::GeomCodec;
use arc_geom::{Circle2, Curve2};
use schemars::JsonSchema;
use serde::Deserialize;

use super::create::{Created, Creator, done, from_meters, keyword_of, machine, option, point_of};
use super::geometry::{Tangent, circle_through, tangent_circle};
use crate::error::{Message, codes};
use crate::settings::remember::{CIRCLE_RADIUS, offered, remembering};

/// A tangent object and the pick point that selects the solution on it.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TangentPick {
    /// A line, a segment or a circle of the drawing.
    pub object: IdParam,
    /// A point near the wanted tangent point on the object, in drawing units.
    pub pick: [f64; 2],
}

/// How a circle is placed.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CircleShape {
    /// The center and the radius.
    Center {
        /// The center, in drawing units.
        center: [f64; 2],
        /// The radius in drawing units, above zero.
        radius: f64,
    },
    /// The center and the diameter.
    Diameter {
        /// The center, in drawing units.
        center: [f64; 2],
        /// The diameter in drawing units, above zero.
        diameter: f64,
    },
    /// Two points of a diameter.
    TwoPoints {
        /// The first point, in drawing units.
        first: [f64; 2],
        /// The second point, in drawing units.
        second: [f64; 2],
    },
    /// Three points on the circle.
    ThreePoints {
        /// The first point, in drawing units.
        first: [f64; 2],
        /// The second point, in drawing units.
        second: [f64; 2],
        /// The third point, in drawing units.
        third: [f64; 2],
    },
    /// Tangent to two objects with the radius.
    TangentRadius {
        /// The first tangent object.
        first: TangentPick,
        /// The second tangent object.
        second: TangentPick,
        /// The radius in drawing units, above zero.
        radius: f64,
    },
}

/// Parameters of `draftbuilder.draw.circle`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CircleParams {
    /// The placement of the circle.
    pub shape: CircleShape,
}

const LABEL: Message = Message {
    message_id: "db-cmd-draw-circle-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-draw-circle-description",
};
const FIRST: Message = Message {
    message_id: "db-cmd-draw-circle-first",
};
const RADIUS: Message = Message {
    message_id: "db-cmd-draw-circle-radius",
};
const DIAMETER: Message = Message {
    message_id: "db-cmd-draw-circle-diameter",
};
const POINT: Message = Message {
    message_id: "db-cmd-draw-circle-point",
};
const OPTION_TWO: Message = Message {
    message_id: "db-cmd-option-two-points",
};
const OPTION_THREE: Message = Message {
    message_id: "db-cmd-option-three-points",
};
const OPTION_DIAMETER: Message = Message {
    message_id: "db-cmd-option-diameter",
};

fn point_prompt(key: &str, message: Message) -> Step {
    Step::Ask(Prompt::new(
        key,
        message.message_id,
        Accept::Point {
            dims: PointDims::Two,
        },
    ))
}

fn distance(state: &PromptState<'_>, key: &str) -> Option<f64> {
    match state.answer(key) {
        Some(PromptInput::Distance(meters)) => Some(from_meters(state.snapshot, *meters)),
        _ => None,
    }
}

fn point(state: &PromptState<'_>, key: &str) -> Option<[f64; 2]> {
    state
        .answer(key)
        .and_then(|input| point_of(state, input))
        .map(|p| [p.x, p.y])
}

fn next(state: &PromptState<'_>) -> Result<Step> {
    let Some(first) = state.answer("first") else {
        let two = option("two", OPTION_TWO, Some('t'));
        let three = option("three", OPTION_THREE, Some('h'));
        return Ok(Step::Ask(
            Prompt::new(
                "first",
                FIRST.message_id,
                Accept::Point {
                    dims: PointDims::Two,
                },
            )
            .or(Accept::keywords([two, three])),
        ));
    };
    match keyword_of(first) {
        None => center_next(state),
        Some("two") => points_next(state, &["p1", "p2"], "two_points"),
        Some(_) => points_next(state, &["p1", "p2", "p3"], "three_points"),
    }
}

fn center_next(state: &PromptState<'_>) -> Result<Step> {
    let Some(center) = point(state, "first") else {
        return Ok(point_prompt("first", FIRST));
    };
    let Some(radius) = state.answer("radius") else {
        let diameter = option("diameter", OPTION_DIAMETER, Some('d'));
        return Ok(Step::Ask(offered(
            state,
            CIRCLE_RADIUS,
            Prompt::new("radius", RADIUS.message_id, Accept::distance())
                .or(Accept::keywords([diameter])),
        )));
    };
    if keyword_of(radius) == Some("diameter") {
        let Some(diameter) = distance(state, "diameter") else {
            return Ok(Step::Ask(Prompt::new(
                "diameter",
                DIAMETER.message_id,
                Accept::distance(),
            )));
        };
        return done(serde_json::json!({
            "shape": {"mode": "diameter", "center": center, "diameter": diameter}
        }));
    }
    let radius = distance(state, "radius").unwrap_or(0.0);
    done(serde_json::json!({
        "shape": {"mode": "center", "center": center, "radius": radius}
    }))
}

/// Asks the points `keys` in order, then completes the `mode` form from them.
fn points_next(state: &PromptState<'_>, keys: &[&str], mode: &str) -> Result<Step> {
    let mut collected = Vec::new();
    for key in keys {
        match point(state, key) {
            Some(p) => collected.push(p),
            None => return Ok(point_prompt(key, POINT)),
        }
    }
    let shape = if mode == "two_points" {
        serde_json::json!({"mode": mode, "first": collected[0], "second": collected[1]})
    } else {
        serde_json::json!({"mode": mode, "first": collected[0], "second": collected[1], "third": collected[2]})
    };
    done(serde_json::json!({"shape": shape}))
}

/// Rejects a zero radius or diameter, and a point equal to the previous point answer.
fn check(state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
    if let PromptInput::Distance(meters) = input
        && *meters <= 0.0
    {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "radius"));
    }
    if let Some(point) = point_of(state, input) {
        let previous = state
            .answers
            .iter()
            .rev()
            .find_map(|a| point_of(state, &a.input));
        if previous.is_some_and(|p| super::geometry::coincident(p, point)) {
            return Err(Error::new(codes::POINTS_COLLINEAR));
        }
    }
    Ok(())
}

/// The tangent object of an entity: a line or segment (with its length) or a circle.
fn tangent_of(snapshot: &Snapshot, id: IdParam) -> Result<Tangent> {
    let reject = || Error::new(codes::TANGENT_OBJECT).with_detail("id", id.0.to_string());
    let handle = snapshot.resolve(id.0)?;
    let key = snapshot
        .type_key(handle)
        .ok_or_else(reject)?
        .as_str()
        .to_owned();
    let object = snapshot.get(handle)?;
    let bytes = match Reader::new(snapshot.registry(), &key, &object.values).get("geometry") {
        Some(Value::Geometry(bytes)) => bytes,
        _ => return Err(reject()),
    };
    match Curve2::decode(&bytes.0, &Limits::DEFAULT)? {
        Curve2::Segment(segment) => {
            let direction = segment.end() - segment.start();
            let length = direction.norm();
            Ok(Tangent::Line {
                origin: segment.start(),
                unit: direction / length,
                span: Some(length),
            })
        }
        Curve2::Line(line) => Ok(Tangent::Line {
            origin: line.point(),
            unit: line.unit_direction(),
            span: None,
        }),
        Curve2::Circle(circle) => Ok(Tangent::Circle {
            center: circle.center(),
            radius: circle.radius(),
        }),
        _ => Err(reject()),
    }
}

fn pick(pick: TangentPick) -> Point2 {
    Point2::new(pick.pick[0], pick.pick[1])
}

fn circle(snapshot: &Snapshot, shape: CircleShape) -> Result<Circle2> {
    let p = |[x, y]: [f64; 2]| Point2::new(x, y);
    match shape {
        CircleShape::Center { center, radius } => Circle2::new(p(center), radius),
        CircleShape::Diameter { center, diameter } => Circle2::new(p(center), diameter / 2.0),
        CircleShape::TwoPoints { first, second } => {
            let (a, b) = (p(first), p(second));
            Circle2::new(a + (b - a) * 0.5, (b - a).norm() / 2.0)
        }
        CircleShape::ThreePoints {
            first,
            second,
            third,
        } => circle_through(p(first), p(second), p(third)),
        CircleShape::TangentRadius {
            first,
            second,
            radius,
        } => tangent_circle(
            tangent_of(snapshot, first.object)?,
            tangent_of(snapshot, second.object)?,
            radius,
            pick(first),
            pick(second),
        ),
    }
}

fn run(ctx: &mut Context<'_>, p: CircleParams) -> Result<Created> {
    let found = circle(ctx.snapshot(), p.shape)?;
    let mut creator = Creator::new(ctx.snapshot())?;
    creator.curve(ctx, &Curve2::Circle(found))?;
    Ok(creator.finish(ctx))
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.draw.circle",
            LABEL,
            DESCRIPTION,
            remembering(run, |p: &CircleParams, per_unit| match &p.shape {
                CircleShape::Center { radius, .. } => vec![(CIRCLE_RADIUS, radius * per_unit)],
                _ => Vec::new(),
            }),
        )
        .with_prompt(machine(next, check)),
    ]
}
