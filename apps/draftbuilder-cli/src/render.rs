//! The `render` subcommand: the model space of a drawing, drawn headless to a PNG file through
//! `arc_app::render`, which frames the drawing, shades it on white and encodes the PNG.

use std::ffi::OsString;
use std::io::Write;
use std::sync::Arc;

use arc_app::render::render_drawing;
use arc_base::{Error, Limits, ObjectId, Result};
use arc_command::Roots;
use arc_command::error::codes;
use arc_command::headless::{Exit, Product};
use arc_doc::package::{Mode, Package};
use arc_doc::{DocumentConfig, LoadOptions, Snapshot};
use arc_gpu::Extraction;
use arc_gpu::draft::{self, DraftOptions};
use serde_json::json;

use crate::{Options, parse, usage};

/// The default image size in pixels.
pub const DEFAULT_SIZE: (u32, u32) = (800, 600);

/// The drawn objects of the model space of `snapshot`, one extraction each.
///
/// # Errors
/// `db.cmd.no_model_space` and the errors of extraction.
pub fn drawing(snapshot: &Snapshot) -> Result<Vec<(ObjectId, Extraction)>> {
    let space = draftbuilder_commands::model_space(snapshot)
        .ok_or_else(|| Error::new(draftbuilder_commands::error::codes::NO_MODEL_SPACE))?;
    Ok(
        draft::space_extractions(snapshot, space, &DraftOptions::default())?
            .into_iter()
            .map(|(id, d)| (id, d.extraction))
            .collect(),
    )
}

/// Runs the subcommand.
///
/// # Errors
/// `cmd.cli.usage`, the package's errors, those of the render and `cmd.path.*` for the output.
pub fn run<W: Write>(args: Vec<OsString>, product: &Product, stdout: &mut W) -> Result<Exit> {
    let o: Options = parse(args)?;
    let (Some(file), Some(output)) = (o.file, o.output) else {
        return Err(usage("missing_flag:--output"));
    };
    let limits = Limits::DEFAULT;
    let package = Package::open(&file, Mode::ReadOnly, &limits)?;
    let document = package.open_document(
        Arc::clone(&product.types),
        Arc::clone(&product.evaluators),
        &LoadOptions {
            limits,
            ..LoadOptions::default()
        },
        DocumentConfig::default(),
    )?;
    let (width, height) = (
        o.width.unwrap_or(DEFAULT_SIZE.0),
        o.height.unwrap_or(DEFAULT_SIZE.1),
    );
    let rendered = render_drawing(&document.snapshot(), &drawing, width, height)?;
    let roots = Roots::new(std::iter::empty::<&std::path::PathBuf>(), &o.write_roots)?;
    roots
        .create_write(&output)?
        .write_all(&rendered.png)
        .map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    let line = json!({"output": output.to_string_lossy(), "width": width, "height": height,
                      "objects": rendered.objects, "adapter": rendered.adapter});
    writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    Ok(Exit::Success)
}
