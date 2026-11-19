//! The `render` subcommand: the model space of a drawing, drawn headless to a PNG file.
//!
//! The package is opened read-only, the model space is extracted with the shared drafting
//! extractor of `arc-gpu` (`draft::space_extractions`), the extents are zoomed to the default
//! top view and the image is shaded on a white background on the adapter the selection rules of
//! `arc-gpu` choose.

use std::ffi::OsString;
use std::io::Write;
use std::sync::Arc;

use arc_base::math::{Point3, Vector3};
use arc_base::{Color, Error, Limits, Tolerance};
use arc_command::Roots;
use arc_command::error::codes;
use arc_command::headless::{Exit, Product};
use arc_doc::package::{Mode, Package};
use arc_doc::{DocumentConfig, LoadOptions, Snapshot};
use arc_geom::view::{Projection, ViewFrame};
use arc_gpu::draft::{self, DraftOptions};
use arc_gpu::{Gpu, GpuSettings, RenderSettings, Renderer, Scene, SceneLimits};
use image::{ExtendedColorType, ImageEncoder};
use serde_json::json;

use crate::{Options, parse, usage};

/// The default image size in pixels.
pub const DEFAULT_SIZE: (u32, u32) = (800, 600);
/// The extents are fitted with this much room around them.
const MARGIN: f64 = 1.1;

const FLAGS: &[&str] = &["--output", "--width", "--height", "--write-root"];

/// A rendered drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// The image, PNG encoded (8-bit RGBA).
    pub png: Vec<u8>,
    /// The objects drawn.
    pub objects: usize,
    /// The name of the adapter the image was rendered on.
    pub adapter: String,
}

fn top_view(center: Point3, height: f64) -> arc_base::Result<ViewFrame> {
    ViewFrame::new(
        center + Vector3::new(0.0, 0.0, height.max(1.0)),
        center,
        Vector3::y(),
        Projection::Orthographic { height },
        1e-3,
        1e3,
    )
}

/// Renders the model space of `snapshot` at `width` by `height` pixels.
///
/// # Errors
/// `db.cmd.no_model_space`, the errors of extraction, the `gpu.*` errors of adapter selection
/// and rendering (among them the one for a machine without a usable adapter), and
/// `cmd.path.io` when the PNG cannot be encoded.
pub fn render(snapshot: &Snapshot, width: u32, height: u32) -> arc_base::Result<Rendered> {
    let space = draftbuilder_commands::model_space(snapshot)
        .ok_or_else(|| Error::new(draftbuilder_commands::error::codes::NO_MODEL_SPACE))?;
    let objects = draft::space_extractions(snapshot, space, &DraftOptions::default())?;
    let drawn = objects.len();
    let gpu = Gpu::new(&GpuSettings::default())?;
    let mut scene = Scene::new(&gpu, SceneLimits::DEFAULT)?;
    let applied = draft::apply(&mut scene, snapshot.revision(), objects);
    if let Some((_, error)) = applied.failed.into_iter().next() {
        return Err(error);
    }
    let aspect = f64::from(width.max(1)) / f64::from(height.max(1));
    let frame = match scene.bounding_sphere(None) {
        Some((center, radius)) => arc_gpu::navigation::zoom_to_sphere(
            &top_view(center, 1.0)?,
            &center,
            radius * MARGIN,
            aspect,
            &Tolerance::DEFAULT,
        )?,
        None => top_view(Point3::origin(), 10.0)?,
    };
    let settings = RenderSettings {
        background: Color::WHITE,
        ..RenderSettings::DEFAULT
    };
    let image =
        Renderer::new(&gpu).render_to_image(&mut scene, &frame, width, height, &settings)?;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&image.rgba8, width, height, ExtendedColorType::Rgba8)
        .map_err(|_| Error::new(codes::PATH_IO).with_detail("reason", "other"))?;
    Ok(Rendered {
        png,
        objects: drawn,
        adapter: gpu.report().name.clone(),
    })
}

/// Runs the subcommand.
///
/// # Errors
/// `cmd.cli.usage`, the package's errors, those of [`render`] and `cmd.path.*` for the output.
pub fn run<W: Write>(
    args: Vec<OsString>,
    product: &Product,
    stdout: &mut W,
) -> Result<Exit, Error> {
    let o: Options = parse(args, FLAGS)?;
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
    let rendered = render(&document.snapshot(), width, height)?;
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
