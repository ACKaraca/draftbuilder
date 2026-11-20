//! Transforms (`draftbuilder.modify.move`, `copy`, `rotate`, `scale`, `mirror`, `align`): the
//! entities move by one similarity transform (uniform scale, rotation, translation, and for mirror a
//! reflection), applied by the foundation's entity transform. Move, rotate, scale and align change
//! the entities in place; copy, mirror (keeping the source) and the copy option of rotate and scale
//! create copies with the source's values and transform those.
//!
//! Each transform is about a base point or a mirror line, and the matrices are the same whichever
//! form asks for them: a rotation by `a` about `b` is `x ↦ R(x − b) + b`, so the stored translation
//! is `b − R b`; a scale by `f` about `b` is `x ↦ f(x − b) + b`; a mirror across the line through
//! `p` with unit direction `u` is `x ↦ M(x − p) + p` with `M v = 2(v·u)u − v`.

use arc_base::math::Point2;
use arc_base::{Dimension, Error, ObjectId, Result, Tolerance};
use arc_command::{
    Accept, CommandSpec, Context, IdParam, KeywordOption, Prompt, PromptState, Step,
};
use arc_draft::{EntityTransformParams, TransformParam, apply_entity_transform};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    angle_answer, copy_entities, distance_answer, entities, entity_selection, erase_entities,
    keyword_answer, number_answer, point, point_answer, refuse_locked, selection_answer,
};
use crate::draw::create::{done, drawing_point, machine};
use crate::error::{Message, codes};
use crate::settings::remember::{ROTATE_ANGLE, SCALE_FACTOR, offered, remembering};

/// The result of a transform command: the objects it produced, in order. For move, rotate, scale
/// and align these are the transformed entities; for copy, mirror and the copy option, the copies.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Changed {
    /// The entities the command left in the drawing, in order.
    pub objects: Vec<IdParam>,
}

fn changed(ids: Vec<ObjectId>) -> Changed {
    Changed {
        objects: ids.into_iter().map(IdParam).collect(),
    }
}

/// Applies `transform` to the entities `ids` in the drawing (through the foundation).
///
/// # Errors
///
/// The errors of [`apply_entity_transform`]: a locked layer, an associated object that is not
/// selected, a kind that cannot take the transform (named by its reason), a non-finite value.
pub(crate) fn transform_entities(
    ctx: &mut Context<'_>,
    ids: &[ObjectId],
    transform: TransformParam,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    apply_entity_transform(
        ctx,
        EntityTransformParams {
            objects: ids.iter().map(|id| IdParam(*id)).collect(),
            transform,
        },
    )
}

/// The transform that moves by `(dx, dy)`.
#[must_use]
pub(crate) fn translation(dx: f64, dy: f64) -> TransformParam {
    TransformParam {
        mirror: None,
        scale: 1.0,
        rotation: 0.0,
        translation: [dx, dy],
    }
}

/// The transform that turns by `angle` about `base`.
#[must_use]
pub(crate) fn rotation_about(base: [f64; 2], angle: f64) -> TransformParam {
    let (s, c) = angle.sin_cos();
    let [bx, by] = base;
    TransformParam {
        mirror: None,
        scale: 1.0,
        rotation: angle,
        translation: [bx - (c * bx - s * by), by - (s * bx + c * by)],
    }
}

/// The transform that scales by `factor` about `base`.
#[must_use]
pub(crate) fn scale_about(base: [f64; 2], factor: f64) -> TransformParam {
    TransformParam {
        mirror: None,
        scale: factor,
        rotation: 0.0,
        translation: [base[0] * (1.0 - factor), base[1] * (1.0 - factor)],
    }
}

/// The transform that mirrors across the line through `first` and `second`.
///
/// # Errors
///
/// `db.cmd.points_collinear` when the two points coincide (the line is not defined).
pub(crate) fn mirror_across(first: [f64; 2], second: [f64; 2]) -> Result<TransformParam> {
    let (dx, dy) = (second[0] - first[0], second[1] - first[1]);
    let length = dx.hypot(dy);
    if !(length.is_finite() && length > Tolerance::DEFAULT.linear) {
        return Err(Error::new(codes::POINTS_COLLINEAR));
    }
    let (ux, uy) = (dx / length, dy / length);
    // M p = 2 (p·u) u − p, for the point p of the line.
    let dot = first[0] * ux + first[1] * uy;
    let (mx, my) = (2.0 * dot * ux - first[0], 2.0 * dot * uy - first[1]);
    Ok(TransformParam {
        mirror: Some([ux, uy]),
        scale: 1.0,
        rotation: 0.0,
        translation: [first[0] - mx, first[1] - my],
    })
}

/// A rotation or a scale given by a value, or by a reference pair: `from` and `to` angles, or a
/// reference length and the new length.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// The current value: an angle in radians for rotate, a length in drawing units for scale.
    pub from: f64,
    /// The new value.
    pub to: f64,
}

/// Parameters of `draftbuilder.modify.move`: the displacement, given as a vector or as a base and a
/// second point.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveParams {
    /// The entities to move; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The displacement in drawing units. Give it, or `base` and `second`.
    #[serde(default)]
    pub displacement: Option<[f64; 2]>,
    /// The base point, in drawing units.
    #[serde(default)]
    pub base: Option<[f64; 2]>,
    /// The second point, in drawing units; the displacement is `second − base`.
    #[serde(default)]
    pub second: Option<[f64; 2]>,
}

/// Parameters of `draftbuilder.modify.copy`: copies of the entities, each displaced by the vector
/// from `base` to one target point, or an array of a set count of copies. The objects times the copies must
/// fit the session's array-length limit, and every point must be finite; both are checked before the first copy.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CopyParams {
    /// The entities to copy; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The base point, in drawing units.
    pub base: [f64; 2],
    /// The target points, in drawing units: one copy of every object for each, in order. Give these or `array`.
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub targets: Vec<[f64; 2]>,
    /// An array of copies instead of targets.
    #[serde(default)]
    pub array: Option<CopyArray>,
}

/// An array of copies: `count` copies of every object, spaced by the vector from the base point to `second`
/// (copy k is displaced by k times that vector), or fitted so that the last copy lands on `second` (copy k is
/// displaced by k over `count` times that vector).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CopyArray {
    /// The second point, in drawing units.
    pub second: [f64; 2],
    /// The number of copies of each object, at least one.
    pub count: u32,
    /// Whether the copies are fitted between the base point and `second`, instead of spaced by it.
    #[serde(default)]
    pub fitted: bool,
}

/// Parameters of `draftbuilder.modify.rotate`: a rotation by an angle, or by the change from a
/// reference angle to a new one.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RotateParams {
    /// The entities to rotate; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The base point, in drawing units.
    pub base: [f64; 2],
    /// The angle in radians, counterclockwise. Give it, or `reference`.
    #[serde(default)]
    pub angle: Option<f64>,
    /// The reference angles: the rotation is `to − from`.
    #[serde(default)]
    pub reference: Option<Reference>,
    /// Whether the entities are copied instead of turned in place: the source stays.
    #[serde(default)]
    pub copy: bool,
}

/// Parameters of `draftbuilder.modify.scale`: a factor, or the change from a reference length to a
/// new one.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScaleParams {
    /// The entities to scale; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The base point, in drawing units.
    pub base: [f64; 2],
    /// The factor, above zero. Give it, or `reference`.
    #[serde(default)]
    pub factor: Option<f64>,
    /// The reference lengths in drawing units: the factor is `to / from`.
    #[serde(default)]
    pub reference: Option<Reference>,
    /// Whether the entities are copied instead of scaled in place: the source stays.
    #[serde(default)]
    pub copy: bool,
}

/// Parameters of `draftbuilder.modify.mirror`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorParams {
    /// The entities to mirror; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The first point of the mirror line, in drawing units.
    pub first: [f64; 2],
    /// The second point of the mirror line, in drawing units.
    pub second: [f64; 2],
    /// Whether the source entities are erased (the mirrored copies stay). Default: they are kept.
    #[serde(default)]
    pub erase_source: bool,
}

/// One pair of align points: a source point and its destination.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AlignPair {
    /// The source point, in drawing units.
    pub from: [f64; 2],
    /// The destination point, in drawing units.
    pub to: [f64; 2],
}

/// Parameters of `draftbuilder.modify.align`: one, two or three pairs of points. One pair moves;
/// two pairs also turn the entities (and scale them when `scale` is set); three pairs must describe
/// the same similarity.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AlignParams {
    /// The entities to align; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The point pairs, one to three.
    #[schemars(length(min = 1, max = 3))]
    pub pairs: Vec<AlignPair>,
    /// Whether two or three pairs also scale the entities to the distance between the destinations.
    #[serde(default)]
    pub scale: bool,
}

const MOVE_LABEL: Message = Message {
    message_id: "db-cmd-modify-move-label",
};
const MOVE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-move-description",
};
const COPY_LABEL: Message = Message {
    message_id: "db-cmd-modify-copy-label",
};
const COPY_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-copy-description",
};
const ROTATE_LABEL: Message = Message {
    message_id: "db-cmd-modify-rotate-label",
};
const ROTATE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-rotate-description",
};
const SCALE_LABEL: Message = Message {
    message_id: "db-cmd-modify-scale-label",
};
const SCALE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-scale-description",
};
const MIRROR_LABEL: Message = Message {
    message_id: "db-cmd-modify-mirror-label",
};
const MIRROR_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-mirror-description",
};
const ALIGN_LABEL: Message = Message {
    message_id: "db-cmd-modify-align-label",
};
const ALIGN_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-align-description",
};
const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};
const BASE: Message = Message {
    message_id: "db-cmd-modify-base",
};
const SECOND: Message = Message {
    message_id: "db-cmd-modify-second",
};
const TARGET: Message = Message {
    message_id: "db-cmd-modify-target",
};
const ANGLE: Message = Message {
    message_id: "db-cmd-modify-angle",
};
const REFERENCE_FROM: Message = Message {
    message_id: "db-cmd-modify-reference-from",
};
const REFERENCE_TO: Message = Message {
    message_id: "db-cmd-modify-reference-to",
};
const FACTOR: Message = Message {
    message_id: "db-cmd-modify-factor",
};
const REFERENCE_SECOND: Message = Message {
    message_id: "db-cmd-modify-reference-second",
};
const REFERENCE_LENGTH: Message = Message {
    message_id: "db-cmd-modify-reference-length",
};
const NEW_LENGTH: Message = Message {
    message_id: "db-cmd-modify-new-length",
};
const FIRST_POINT: Message = Message {
    message_id: "db-cmd-modify-first-point",
};
const SECOND_POINT: Message = Message {
    message_id: "db-cmd-modify-second-point",
};
const ERASE_SOURCE: Message = Message {
    message_id: "db-cmd-modify-erase-source",
};
const SOURCE: Message = Message {
    message_id: "db-cmd-modify-source",
};
const DESTINATION: Message = Message {
    message_id: "db-cmd-modify-destination",
};
const OPTION_COPY: Message = Message {
    message_id: "db-cmd-modify-option-copy",
};
const OPTION_REFERENCE: Message = Message {
    message_id: "db-cmd-modify-option-reference",
};
const OPTION_SCALE: Message = Message {
    message_id: "db-cmd-modify-option-scale",
};
const OPTION_END: Message = Message {
    message_id: "db-cmd-modify-option-end",
};
const OPTION_ARRAY: Message = Message {
    message_id: "db-cmd-modify-option-array",
};
const OPTION_SPACED: Message = Message {
    message_id: "db-cmd-modify-option-spaced",
};
const OPTION_FITTED: Message = Message {
    message_id: "db-cmd-modify-option-fitted",
};
const COUNT: Message = Message {
    message_id: "db-cmd-modify-count",
};
const ARRAY_SECOND: Message = Message {
    message_id: "db-cmd-modify-array-second",
};
const ARRAY_FIT: Message = Message {
    message_id: "db-cmd-modify-array-fit",
};
const OPTION_SINGLE: Message = Message {
    message_id: "db-cmd-modify-option-single",
};
const OPTION_YES: Message = Message {
    message_id: "db-cmd-modify-option-yes",
};
const OPTION_NO: Message = Message {
    message_id: "db-cmd-modify-option-no",
};

fn option(id: &str, label: Message) -> KeywordOption {
    KeywordOption::new(id, label.message_id)
}

/// The objects answer of every transform's prompt.
fn objects_prompt() -> Prompt {
    Prompt::new("objects", OBJECTS.message_id, entity_selection())
}

/// The objects, as the typed form takes them: identifier texts.
fn objects_json(state: &PromptState<'_>) -> Option<serde_json::Value> {
    selection_answer(state, "objects").map(|ids| serde_json::json!(ids))
}

/// The number of keyword answers `copy` that the prompt has taken: an odd number means copy.
fn copy_toggled(state: &PromptState<'_>, key: &str) -> bool {
    state
        .answers
        .iter()
        .filter(|a| {
            a.key == key && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == "copy")
        })
        .count()
        % 2
        == 1
}

fn reference_asked(state: &PromptState<'_>, key: &str) -> bool {
    state.answers.iter().any(|a| {
        a.key == key && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == "reference")
    })
}

// ----- move -------------------------------------------------------------------------------------

fn move_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = objects_json(state) else {
        return Ok(Step::Ask(objects_prompt()));
    };
    let Some(base) = point_answer(state, "base") else {
        return Ok(Step::Ask(Prompt::new("base", BASE.message_id, point())));
    };
    let Some(second) = point_answer(state, "second") else {
        return Ok(Step::Ask(Prompt::new("second", SECOND.message_id, point())));
    };
    done(serde_json::json!({"objects": objects, "base": base, "second": second}))
}

fn move_check(
    state: &PromptState<'_>,
    _: &Prompt,
    input: &arc_command::PromptInput,
) -> arc_base::Result<()> {
    if let (Some(base), arc_command::PromptInput::Point(p)) = (point_answer(state, "base"), input)
        && state.answer("second").is_none()
    {
        let second = drawing_point(state.snapshot, *p);
        if (second.x - base[0]).hypot(second.y - base[1]) <= Tolerance::DEFAULT.linear {
            return Err(Error::new(codes::POINTS_COLLINEAR));
        }
    }
    Ok(())
}

fn move_run(ctx: &mut Context<'_>, p: MoveParams) -> Result<Changed> {
    let displacement = match (p.displacement, p.base, p.second) {
        (Some(d), None, None) => d,
        (None, Some(b), Some(s)) => [s[0] - b[0], s[1] - b[1]],
        _ => return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "displacement")),
    };
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    refuse_locked(ctx.snapshot(), &ids)?;
    transform_entities(ctx, &ids, translation(displacement[0], displacement[1]))?;
    ctx.set_selection(ids.clone())?;
    Ok(changed(ids))
}

// ----- copy -------------------------------------------------------------------------------------

fn copy_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = objects_json(state) else {
        return Ok(Step::Ask(objects_prompt()));
    };
    let Some(base) = point_answer(state, "base") else {
        return Ok(Step::Ask(Prompt::new("base", BASE.message_id, point())));
    };
    if reference_keyword(state, "target", "array") {
        return copy_array_next(state, objects, base);
    }
    let targets: Vec<[f64; 2]> = state
        .answers
        .iter()
        .filter(|a| a.key == "target")
        .filter_map(|a| match &a.input {
            arc_command::PromptInput::Point(p) => {
                let q = drawing_point(state.snapshot, *p);
                Some([q.x, q.y])
            }
            _ => None,
        })
        .collect();
    let single = state.answers.iter().any(|a| {
        a.key == "target"
            && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == "single")
    });
    let finished = single
        || state.answers.iter().any(|a| {
            a.key == "target"
                && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == "end")
        });
    if finished && !targets.is_empty() {
        return done(serde_json::json!({"objects": objects, "base": base, "targets": targets}));
    }
    let mut prompt = Prompt::new("target", TARGET.message_id, point()).or(Accept::keywords([
        option("single", OPTION_SINGLE),
        option("array", OPTION_ARRAY),
        option("end", OPTION_END),
    ]));
    if !targets.is_empty() {
        prompt = prompt.with_default(arc_command::PromptInput::Keyword("end".to_owned()));
    }
    Ok(Step::Ask(prompt))
}

/// Whether the keyword `word` was answered for the prompt `key`.
fn reference_keyword(state: &PromptState<'_>, key: &str, word: &str) -> bool {
    state.answers.iter().any(|a| {
        a.key == key && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == word)
    })
}

/// The array form of copy: the count, the second point, then spaced or fitted (spaced by default).
fn copy_array_next(
    state: &PromptState<'_>,
    objects: serde_json::Value,
    base: [f64; 2],
) -> arc_base::Result<Step> {
    let Some(count) = super::count_answer(state, "count") else {
        return Ok(Step::Ask(Prompt::new(
            "count",
            COUNT.message_id,
            super::count_accept(),
        )));
    };
    let Some(second) = point_answer(state, "array_second") else {
        return Ok(Step::Ask(Prompt::new(
            "array_second",
            ARRAY_SECOND.message_id,
            point(),
        )));
    };
    let fitted = reference_keyword(state, "array_fit", "fitted");
    if !fitted && !reference_keyword(state, "array_fit", "spaced") {
        return Ok(Step::Ask(
            Prompt::new(
                "array_fit",
                ARRAY_FIT.message_id,
                Accept::keywords([
                    option("spaced", OPTION_SPACED),
                    option("fitted", OPTION_FITTED),
                ]),
            )
            .with_default(arc_command::PromptInput::Keyword("spaced".to_owned())),
        ));
    }
    done(serde_json::json!({
        "objects": objects, "base": base,
        "array": {"second": second, "count": count, "fitted": fitted}
    }))
}

/// Refuses an `end` or `single` before any target.
fn copy_check(state: &PromptState<'_>, _: &Prompt, input: &arc_command::PromptInput) -> Result<()> {
    let ends = matches!(input, arc_command::PromptInput::Keyword(k) if k == "end" || k == "single");
    let has_target = state
        .answers
        .iter()
        .any(|a| a.key == "target" && matches!(a.input, arc_command::PromptInput::Point(_)));
    if ends && !has_target {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "targets"));
    }
    Ok(())
}

/// The displacements of an array of copies: copy `k` (from one) is `k` times the spacing, the spacing being
/// the base-to-second vector (spaced) or that vector over `count` (fitted). The copies of every object must fit
/// the array-length limit, which is checked before any displacement is made.
fn array_displacements(
    base: [f64; 2],
    array: CopyArray,
    objects: usize,
    limit: usize,
) -> Result<Vec<[f64; 2]>> {
    if array.count == 0 {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "count"));
    }
    let count = usize::try_from(array.count).unwrap_or(usize::MAX);
    if objects
        .checked_mul(count)
        .is_none_or(|copies| copies > limit)
    {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", "array_length"));
    }
    if !array.second.iter().all(|v| v.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "second"));
    }
    let (dx, dy) = (array.second[0] - base[0], array.second[1] - base[1]);
    if dx.hypot(dy) <= Tolerance::DEFAULT.linear {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "second"));
    }
    let divisor = if array.fitted {
        f64::from(array.count)
    } else {
        1.0
    };
    let (sx, sy) = (dx / divisor, dy / divisor);
    Ok((1..=count)
        .map(|k| [sx * k as f64, sy * k as f64])
        .collect())
}

fn copy_run(ctx: &mut Context<'_>, p: CopyParams) -> Result<Changed> {
    // Everything is checked before the first copy: the points are finite, and the copies (objects times
    // targets or times the array count) stay within the session's array-length limit.
    if !p.base.iter().all(|v| v.is_finite()) || !p.targets.iter().flatten().all(|v| v.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "targets"));
    }
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let limit = usize::try_from(ctx.limits().array_length).unwrap_or(usize::MAX);
    let displacements: Vec<[f64; 2]> = match p.array {
        Some(array) => {
            if !p.targets.is_empty() {
                return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "targets"));
            }
            array_displacements(p.base, array, ids.len(), limit)?
        }
        None => {
            if p.targets.is_empty() {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "targets"));
            }
            p.targets
                .iter()
                .map(|t| [t[0] - p.base[0], t[1] - p.base[1]])
                .collect()
        }
    };
    if ids
        .len()
        .checked_mul(displacements.len())
        .is_none_or(|copies| copies > limit)
    {
        return Err(Error::new(codes::LIMIT_EXCEEDED).with_detail("limit", "array_length"));
    }
    let mut out = Vec::new();
    for displacement in displacements {
        let copies = copy_entities(ctx, &ids)?;
        transform_entities(ctx, &copies, translation(displacement[0], displacement[1]))?;
        out.extend(copies);
    }
    ctx.set_selection(out.clone())?;
    Ok(changed(out))
}

// ----- rotate -----------------------------------------------------------------------------------

/// A reference angle of the rotate prompt: a typed angle, or the direction from `base` to a picked point.
fn reference_angle(state: &PromptState<'_>, key: &str, base: [f64; 2]) -> Option<f64> {
    match state.answer(key)? {
        arc_command::PromptInput::Angle(radians) => Some(*radians),
        arc_command::PromptInput::Point(p) => {
            let q = drawing_point(state.snapshot, *p);
            Some((q.y - base[1]).atan2(q.x - base[0]))
        }
        _ => None,
    }
}

/// Refuses a picked reference point that is the base point itself: it gives no direction.
fn reference_check(
    state: &PromptState<'_>,
    prompt: &Prompt,
    input: &arc_command::PromptInput,
) -> Result<()> {
    if let (arc_command::PromptInput::Point(p), Some(base)) = (input, point_answer(state, "base")) {
        let q = drawing_point(state.snapshot, *p);
        if (q.x - base[0]).hypot(q.y - base[1]) <= Tolerance::DEFAULT.linear {
            return Err(Error::new(codes::VALUE_INVALID).with_detail("field", prompt.key.clone()));
        }
    }
    Ok(())
}

fn rotate_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = objects_json(state) else {
        return Ok(Step::Ask(objects_prompt()));
    };
    let Some(base) = point_answer(state, "base") else {
        return Ok(Step::Ask(Prompt::new("base", BASE.message_id, point())));
    };
    let copy = copy_toggled(state, "angle");
    if reference_asked(state, "angle") {
        let Some(from) = reference_angle(state, "reference_from", base) else {
            return Ok(Step::Ask(
                Prompt::new("reference_from", REFERENCE_FROM.message_id, Accept::angle())
                    .or(point()),
            ));
        };
        let Some(to) = reference_angle(state, "reference_to", base) else {
            return Ok(Step::Ask(
                Prompt::new("reference_to", REFERENCE_TO.message_id, Accept::angle()).or(point()),
            ));
        };
        return done(serde_json::json!({
            "objects": objects, "base": base, "copy": copy,
            "reference": {"from": from, "to": to}
        }));
    }
    if let Some(angle) = angle_answer(state, "angle") {
        return done(
            serde_json::json!({"objects": objects, "base": base, "angle": angle, "copy": copy}),
        );
    }
    Ok(Step::Ask(offered(
        state,
        ROTATE_ANGLE,
        Prompt::new("angle", ANGLE.message_id, Accept::angle()).or(Accept::keywords([
            option("copy", OPTION_COPY),
            option("reference", OPTION_REFERENCE),
        ])),
    )))
}

fn rotate_run(ctx: &mut Context<'_>, p: RotateParams) -> Result<Changed> {
    let angle = match (p.angle, p.reference) {
        (Some(a), None) => a,
        (None, Some(r)) => r.to - r.from,
        _ => return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "angle")),
    };
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let target = if p.copy {
        copy_entities(ctx, &ids)?
    } else {
        refuse_locked(ctx.snapshot(), &ids)?;
        ids
    };
    transform_entities(ctx, &target, rotation_about(p.base, angle))?;
    ctx.set_selection(target.clone())?;
    Ok(changed(target))
}

// ----- scale ------------------------------------------------------------------------------------

fn scale_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = objects_json(state) else {
        return Ok(Step::Ask(objects_prompt()));
    };
    let Some(base) = point_answer(state, "base") else {
        return Ok(Step::Ask(Prompt::new("base", BASE.message_id, point())));
    };
    let copy = copy_toggled(state, "factor");
    if reference_asked(state, "factor") {
        // The reference length is a typed distance, or the distance between two picked points; the new length
        // is a typed distance, or the distance from the base to a picked point.
        let from = match state.answer("reference_length") {
            Some(arc_command::PromptInput::Point(p)) => {
                let Some(second) = point_answer(state, "reference_second") else {
                    return Ok(Step::Ask(Prompt::new(
                        "reference_second",
                        REFERENCE_SECOND.message_id,
                        point(),
                    )));
                };
                let first = drawing_point(state.snapshot, *p);
                (second[0] - first.x).hypot(second[1] - first.y)
            }
            _ => match distance_answer(state, "reference_length") {
                Some(length) => length,
                None => {
                    return Ok(Step::Ask(
                        Prompt::new(
                            "reference_length",
                            REFERENCE_LENGTH.message_id,
                            Accept::distance(),
                        )
                        .or(point()),
                    ));
                }
            },
        };
        let to = match state.answer("new_length") {
            Some(arc_command::PromptInput::Point(p)) => {
                let q = drawing_point(state.snapshot, *p);
                (q.x - base[0]).hypot(q.y - base[1])
            }
            _ => match distance_answer(state, "new_length") {
                Some(length) => length,
                None => {
                    return Ok(Step::Ask(
                        Prompt::new("new_length", NEW_LENGTH.message_id, Accept::distance())
                            .or(point()),
                    ));
                }
            },
        };
        return done(serde_json::json!({
            "objects": objects, "base": base, "copy": copy,
            "reference": {"from": from, "to": to}
        }));
    }
    if let Some(factor) = number_answer(state, "factor") {
        return done(
            serde_json::json!({"objects": objects, "base": base, "factor": factor, "copy": copy}),
        );
    }
    Ok(Step::Ask(offered(
        state,
        SCALE_FACTOR,
        Prompt::new("factor", FACTOR.message_id, Accept::number(Dimension::NONE)).or(
            Accept::keywords([
                option("copy", OPTION_COPY),
                option("reference", OPTION_REFERENCE),
            ]),
        ),
    )))
}

fn scale_check(
    state: &PromptState<'_>,
    prompt: &Prompt,
    input: &arc_command::PromptInput,
) -> arc_base::Result<()> {
    match input {
        arc_command::PromptInput::Number(q) if !(q.value().is_finite() && q.value() > 0.0) => {
            Err(Error::new(codes::VALUE_INVALID).with_detail("field", "factor"))
        }
        // A reference length of zero or a new length from the base point itself gives no scale.
        arc_command::PromptInput::Point(p) if prompt.key == "reference_second" => {
            if let Some(arc_command::PromptInput::Point(a)) = state.answer("reference_length") {
                let a = drawing_point(state.snapshot, *a);
                let q = drawing_point(state.snapshot, *p);
                if (q.x - a.x).hypot(q.y - a.y) <= Tolerance::DEFAULT.linear {
                    return Err(
                        Error::new(codes::VALUE_INVALID).with_detail("field", "reference_second")
                    );
                }
            }
            Ok(())
        }
        arc_command::PromptInput::Point(_) => reference_check(state, prompt, input),
        _ => Ok(()),
    }
}

fn scale_run(ctx: &mut Context<'_>, p: ScaleParams) -> Result<Changed> {
    let factor = match (p.factor, p.reference) {
        (Some(f), None) => f,
        (None, Some(r)) => {
            if !(r.from.is_finite() && r.from > 0.0) {
                return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "reference"));
            }
            r.to / r.from
        }
        _ => return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "factor")),
    };
    if !(factor.is_finite() && factor > 0.0) {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "factor"));
    }
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let target = if p.copy {
        copy_entities(ctx, &ids)?
    } else {
        refuse_locked(ctx.snapshot(), &ids)?;
        ids
    };
    transform_entities(ctx, &target, scale_about(p.base, factor))?;
    ctx.set_selection(target.clone())?;
    Ok(changed(target))
}

// ----- mirror -----------------------------------------------------------------------------------

fn mirror_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = objects_json(state) else {
        return Ok(Step::Ask(objects_prompt()));
    };
    let Some(first) = point_answer(state, "first") else {
        return Ok(Step::Ask(Prompt::new(
            "first",
            FIRST_POINT.message_id,
            point(),
        )));
    };
    let Some(second) = point_answer(state, "second") else {
        return Ok(Step::Ask(Prompt::new(
            "second",
            SECOND_POINT.message_id,
            point(),
        )));
    };
    let erase = match keyword_answer(state, "erase") {
        Some(answer) => answer == "yes",
        None => {
            return Ok(Step::Ask(
                Prompt::new(
                    "erase",
                    ERASE_SOURCE.message_id,
                    Accept::keywords([option("yes", OPTION_YES), option("no", OPTION_NO)]),
                )
                .with_default(arc_command::PromptInput::Keyword("no".to_owned())),
            ));
        }
    };
    done(
        serde_json::json!({"objects": objects, "first": first, "second": second, "erase_source": erase}),
    )
}

fn mirror_run(ctx: &mut Context<'_>, p: MirrorParams) -> Result<Changed> {
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let transform = mirror_across(p.first, p.second)?;
    if p.erase_source {
        refuse_locked(ctx.snapshot(), &ids)?;
    }
    let copies = copy_entities(ctx, &ids)?;
    transform_entities(ctx, &copies, transform)?;
    if p.erase_source {
        erase_entities(ctx, &ids)?;
    }
    ctx.set_selection(copies.clone())?;
    Ok(changed(copies))
}

// ----- align ------------------------------------------------------------------------------------

/// The similarity of the first two pairs (and the scale rule), as a transform.
fn similarity(pairs: &[AlignPair], scale: bool) -> Result<TransformParam> {
    let first = pairs[0];
    let (fx, fy) = (first.from[0], first.from[1]);
    if pairs.len() == 1 {
        return Ok(translation(first.to[0] - fx, first.to[1] - fy));
    }
    let second = pairs[1];
    let v = (second.from[0] - fx, second.from[1] - fy);
    let w = (second.to[0] - first.to[0], second.to[1] - first.to[1]);
    let (lv, lw) = (v.0.hypot(v.1), w.0.hypot(w.1));
    if !(lv.is_finite()
        && lv > Tolerance::DEFAULT.linear
        && lw.is_finite()
        && lw > Tolerance::DEFAULT.linear)
    {
        return Err(Error::new(codes::POINTS_COLLINEAR));
    }
    let angle = w.1.atan2(w.0) - v.1.atan2(v.0);
    let factor = if scale { lw / lv } else { 1.0 };
    let (s, c) = angle.sin_cos();
    let rx = factor * (c * fx - s * fy);
    let ry = factor * (s * fx + c * fy);
    Ok(TransformParam {
        mirror: None,
        scale: factor,
        rotation: angle,
        translation: [first.to[0] - rx, first.to[1] - ry],
    })
}

fn align_pairs(state: &PromptState<'_>) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let mut sources = Vec::new();
    let mut destinations = Vec::new();
    let mut pending: Option<[f64; 2]> = None;
    for answer in state.answers {
        let arc_command::PromptInput::Point(p) = &answer.input else {
            continue;
        };
        let q = drawing_point(state.snapshot, *p);
        let q = [q.x, q.y];
        match answer.key.as_str() {
            "source" => pending = Some(q),
            "destination" => {
                if let Some(from) = pending.take() {
                    sources.push(from);
                    destinations.push(q);
                }
            }
            _ => {}
        }
    }
    (sources, destinations)
}

fn align_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = objects_json(state) else {
        return Ok(Step::Ask(objects_prompt()));
    };
    let (sources, destinations) = align_pairs(state);
    let scale = state
        .answers
        .iter()
        .filter(|a| {
            a.key == "source"
                && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == "scale")
        })
        .count()
        % 2
        == 1;
    let ended = state.answers.iter().any(|a| {
        a.key == "source" && matches!(&a.input, arc_command::PromptInput::Keyword(k) if k == "end")
    });
    if sources.len() == 3 || (ended && !sources.is_empty()) {
        let pairs: Vec<serde_json::Value> = sources
            .iter()
            .zip(&destinations)
            .map(|(from, to)| serde_json::json!({"from": from, "to": to}))
            .collect();
        return done(serde_json::json!({"objects": objects, "pairs": pairs, "scale": scale}));
    }
    if state
        .answers
        .last()
        .is_some_and(|a| a.key == "source" && matches!(a.input, arc_command::PromptInput::Point(_)))
    {
        return Ok(Step::Ask(Prompt::new(
            "destination",
            DESTINATION.message_id,
            point(),
        )));
    }
    Ok(Step::Ask(
        Prompt::new("source", SOURCE.message_id, point()).or(Accept::keywords([
            option("scale", OPTION_SCALE),
            option("end", OPTION_END),
        ])),
    ))
}

fn align_run(ctx: &mut Context<'_>, p: AlignParams) -> Result<Changed> {
    if p.pairs.is_empty() || p.pairs.len() > 3 {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "pairs"));
    }
    let ids = entities(ctx.snapshot(), ctx.limits(), &p.objects)?;
    let transform = similarity(&p.pairs, p.scale)?;
    if p.pairs.len() == 3 {
        let third = p.pairs[2];
        let [x, y] = third.from;
        let mapped = Point2::new(
            transform.scale * (transform.rotation.cos() * x - transform.rotation.sin() * y)
                + transform.translation[0],
            transform.scale * (transform.rotation.sin() * x + transform.rotation.cos() * y)
                + transform.translation[1],
        );
        let error = (mapped.x - third.to[0]).hypot(mapped.y - third.to[1]);
        if !(error.is_finite() && error <= 1e-6 * (1.0 + third.to[0].abs() + third.to[1].abs())) {
            return Err(Error::new(codes::ALIGN_NOT_SIMILARITY));
        }
    }
    refuse_locked(ctx.snapshot(), &ids)?;
    transform_entities(ctx, &ids, transform)?;
    ctx.set_selection(ids.clone())?;
    Ok(changed(ids))
}

// ----- registration -----------------------------------------------------------------------------

/// The transform commands of the area.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.modify.move",
            MOVE_LABEL,
            MOVE_DESCRIPTION,
            move_run,
        )
        .with_prompt(machine(move_next, move_check)),
        super::spec(
            "draftbuilder.modify.copy",
            COPY_LABEL,
            COPY_DESCRIPTION,
            copy_run,
        )
        .with_prompt(machine(copy_next, copy_check)),
        super::spec(
            "draftbuilder.modify.rotate",
            ROTATE_LABEL,
            ROTATE_DESCRIPTION,
            remembering(rotate_run, |p: &RotateParams, _| {
                p.angle.map(|a| vec![(ROTATE_ANGLE, a)]).unwrap_or_default()
            }),
        )
        .with_prompt(machine(rotate_next, reference_check)),
        super::spec(
            "draftbuilder.modify.scale",
            SCALE_LABEL,
            SCALE_DESCRIPTION,
            remembering(scale_run, |p: &ScaleParams, _| {
                p.factor
                    .map(|f| vec![(SCALE_FACTOR, f)])
                    .unwrap_or_default()
            }),
        )
        .with_prompt(machine(scale_next, scale_check)),
        super::spec(
            "draftbuilder.modify.mirror",
            MIRROR_LABEL,
            MIRROR_DESCRIPTION,
            mirror_run,
        )
        .with_prompt(machine(mirror_next, crate::draw::create::accept_all)),
        super::spec(
            "draftbuilder.modify.align",
            ALIGN_LABEL,
            ALIGN_DESCRIPTION,
            align_run,
        )
        .with_prompt(machine(align_next, crate::draw::create::accept_all)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_about_a_base_keeps_the_base_fixed() {
        let t = rotation_about([2.0, 3.0], std::f64::consts::FRAC_PI_2);
        // The base maps to itself: R b + t = b.
        let (s, c) = t.rotation.sin_cos();
        let mapped = [
            c * 2.0 - s * 3.0 + t.translation[0],
            s * 2.0 + c * 3.0 + t.translation[1],
        ];
        assert!((mapped[0] - 2.0).abs() < 1e-12 && (mapped[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn mirror_across_the_x_axis_offset_by_y() {
        let t = mirror_across([0.0, 1.0], [5.0, 1.0]).unwrap();
        assert_eq!(t.mirror, Some([1.0, 0.0]));
        // M maps (x, y) to (x, -y); (2, 3) mirrors across y = 1 to (2, -1).
        let mapped = [2.0 + t.translation[0], -3.0 + t.translation[1]];
        assert!(
            (mapped[0] - 2.0).abs() < 1e-12 && (mapped[1] + 1.0).abs() < 1e-12,
            "{mapped:?}"
        );
    }
}
