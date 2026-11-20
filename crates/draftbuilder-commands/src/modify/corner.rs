//! FILLET and CHAMFER (`draftbuilder.modify.fillet`, typed `FILLET`, alias `F`;
//! `draftbuilder.modify.chamfer`, typed `CHAMFER`, alias `CHA`). Two entities with a pick point each
//! are joined at their corner by a fillet arc of a radius, or by a chamfer segment of two distances
//! or a distance and an angle. The inputs are trimmed or extended to the joint when `trim` is set (the
//! default); otherwise they are kept and only the joint is created. A radius or distance of zero gives
//! the sharp corner, and two parallel lines receive a semicircular end (the foundation's rules).
//!
//! The polyline form (`polyline`) applies the corner at every vertex of one polyline, whose segments
//! are straight; the vertices it leaves are reported with their reason. The joint takes the layer and
//! appearance of the first entity; the inputs keep their identifiers when they keep their kind.

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptInput, PromptState, Step,
};
use arc_geom::{ChamferSize, CornerMode, Curve2, Replacement, SkipReason};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{distance_answer, entities, point, point_answer, refuse_locked, replace_with_pieces};
use crate::draw::create::{Creator, accept_all, done, machine};
use crate::error::{Message, codes};
use crate::settings::remember::{
    CHAMFER_FIRST, CHAMFER_SECOND, FILLET_RADIUS, offered, remembering,
};

/// An entity of a corner and the point that picks the part kept.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CornerPick {
    /// The entity: a line, segment, ray, arc or circle for a fillet; a line or segment for a chamfer.
    pub object: IdParam,
    /// A point on the part of the entity that is kept, in drawing units.
    pub pick: [f64; 2],
}

/// A distance along the first entity and the angle of the chamfer to it, inside the corner.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DistanceAngle {
    /// The distance along the first entity, drawing units.
    pub distance: f64,
    /// The angle in radians, strictly between 0 and pi.
    pub angle: f64,
}

/// Parameters of `draftbuilder.modify.fillet`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FilletParams {
    /// The first entity of the corner (with `second`).
    #[serde(default)]
    pub first: Option<CornerPick>,
    /// The second entity of the corner.
    #[serde(default)]
    pub second: Option<CornerPick>,
    /// A polyline whose every vertex is filleted (instead of `first` and `second`).
    #[serde(default)]
    pub polyline: Option<IdParam>,
    /// The radius in drawing units; zero gives the sharp corner.
    pub radius: f64,
    /// Whether the inputs are trimmed or extended to the joint.
    #[serde(default = "yes")]
    pub trim: bool,
    /// Further corners filleted in turn after `first` and `second`, with the same radius and trim (multiple mode).
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub more: Vec<CornerPair>,
}

/// One further corner of a multiple fillet: two entities and the point that picks the part kept of each.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CornerPair {
    /// The first entity of the corner.
    pub first: CornerPick,
    /// The second entity of the corner.
    pub second: CornerPick,
}

/// The size of a chamfer: two distances, or a distance and an angle.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "by", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChamferSizeParam {
    /// The distances along the first and along the second entity.
    Distances {
        /// The two distances, drawing units; zero on both makes the sharp corner.
        distances: [f64; 2],
    },
    /// A distance along the first entity and the angle of the chamfer to it, inside the corner.
    DistanceAngle {
        /// The distance along the first entity, drawing units.
        distance: f64,
        /// The angle in radians, strictly between 0 and pi.
        angle: f64,
    },
}

/// What a chamfer is made at: two entities, or every vertex of one polyline.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(tag = "form", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChamferCorner {
    /// The corner of two entities, each with the point that picks the part kept.
    Pair {
        /// The first entity.
        first: CornerPick,
        /// The second entity.
        second: CornerPick,
        /// The size of the chamfer.
        size: ChamferSizeParam,
    },
    /// Every vertex of a polyline whose segments are straight, at one distance.
    Polyline {
        /// The polyline.
        polyline: IdParam,
        /// The distance at every vertex, drawing units; zero leaves the polyline as it is.
        distance: f64,
    },
}

/// Parameters of `draftbuilder.modify.chamfer`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChamferParams {
    /// The corner to chamfer.
    pub corner: ChamferCorner,
    /// Whether the inputs are trimmed or extended to the joint.
    #[serde(default = "yes")]
    pub trim: bool,
}

fn yes() -> bool {
    true
}

/// A vertex a polyline corner left unchanged, and why.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Skipped {
    /// The vertex index in the polyline.
    pub index: usize,
    /// `bulged_segment` or `too_short`.
    pub reason: String,
}

/// The result of a corner command: the entities it produced, and the polyline vertices it skipped.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Corners {
    /// The joint and the entities it changed, in order.
    pub objects: Vec<IdParam>,
    /// The polyline vertices left unchanged (polyline form only).
    pub skipped: Vec<Skipped>,
}

const FILLET_LABEL: Message = Message {
    message_id: "db-cmd-modify-fillet-label",
};
const FILLET_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-fillet-description",
};
const CHAMFER_LABEL: Message = Message {
    message_id: "db-cmd-modify-chamfer-label",
};
const CHAMFER_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-chamfer-description",
};

fn mode(trim: bool) -> CornerMode {
    if trim {
        CornerMode::Trim
    } else {
        CornerMode::Keep
    }
}

/// Applies a corner result to the two entities and creates its joint with the first entity's
/// appearance. Returns the identifiers in order: the joint, then the entities.
fn apply(
    ctx: &mut Context<'_>,
    first: CornerPick,
    second: CornerPick,
    joint: Option<Curve2>,
    first_result: Replacement,
    second_result: Replacement,
) -> Result<Vec<ObjectId>> {
    let snapshot = ctx.snapshot().clone();
    let first_id = first.object.0;
    let first_values = snapshot.get(snapshot.resolve(first_id)?)?.values.clone();
    let owner = snapshot.get(snapshot.resolve(first_id)?)?.header.owner;
    let mut out = Vec::new();
    if let Some(joint) = joint {
        let key = super::key_of_curve(&joint);
        let values = super::appearance_values(&snapshot, key, &first_values, &joint);
        let mut creator = Creator::new(&snapshot)?;
        out.push(creator.add_values(ctx, key, owner, values)?);
    }
    for (id, result) in [(first_id, first_result), (second.object.0, second_result)] {
        match result {
            Replacement::Trimmed(curve) => out.extend(replace_with_pieces(ctx, id, &[curve])?),
            Replacement::Unchanged => out.push(id),
        }
    }
    Ok(out)
}

fn pair_check(
    first: &Option<CornerPick>,
    second: &Option<CornerPick>,
    polyline: &Option<IdParam>,
) -> Result<()> {
    match (first, second, polyline) {
        (Some(a), Some(b), None) if a.object.0 != b.object.0 => Ok(()),
        (None, None, Some(_)) => Ok(()),
        _ => Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "first")),
    }
}

fn fillet_run(ctx: &mut Context<'_>, p: FilletParams) -> Result<Corners> {
    if let Some(polyline) = p.polyline {
        pair_check(&p.first, &p.second, &Some(polyline))?;
        let snapshot = ctx.snapshot().clone();
        let ids = entities(&snapshot, ctx.limits(), &[polyline])?;
        refuse_locked(&snapshot, &ids)?;
        let Curve2::Polyline(source) = super::required_curve(&snapshot, polyline.0)? else {
            return Err(
                Error::new(codes::GEOMETRY_UNSUPPORTED).with_detail("id", polyline.0.to_string())
            );
        };
        let result = arc_geom::fillet_polyline_vertices(&source, p.radius, &Tolerance::DEFAULT)?;
        let objects = replace_with_pieces(ctx, polyline.0, &[Curve2::Polyline(result.polyline)])?;
        return Ok(Corners {
            objects: objects.into_iter().map(IdParam).collect(),
            skipped: skipped(&result.skipped),
        });
    }
    pair_check(&p.first, &p.second, &None)?;
    let (first, second) = (
        p.first.ok_or_else(|| Error::new(codes::PARAMS_CONFLICT))?,
        p.second.ok_or_else(|| Error::new(codes::PARAMS_CONFLICT))?,
    );
    let limit = usize::try_from(ctx.limits().array_length).unwrap_or(usize::MAX);
    if p.more.len() >= limit {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", "array_length"));
    }
    // Multiple mode: each further corner is filleted in turn, on the drawing the earlier ones left.
    let mut objects = fillet_pair(ctx, first, second, p.radius, p.trim)?;
    for pair in &p.more {
        pair_check(&Some(pair.first), &Some(pair.second), &None)?;
        objects.extend(fillet_pair(ctx, pair.first, pair.second, p.radius, p.trim)?);
    }
    ctx.set_selection(objects.clone())?;
    Ok(Corners {
        objects: objects.into_iter().map(IdParam).collect(),
        skipped: Vec::new(),
    })
}

/// Fillets one corner: the two entities are trimmed to the joint, or only the joint is made when `trim` is off.
/// The entities are read from the drawing as it is now, so a corner may use an entity an earlier one changed.
fn fillet_pair(
    ctx: &mut Context<'_>,
    first: CornerPick,
    second: CornerPick,
    radius: f64,
    trim: bool,
) -> Result<Vec<ObjectId>> {
    if !first.pick.iter().chain(&second.pick).all(|v| v.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "pick"));
    }
    let snapshot = ctx.snapshot().clone();
    let a = super::required_curve(&snapshot, first.object.0)?;
    let b = super::required_curve(&snapshot, second.object.0)?;
    if trim {
        refuse_locked(&snapshot, &[first.object.0, second.object.0])?;
    }
    let result = arc_geom::fillet(
        &a,
        Point2::new(first.pick[0], first.pick[1]),
        &b,
        Point2::new(second.pick[0], second.pick[1]),
        radius,
        mode(trim),
        &Tolerance::DEFAULT,
    )?;
    apply(
        ctx,
        first,
        second,
        result.joint,
        result.first,
        result.second,
    )
}

fn chamfer_run(ctx: &mut Context<'_>, p: ChamferParams) -> Result<Corners> {
    match p.corner {
        ChamferCorner::Polyline { polyline, distance } => {
            let snapshot = ctx.snapshot().clone();
            refuse_locked(&snapshot, &[polyline.0])?;
            let Curve2::Polyline(source) = super::required_curve(&snapshot, polyline.0)? else {
                return Err(Error::new(codes::GEOMETRY_UNSUPPORTED)
                    .with_detail("id", polyline.0.to_string()));
            };
            let result =
                arc_geom::chamfer_polyline_vertices(&source, distance, &Tolerance::DEFAULT)?;
            let objects =
                replace_with_pieces(ctx, polyline.0, &[Curve2::Polyline(result.polyline)])?;
            Ok(Corners {
                objects: objects.into_iter().map(IdParam).collect(),
                skipped: skipped(&result.skipped),
            })
        }
        ChamferCorner::Pair {
            first,
            second,
            size,
        } => {
            if first.object.0 == second.object.0 {
                return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "second"));
            }
            let size = match size {
                ChamferSizeParam::Distances {
                    distances: [d1, d2],
                } => ChamferSize::Distances {
                    first: d1,
                    second: d2,
                },
                ChamferSizeParam::DistanceAngle { distance, angle } => {
                    ChamferSize::DistanceAngle { distance, angle }
                }
            };
            let snapshot = ctx.snapshot().clone();
            let a = super::required_curve(&snapshot, first.object.0)?;
            let b = super::required_curve(&snapshot, second.object.0)?;
            if p.trim {
                refuse_locked(&snapshot, &[first.object.0, second.object.0])?;
            }
            let result = arc_geom::chamfer(
                &a,
                Point2::new(first.pick[0], first.pick[1]),
                &b,
                Point2::new(second.pick[0], second.pick[1]),
                size,
                mode(p.trim),
                &Tolerance::DEFAULT,
            )?;
            let objects = apply(
                ctx,
                first,
                second,
                result.joint,
                result.first,
                result.second,
            )?;
            ctx.set_selection(objects.clone())?;
            Ok(Corners {
                objects: objects.into_iter().map(IdParam).collect(),
                skipped: Vec::new(),
            })
        }
    }
}

fn skipped(list: &[arc_geom::SkippedVertex]) -> Vec<Skipped> {
    list.iter()
        .map(|s| Skipped {
            index: s.index,
            reason: match s.reason {
                SkipReason::BulgedSegment => "bulged_segment".to_owned(),
                SkipReason::TooShort => "too_short".to_owned(),
            },
        })
        .collect()
}

/// The type key a polyline pick accepts.
const POLYLINE_TYPE: &str = arc_draft::types::POLYLINE;

const OPTION_MULTIPLE: Message = Message {
    message_id: "db-cmd-modify-option-multiple",
};
const OPTION_END: Message = Message {
    message_id: "db-cmd-modify-option-end",
};
const FIRST: Message = Message {
    message_id: "db-cmd-modify-corner-first",
};
const FIRST_PICK: Message = Message {
    message_id: "db-cmd-modify-corner-first-pick",
};
const SECOND: Message = Message {
    message_id: "db-cmd-modify-corner-second",
};
const SECOND_PICK: Message = Message {
    message_id: "db-cmd-modify-corner-second-pick",
};
const POLYLINE_PICK: Message = Message {
    message_id: "db-cmd-modify-corner-polyline",
};
const RADIUS: Message = Message {
    message_id: "db-cmd-modify-corner-radius",
};
const DISTANCE_FIRST: Message = Message {
    message_id: "db-cmd-modify-corner-distance-first",
};
const DISTANCE_SECOND: Message = Message {
    message_id: "db-cmd-modify-corner-distance-second",
};
const DISTANCE: Message = Message {
    message_id: "db-cmd-modify-corner-distance",
};
const OPTION_POLYLINE: Message = Message {
    message_id: "db-cmd-modify-option-polyline",
};
const OPTION_NOTRIM: Message = Message {
    message_id: "db-cmd-modify-option-notrim",
};

/// The prompt machine of a corner: the first entity and its pick, the second entity and its pick,
/// then the radius (fillet) or the two distances (chamfer). The keyword `polyline` on the first
/// prompt asks for one polyline instead, and the keyword `notrim` keeps the inputs; each answer of
/// the keyword toggles it.
/// The radius prompt of a fillet, offering the remembered radius (DB-CMD-004).
fn radius_ask(state: &PromptState<'_>) -> Step {
    Step::Ask(offered(
        state,
        FILLET_RADIUS,
        Prompt::new("radius", RADIUS.message_id, Accept::distance()),
    ))
}

/// A chamfer distance prompt `key`, offering the remembered distance `remembered` (DB-CMD-004).
fn distance_ask(state: &PromptState<'_>, key: &str, message: Message, remembered: &str) -> Step {
    Step::Ask(offered(
        state,
        remembered,
        Prompt::new(key, message.message_id, Accept::distance()),
    ))
}

fn corner_next(state: &PromptState<'_>, chamfer: bool) -> arc_base::Result<Step> {
    let polyline_mode = state.answers.iter().any(|a| {
        a.key == "first" && matches!(&a.input, PromptInput::Keyword(k) if k == "polyline")
    });
    let trim = state
        .answers
        .iter()
        .filter(|a| {
            a.key == "first" && matches!(&a.input, PromptInput::Keyword(k) if k == "notrim")
        })
        .count()
        % 2
        == 0;
    if polyline_mode {
        return polyline_corner(state, chamfer, trim);
    }
    if !chamfer && keyword_on(state, "first", "multiple") {
        return fillet_multi_next(state, trim);
    }
    if !has_pick(state, "first") {
        let mut options = vec![
            KeywordOption::new("polyline", OPTION_POLYLINE.message_id),
            KeywordOption::new("notrim", OPTION_NOTRIM.message_id),
        ];
        if !chamfer {
            options.push(KeywordOption::new("multiple", OPTION_MULTIPLE.message_id));
        }
        return Ok(Step::Ask(
            Prompt::new(
                "first",
                FIRST.message_id,
                Accept::Pick { types: Vec::new() },
            )
            .or(Accept::keywords(options)),
        ));
    }
    if state.answer("first_pick").is_none() {
        return Ok(Step::Ask(Prompt::new(
            "first_pick",
            FIRST_PICK.message_id,
            point(),
        )));
    }
    if !has_pick(state, "second") {
        return Ok(Step::Ask(Prompt::new(
            "second",
            SECOND.message_id,
            Accept::Pick { types: Vec::new() },
        )));
    }
    if state.answer("second_pick").is_none() {
        return Ok(Step::Ask(Prompt::new(
            "second_pick",
            SECOND_PICK.message_id,
            point(),
        )));
    }
    let first = pick_object(state, "first");
    let second = pick_object(state, "second");
    let first_pick = point_of(state, "first_pick");
    let second_pick = point_of(state, "second_pick");
    if chamfer {
        let Some(d1) = distance_of(state, "distance_first") else {
            return Ok(distance_ask(
                state,
                "distance_first",
                DISTANCE_FIRST,
                CHAMFER_FIRST,
            ));
        };
        let Some(d2) = distance_of(state, "distance_second") else {
            return Ok(distance_ask(
                state,
                "distance_second",
                DISTANCE_SECOND,
                CHAMFER_SECOND,
            ));
        };
        return done(serde_json::json!({
            "corner": {
                "form": "pair",
                "first": {"object": first, "pick": first_pick},
                "second": {"object": second, "pick": second_pick},
                "size": {"by": "distances", "distances": [d1, d2]}
            },
            "trim": trim
        }));
    }
    let Some(radius) = distance_of(state, "radius") else {
        return Ok(radius_ask(state));
    };
    done(serde_json::json!({
        "first": {"object": first, "pick": first_pick},
        "second": {"object": second, "pick": second_pick},
        "radius": radius, "trim": trim
    }))
}

/// The prompt answers of the polyline form: the polyline, then the distance (chamfer) or the radius.
fn polyline_corner(state: &PromptState<'_>, chamfer: bool, trim: bool) -> arc_base::Result<Step> {
    if !has_pick(state, "polyline") {
        return Ok(Step::Ask(Prompt::new(
            "polyline",
            POLYLINE_PICK.message_id,
            Accept::Pick {
                types: vec![POLYLINE_TYPE.to_owned()],
            },
        )));
    }
    let polyline = pick_object(state, "polyline");
    if chamfer {
        let Some(distance) = distance_of(state, "distance") else {
            return Ok(Step::Ask(Prompt::new(
                "distance",
                DISTANCE.message_id,
                Accept::distance(),
            )));
        };
        return done(serde_json::json!({
            "corner": {"form": "polyline", "polyline": polyline, "distance": distance},
            "trim": trim
        }));
    }
    let Some(radius) = distance_of(state, "radius") else {
        return Ok(radius_ask(state));
    };
    done(serde_json::json!({"polyline": polyline, "radius": radius, "trim": trim}))
}

/// Whether the prompt `key` has its pick answered.
fn has_pick(state: &PromptState<'_>, key: &str) -> bool {
    pick_object_opt(state, key).is_some()
}

/// The identifier text of the object picked by the answer `key`, empty when none.
fn pick_object(state: &PromptState<'_>, key: &str) -> String {
    pick_object_opt(state, key).unwrap_or_default()
}

fn pick_object_opt(state: &PromptState<'_>, key: &str) -> Option<String> {
    state
        .answers
        .iter()
        .find_map(|a| match (a.key.as_str(), &a.input) {
            (k, PromptInput::Pick(id)) if k == key => Some(id.to_string()),
            _ => None,
        })
}

/// The point of the answer `key` in drawing units.
fn point_of(state: &PromptState<'_>, key: &str) -> [f64; 2] {
    point_answer(state, key).unwrap_or([0.0, 0.0])
}

/// The distance of the answer `key` in drawing units.
fn distance_of(state: &PromptState<'_>, key: &str) -> Option<f64> {
    distance_answer(state, key)
}

/// Whether the keyword `word` was answered for the prompt `key`.
fn keyword_on(state: &PromptState<'_>, key: &str, word: &str) -> bool {
    state
        .answers
        .iter()
        .any(|a| a.key == key && matches!(&a.input, PromptInput::Keyword(k) if k == word))
}

/// A corner pair: the first entity and its pick, the second entity and its pick (drawing units).
type CornerAnswer = (String, [f64; 2], String, [f64; 2]);

/// The entity and pick answered so far for a corner pair that is not complete.
#[derive(Default)]
struct PartialCorner {
    first: Option<String>,
    first_pick: Option<[f64; 2]>,
    second: Option<String>,
}

/// The complete corner pairs answered in multiple mode, in order, and the pair in progress.
fn corner_pairs(state: &PromptState<'_>) -> (Vec<CornerAnswer>, PartialCorner) {
    let mut pairs = Vec::new();
    let mut partial = PartialCorner::default();
    for answer in state.answers {
        match (answer.key.as_str(), &answer.input) {
            ("first", PromptInput::Pick(id)) => {
                partial = PartialCorner {
                    first: Some(id.to_string()),
                    ..PartialCorner::default()
                };
            }
            ("first_pick", PromptInput::Point(p)) => {
                let q = crate::draw::create::drawing_point(state.snapshot, *p);
                partial.first_pick = Some([q.x, q.y]);
            }
            ("second", PromptInput::Pick(id)) => partial.second = Some(id.to_string()),
            ("second_pick", PromptInput::Point(p)) => {
                if let (Some(first), Some(first_pick), Some(second)) = (
                    partial.first.clone(),
                    partial.first_pick,
                    partial.second.clone(),
                ) {
                    let q = crate::draw::create::drawing_point(state.snapshot, *p);
                    pairs.push((first, first_pick, second, [q.x, q.y]));
                    partial = PartialCorner::default();
                }
            }
            _ => {}
        }
    }
    (pairs, partial)
}

/// The prompts of multiple mode: a corner pair as the single form asks it, then the radius after the first pair,
/// then the next pair, until `end` is answered at the first entity prompt.
fn fillet_multi_next(state: &PromptState<'_>, trim: bool) -> arc_base::Result<Step> {
    let (pairs, partial) = corner_pairs(state);
    if partial.first.is_none() && pairs.is_empty() {
        return Ok(Step::Ask(Prompt::new(
            "first",
            FIRST.message_id,
            Accept::Pick { types: Vec::new() },
        )));
    }
    if partial.first.is_some() && partial.first_pick.is_none() {
        return Ok(Step::Ask(Prompt::new(
            "first_pick",
            FIRST_PICK.message_id,
            point(),
        )));
    }
    if partial.first.is_some() && partial.second.is_none() {
        return Ok(Step::Ask(Prompt::new(
            "second",
            SECOND.message_id,
            Accept::Pick { types: Vec::new() },
        )));
    }
    if partial.first.is_some() {
        return Ok(Step::Ask(Prompt::new(
            "second_pick",
            SECOND_PICK.message_id,
            point(),
        )));
    }
    let Some(radius) = distance_of(state, "radius") else {
        return Ok(radius_ask(state));
    };
    let ended = state.answers.last().is_some_and(|a| {
        a.key == "first" && matches!(&a.input, PromptInput::Keyword(k) if k == "end")
    });
    let corner = |object: &str, pick: [f64; 2]| serde_json::json!({"object": object, "pick": pick});
    match pairs.first() {
        Some((first, first_pick, second, second_pick)) if ended => {
            let more: Vec<serde_json::Value> = pairs
                .iter()
                .skip(1)
                .map(|(f, fp, s, sp)| {
                    serde_json::json!({"first": corner(f, *fp), "second": corner(s, *sp)})
                })
                .collect();
            done(serde_json::json!({
                "first": corner(first, *first_pick),
                "second": corner(second, *second_pick),
                "more": more,
                "radius": radius,
                "trim": trim
            }))
        }
        _ => Ok(Step::Ask(
            Prompt::new(
                "first",
                FIRST.message_id,
                Accept::Pick { types: Vec::new() },
            )
            .or(Accept::keywords([KeywordOption::new(
                "end",
                OPTION_END.message_id,
            )]))
            .with_default(PromptInput::Keyword("end".to_owned())),
        )),
    }
}

fn fillet_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    corner_next(state, false)
}

fn chamfer_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    corner_next(state, true)
}

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.modify.fillet",
            FILLET_LABEL,
            FILLET_DESCRIPTION,
            remembering(fillet_run, |p: &FilletParams, per_unit| {
                vec![(FILLET_RADIUS, p.radius * per_unit)]
            }),
        )
        .with_prompt(machine(fillet_next, accept_all)),
        super::spec(
            "draftbuilder.modify.chamfer",
            CHAMFER_LABEL,
            CHAMFER_DESCRIPTION,
            remembering(chamfer_run, |p: &ChamferParams, per_unit| match &p.corner {
                ChamferCorner::Pair {
                    size: ChamferSizeParam::Distances { distances: [a, b] },
                    ..
                } => vec![
                    (CHAMFER_FIRST, a * per_unit),
                    (CHAMFER_SECOND, b * per_unit),
                ],
                _ => Vec::new(),
            }),
        )
        .with_prompt(machine(chamfer_next, accept_all)),
    ]
}
