//! `draftbuilder.drawing.open_dxf`: reads a DXF file into the document as a drawing database.

use std::path::Path;

use arc_base::id::Identifier;
use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{Header, Value};
use arc_draft::types::{SPACE, SpaceKind};
use arc_draft_dxf::apply::{Applied, apply};
use arc_draft_dxf::fields::Builder;
use arc_draft_dxf::plan::{ImportOptions, plan_with_report};
use arc_io_dxf::{Progress, ReadOptions};
use draftbuilder_io::open::{fingerprint_of, seed_top, unit_of};
use draftbuilder_model::database::{self, Database, Origin, SOURCE_DOCUMENT};
use draftbuilder_model::{Fingerprint, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, OPEN_PROGRESS, codes};

/// The command identifier.
pub const ID: &str = "draftbuilder.drawing.open_dxf";
const LABEL: Message = Message {
    message_id: "db-cmd-open-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-open-description",
};

/// Parameters of `draftbuilder.drawing.open_dxf`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenParams {
    /// The file to read, inside the session's read roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
}

/// What `draftbuilder.drawing.open_dxf` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenResult {
    /// The model space that holds the drawing's entities.
    pub space: IdParam,
    /// Layers created.
    pub layers: u64,
    /// Entities created, in the space and in blocks.
    pub entities: u64,
    /// Objects (layers, linetypes, styles, block definitions and entities) bound to their
    /// source handle.
    pub handles: u64,
    /// The fingerprint in the form files use.
    pub fingerprint: String,
    /// Whether DraftBuilder assigned the fingerprint because the file had none.
    pub fingerprint_assigned: bool,
    /// The highest handle the drawing has had, in hexadecimal.
    pub handle_top: String,
    /// The loss and preservation report, in the form of `schemas/loss-report.schema.json` of
    /// `arc-core`.
    pub report: serde_json::Value,
}

fn is_empty(ctx: &Context<'_>) -> bool {
    let snapshot = ctx.snapshot();
    Database::of(snapshot).top() == 0
        && snapshot
            .iter_type(SPACE)
            .is_ok_and(|mut s| s.next().is_none())
}

/// Reads the file, creates the model space and the objects of the drawing; the import binds the
/// handles.
fn open(ctx: &mut Context<'_>, p: OpenParams) -> Result<OpenResult> {
    if !is_empty(ctx) {
        return Err(Error::new(codes::DRAWING_NOT_EMPTY));
    }
    let file = ctx.open_read(Path::new(&p.path))?;
    let mut options = ReadOptions::with_limits(*ctx.limits());
    let cancel = options.cancel.clone();
    let output = {
        let probe: &Context<'_> = ctx;
        let mut tick = |_: Progress| {
            if probe.checkpoint().is_err() {
                cancel.cancel();
            }
        };
        options.progress = Some(&mut tick);
        arc_io_dxf::read_stream(file, &mut options)?
    };
    ctx.checkpoint()?;
    ctx.progress(0.5, OPEN_PROGRESS.message_id);
    let unit = unit_of(&output.drawing);
    let plan = plan_with_report(
        &output.drawing,
        &ImportOptions {
            target_unit: unit,
            limits: *ctx.limits(),
            source_document: SOURCE_DOCUMENT.to_owned(),
            ..ImportOptions::default()
        },
        &output.report,
    );

    let space: ObjectId = ctx.new_id();
    let mut values = Builder::new(ctx.snapshot().registry(), SPACE);
    values
        .set("name", Value::String("Model".to_owned()))?
        .set("kind", Value::Enum(SpaceKind::Model as u32))?
        .set("unit", Value::Enum(unit.code()))?;
    ctx.txn()?
        .create(SPACE, space, Header::default(), values.values)?;
    let Applied {
        layers,
        entities,
        created,
        ..
    } = apply(ctx, space, &plan)?;

    let bound = created.iter().filter_map(|c| c.source).map(|h| h.0);
    let top = bound.clone().fold(seed_top(&output.drawing), u64::max);
    let handles = bound.count() as u64;
    database::raise_top(ctx.txn()?, top)?;
    let (fingerprint, origin) = match fingerprint_of(&output.drawing) {
        Some(f) => (f, Origin::File),
        None => (fresh_fingerprint(ctx)?, Origin::Assigned),
    };
    database::set_fingerprint(ctx.txn()?, fingerprint, origin)?;
    Ok(OpenResult {
        space: IdParam(space),
        layers,
        entities,
        handles,
        fingerprint: fingerprint.to_braced(),
        fingerprint_assigned: origin == Origin::Assigned,
        handle_top: Handle::new(top).map(Handle::to_hex).unwrap_or_default(),
        report: serde_json::to_value(&plan.report)
            .map_err(|_| Error::new(arc_command::error::codes::RESULT_INVALID))?,
    })
}

/// A new fingerprint: the 128 bits of a fresh identifier.
///
/// # Errors
/// `base.id.invalid` in the vanishing case that the identifier is all zero.
pub(crate) fn fresh_fingerprint(ctx: &mut Context<'_>) -> Result<Fingerprint> {
    let id: ObjectId = ctx.new_id();
    Fingerprint::new(id.as_u128()).ok_or_else(|| Error::new(arc_base::error::codes::ID_INVALID))
}

/// Registers `draftbuilder.drawing.open_dxf` (permission classes `edit` and `file_read`,
/// undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, open)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Edit, Permission::FileRead]),
    )
}
