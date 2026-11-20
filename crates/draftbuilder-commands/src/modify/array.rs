//! Arrays (`draftbuilder.modify.array_rect`, typed `ARRAYRECT`; `array_polar`, `ARRAYPOLAR`;
//! `array_path`, `ARRAYPATH`). The selected entities become the content of a new anonymous block
//! definition, placed at the origin, and an array entity inserts that definition in the chosen
//! layout: rows and columns, a polar pattern about a center, or a path. The first item is the
//! source in its place, because the block is inserted at the origin with no scale or rotation.
//!
//! The array is stored as the foundation stores arrays (`core.draft.array`), so an application
//! without arrays sees a block reference. The layout values are checked by the foundation's own
//! parameter validation before the array is created. The array takes the layer of its first source
//! entity; the items keep the layers of the content.
//!
//! The selected entities are moved into the definition, as the foundation's block definition moves
//! them. An entity that refers to another entity (a dimension, a hatch with a boundary) is refused,
//! since the reference would not follow the move. A path array refers to its path entity, which must
//! not be one of the selected entities. Associative editing of an array (ARRAYEDIT) is not built.

use arc_base::math::{Point2, Vector3};
use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{Accept, CommandSpec, Context, IdParam, Prompt, PromptInput, PromptState, Step};
use arc_doc::{Header, Snapshot, Value};
use arc_draft::array::ARRAY;
use arc_draft::array::{Params, field};
use arc_draft::types::{self, BLOCK, entity, table};
use arc_draft_dxf::fields::name_of;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    entities, entity_selection, point, point_answer, refuse_associated_copy, refuse_locked,
    selection_answer,
};
use crate::draw::create::{Creator, done, machine};
use crate::error::{Message, codes};

/// Parameters of `draftbuilder.modify.array_rect`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RectParams {
    /// The entities that form the item; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The number of rows (along the array's y axis).
    pub rows: i64,
    /// The number of columns (along its x axis).
    pub columns: i64,
    /// The distance between rows, drawing units.
    #[serde(default)]
    pub row_spacing: f64,
    /// The distance between columns, drawing units.
    #[serde(default)]
    pub column_spacing: f64,
    /// The angle of the array's x axis in radians.
    #[serde(default)]
    pub angle: f64,
}

/// Parameters of `draftbuilder.modify.array_polar`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolarParams {
    /// The entities that form the item; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The center of the pattern, drawing units.
    pub center: [f64; 2],
    /// The number of items, the first being the source.
    pub count: i64,
    /// The angle from the first to the last item in radians; a full turn when absent.
    #[serde(default)]
    pub total_angle: Option<f64>,
    /// Whether the items turn with their position; true when absent.
    #[serde(default)]
    pub rotate_items: Option<bool>,
}

/// Parameters of `draftbuilder.modify.array_path`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathParams {
    /// The entities that form the item; the selection keywords are accepted.
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The path entity: a line, arc, polyline or spline.
    pub path: IdParam,
    /// The number of items; used when `spacing` is zero.
    #[serde(default)]
    pub count: Option<i64>,
    /// The distance between items along the path, drawing units; zero uses `count`.
    #[serde(default)]
    pub spacing: f64,
    /// Whether the items turn with the path; true when absent.
    #[serde(default)]
    pub align: Option<bool>,
}

/// The array created: its identifier and the anonymous block definition it inserts.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Arrayed {
    /// The array entity.
    pub objects: Vec<IdParam>,
    /// The block definition holding the item.
    pub block: IdParam,
}

const RECT_LABEL: Message = Message {
    message_id: "db-cmd-modify-array-rect-label",
};
const RECT_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-array-rect-description",
};
const POLAR_LABEL: Message = Message {
    message_id: "db-cmd-modify-array-polar-label",
};
const POLAR_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-array-polar-description",
};
const PATH_LABEL: Message = Message {
    message_id: "db-cmd-modify-array-path-label",
};
const PATH_DESCRIPTION: Message = Message {
    message_id: "db-cmd-modify-array-path-description",
};

/// A fresh anonymous block name: `*A` followed by the first number no block has.
fn anonymous_name(snapshot: &Snapshot) -> String {
    let taken: Vec<String> = snapshot
        .iter_type(BLOCK)
        .map(|iter| {
            iter.filter_map(|(_, o)| name_of(snapshot, Some(o.id)))
                .collect()
        })
        .unwrap_or_default();
    (1..)
        .map(|n| format!("*A{n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_default()
}

/// Creates the block definition of `objects`: their content moves into it, placed at the origin.
fn define(ctx: &mut Context<'_>, objects: &[ObjectId]) -> Result<ObjectId> {
    let snapshot = ctx.snapshot().clone();
    let name = anonymous_name(&snapshot);
    let mut values = types::defaults(snapshot.registry(), BLOCK);
    values[table::NAME] = Some(Value::String(name));
    values[table::BLOCK_BASE] = Some(Value::Point2(Point2::origin()));
    values[table::BLOCK_ATTRIBUTES] = Some(Value::List(Vec::new()));
    let block = ctx.new_id::<ObjectId>();
    ctx.txn()?.create(BLOCK, block, Header::default(), values)?;
    let txn = ctx.txn()?;
    for id in objects {
        txn.relink(*id, Some(block))?;
    }
    Ok(block)
}

/// Creates the array entity for `block` with the layout values `fields`, on the layer of the first
/// source entity, owned by the model space; validated with the foundation's own rules.
fn insert(
    ctx: &mut Context<'_>,
    block: ObjectId,
    layer: Option<ObjectId>,
    fields: Vec<(usize, Value)>,
) -> Result<ObjectId> {
    let snapshot = ctx.snapshot().clone();
    let mut check = vec![None; entity::KIND + 18];
    let mut kind = vec![(field::BLOCK, Value::Reference(block))];
    kind.extend(fields);
    for (index, value) in &kind {
        if let Some(slot) = check.get_mut(*index) {
            *slot = Some(value.clone());
        }
    }
    Params::from_values(&check).validate()?;
    let mut creator = Creator::new(&snapshot)?;
    let space = crate::model_space(&snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
    let mut values = types::defaults(snapshot.registry(), ARRAY);
    values[entity::LAYER] = layer.map(Value::Reference);
    values[field::BLOCK] = Some(Value::Reference(block));
    values[field::POSITION] = Some(Value::Point2(Point2::origin()));
    values[field::SCALE] = Some(Value::Vector3(Vector3::new(1.0, 1.0, 1.0)));
    values[field::ROTATION] = Some(Value::F64(0.0));
    for (index, value) in kind {
        values[index] = Some(value);
    }
    creator.add_values(ctx, ARRAY, Some(space), values)
}

/// The source entities of an array: validated, locked-checked and free of references to entities.
fn sources(
    ctx: &Context<'_>,
    list: &[IdParam],
    exclude: Option<ObjectId>,
) -> Result<Vec<ObjectId>> {
    let snapshot = ctx.snapshot();
    let ids = entities(snapshot, ctx.limits(), list)?;
    if let Some(path) = exclude
        && ids.contains(&path)
    {
        return Err(Error::new(codes::PARAMS_CONFLICT).with_detail("field", "path"));
    }
    refuse_locked(snapshot, &ids)?;
    for id in &ids {
        refuse_associated_copy(snapshot, *id)?;
    }
    Ok(ids)
}

fn layer_of_first(snapshot: &Snapshot, ids: &[ObjectId]) -> Option<ObjectId> {
    let object = snapshot.get(snapshot.resolve(*ids.first()?).ok()?).ok()?;
    match object.values.get(entity::LAYER) {
        Some(Some(Value::Reference(layer))) => Some(*layer),
        _ => None,
    }
}

fn rect_run(ctx: &mut Context<'_>, p: RectParams) -> Result<Arrayed> {
    let ids = sources(ctx, &p.objects, None)?;
    let layer = layer_of_first(ctx.snapshot(), &ids);
    let block = define(ctx, &ids)?;
    let array = insert(
        ctx,
        block,
        layer,
        vec![
            (field::KIND, Value::Enum(0)),
            (field::ROWS, Value::I64(p.rows)),
            (field::COLUMNS, Value::I64(p.columns)),
            (field::ROW_SPACING, Value::F64(p.row_spacing)),
            (field::COLUMN_SPACING, Value::F64(p.column_spacing)),
            (field::ANGLE, Value::F64(p.angle)),
        ],
    )?;
    finish(ctx, array, block)
}

fn polar_run(ctx: &mut Context<'_>, p: PolarParams) -> Result<Arrayed> {
    let ids = sources(ctx, &p.objects, None)?;
    let layer = layer_of_first(ctx.snapshot(), &ids);
    let block = define(ctx, &ids)?;
    let array = insert(
        ctx,
        block,
        layer,
        vec![
            (field::KIND, Value::Enum(1)),
            (
                field::CENTER,
                Value::Point2(Point2::new(p.center[0], p.center[1])),
            ),
            (field::COUNT, Value::I64(p.count)),
            (
                field::TOTAL_ANGLE,
                Value::F64(p.total_angle.unwrap_or(std::f64::consts::TAU)),
            ),
            (
                field::ROTATE_ITEMS,
                Value::Bool(p.rotate_items.unwrap_or(true)),
            ),
        ],
    )?;
    finish(ctx, array, block)
}

fn path_run(ctx: &mut Context<'_>, p: PathParams) -> Result<Arrayed> {
    let ids = sources(ctx, &p.objects, Some(p.path.0))?;
    let layer = layer_of_first(ctx.snapshot(), &ids);
    let block = define(ctx, &ids)?;
    let array = insert(
        ctx,
        block,
        layer,
        vec![
            (field::KIND, Value::Enum(2)),
            (field::PATH, Value::Reference(p.path.0)),
            (field::COUNT, Value::I64(p.count.unwrap_or(1))),
            (field::SPACING, Value::F64(p.spacing)),
            (field::ALIGN, Value::Bool(p.align.unwrap_or(true))),
        ],
    )?;
    finish(ctx, array, block)
}

fn finish(ctx: &mut Context<'_>, array: ObjectId, block: ObjectId) -> Result<Arrayed> {
    ctx.set_selection(vec![array])?;
    Ok(Arrayed {
        objects: vec![IdParam(array)],
        block: IdParam(block),
    })
}

/// The prompt machine of the rectangular array: the entities, the rows, the columns, the spacings and
/// the angle.
fn rect_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        )));
    };
    let Some(rows) = whole(state, "rows") else {
        return Ok(Step::Ask(Prompt::new(
            "rows",
            ROWS.message_id,
            whole_accept(),
        )));
    };
    let Some(columns) = whole(state, "columns") else {
        return Ok(Step::Ask(Prompt::new(
            "columns",
            COLUMNS.message_id,
            whole_accept(),
        )));
    };
    let Some(row_spacing) = super::distance_answer(state, "row_spacing") else {
        return Ok(Step::Ask(Prompt::new(
            "row_spacing",
            ROW_SPACING.message_id,
            Accept::distance(),
        )));
    };
    let Some(column_spacing) = super::distance_answer(state, "column_spacing") else {
        return Ok(Step::Ask(Prompt::new(
            "column_spacing",
            COLUMN_SPACING.message_id,
            Accept::distance(),
        )));
    };
    let Some(angle) = super::angle_answer(state, "angle") else {
        return Ok(Step::Ask(
            Prompt::new("angle", ANGLE.message_id, Accept::angle())
                .with_default(PromptInput::Angle(0.0)),
        ));
    };
    done(serde_json::json!({
        "objects": objects, "rows": rows, "columns": columns,
        "row_spacing": row_spacing, "column_spacing": column_spacing, "angle": angle
    }))
}

/// The prompt machine of the polar array: the entities, the center, the count and the total angle.
fn polar_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        )));
    };
    let Some(center) = point_answer(state, "center") else {
        return Ok(Step::Ask(Prompt::new("center", CENTER.message_id, point())));
    };
    let Some(count) = whole(state, "count") else {
        return Ok(Step::Ask(Prompt::new(
            "count",
            COUNT.message_id,
            whole_accept(),
        )));
    };
    let Some(total_angle) = super::angle_answer(state, "total_angle") else {
        return Ok(Step::Ask(
            Prompt::new("total_angle", TOTAL.message_id, Accept::angle())
                .with_default(PromptInput::Angle(std::f64::consts::TAU)),
        ));
    };
    done(serde_json::json!({
        "objects": objects, "center": center, "count": count, "total_angle": total_angle
    }))
}

/// The prompt machine of the path array: the entities, the path, then the count.
fn path_next(state: &PromptState<'_>) -> arc_base::Result<Step> {
    let Some(objects) = selection_answer(state, "objects") else {
        return Ok(Step::Ask(Prompt::new(
            "objects",
            OBJECTS.message_id,
            entity_selection(),
        )));
    };
    let path = match state.answer("path") {
        Some(PromptInput::Pick(id)) => id.to_string(),
        _ => {
            return Ok(Step::Ask(Prompt::new(
                "path",
                PATH.message_id,
                Accept::Pick {
                    types: vec![
                        arc_draft::types::LINE.to_owned(),
                        arc_draft::types::ARC.to_owned(),
                        arc_draft::types::POLYLINE.to_owned(),
                        arc_draft::types::SPLINE.to_owned(),
                    ],
                },
            )));
        }
    };
    let Some(count) = whole(state, "count") else {
        return Ok(Step::Ask(Prompt::new(
            "count",
            COUNT.message_id,
            whole_accept(),
        )));
    };
    done(serde_json::json!({"objects": objects, "path": path, "count": count}))
}

/// The whole number of a dimensionless prompt answer.
fn whole(state: &PromptState<'_>, key: &str) -> Option<i64> {
    let value = super::number_answer(state, key)?;
    format!("{:.0}", value.round()).parse().ok()
}

fn whole_accept() -> Accept {
    Accept::Number {
        dimension: Dimension::NONE,
        min: Some(1.0),
        max: Some(10_000.0),
        integer: true,
    }
}

const OBJECTS: Message = Message {
    message_id: "db-cmd-modify-objects",
};
const ROWS: Message = Message {
    message_id: "db-cmd-modify-array-rows",
};
const COLUMNS: Message = Message {
    message_id: "db-cmd-modify-array-columns",
};
const ROW_SPACING: Message = Message {
    message_id: "db-cmd-modify-array-row-spacing",
};
const COLUMN_SPACING: Message = Message {
    message_id: "db-cmd-modify-array-column-spacing",
};
const ANGLE: Message = Message {
    message_id: "db-cmd-modify-array-angle",
};
const CENTER: Message = Message {
    message_id: "db-cmd-modify-array-center",
};
const COUNT: Message = Message {
    message_id: "db-cmd-modify-array-count",
};
const TOTAL: Message = Message {
    message_id: "db-cmd-modify-array-total-angle",
};
const PATH: Message = Message {
    message_id: "db-cmd-modify-array-path",
};

/// The commands of the family.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        super::spec(
            "draftbuilder.modify.array_rect",
            RECT_LABEL,
            RECT_DESCRIPTION,
            rect_run,
        )
        .with_prompt(machine(rect_next, crate::draw::create::accept_all)),
        super::spec(
            "draftbuilder.modify.array_polar",
            POLAR_LABEL,
            POLAR_DESCRIPTION,
            polar_run,
        )
        .with_prompt(machine(polar_next, crate::draw::create::accept_all)),
        super::spec(
            "draftbuilder.modify.array_path",
            PATH_LABEL,
            PATH_DESCRIPTION,
            path_run,
        )
        .with_prompt(machine(path_next, crate::draw::create::accept_all)),
    ]
}
