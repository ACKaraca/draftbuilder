//! The `session` subcommand: a script from standard input over one open package.
//!
//! One JSON object per line:
//!
//! - `{"command": "<id>", "params": {...}, "seed": N}` runs a command (`params` and `seed`
//!   are optional) and answers `{"ok": <result>}`;
//! - `{"undo": true}` and `{"redo": true}` answer `{"ok": {"changed": <bool>}}`;
//! - `{"list": "entities"}` answers the model space entities in draw order, each with its
//!   object identifier, type, handle and stable identifier.
//!
//! The first failing line stops the session with the error on standard error and the exit code
//! of its category; nothing is saved then. At the end of input, `--save` saves the package.

use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::sync::Arc;

use arc_base::{Error, Limits};
use arc_command::error::codes;
use arc_command::headless::{Exit, Product};
use arc_command::{Invocation, Invoker, Permission, Roots, Session, Surface};
use arc_doc::package::{Metadata, Mode, Package};
use arc_doc::{DocumentConfig, LoadOptions, Snapshot};
use draftbuilder_io::save::space_entities;
use draftbuilder_model::{Database, Handle};
use serde_json::{Value, json};

use crate::{Options, parse, usage};

/// The longest request line accepted, in bytes.
pub const MAX_LINE_BYTES: u64 = 1 << 20;

const FLAGS: &[&str] = &["--save", "--allow-edit", "--read-root", "--write-root"];

fn io_error(error: std::io::Error) -> Error {
    Error::new(codes::PATH_IO).with_source(error)
}

/// Opens the package, or creates it when `create` is set and there is none.
fn open_package(
    path: &Path,
    product: &Product,
    limits: &Limits,
    save: bool,
) -> Result<Package, Error> {
    if save && !path.exists() {
        return Package::create(path, &Metadata::default(), &product.owner);
    }
    let mode = if save {
        Mode::Edit(&product.owner)
    } else {
        Mode::ReadOnly
    };
    Package::open(path, mode, limits)
}

/// Reads one line of at most [`MAX_LINE_BYTES`] bytes into `line`; false at the end of input.
fn read_line<R: BufRead>(input: &mut R, line: &mut String) -> Result<bool, Error> {
    line.clear();
    let read = input
        .by_ref()
        .take(MAX_LINE_BYTES + 1)
        .read_line(line)
        .map_err(io_error)?;
    if line.len() as u64 > MAX_LINE_BYTES {
        return Err(usage("line_too_long"));
    }
    Ok(read > 0)
}

/// The model space entities of `snapshot` with their handles and stable identifiers.
fn entities(snapshot: &Snapshot) -> Value {
    let database = Database::of(snapshot);
    let ids = draftbuilder_commands::model_space(snapshot)
        .map(|space| space_entities(snapshot, space))
        .unwrap_or_default();
    Value::Array(
        ids.into_iter()
            .map(|id| {
                let kind = snapshot
                    .resolve(id)
                    .ok()
                    .and_then(|h| snapshot.type_key(h))
                    .map(|k| k.as_str().to_owned());
                json!({
                    "id": id.to_string(),
                    "type": kind,
                    "handle": database.handle_of(id).map(Handle::to_hex),
                    "stable_id": database.stable_id_of(id).map(|s| s.to_string()),
                })
            })
            .collect(),
    )
}

/// One request: its answer, or the error that stops the session.
fn request(
    line: &str,
    product: &Product,
    document: &arc_doc::Document,
    session: &Session,
) -> Result<Value, Error> {
    let request: Value = serde_json::from_str(line).map_err(|_| usage("request"))?;
    let flag = |name: &str| request.get(name).and_then(Value::as_bool) == Some(true);
    if flag("undo") || flag("redo") {
        let pending = if flag("undo") {
            document.undo()
        } else {
            document.redo()
        };
        return Ok(json!({"changed": pending.wait()?.value}));
    }
    if request.get("list").and_then(Value::as_str) == Some("entities") {
        return Ok(entities(&document.snapshot()));
    }
    let command = request
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| usage("request"))?;
    let invocation = Invocation {
        seed: request.get("seed").and_then(Value::as_u64).unwrap_or(0),
        ..Invocation::new(command, request.get("params").cloned().unwrap_or(json!({})))
    };
    Ok(Invoker::new(&product.registry)
        .invoke(document, session, invocation)?
        .value)
}

/// Runs the session.
///
/// # Errors
/// `cmd.cli.usage` for the arguments and request lines, the package's errors, and the errors of
/// the commands, undo and redo.
pub fn run<R: BufRead, W: Write, E: Write>(
    args: Vec<std::ffi::OsString>,
    product: &Product,
    mut stdin: R,
    stdout: &mut W,
    _stderr: &mut E,
) -> Result<Exit, Error> {
    let Options {
        file,
        save,
        allow_edit,
        read_roots,
        write_roots,
        ..
    } = parse(args, FLAGS)?;
    let path = file.ok_or_else(|| usage("arguments"))?;
    let limits = Limits::DEFAULT;
    let mut package = open_package(&path, product, &limits, save)?;
    let document = package.open_document(
        Arc::clone(&product.types),
        Arc::clone(&product.evaluators),
        &LoadOptions {
            limits,
            ..LoadOptions::default()
        },
        DocumentConfig::default(),
    )?;
    draftbuilder_commands::install(&document);
    let mut granted = vec![Permission::Read];
    if allow_edit {
        granted.push(Permission::Edit);
    }
    if !read_roots.is_empty() || !write_roots.is_empty() {
        granted.push(Permission::FileRead);
    }
    if !write_roots.is_empty() {
        granted.push(Permission::FileWrite);
    }
    let mut session = Session::new(Surface::Automation, granted);
    session.roots = Roots::new(&read_roots, &write_roots)?;
    session.clock = Arc::clone(&product.clock);
    let mut line = String::new();
    while read_line(&mut stdin, &mut line)? {
        if line.trim().is_empty() {
            continue;
        }
        let answer = request(&line, product, &document, &session)?;
        writeln!(stdout, "{}", json!({"ok": answer}))
            .and_then(|()| stdout.flush())
            .map_err(io_error)?;
    }
    if save {
        package.save_document(&document, &[], None, &mut |_| Ok(()))?;
    }
    Ok(Exit::Success)
}
