//! `script`: runs a text file of typed command lines on a package (DB-AUTO-030, DB-CMD-011,
//! DB-CMD-012). The lines are run by [`draftbuilder_commands::script`] exactly as a user types
//! them; this module opens or creates the package, applies the roots and permissions of the flags,
//! saves the package when the script succeeds and writes the report.
//!
//! The arguments are `script <file.draftb> --script <file> [--create] [--save] [--allow-edit]
//! [--report <file>] [--read-root <dir>]... [--write-root <dir>]...`. The script file is read
//! through a read root, the report written through a write root. Standard output carries the
//! report as JSON; the echo of the lines (the command echo setting) and the failure go to standard
//! error. A failure exits with the code of its error's category and leaves the package unsaved.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_base::{Error, Limits};
use arc_command::error::codes;
use arc_command::headless::{self, Exit, Product};
use arc_command::{Permission, Roots, Session, Surface};
use arc_doc::package::{self, Contents, Metadata, Mode, Package};
use arc_doc::{DocumentConfig, LoadOptions};

/// The arguments of `script`.
#[derive(Debug, Default)]
struct Options {
    file: Option<PathBuf>,
    script: Option<PathBuf>,
    report: Option<PathBuf>,
    seed: Option<u64>,
    create: bool,
    save: bool,
    allow_edit: bool,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
}

/// A seed: a decimal `u64` without a sign, the syntax of the foundation's `run --seed`.
fn seed_number(text: &str) -> Result<u64, Error> {
    if text.starts_with('+') {
        return Err(usage("seed"));
    }
    text.parse::<u64>().map_err(|_| usage("seed"))
}

fn usage(reason: impl Into<String>) -> Error {
    Error::new(codes::CLI_USAGE).with_detail("reason", reason.into())
}

/// Parses the arguments of `script` (without the subcommand name).
fn parse(args: Vec<OsString>) -> Result<Options, Error> {
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
        match name.as_str() {
            "--create" | "--save" | "--allow-edit" | "--json" if inline.is_none() => {
                match name.as_str() {
                    "--create" => o.create = true,
                    "--save" => o.save = true,
                    "--allow-edit" => o.allow_edit = true,
                    _ => {}
                }
                continue;
            }
            "--script" | "--report" | "--read-root" | "--write-root" | "--seed" => {}
            _ => return Err(usage(format!("unknown_flag:{name}"))),
        }
        let value = match inline {
            Some(value) => value,
            None => args
                .next()
                .and_then(|v| v.into_string().ok())
                .ok_or_else(|| usage(format!("missing_value:{name}")))?,
        };
        match name.as_str() {
            "--script" if o.script.is_none() => o.script = Some(PathBuf::from(value)),
            "--report" if o.report.is_none() => o.report = Some(PathBuf::from(value)),
            "--seed" if o.seed.is_none() => o.seed = Some(seed_number(&value)?),
            "--read-root" => o.read_roots.push(PathBuf::from(value)),
            "--write-root" => o.write_roots.push(PathBuf::from(value)),
            _ => return Err(usage(format!("repeated:{name}"))),
        }
    }
    if o.file.is_none() {
        return Err(usage("arguments"));
    }
    if o.script.is_none() {
        return Err(usage("missing_value:--script"));
    }
    Ok(o)
}

/// The session of the script: the ceiling of the flags, the roots and the clock of the product,
/// with the product's answer for the keyword `last`.
fn new_session(o: &Options, product: &Product) -> Result<Session, Error> {
    let mut ceiling = std::collections::BTreeSet::from([Permission::Read]);
    if o.allow_edit {
        ceiling.insert(Permission::Edit);
    }
    if !o.read_roots.is_empty() || !o.write_roots.is_empty() {
        ceiling.insert(Permission::FileRead);
    }
    if !o.write_roots.is_empty() {
        ceiling.insert(Permission::FileWrite);
    }
    let mut session = Session::new(Surface::CommandLine, ceiling);
    session.roots = Roots::new(&o.read_roots, &o.write_roots)?;
    session.clock = Arc::clone(&product.clock);
    Ok(session.with_last_hook(product.last.clone()))
}

/// The script text, read through a read root and refused beyond the script size cap.
fn read_script(roots: &Roots, path: &Path) -> Result<String, Error> {
    let cap = draftbuilder_commands::script::MAX_SCRIPT_BYTES;
    let mut bytes = Vec::new();
    let file = roots.open_read(path)?;
    file.take(u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    if bytes.len() > cap {
        return Err(
            Error::new(draftbuilder_commands::error::codes::SCRIPT_LIMIT)
                .with_detail("limit", "script_bytes")
                .with_detail("max", cap.to_string()),
        );
    }
    String::from_utf8(bytes).map_err(|_| Error::new(codes::PATH_IO).with_detail("reason", "utf8"))
}

/// Runs `script` with the arguments `args` (without the program name and the subcommand name).
/// Returns the exit code.
pub fn run<E, W>(args: Vec<OsString>, product: &Product, mut stdout: W, stderr: &mut E) -> Exit
where
    E: Write,
    W: Write,
{
    let o = match parse(args) {
        Ok(o) => o,
        Err(error) => return headless::report_error(error, stderr),
    };
    match execute(&o, product, stderr) {
        Ok((json, exit)) => {
            let _ = writeln!(stdout, "{json}");
            exit
        }
        Err(error) => headless::report_error(error, stderr),
    }
}

/// The body of [`run`]. Every path is checked against the roots before anything runs: the report, the
/// script, and the package (a new name for `--create`, an existing package otherwise, and a write
/// root for the save). A created package is removed unless the script succeeded and `--save` was
/// given; the placeholder is removed too when the package cannot be written.
fn execute<E: Write>(
    o: &Options,
    product: &Product,
    stderr: &mut E,
) -> Result<(String, Exit), Error> {
    let roots = Roots::new(&o.read_roots, &o.write_roots)?;
    let limits = Limits::DEFAULT;
    let (Some(file), Some(script)) = (o.file.as_deref(), o.script.as_deref()) else {
        return Err(usage("arguments"));
    };
    if let Some(report) = &o.report {
        roots.check_write(report)?;
    }
    let text = read_script(&roots, script)?;
    let created = if o.create {
        Some(roots.reserve(file)?)
    } else {
        roots.open_read(file)?;
        if o.save {
            roots.check_write(file)?;
        }
        None
    };
    if let Some(target) = &created {
        let metadata = Metadata {
            created_by: Some(format!("{} {}", product.name, product.version)),
            ..Metadata::default()
        };
        let contents = Contents {
            metadata: &metadata,
            types: &[],
            objects: &[],
            previews: &[],
            snapshot: None,
        };
        if let Err(error) = package::write(target, &contents, &mut |_| Ok(())) {
            let _ = std::fs::remove_file(target);
            return Err(error);
        }
    }
    let target = created.clone().unwrap_or_else(|| file.to_path_buf());
    let editing = o.save || o.create;
    // Without `--seed` the run draws its own, so two runs create different identifiers.
    let seed = o.seed.unwrap_or_else(crate::os_seed);
    let report = match package_run(
        &target,
        editing,
        o.save,
        product,
        &limits,
        &mut new_session(o, product)?,
        &text,
        seed,
        stderr,
    ) {
        Ok(report) => report,
        Err(error) => {
            if created.is_some() {
                let _ = std::fs::remove_file(&target);
            }
            return Err(error);
        }
    };
    let kept = o.save && report.failure.is_none();
    if created.is_some() && !kept {
        let _ = std::fs::remove_file(&target);
    }
    let json = report.to_json().to_string();
    if let Some(path) = &o.report {
        let mut out = roots.create_write(path)?;
        out.write_all(json.as_bytes())
            .and_then(|()| out.write_all(b"\n"))
            .map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    }
    match report.failure {
        Some(failure) => Ok((json, headless::report_error(failure.error, stderr))),
        None => Ok((json, Exit::Success)),
    }
}

/// Opens the package (in edit mode when `editing`), runs the script on its document and saves the
/// package when `save` is set and the script succeeded.
#[allow(clippy::too_many_arguments, reason = "the parts of one run")]
fn package_run<E: Write>(
    target: &Path,
    editing: bool,
    save: bool,
    product: &Product,
    limits: &Limits,
    session: &mut Session,
    text: &str,
    seed: u64,
    stderr: &mut E,
) -> Result<draftbuilder_commands::script::Report, Error> {
    let mode = if editing {
        Mode::Edit(&product.owner)
    } else {
        Mode::ReadOnly
    };
    let mut package = Package::open(target, mode, limits)?;
    let options = LoadOptions {
        limits: *limits,
        ..LoadOptions::default()
    };
    let document = package.open_document(
        Arc::clone(&product.types),
        Arc::clone(&product.evaluators),
        &options,
        DocumentConfig::default(),
    )?;
    draftbuilder_commands::install(&document);
    let report = draftbuilder_commands::script::run(
        &document,
        &product.registry,
        session,
        text,
        seed,
        stderr,
    );
    if save && report.failure.is_none() {
        package.save_document(&document, &[], None, &mut |_| Ok(()))?;
    }
    Ok(report)
}
