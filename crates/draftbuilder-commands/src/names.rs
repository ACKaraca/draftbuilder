//! Name forms of a typed command (DB-CMD-014 and DB-CMD-013): the prefixes that select how a
//! typed name is read.
//!
//! The product registers one name resolver ([`resolver`]) with the registry, so every surface
//! that resolves names (the command line, the protocol, macros, scripts and the CLI) reads the
//! prefixes the same way:
//!
//! - no prefix: the registry's own rules (identifier, alias, localized name); the resolver does
//!   not answer;
//! - `_NAME`: the English name, a row of [`crate::vocabulary`] or an identifier; no flags;
//! - `-NAME`: the prompt form; the flag `no_dialogs` is set, since the product's commands never
//!   open a dialog and the prompt machine is the interactive form;
//! - `.NAME`: the built-in command, a row of the vocabulary or an identifier, never a user alias
//!   a script or an alias file added at run time; the flag `builtin_only` is set.
//!
//! The commands read these flags with `Context::name_flags`. No command opens a dialog today, so
//! none reads `no_dialogs` yet.

use std::collections::BTreeMap;

use arc_base::{Error, Result};
use arc_command::error::codes;
use arc_command::line::{CandidateKind, Completion, complete};
use arc_command::{NameFlags, NameMatch, NameResolver, Registry};

use crate::vocabulary::NAMES;

/// The form a typed name was written in.
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

/// The form and the name of a typed text, with surrounding whitespace and the prefix removed.
#[must_use]
pub fn parse(text: &str) -> (Form, &str) {
    let text = text.trim();
    let form = match text.chars().next() {
        Some('_') => Form::English,
        Some('-') => Form::Prompt,
        Some('.') => Form::Builtin,
        _ => Form::Typed,
    };
    let name = match form {
        Form::Typed => text,
        _ => text.get(1..).unwrap_or(""),
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

/// Resolves a typed name, with its prefix, through the registry (and so through the product's
/// resolver).
///
/// # Errors
///
/// As [`Registry::resolve`]: `cmd.unknown` when no command has the name.
pub fn resolve(registry: &Registry, text: &str) -> Result<Resolution> {
    resolve_with(registry, &BTreeMap::new(), text)
}

/// Resolves a typed name as [`resolve`], with the localized names of the active catalog.
///
/// # Errors
///
/// As [`resolve`], and `cmd.name.ambiguous` for a localized name that several commands share.
pub fn resolve_with(
    registry: &Registry,
    localized: &BTreeMap<String, String>,
    text: &str,
) -> Result<Resolution> {
    let (form, name) = parse(text);
    if name.is_empty() {
        return Err(Error::new(codes::UNKNOWN).with_detail("name", name));
    }
    let resolved = registry.resolve(text.trim(), localized)?;
    Ok(Resolution {
        form,
        id: resolved.spec.id().to_owned(),
    })
}

/// Completes a typed line as the registry reads it: surrounding whitespace is ignored, and a first
/// word with a prefix keeps the prefix on its command candidates. The built-in form offers only
/// the names it resolves (identifiers and vocabulary names), never a user alias.
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
    let width = prefix.len_utf8();
    let mut out = complete(
        registry,
        &BTreeMap::new(),
        text.get(width..).unwrap_or(""),
        cursor.saturating_sub(width),
    );
    if let Some(span) = out.span.as_mut() {
        if span.start > 0 {
            span.start += width;
        }
        span.end += width;
    }
    for candidate in &mut out.candidates {
        if candidate.kind == CandidateKind::Command {
            candidate.text.insert(0, prefix);
        }
    }
    if prefix == '.' {
        out.candidates.retain(|c| {
            c.kind != CandidateKind::Command
                || resolve(registry, &c.text).is_ok_and(|r| r.form == Form::Builtin)
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

/// The prefix characters the product reads, with the flags each one sets.
fn prefix_flags(prefix: char) -> Option<NameFlags> {
    match prefix {
        '_' => Some(NameFlags::default()),
        '-' => Some(NameFlags {
            no_dialogs: true,
            builtin_only: false,
        }),
        '.' => Some(NameFlags {
            no_dialogs: false,
            builtin_only: true,
        }),
        _ => None,
    }
}

/// The resolver of the product. It answers only the prefixed forms, and it decides from the name alone, so a
/// command registered after the resolver is installed resolves like any other: a prefixed name that is an
/// identifier (it contains a dot) is the command of that identifier, and any other prefixed name is looked
/// up in the vocabulary's typed names and aliases. A command the registry does not hold fails there with
/// `cmd.unknown`, so no registration order matters.
#[must_use]
pub fn resolver() -> NameResolver {
    let mut typed: BTreeMap<String, String> = BTreeMap::new();
    for row in NAMES {
        for name in row.typed.into_iter().chain(row.aliases.iter().copied()) {
            typed.insert(name.to_ascii_lowercase(), row.id.to_owned());
        }
    }
    NameResolver::new(move |name| {
        let mut chars = name.chars();
        let flags = prefix_flags(chars.next()?)?;
        let rest = chars.as_str().to_ascii_lowercase();
        if rest.is_empty() {
            return None;
        }
        let command = if rest.contains('.') {
            rest
        } else {
            typed.get(&rest)?.clone()
        };
        Some(NameMatch { command, flags })
    })
}

/// Installs the product's name resolver on `registry`.
///
/// # Errors
///
/// `cmd.duplicate` when the registry already has a resolver whose aliases conflict.
pub fn install(registry: &mut Registry) -> Result<()> {
    registry.set_name_resolver(resolver())
}

#[cfg(test)]
mod tests {
    use super::prefix_flags;

    #[test]
    fn each_prefix_sets_its_flags() {
        assert!(prefix_flags('_').is_some_and(|f| !f.no_dialogs && !f.builtin_only));
        assert!(prefix_flags('-').is_some_and(|f| f.no_dialogs && !f.builtin_only));
        assert!(prefix_flags('.').is_some_and(|f| f.builtin_only));
        assert!(prefix_flags('m').is_none());
    }
}
