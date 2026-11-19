//! The headless CLI of DraftBuilder.
//!
//! [`run`] is the program. The shared subcommands of the Arc suite
//! ([`arc_command::headless`]: `commands`, `describe`, `validate-params`, `run`, `play`,
//! `batch`, `inspect`, `validate`, `serve`, `mcp`) run over the DraftBuilder type set and
//! command set. The product adds two:
//!
//! - `session <file> [--save] [--allow-edit] [--read-root D] [--write-root D]` ([`session`])
//!   runs a script from standard input in one process, one JSON request per line: a command,
//!   `undo` or `redo`. The shared runner cannot undo, because every `run` is a process of its
//!   own and the undo history does not survive a restart.
//! - `render <file> --output <png> [--width N] [--height N] [--write-root D]` ([`render`])
//!   draws the model space of the drawing headless to a PNG file.
//!
//! The working file is a document package (extension `draftb`): it holds the drawing database
//! of an opened DXF drawing so that several processes can work on it. `session` creates it
//! with `--save` when it does not exist.

pub mod render;
pub mod session;

use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use arc_base::Error;
use arc_command::error::codes;
use arc_command::headless::{self, Exit, Product};

pub use draftbuilder_commands::product;

/// The arguments of a product subcommand.
#[derive(Debug, Default)]
pub struct Options {
    /// The package.
    pub file: Option<PathBuf>,
    /// Save the package at the end.
    pub save: bool,
    /// Allow commands that change the document.
    pub allow_edit: bool,
    /// The read roots.
    pub read_roots: Vec<PathBuf>,
    /// The write roots.
    pub write_roots: Vec<PathBuf>,
    /// The output file of `render`.
    pub output: Option<PathBuf>,
    /// The image width of `render`.
    pub width: Option<u32>,
    /// The image height of `render`.
    pub height: Option<u32>,
}

fn usage(reason: impl Into<String>) -> Error {
    Error::new(codes::CLI_USAGE).with_detail("reason", reason.into())
}

fn size(name: &str, value: &str) -> Result<u32, Error> {
    value
        .parse::<u32>()
        .ok()
        .filter(|v| *v > 0 && !value.starts_with('+'))
        .ok_or_else(|| usage(format!("size:{name}")))
}

/// Parses `args` of a product subcommand; `allowed` names the flags it accepts.
///
/// # Errors
/// `cmd.cli.usage` with the reason for an unknown or repeated flag, a missing value or a wrong
/// number of arguments.
pub fn parse(args: Vec<OsString>, allowed: &[&str]) -> Result<Options, Error> {
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
        if name != "--json" && !allowed.contains(&name.as_str()) {
            return Err(usage(format!("unknown_flag:{name}")));
        }
        if matches!(name.as_str(), "--json" | "--save" | "--allow-edit") {
            if inline.is_some() {
                return Err(usage(format!("unexpected_value:{name}")));
            }
            match name.as_str() {
                "--save" => o.save = true,
                "--allow-edit" => o.allow_edit = true,
                _ => {}
            }
            continue;
        }
        let value = match inline {
            Some(v) => v,
            None => args
                .next()
                .and_then(|v| v.into_string().ok())
                .ok_or_else(|| usage(format!("missing_value:{name}")))?,
        };
        let repeated = || usage(format!("repeated:{name}"));
        match name.as_str() {
            "--read-root" => o.read_roots.push(PathBuf::from(value)),
            "--write-root" => o.write_roots.push(PathBuf::from(value)),
            "--output" if o.output.is_none() => o.output = Some(PathBuf::from(value)),
            "--width" if o.width.is_none() => o.width = Some(size(&name, &value)?),
            "--height" if o.height.is_none() => o.height = Some(size(&name, &value)?),
            _ => return Err(repeated()),
        }
    }
    if o.file.is_none() {
        return Err(usage("arguments"));
    }
    Ok(o)
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
    let sub = args.first().and_then(|a| a.to_str()).map(str::to_owned);
    if !matches!(sub.as_deref(), Some("session" | "render")) {
        return headless::run(args, product, stdin, stdout, stderr);
    }
    args.remove(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if sub.as_deref() == Some("session") {
            session::run(args, product, stdin, &mut stdout, &mut stderr)
        } else {
            render::run(args, product, &mut stdout)
        }
    }));
    let error = match outcome {
        Ok(Ok(exit)) => return exit,
        Ok(Err(error)) => error,
        Err(_) => Error::new(codes::FAULT),
    };
    headless::report_error(error, &mut stderr)
}
