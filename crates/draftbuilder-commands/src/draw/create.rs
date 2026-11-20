//! Support shared by the drawing commands: where new entities go and with which properties, how
//! coordinates map between the prompts and the drawing, the creation of entities in the
//! transaction of a command, and the prompt machine adapter.
//!
//! New entities go to the model space, on layer `0` when the drawing has it, with the by-layer
//! appearance of the foundation's defaults. Taking the current layer and the current properties
//! of the drawing is not built yet: [`Creator::new`] is the one place that chooses them.
//!
//! Coordinates: the typed parameters of the commands are numbers in the drawing's units, the unit
//! its model space declares (`millimeter`, ...) and in meters for an unitless drawing. The prompts
//! answer in meters (the foundation's convention); [`from_meters`] converts them to the same units.

use std::sync::Arc;

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result};
use arc_command::{
    Context, IdParam, KeywordOption, Point, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_doc::{GeometryBytes, Header, Snapshot, Value};
use arc_draft::types::{self, ENTITIES, SPACE, SpaceUnit, entity};
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use schemars::JsonSchema;
use serde::Serialize;

use crate::error::{Message, codes};
use crate::select::candidates::default_layer;

/// The spacing of draw-order keys of entities created one after another: the spacing the
/// foundation leaves beyond the end of a container.
const ORDER_STEP: i64 = 1 << 20;

/// The result of a command that creates entities: their identifiers in creation order.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Created {
    /// The new entities, in the order they were created.
    pub ids: Vec<IdParam>,
}

/// The meters that one drawing unit measures: the unit's size for a unit, 1 for an unitless
/// drawing, whose coordinates are meters.
#[must_use]
pub(crate) fn meters_per_unit(snapshot: &Snapshot) -> f64 {
    crate::model_space(snapshot)
        .and_then(|space| snapshot.resolve(space).ok())
        .and_then(|handle| snapshot.get(handle).ok())
        .and_then(|object| {
            Reader::new(snapshot.registry(), SPACE, &object.values).enum_code("unit")
        })
        .and_then(SpaceUnit::of)
        .and_then(SpaceUnit::unit)
        .map_or(1.0, |unit| unit.scale)
}

/// A length in meters as a length in the drawing's units.
#[must_use]
pub(crate) fn from_meters(snapshot: &Snapshot, meters: f64) -> f64 {
    meters / meters_per_unit(snapshot)
}

/// A point of a prompt, in the drawing's units.
#[must_use]
pub(crate) fn drawing_point(snapshot: &Snapshot, point: Point) -> Point2 {
    Point2::new(
        from_meters(snapshot, point.x),
        from_meters(snapshot, point.y),
    )
}

/// Creates entities in the model space of a drawing, inside the transaction of a command. Keeps
/// the draw order of each new entity above the ones before it.
pub(crate) struct Creator {
    space: ObjectId,
    layer: Option<ObjectId>,
    next_order: i64,
    ids: Vec<IdParam>,
}

impl Creator {
    /// A creator for the model space of `snapshot`.
    ///
    /// # Errors
    ///
    /// `db.cmd.no_model_space` when the drawing has none; `db.cmd.limit_exceeded` when the draw
    /// order of the model space cannot take another entity.
    pub(crate) fn new(snapshot: &Snapshot) -> Result<Self> {
        let space =
            crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
        let top = top_order(snapshot, space);
        let next_order = match top {
            None => 0,
            Some(key) => key
                .checked_add(ORDER_STEP)
                .ok_or_else(|| Error::new(codes::LIMIT_EXCEEDED))?,
        };
        Ok(Self {
            space,
            layer: default_layer(snapshot),
            next_order,
            ids: Vec::new(),
        })
    }

    /// Creates an entity of type `key` with the given field values over the defaults of the
    /// type: the layer, then the draw order, are set here.
    ///
    /// # Errors
    ///
    /// The errors of the transaction.
    pub(crate) fn add(
        &mut self,
        ctx: &mut Context<'_>,
        key: &str,
        fields: Vec<(usize, Value)>,
    ) -> Result<()> {
        let mut values = types::defaults(ctx.snapshot().registry(), key);
        set(&mut values, entity::LAYER, self.layer.map(Value::Reference));
        for (index, value) in fields {
            set(&mut values, index, Some(value));
        }
        set(
            &mut values,
            entity::DRAW_ORDER,
            Some(Value::I64(self.next_order)),
        );
        self.next_order = self
            .next_order
            .checked_add(ORDER_STEP)
            .ok_or_else(|| Error::new(codes::LIMIT_EXCEEDED))?;
        let id: ObjectId = ctx.new_id();
        let header = Header {
            owner: Some(self.space),
            ..Header::default()
        };
        ctx.txn()?.create(key, id, header, values)?;
        self.ids.push(IdParam(id));
        Ok(())
    }

    /// Adds a curve as the entity of its kind.
    ///
    /// # Errors
    ///
    /// As [`Creator::add`].
    pub(crate) fn curve(&mut self, ctx: &mut Context<'_>, curve: &Curve2) -> Result<()> {
        let key = match curve {
            Curve2::Segment(_) => types::LINE,
            Curve2::Ray(_) => types::RAY,
            Curve2::Line(_) => types::CONSTRUCTION_LINE,
            Curve2::Circle(_) => types::CIRCLE,
            Curve2::Arc(_) => types::ARC,
            Curve2::Ellipse(_) => types::ELLIPSE,
            Curve2::EllipticalArc(_) => types::ELLIPTICAL_ARC,
            Curve2::Polyline(_) => types::POLYLINE,
            Curve2::Nurbs(_) => types::SPLINE,
        };
        let geometry = Value::Geometry(GeometryBytes(curve.encode()));
        self.add(ctx, key, vec![(entity::KIND, geometry)])
    }

    /// Adds a point entity at `position`.
    ///
    /// # Errors
    ///
    /// As [`Creator::add`].
    pub(crate) fn point(&mut self, ctx: &mut Context<'_>, position: Point2) -> Result<()> {
        self.add(
            ctx,
            types::POINT,
            vec![(entity::KIND, Value::Point2(position))],
        )
    }

    /// Copies the entity `source` into the drawing: a new identifier, the stored values of the
    /// source (its layer, its appearance and every field of its kind), its owner, and a draw order
    /// above the entities before it. The source is not changed. The caller has refused the kinds
    /// that refer to other entities.
    ///
    /// # Errors
    ///
    /// `draft.object.wrong_type` for `source` when it names no entity, `db.cmd.limit_exceeded`
    /// when the draw order cannot take another entity, and the errors of the transaction.
    pub(crate) fn copy_of(&mut self, ctx: &mut Context<'_>, source: ObjectId) -> Result<ObjectId> {
        let snapshot = ctx.snapshot().clone();
        let key = crate::modify::entity_key(&snapshot, source)?;
        let object = snapshot.get(snapshot.resolve(source)?)?;
        let mut values = object.values.clone();
        set(
            &mut values,
            entity::DRAW_ORDER,
            Some(Value::I64(self.next_order)),
        );
        self.next_order = self
            .next_order
            .checked_add(ORDER_STEP)
            .ok_or_else(|| Error::new(codes::LIMIT_EXCEEDED))?;
        let id: ObjectId = ctx.new_id();
        let header = Header {
            owner: object.header.owner,
            ..Header::default()
        };
        ctx.txn()?.create_copy(source, &key, id, header, values)?;
        self.ids.push(IdParam(id));
        Ok(id)
    }

    /// Adds an entity of `key` created from `values` as they are, owned by `owner`, with a draw order above
    /// the entities created before it.
    ///
    /// # Errors
    ///
    /// `db.cmd.limit_exceeded` when the draw order is spent, and the errors of the transaction.
    pub(crate) fn add_values(
        &mut self,
        ctx: &mut Context<'_>,
        key: &str,
        owner: Option<ObjectId>,
        values: Vec<Option<Value>>,
    ) -> Result<ObjectId> {
        let order = self.next_order;
        self.next_order = self
            .next_order
            .checked_add(ORDER_STEP)
            .ok_or_else(|| Error::new(codes::LIMIT_EXCEEDED))?;
        self.add_values_at(ctx, key, owner, values, order)
    }

    /// Adds an entity of `key` created from `values` as they are (its layer, its draw order and its
    /// fields are the caller's), owned by `owner`, with the draw order `order`. Used for the pieces of a
    /// changed geometry, whose kind may differ from the source's.
    ///
    /// # Errors
    ///
    /// The errors of the transaction.
    pub(crate) fn add_values_at(
        &mut self,
        ctx: &mut Context<'_>,
        key: &str,
        owner: Option<ObjectId>,
        values: Vec<Option<Value>>,
        order: i64,
    ) -> Result<ObjectId> {
        let mut values = values;
        set(&mut values, entity::DRAW_ORDER, Some(Value::I64(order)));
        let id: ObjectId = ctx.new_id();
        let header = Header {
            owner,
            ..Header::default()
        };
        ctx.txn()?.create(key, id, header, values)?;
        self.ids.push(IdParam(id));
        Ok(id)
    }

    /// The identifiers the creator made, in creation order.
    pub(crate) fn into_ids(self) -> Vec<ObjectId> {
        self.ids.into_iter().map(|id| id.0).collect()
    }

    /// Ends the command: the created entities become the selection after it. Returns them.
    pub(crate) fn finish(self, ctx: &mut Context<'_>) -> Created {
        ctx.set_selection_after(self.ids.iter().map(|id| id.0).collect());
        Created { ids: self.ids }
    }
}

/// Sets `values[index]`, when the type has that field.
fn set(values: &mut [Option<Value>], index: usize, value: Option<Value>) {
    if let Some(slot) = values.get_mut(index) {
        *slot = value;
    }
}

/// The largest draw-order key of an entity owned by `space`, `None` when it has none.
fn top_order(snapshot: &Snapshot, space: ObjectId) -> Option<i64> {
    ENTITIES
        .iter()
        .filter_map(|key| snapshot.iter_type(key).ok())
        .flatten()
        .filter(|(_, object)| object.header.owner == Some(space))
        .filter_map(|(_, object)| match object.values.get(entity::DRAW_ORDER) {
            Some(Some(Value::I64(key))) => Some(*key),
            _ => None,
        })
        .max()
}

/// The next step of a prompt machine, from the answers so far.
pub(crate) type NextFn = fn(&PromptState<'_>) -> Result<Step>;

/// The check of an answer that fits its prompt, before it is taken.
pub(crate) type CheckFn = fn(&PromptState<'_>, &Prompt, &PromptInput) -> Result<()>;

/// A prompt machine written as two functions over the answers so far.
#[derive(Debug)]
pub(crate) struct Machine {
    next: NextFn,
    check: CheckFn,
}

impl PromptMachine for Machine {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        (self.next)(state)
    }

    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        (self.check)(state, prompt, input)
    }
}

/// The prompt machine of `next` with the check `check`.
pub(crate) fn machine(next: NextFn, check: CheckFn) -> Arc<dyn PromptMachine> {
    Arc::new(Machine { next, check })
}

/// A check that accepts every answer.
pub(crate) fn accept_all(_: &PromptState<'_>, _: &Prompt, _: &PromptInput) -> Result<()> {
    Ok(())
}

/// The complete parameters of a machine: the object of `value`.
///
/// # Errors
///
/// `db.cmd.value_invalid` when `value` is not an object.
pub(crate) fn done(value: serde_json::Value) -> Result<Step> {
    match value {
        serde_json::Value::Object(map) => Ok(Step::Done(map)),
        _ => Err(Error::new(codes::VALUE_INVALID).with_detail("field", "parameters")),
    }
}

/// The point answers of the prompt `key`, in order, with the keyword answers among them.
pub(crate) fn answers<'a>(
    state: &'a PromptState<'_>,
) -> impl Iterator<Item = &'a PromptInput> + 'a {
    state.answers.iter().map(|answer| &answer.input)
}

/// The answer of a point prompt as a point in the drawing's units.
#[must_use]
pub(crate) fn point_of(state: &PromptState<'_>, input: &PromptInput) -> Option<Point2> {
    match input {
        PromptInput::Point(point) => Some(drawing_point(state.snapshot, *point)),
        _ => None,
    }
}

/// The keyword answered, when the answer is a keyword.
#[must_use]
pub(crate) fn keyword_of(input: &PromptInput) -> Option<&str> {
    match input {
        PromptInput::Keyword(id) => Some(id.as_str()),
        _ => None,
    }
}

/// The label of the keyword option undo.
pub(crate) const OPTION_UNDO: Message = Message {
    message_id: "db-cmd-option-undo",
};
/// The label of the keyword option close.
pub(crate) const OPTION_CLOSE: Message = Message {
    message_id: "db-cmd-option-close",
};
/// The label of the keyword option that finishes a command (Enter).
pub(crate) const OPTION_END: Message = Message {
    message_id: "db-cmd-option-end",
};
/// The label of the keyword option horizontal.
pub(crate) const OPTION_HORIZONTAL: Message = Message {
    message_id: "db-cmd-option-horizontal",
};
/// The label of the keyword option vertical.
pub(crate) const OPTION_VERTICAL: Message = Message {
    message_id: "db-cmd-option-vertical",
};
/// The label of the keyword option angle.
pub(crate) const OPTION_ANGLE: Message = Message {
    message_id: "db-cmd-option-angle",
};
/// The label of the keyword option center.
pub(crate) const OPTION_CENTER: Message = Message {
    message_id: "db-cmd-option-center",
};
/// The label of the keyword option radius.
pub(crate) const OPTION_RADIUS: Message = Message {
    message_id: "db-cmd-option-radius",
};
/// The label of the keyword option second point.
pub(crate) const OPTION_SECOND: Message = Message {
    message_id: "db-cmd-option-second",
};
/// The label of the keyword option direction.
pub(crate) const OPTION_DIRECTION: Message = Message {
    message_id: "db-cmd-option-direction",
};
/// The label of the keyword option arc.
pub(crate) const OPTION_ARC: Message = Message {
    message_id: "db-cmd-option-arc",
};
/// The label of the keyword option line.
pub(crate) const OPTION_LINE: Message = Message {
    message_id: "db-cmd-option-line",
};
/// The label of the keyword option width.
pub(crate) const OPTION_WIDTH: Message = Message {
    message_id: "db-cmd-option-width",
};

/// A keyword option with its identifier, its label and its shortcut letter.
pub(crate) fn option(id: &str, label: Message, shortcut: Option<char>) -> KeywordOption {
    let option = KeywordOption::new(id, label.message_id);
    match shortcut {
        Some(letter) => option.with_shortcut(letter),
        None => option,
    }
}
