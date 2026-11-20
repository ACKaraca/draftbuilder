//! `draftbuilder.drawing.open` and `draftbuilder.drawing.open_dxf`: reads a DWG or DXF file into
//! the document as a drawing database.
//!
//! Both formats take one path. The reader of the format gives a neutral drawing and a loss
//! report; the drawing is planned by `arc-draft-dxf` and applied in one undoable transaction, and
//! the reader's report is merged into the plan's report. A DWG file is read by `arc-io-dwg`, which
//! is experimental for files that are not trusted (docs/architecture/interoperability.md).

use std::io::Read;
use std::path::Path;

use arc_base::id::Identifier;
use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{Header, Value};
use arc_draft::types::{SPACE, SpaceKind, SpaceUnit};
use arc_draft_dxf::apply::{Applied, apply};
use arc_draft_dxf::fields::Builder;
use arc_draft_dxf::plan::{ImportOptions, plan_with_report};
use arc_io_dxf::{Progress, ReadOptions, ReadOutput};
use draftbuilder_io::open::{fingerprint_of, seed_top, unit_of};
use draftbuilder_model::database::{self, Database, Origin, SOURCE_DOCUMENT};
use draftbuilder_model::{Fingerprint, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Message, OPEN_PROGRESS, codes};

/// The identifier of the DXF-only open.
pub const ID: &str = "draftbuilder.drawing.open_dxf";
/// The identifier of the open of either format.
pub const OPEN_ID: &str = "draftbuilder.drawing.open";

const LABEL: Message = Message {
    message_id: "db-cmd-open-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-open-description",
};
const OPEN_LABEL: Message = Message {
    message_id: "db-cmd-open-drawing-label",
};
const OPEN_DESCRIPTION: Message = Message {
    message_id: "db-cmd-open-drawing-description",
};

/// The drawing formats the product reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// AutoCAD DXF, ASCII or binary.
    Dxf,
    /// AutoCAD DWG.
    Dwg,
}

impl Format {
    /// The format named by the file name of `path`, ignoring its case. The whole name counts, so
    /// a file named exactly `.dwg` is a DWG file.
    ///
    /// # Errors
    /// `db.cmd.format_unknown` when the name does not end in `.dwg` or `.dxf`.
    pub fn of_path(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if name.ends_with(".dxf") {
            Ok(Self::Dxf)
        } else if name.ends_with(".dwg") {
            Ok(Self::Dwg)
        } else {
            Err(Error::new(codes::FORMAT_UNKNOWN))
        }
    }

    /// The name of the format in results.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Dxf => "dxf",
            Self::Dwg => "dwg",
        }
    }
}

/// Parameters of `draftbuilder.drawing.open_dxf` and `draftbuilder.drawing.open`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenParams {
    /// The file to read, inside the session's read roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
    /// The unit of the drawing. Absent: the unit the file declares, or unitless when it declares
    /// none. The file's content is scaled from its unit into this one, and the report states the
    /// factor.
    #[serde(default)]
    pub unit: Option<SpaceUnit>,
}

/// What the open commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenResult {
    /// The model space that holds the drawing's entities.
    pub space: IdParam,
    /// The format the file was read as: `dxf` or `dwg`.
    pub format: String,
    /// The unit of the drawing.
    pub unit: SpaceUnit,
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

pub(crate) fn is_empty(ctx: &Context<'_>) -> bool {
    let snapshot = ctx.snapshot();
    Database::of(snapshot).top() == 0
        && snapshot
            .iter_type(SPACE)
            .is_ok_and(|mut s| s.next().is_none())
}

/// The bytes at the start of a file that name its format.
const HEAD_BYTES: u64 = 128;

/// The format a file's first bytes name: a DWG version signature (`AC` and four digits), the
/// binary DXF sentinel, or the `SECTION` of an ASCII DXF. `None` when they name neither.
fn sniff(head: &[u8]) -> Option<Format> {
    if head.get(..2) == Some(b"AC".as_slice())
        && head
            .get(2..6)
            .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
    {
        return Some(Format::Dwg);
    }
    if head.starts_with(b"AutoCAD Binary DXF") || head.windows(7).any(|w| w == b"SECTION") {
        return Some(Format::Dxf);
    }
    None
}

/// The format of the file at `path`: the content decides when it names one, and the file name
/// is used only when the content names neither.
///
/// # Errors
/// The errors of opening the file, and `db.cmd.format_unknown` when neither the content nor the
/// name names a format.
pub(crate) fn detect(ctx: &Context<'_>, path: &Path) -> Result<Format> {
    let mut head = Vec::new();
    ctx.open_read(path)?
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    match sniff(&head) {
        Some(format) => Ok(format),
        None => Format::of_path(path),
    }
}

/// Reads all of `file`, at most one byte more than `limit`; the reader refuses a longer input.
fn read_bounded(file: std::fs::File, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    Ok(bytes)
}

/// Reads the file at `path` in `format` into a neutral drawing and its report, with the session's
/// limits, cancellation and progress.
pub(crate) fn read_drawing(
    ctx: &mut Context<'_>,
    path: &Path,
    format: Format,
) -> Result<ReadOutput> {
    let file = ctx.open_read(path)?;
    let mut options = ReadOptions::with_limits(*ctx.limits());
    let cancel = options.cancel.clone();
    let output = match format {
        Format::Dxf => {
            let probe: &Context<'_> = ctx;
            let mut tick = |_: Progress| {
                if probe.checkpoint().is_err() {
                    cancel.cancel();
                }
            };
            options.progress = Some(&mut tick);
            arc_io_dxf::read_stream(file, &mut options)?
        }
        Format::Dwg => {
            let bytes = read_bounded(file, ctx.limits().input_bytes)?;
            let probe: &Context<'_> = ctx;
            let mut tick = |_: Progress| {
                if probe.checkpoint().is_err() {
                    cancel.cancel();
                }
            };
            options.progress = Some(&mut tick);
            arc_io_dwg::read(&bytes, &mut options)?
        }
    };
    ctx.checkpoint()?;
    Ok(output)
}

/// Opens the drawing at `path` into the empty document: reads it, plans the import, creates the
/// model space and the objects, and binds the handles.
pub(crate) fn open_drawing(
    ctx: &mut Context<'_>,
    path: &Path,
    format: Format,
    unit: Option<SpaceUnit>,
) -> Result<OpenResult> {
    if !is_empty(ctx) {
        return Err(Error::new(codes::DRAWING_NOT_EMPTY));
    }
    let output = read_drawing(ctx, path, format)?;
    ctx.progress(0.5, OPEN_PROGRESS.message_id);
    let target = unit.unwrap_or_else(|| unit_of(&output.drawing));
    // The mapper states the conversion it applied (`draft.dxf.units_converted`, with the source
    // code and the factor) or that it converted nothing (`draft.dxf.units_unitless`); the report
    // carries that item and no other.
    let mut plan = plan_with_report(
        &output.drawing,
        &ImportOptions {
            target_unit: target,
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
        .set("unit", Value::Enum(target.code()))?;
    ctx.txn()?
        .create(SPACE, space, Header::default(), values.values)?;
    let Applied {
        layers,
        entities,
        created,
        ..
    } = apply(ctx, space, &plan)?;
    // A file without layer 0 gets the reserved layer, and the report says so.
    if crate::layer::ensure_layer_zero(ctx)? {
        draftbuilder_io::report::note(
            &mut plan.report,
            draftbuilder_io::error::LAYER_ZERO_CREATED,
            "layer",
            1,
        );
    }
    // Layer names of the file that differ only by case are one name in the product; the report
    // counts the groups the file has.
    let groups = file_layer_case_groups(&output.drawing);
    draftbuilder_io::report::note(
        &mut plan.report,
        draftbuilder_io::error::LAYER_NAME_CASE_DUPLICATE,
        "layer",
        groups,
    );

    let bound = created.iter().filter_map(|c| c.source).map(|h| h.0);
    let top = bound.clone().fold(seed_top(&output.drawing), u64::max);
    let handles = bound.count() as u64;
    database::raise_top(ctx.txn()?, top)?;
    let (fingerprint, origin) = match fingerprint_of(&output.drawing) {
        Some(f) => (f, Origin::File),
        None => (fresh_fingerprint(ctx)?, Origin::Assigned),
    };
    database::set_fingerprint(ctx.txn()?, fingerprint, origin)?;
    crate::settings::header::load_header(ctx, &output.drawing)?;
    Ok(OpenResult {
        space: IdParam(space),
        format: format.name().to_owned(),
        unit: target,
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

/// The number of groups of layer names in the file's layer table that are equal under the product's
/// fold but not byte for byte, each group counted once.
pub(crate) fn file_layer_case_groups(drawing: &arc_io_dxf::Drawing) -> u64 {
    let Some(table) = drawing.tables.layers.as_ref() else {
        return 0;
    };
    let mut counts: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for entry in &table.entries {
        *counts.entry(crate::fold::fold(&entry.name)).or_insert(0) += 1;
    }
    u64::try_from(counts.values().filter(|count| **count > 1).count()).unwrap_or(u64::MAX)
}

/// A new fingerprint: the 128 bits of a fresh identifier.
///
/// # Errors
/// `base.id.invalid` in the vanishing case that the identifier is all zero.
pub(crate) fn fresh_fingerprint(ctx: &mut Context<'_>) -> Result<Fingerprint> {
    let id: ObjectId = ctx.new_id();
    Fingerprint::new(id.as_u128()).ok_or_else(|| Error::new(arc_base::error::codes::ID_INVALID))
}

/// `draftbuilder.drawing.open_dxf`: a DXF file only.
fn open_dxf(ctx: &mut Context<'_>, p: OpenParams) -> Result<OpenResult> {
    let path = Path::new(&p.path);
    match detect(ctx, path)? {
        Format::Dxf => open_drawing(ctx, path, Format::Dxf, p.unit),
        Format::Dwg => Err(Error::new(codes::FORMAT_MISMATCH)),
    }
}

/// `draftbuilder.drawing.open`: a DWG or DXF file, by its content.
fn open(ctx: &mut Context<'_>, p: OpenParams) -> Result<OpenResult> {
    let path = Path::new(&p.path);
    let format = detect(ctx, path)?;
    open_drawing(ctx, path, format, p.unit)
}

/// Registers `draftbuilder.drawing.open_dxf` and `draftbuilder.drawing.open` (permission classes
/// `edit` and `file_read`, undoable).
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, open_dxf)
            .with_undo(UndoBehavior::Undoable)
            .with_permissions([Permission::Edit, Permission::FileRead]),
    )?;
    registry.register(
        CommandSpec::new(
            OPEN_ID,
            OPEN_LABEL.message_id,
            OPEN_DESCRIPTION.message_id,
            open,
        )
        .with_undo(UndoBehavior::Undoable)
        .with_permissions([Permission::Edit, Permission::FileRead]),
    )
}
