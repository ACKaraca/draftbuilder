//! `draftbuilder.modify.move`: moves objects by an offset in the plane of the drawing.
//!
//! Reuses what the foundation offers: curve geometry is decoded and transformed by `arc-geom`
//! (`Curve2::transformed`), and the result is written back with the transaction's field
//! modification, exactly as `core.draft.entity.modify` writes a field. The foundation has no
//! relative move, so the product wires one.

use arc_base::math::{Matrix3, Point2, Transform2};
use arc_base::{Error, Limits, Result, Tolerance};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{FieldPath, GeometryBytes, Value};
use arc_draft::text::{RICH_TEXT, TEXT};
use arc_draft::types::{
    ARC, BLOCK_REF, CIRCLE, CONSTRUCTION_LINE, ELLIPSE, ELLIPTICAL_ARC, LINE, POINT, POLYLINE, RAY,
    SPLINE, WIPEOUT,
};
use arc_draft_dxf::fields::Reader;
use arc_geom::Curve2;
use arc_geom::encoding::GeomCodec;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::{Message, codes};

/// The command identifier.
pub const ID: &str = "draftbuilder.modify.move";
const LABEL: Message = Message {
    message_id: "db-cmd-move-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-move-description",
};

/// The types whose position is the field `position`.
const POSITIONED: &[&str] = &[POINT, TEXT, RICH_TEXT, BLOCK_REF];
/// The types whose shape is the field `geometry`.
const SHAPED: &[&str] = &[
    LINE,
    RAY,
    CONSTRUCTION_LINE,
    CIRCLE,
    ARC,
    ELLIPSE,
    ELLIPTICAL_ARC,
    POLYLINE,
    SPLINE,
    WIPEOUT,
];

/// Parameters of `draftbuilder.modify.move`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveParams {
    /// The objects to move.
    #[schemars(length(min = 1, max = 100_000))]
    pub objects: Vec<IdParam>,
    /// The offset `[dx, dy]` in drawing units.
    pub offset: [f64; 2],
}

fn translation([dx, dy]: [f64; 2]) -> Transform2 {
    Transform2::from_matrix_unchecked(Matrix3::new(1.0, 0.0, dx, 0.0, 1.0, dy, 0.0, 0.0, 1.0))
}

/// The new value of the field of `id` that moving changes, or `None` when `key` is not movable.
fn moved(
    reader: &Reader<'_>,
    key: &str,
    offset: [f64; 2],
    limits: &Limits,
) -> Result<Option<(&'static str, Value)>> {
    if POSITIONED.contains(&key) {
        return Ok(match reader.get("position") {
            Some(Value::Point2(p)) => Some((
                "position",
                Value::Point2(Point2::new(p.x + offset[0], p.y + offset[1])),
            )),
            _ => None,
        });
    }
    if SHAPED.contains(&key) {
        let Some(Value::Geometry(bytes)) = reader.get("geometry") else {
            return Ok(None);
        };
        let curve = Curve2::decode(&bytes.0, limits)?;
        let curve = curve.transformed(&translation(offset), &Tolerance::DEFAULT)?;
        return Ok(Some((
            "geometry",
            Value::Geometry(GeometryBytes(curve.encode())),
        )));
    }
    Ok(None)
}

fn translate(ctx: &mut Context<'_>, p: MoveParams) -> Result<()> {
    if p.offset.iter().any(|v| !v.is_finite()) {
        return Err(Error::new(codes::OFFSET_INVALID));
    }
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    for IdParam(id) in p.objects {
        let snapshot = txn.snapshot();
        let handle = snapshot.resolve(id)?;
        let key = snapshot
            .type_key(handle)
            .map(|k| k.as_str().to_owned())
            .unwrap_or_default();
        let object = snapshot.get(handle)?;
        let reader = Reader::new(snapshot.registry(), &key, &object.values);
        let Some((field, value)) = moved(&reader, &key, p.offset, &limits)? else {
            return Err(Error::new(codes::MOVE_UNSUPPORTED).with_arg("type", key));
        };
        txn.modify(id, FieldPath::new(field), Some(value))?;
    }
    Ok(())
}

/// Registers `draftbuilder.modify.move` (permission class `edit`, undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, translate)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Edit]),
    )
}
