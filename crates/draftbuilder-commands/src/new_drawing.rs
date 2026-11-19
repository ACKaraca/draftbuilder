//! `draftbuilder.drawing.new`: creates an empty drawing, from a built-in default or from a
//! template drawing.
//!
//! A template contributes its symbol tables (layers, linetypes, text styles and dimension styles)
//! and its unit. Its entities, blocks, objects, layouts, viewports and page setups are not
//! copied: layouts and page setups are not mapped yet. The template's handles are not kept, and the
//! new drawing takes a new fingerprint.

use std::path::Path;

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{Header, Value};
use arc_draft::types::{SPACE, SpaceKind, SpaceUnit};
use arc_draft_dxf::apply::{Applied, apply};
use arc_draft_dxf::fields::Builder;
use arc_draft_dxf::plan::{ImportOptions, plan_with_report};
use std::collections::BTreeSet;

use arc_io_dxf::Drawing;
use draftbuilder_io::open::unit_of;
use draftbuilder_model::database::{self, Origin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, codes};
use crate::open::{detect, fresh_fingerprint, is_empty, read_drawing};

/// The identifier of the command.
pub const ID: &str = "draftbuilder.drawing.new";
const LABEL: Message = Message {
    message_id: "db-cmd-new-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-new-description",
};

/// The unit system of the built-in default drawing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum System {
    /// Millimeters.
    #[default]
    Metric,
    /// Inches.
    Imperial,
}

impl System {
    fn unit(self) -> SpaceUnit {
        match self {
            Self::Metric => SpaceUnit::Millimeter,
            Self::Imperial => SpaceUnit::Inch,
        }
    }
}

/// Parameters of `draftbuilder.drawing.new`. Every parameter is optional.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewParams {
    /// The unit system of the built-in default, used when neither `unit` nor the template gives
    /// a unit.
    #[serde(default)]
    pub system: System,
    /// The unit of the new drawing; it wins over the template's unit.
    #[serde(default)]
    pub unit: Option<SpaceUnit>,
    /// A DWG or DXF drawing whose symbol tables and unit the new drawing takes.
    #[serde(default)]
    pub template: Option<String>,
}

/// What `draftbuilder.drawing.new` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct NewResult {
    /// The model space of the new drawing.
    pub space: IdParam,
    /// The unit of the new drawing.
    pub unit: SpaceUnit,
    /// Layers created from the template.
    pub layers: u64,
    /// The fingerprint of the new drawing, in the form files use.
    pub fingerprint: String,
    /// Always null: a new drawing is not a transfer, so it has no loss report. The template's
    /// entities, blocks and objects are not copied and are not counted.
    pub report: serde_json::Value,
}

/// The template's drawing without its geometry, blocks and objects: the symbol tables and the
/// header remain.
fn tables_only(drawing: &mut Drawing) {
    drawing.entities.clear();
    drawing.blocks.clear();
    drawing.objects.clear();
    drawing.preserved_sections.clear();
    let tables = &mut drawing.tables;
    tables.vports = None;
    tables.views = None;
    tables.ucs = None;
    tables.appids = None;
    tables.block_records = None;
    tables.preserved.clear();
}

fn new_drawing(ctx: &mut Context<'_>, p: NewParams) -> Result<NewResult> {
    if !is_empty(ctx) {
        return Err(Error::new(codes::DRAWING_NOT_EMPTY));
    }
    let template = match p.template.as_deref() {
        Some(path) => {
            let path = Path::new(path);
            let format = detect(ctx, path)?;
            let mut output = read_drawing(ctx, path, format)?;
            tables_only(&mut output.drawing);
            Some(output)
        }
        None => None,
    };
    let declared = template
        .as_ref()
        .map(|t| unit_of(&t.drawing))
        .filter(|u| *u != SpaceUnit::Unitless);
    let unit = p.unit.or(declared).unwrap_or_else(|| p.system.unit());

    let space: ObjectId = ctx.new_id();
    let mut values = Builder::new(ctx.snapshot().registry(), SPACE);
    values
        .set("name", Value::String("Model".to_owned()))?
        .set("kind", Value::Enum(SpaceKind::Model as u32))?
        .set("unit", Value::Enum(unit.code()))?;
    ctx.txn()?
        .create(SPACE, space, Header::default(), values.values)?;

    let layers = match template {
        Some(output) => {
            let plan = plan_with_report(
                &output.drawing,
                &ImportOptions {
                    target_unit: unit,
                    limits: *ctx.limits(),
                    ..ImportOptions::default()
                },
                &output.report,
            );
            // The tables are applied with the handles of the template bound under the empty source
            // label; the new drawing keeps none of them, so the rows the apply added are removed.
            let before: BTreeSet<i64> = ctx.snapshot().foreign_rows().map(|r| r.id).collect();
            let Applied { layers, .. } = apply(ctx, space, &plan)?;
            let txn = ctx.txn()?;
            let added: Vec<i64> = txn
                .snapshot()
                .foreign_rows()
                .map(|r| r.id)
                .filter(|id| !before.contains(id))
                .collect();
            for id in added {
                txn.remove_foreign(id)?;
            }
            layers
        }
        None => 0,
    };
    let fingerprint = fresh_fingerprint(ctx)?;
    database::set_fingerprint(ctx.txn()?, fingerprint, Origin::Assigned)?;
    Ok(NewResult {
        space: IdParam(space),
        unit,
        layers,
        fingerprint: fingerprint.to_braced(),
        report: serde_json::Value::Null,
    })
}

/// Registers `draftbuilder.drawing.new` (permission classes `edit` and `file_read`, undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, new_drawing)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Edit, Permission::FileRead]),
    )
}
