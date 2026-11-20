//! The headless CLI of DraftBuilder.
//!
//! [`run`] is the program. The shared subcommands of the Arc suite
//! ([`arc_command::headless`]: `commands`, `describe`, `validate-params`, `run`, `play`,
//! `batch`, `inspect`, `validate`, `serve`, `mcp`) run over the DraftBuilder type set and
//! command set, with the product's drafting validators installed on every document and
//! `view.render` answered by the drafting renderer. The product adds `render`
//! (`render <file> --output <png> [--width N] [--height N] [--write-root D]`, module
//! [`render`]), which draws the model space headless to a PNG file, and `script`
//! (`script <file> --script <commands> [--create] [--save] [--report <file>]`, module
//! [`script`]), which runs a text file of typed command lines on the package.
//!
//! The working file is a document package (extension `draftb`): it holds the drawing database
//! of an opened DXF drawing so that several processes can work on it. A script that opens a
//! DXF drawing, moves, undoes and saves runs over `serve --stdio --create <file> --save
//! --allow-edit` with the protocol methods `command.run` and `history.undo`.

pub mod render;
pub mod script;

use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use arc_app::render::SnapshotRenderer;
use arc_base::Error;
use arc_command::error::codes;
use arc_command::headless::{self, Exit, Product};

pub use draftbuilder_commands::product;

fn usage(reason: impl Into<String>) -> Error {
    Error::new(codes::CLI_USAGE).with_detail("reason", reason.into())
}

/// The arguments of `render`.
#[derive(Debug, Default)]
pub struct Options {
    /// The package.
    pub file: Option<PathBuf>,
    /// The output file.
    pub output: Option<PathBuf>,
    /// The image width.
    pub width: Option<u32>,
    /// The image height.
    pub height: Option<u32>,
    /// The write roots.
    pub write_roots: Vec<PathBuf>,
}

fn size(name: &str, value: &str) -> Result<u32, Error> {
    value
        .parse::<u32>()
        .ok()
        .filter(|v| *v > 0 && !value.starts_with('+'))
        .ok_or_else(|| usage(format!("size:{name}")))
}

/// Parses the arguments of `render`.
///
/// # Errors
/// `cmd.cli.usage` with the reason for an unknown or repeated flag, a missing value or a wrong
/// number of arguments.
pub fn parse(args: Vec<OsString>) -> Result<Options, Error> {
    let mut o = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let Some(flag) = arg.to_str().filter(|a| a.starts_with("--")) else {
            if o.file.replace(PathBuf::from(arg)).is_some() {
                return Err(usage("arguments"));
            }
            continue;
        };
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (flag.to_owned(), None),
        };
        if name == "--json" && inline.is_none() {
            continue;
        }
        let value = match inline {
            Some(v) => v,
            None => args
                .next()
                .and_then(|v| v.into_string().ok())
                .ok_or_else(|| usage(format!("missing_value:{name}")))?,
        };
        match name.as_str() {
            "--write-root" => o.write_roots.push(PathBuf::from(value)),
            "--output" if o.output.is_none() => o.output = Some(PathBuf::from(value)),
            "--width" if o.width.is_none() => o.width = Some(size(&name, &value)?),
            "--height" if o.height.is_none() => o.height = Some(size(&name, &value)?),
            "--output" | "--width" | "--height" => return Err(usage(format!("repeated:{name}"))),
            _ => return Err(usage(format!("unknown_flag:{name}"))),
        }
    }
    if o.file.is_none() {
        return Err(usage("arguments"));
    }
    Ok(o)
}

/// The renderer of `view.render`: the model space through the drafting extractor of `arc-gpu`.
fn renderer() -> SnapshotRenderer {
    SnapshotRenderer::with_drawing(render::drawing)
}

/// Runs the command-line program: `session` and `render` here, every other subcommand through
/// the shared runner. Never panics: a fault is contained and reported as [`Exit::Internal`].
pub fn run<R, W, E>(
    args: impl IntoIterator<Item = OsString>,
    product: &Product,
    stdin: R,
    mut stdout: W,
    mut stderr: E,
) -> Exit
where
    R: BufRead,
    W: Write + Send,
    E: Write + Send,
{
    let mut args: Vec<OsString> = args.into_iter().collect();
    if args.first().and_then(|a| a.to_str()) == Some("script") {
        args.remove(0);
        return script::run(args, product, &mut stdout, &mut stderr);
    }
    if args.first().and_then(|a| a.to_str()) != Some("render") {
        let setup = |document: &arc_doc::Document| {
            draftbuilder_commands::install(document);
            Ok(())
        };
        return headless::run_with_setup(
            args,
            product,
            Some(&renderer()),
            Some(&setup),
            stdin,
            stdout,
            stderr,
        );
    }
    args.remove(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| render::run(args, product, &mut stdout)));
    let error = match outcome {
        Ok(Ok(exit)) => return exit,
        Ok(Err(error)) => error,
        Err(_) => Error::new(codes::FAULT),
    };
    headless::report_error(error, &mut stderr)
}
