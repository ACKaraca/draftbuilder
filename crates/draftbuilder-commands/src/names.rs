//! Name forms of a typed command (DB-CMD-014 and DB-CMD-013): the prefixes that select how a
//! typed name is read, resolved and completed.
//!
//! The name itself is resolved by arc-command's [`Registry::resolve`] (identifier, deprecated
//! alias, user alias, localized name) and completed by [`arc_command::complete`]; this module only
//! strips the prefix and decides what the prefix asks for:
//!
//! - no prefix: the typed name, as resolved by the registry;
//! - `_NAME`: the English name. The product has one catalog, so the localized names are not
//!   consulted;
//! - `-NAME`: the prompt form. The product's commands never open a dialog, so the form runs the
//!   same command; the form is reported so that a command line can start the prompt machine;
//! - `.NAME`: the built-in command. Only identifiers and the names of [`crate::vocabulary`] are
//!   consulted, never the user aliases a script or an alias file adds at run time.
//!
//! [`resolve`] is used by the CLI `run` subcommand, and [`complete_line`] is the completion for a
//! command line. The automation protocol passes a command name to arc-command unchanged, and arc-command has no
//! hook for the prefixes, so the protocol accepts identifiers, typed names and aliases only (see
//! `docs/architecture/command-line.md`).

use std::collections::BTreeMap;

use arc_base::{Error, Result};
use arc_command::Registry;
use arc_command::error::codes;
use arc_command::line::{Completion, complete};

use crate::vocabulary::NAMES;

/// How a typed name asks to be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// No prefix.
    Typed,
    /// `_` prefix: the English name.
    English,
    /// `-` prefix: the prompt form.
    Prompt,
    /// `.` prefix: the built-in command.
    Builtin,
}

impl Form {
    /// The prefix character of the form, `None` for the typed name.
    #[must_use]
    pub fn prefix(self) -> Option<char> {
        match self {
            Self::Typed => None,
            Self::English => Some('_'),
            Self::Prompt => Some('-'),
            Self::Builtin => Some('.'),
        }
    }
}

/// The form and the name of a typed text, the prefix removed.
#[must_use]
pub fn parse(text: &str) -> (Form, &str) {
    let text = text.trim();
    let form = match text.chars().next() {
        Some('_') => Form::English,
        Some('-') => Form::Prompt,
        Some('.') => Form::Builtin,
        _ => Form::Typed,
    };
    let name = match form.prefix() {
        Some(_) => text.get(1..).unwrap_or(""),
        None => text,
    };
    (form, name)
}

/// A typed name resolved to its command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The form the name was typed in.
    pub form: Form,
    /// The identifier of the command.
    pub id: String,
}

fn unknown(name: &str) -> Error {
    Error::new(codes::UNKNOWN).with_detail("name", name)
}

/// Resolves a typed name, with its prefix, to a registered command.
///
/// # Errors
/// `cmd.unknown` (detail `name`, the name without its prefix) when no command has the name, and
/// the errors of [`Registry::resolve`] (`cmd.name.ambiguous`).
pub fn resolve(registry: &Registry, text: &str) -> Result<Resolution> {
    resolve_with(registry, &BTreeMap::new(), text)
}

/// Resolves a typed name as [`resolve`], with the localized names of the active catalog (identifier
/// to localized name). A localized name shared by several commands is `cmd.name.ambiguous`. The
/// built-in form does not consult localized names.
///
/// # Errors
/// As [`resolve`], and `cmd.name.ambiguous` for a localized name that several commands share.
pub fn resolve_with(
    registry: &Registry,
    localized: &BTreeMap<String, String>,
    text: &str,
) -> Result<Resolution> {
    let (form, name) = parse(text);
    if name.is_empty() {
        return Err(unknown(name));
    }
    let id = match form {
        Form::Builtin => builtin(registry, name).ok_or_else(|| unknown(name))?,
        Form::Typed | Form::English | Form::Prompt => {
            registry.resolve(name, localized)?.spec.id().to_owned()
        }
    };
    Ok(Resolution { form, id })
}

/// The identifier a built-in name stands for: a registered identifier, or a row of the
/// vocabulary whose typed name or alias is the name (ASCII case ignored).
fn builtin(registry: &Registry, name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if registry.get(&lower).is_some() {
        return Some(lower);
    }
    NAMES
        .iter()
        .find(|row| {
            row.typed
                .into_iter()
                .chain(row.aliases.iter().copied())
                .any(|n| n.eq_ignore_ascii_case(name))
        })
        .filter(|row| registry.get(row.id).is_some())
        .map(|row| row.id.to_owned())
}

/// Completes a typed line as [`resolve`] reads it: surrounding whitespace is ignored, and a first
/// word with a prefix keeps the prefix in its command candidates. The built-in form offers only the
/// names it resolves (identifiers and vocabulary names), never the user aliases a script added.
#[must_use]
pub fn complete_line(registry: &Registry, text: &str, cursor: usize) -> Completion {
    let lead = text.len() - text.trim_start().len();
    let text = text.trim();
    let cursor = cursor.saturating_sub(lead).min(text.len());
    let prefix = text.chars().next().filter(|c| matches!(c, '_' | '-' | '.'));
    let Some(prefix) = prefix else {
        let mut out = complete(registry, &BTreeMap::new(), text, cursor);
        shift(&mut out, lead);
        return out;
    };
    let shift_by = prefix.len_utf8();
    let inner = text.get(shift_by..).unwrap_or("");
    let mut out = complete(
        registry,
        &BTreeMap::new(),
        inner,
        cursor.saturating_sub(shift_by),
    );
    // A span that starts at the first word keeps the prefix in the range it replaces, since the
    // command candidates written back carry it.
    if let Some(span) = out.span.as_mut() {
        if span.start > 0 {
            span.start += shift_by;
        }
        span.end += shift_by;
    }
    for candidate in &mut out.candidates {
        if candidate.kind == arc_command::line::CandidateKind::Command {
            candidate.text.insert(0, prefix);
        }
    }
    if prefix == '.' {
        out.candidates.retain(|c| {
            c.kind != arc_command::line::CandidateKind::Command
                || builtin(registry, c.text.get(1..).unwrap_or("")).is_some()
        });
    }
    shift(&mut out, lead);
    out
}

/// Moves every span of a completion by `by` bytes.
fn shift(out: &mut Completion, by: usize) {
    if let Some(span) = out.span.as_mut() {
        span.start += by;
        span.end += by;
    }
}
