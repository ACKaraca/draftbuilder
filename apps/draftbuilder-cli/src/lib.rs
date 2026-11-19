//! The headless CLI of DraftBuilder.
//!
//! [`run`] is the program. The shared subcommands of the Arc suite
//! ([`arc_command::headless`]: `commands`, `describe`, `validate-params`, `run`, `play`,
//! `batch`, `inspect`, `validate`, `serve`, `mcp`) run over the DraftBuilder type set and
//! command set, with the product's drafting validators installed on every document and
//! `view.render` answered by the drafting renderer. The product adds `render`
//! (`render <file> --output <png> [--width N] [--height N] [--write-root D]`, module
//! [`render`]), which draws the model space headless to a PNG file.
//!
//! The working file is a document package (extension `draftb`): it holds the drawing database
//! of an opened DXF drawing so that several processes can work on it. A script that opens a
//! DXF drawing, moves, undoes and saves runs over `serve --stdio --create <file> --save
//! --allow-edit` with the protocol methods `command.run` and `history.undo`.

pub mod render;

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

/// The `run` flags that take a value in the next argument.
const RUN_VALUE_FLAGS: &[&str] = &[
    "--read-root",
    "--write-root",
    "--params",
    "--macro",
    "--record",
    "--seed",
];

/// Where the names given to `--command` in a `run` line are: the index of the argument after a
/// `--command` flag, or the index of an argument `--command=NAME`. A value of another flag is
/// skipped, so an argument that merely equals `--command` is not a name.
fn command_positions(args: &[OsString]) -> Vec<usize> {
    let mut out = Vec::new();
    if args.first().and_then(|a| a.to_str()) != Some("run") {
        return out;
    }
    let mut i = 1;
    while i < args.len() {
        match args.get(i).and_then(|a| a.to_str()) {
            Some("--command") => {
                if i + 1 < args.len() {
                    out.push(i + 1);
                }
                i += 2;
            }
            Some(flag) if flag.starts_with("--command=") => {
                out.push(i);
                i += 1;
            }
            Some(flag) if RUN_VALUE_FLAGS.contains(&flag) => i += 2,
            _ => i += 1,
        }
    }
    out
}

/// Replaces the names given to `--command` in a `run` line by the identifiers they resolve to, so
/// the name forms of [`draftbuilder_commands::names`] run the command. A name that resolves to no
/// command is left as typed, so the runner reports it as `cmd.unknown`; any other resolution error,
/// such as `cmd.name.ambiguous`, is returned.
///
/// # Errors
/// The errors of [`draftbuilder_commands::names::resolve`] other than `cmd.unknown`.
fn resolve_command_names(args: &mut [OsString], product: &Product) -> Result<(), Error> {
    for at in command_positions(args) {
        let Some(text) = args.get(at).and_then(|a| a.to_str()).map(str::to_owned) else {
            continue;
        };
        let (flag, name) = match text.strip_prefix("--command=") {
            Some(name) => ("--command=", name.to_owned()),
            None => ("", text.clone()),
        };
        match draftbuilder_commands::names::resolve(&product.registry, &name) {
            Ok(resolved) => args[at] = OsString::from(format!("{flag}{}", resolved.id)),
            Err(error) if error.code() == codes::UNKNOWN.code => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
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
    if let Err(error) = resolve_command_names(&mut args, product) {
        return headless::report_error(error, &mut stderr);
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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::command_positions;

    fn args(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_the_value_of_a_command_flag_is_a_name() {
        let line = args(&["run", "p.draftb", "--command", "NEW", "--params", "{}"]);
        assert_eq!(command_positions(&line), [3]);
        let inline = args(&["run", "p.draftb", "--command=-NEW"]);
        assert_eq!(command_positions(&inline), [2]);
    }

    #[test]
    fn an_argument_that_equals_the_flag_is_not_a_name() {
        // `--command` here is the value of `--read-root`: the name after it is not a command name.
        let line = args(&["run", "p.draftb", "--read-root", "--command", "NEW"]);
        assert_eq!(command_positions(&line), Vec::<usize>::new());
        let params = args(&[
            "run",
            "p.draftb",
            "--params",
            "--command",
            "--command",
            "NEW",
        ]);
        assert_eq!(command_positions(&params), [5]);
    }

    #[test]
    fn other_subcommands_have_no_command_names() {
        let line = args(&["serve", "--command", "NEW"]);
        assert_eq!(command_positions(&line), Vec::<usize>::new());
    }
}
