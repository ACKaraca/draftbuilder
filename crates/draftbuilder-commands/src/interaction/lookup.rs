//! Lookups of the command line that read the registry and the session and change nothing: the
//! command palette (DB-CMD-008), the ranked suggestions for a typed text (DB-CMD-007) and help for a
//! command or the active prompt (DB-CMD-046).
//!
//! The functions take the registry and the values they need, so a host calls them through
//! [`super::Controller`], which holds both. Labels and descriptions are message identifiers: the host
//! looks them up in its catalog, so the text shown follows the interface language.

use std::collections::BTreeSet;

use arc_command::{CommandSpec, Registry};

use super::PromptOut;
use crate::names;
use crate::settings;
use crate::vocabulary::NAMES;

/// The longest text a lookup reads, in bytes. A longer text matches nothing.
pub const MAX_QUERY_BYTES: usize = 64;

/// The most entries one palette search returns.
pub const MAX_RESULTS: usize = 100;

/// The most suggestions one typed text gets.
pub const MAX_SUGGESTIONS: usize = 20;

/// The most recently run commands an empty line suggests.
pub const RECENT_COMMANDS: usize = 10;

/// A command as the palette and help show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    /// The command identifier.
    pub id: String,
    /// The label's message identifier.
    pub label_id: String,
    /// The description's message identifier.
    pub description_id: String,
    /// The typed names and aliases, in upper case, in alias order.
    pub aliases: Vec<String>,
}

/// The kind of a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    /// A conventional typed name of a command.
    Command,
    /// An alias of a command.
    Alias,
    /// A setting, by its product name or its compatibility name.
    Setting,
    /// The command whose typed name is closest to a text that matches no name (autocorrect).
    Corrected,
}

/// One suggestion for a typed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The text to insert: an upper-case name for a command or an alias, the setting's name otherwise.
    pub text: String,
    /// What the text names.
    pub kind: SuggestionKind,
    /// The command the text runs, for a command, an alias or a corrected name.
    pub command: Option<String>,
}

/// Help for one command, and for the prompt it waits at when it is the active command.
#[derive(Debug, Clone, PartialEq)]
pub struct Help {
    /// The command.
    pub command: CommandEntry,
    /// The active prompt, when help is asked for the active command.
    pub prompt: Option<PromptOut>,
}

/// The commands a surface exposes, in identifier order.
fn exposed(registry: &Registry) -> impl Iterator<Item = &std::sync::Arc<CommandSpec>> + '_ {
    registry.iter().filter(|spec| !spec.exposure().is_empty())
}

/// The entry of `spec`, with the typed names and aliases the registry holds for it.
fn entry(registry: &Registry, spec: &CommandSpec) -> CommandEntry {
    CommandEntry {
        id: spec.id().to_owned(),
        label_id: spec.label().to_owned(),
        description_id: spec.description().to_owned(),
        aliases: registry
            .user_aliases()
            .filter(|(_, id)| *id == spec.id())
            .map(|(alias, _)| alias.to_ascii_uppercase())
            .collect(),
    }
}

/// The words of a label's message identifier, as the palette matches them: the identifier without
/// its `db-cmd-` prefix and `-label` suffix, with the dashes as spaces.
fn label_words(label_id: &str) -> String {
    let rest = label_id.strip_prefix("db-cmd-").unwrap_or(label_id);
    rest.strip_suffix("-label")
        .unwrap_or(rest)
        .replace('-', " ")
}

/// How well `entry` matches `query` (lower case): 0 for an exact identifier or alias, 1 for a prefix
/// of one, 2 for a part of an identifier, an alias or the label's words, and `None` for no match. An
/// empty query matches every entry at 3.
fn rank(entry: &CommandEntry, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(3);
    }
    let names: Vec<String> = std::iter::once(entry.id.to_ascii_lowercase())
        .chain(entry.aliases.iter().map(|a| a.to_ascii_lowercase()))
        .collect();
    if names.iter().any(|name| name == query) {
        return Some(0);
    }
    if names.iter().any(|name| name.starts_with(query)) {
        return Some(1);
    }
    let found = names.iter().any(|name| name.contains(query))
        || label_words(&entry.label_id).contains(query);
    found.then_some(2)
}

/// The commands whose identifier, typed name, alias or label matches `query`, best matches first and
/// then by identifier, at most `limit` (and [`MAX_RESULTS`]). A query longer than [`MAX_QUERY_BYTES`]
/// matches nothing.
#[must_use]
pub fn search(registry: &Registry, query: &str, limit: usize) -> Vec<CommandEntry> {
    let query = query.trim().to_ascii_lowercase();
    if query.len() > MAX_QUERY_BYTES {
        return Vec::new();
    }
    let mut ranked: Vec<(u8, CommandEntry)> = exposed(registry)
        .filter_map(|spec| {
            let entry = entry(registry, spec);
            rank(&entry, &query).map(|rank| (rank, entry))
        })
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    ranked
        .into_iter()
        .take(limit.min(MAX_RESULTS))
        .map(|(_, entry)| entry)
        .collect()
}

/// The entry of the command a typed name runs, in any form the command line reads (`_NAME`, `-NAME`,
/// `.NAME`, an alias, an identifier). `None` when no command has the name.
#[must_use]
pub fn entry_named(registry: &Registry, name: &str) -> Option<CommandEntry> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_QUERY_BYTES {
        return None;
    }
    let resolution = names::resolve(registry, name).ok()?;
    registry
        .get(&resolution.id)
        .map(|spec| entry(registry, spec))
}

/// Every name the command line takes for a command or a setting, before any filter: the typed names
/// and aliases the registry holds for exposed commands, and the setting names. Each name appears once.
fn names_of(registry: &Registry) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for (alias, id) in registry.user_aliases() {
        let exposed = registry
            .get(id)
            .is_some_and(|spec| !spec.exposure().is_empty());
        if !exposed {
            continue;
        }
        let text = alias.to_ascii_uppercase();
        let kind = if NAMES.iter().any(|row| row.typed == Some(text.as_str())) {
            SuggestionKind::Command
        } else {
            SuggestionKind::Alias
        };
        out.push(Suggestion {
            text,
            kind,
            command: Some(id.to_owned()),
        });
    }
    for setting in settings::TABLE {
        out.push(Suggestion {
            text: setting.name.to_owned(),
            kind: SuggestionKind::Setting,
            command: None,
        });
        if let Some(compat) = setting.compat {
            out.push(Suggestion {
                text: compat.to_owned(),
                kind: SuggestionKind::Setting,
                command: None,
            });
        }
    }
    out
}

/// The edit distance between `a` and `b` when it is at most `bound`, else `None`.
fn distance_within(a: &str, b: &str, bound: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > bound {
        return None;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (previous[j] + cost)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        previous = current;
    }
    let distance = previous.last().copied().unwrap_or(usize::MAX);
    (distance <= bound).then_some(distance)
}

/// The suggestions for the text typed on the command line, with `recent` the identifiers of the
/// commands run most recently first.
///
/// An empty text suggests the recent commands. Otherwise the names that start with the text come
/// first, then the names that contain it; within those, the names of recent commands come first, then
/// by name. When nothing matches and `autocorrect` is set, the command whose typed name is closest
/// (within a third of the text's length, at least one edit) is suggested as a correction. A text longer
/// than [`MAX_QUERY_BYTES`] gets no suggestion.
#[must_use]
pub fn suggest(
    registry: &Registry,
    text: &str,
    recent: &[String],
    autocorrect: bool,
) -> Vec<Suggestion> {
    let text = text.trim().to_ascii_lowercase();
    if text.len() > MAX_QUERY_BYTES {
        return Vec::new();
    }
    let names = names_of(registry);
    if text.is_empty() {
        return recent
            .iter()
            .take(RECENT_COMMANDS)
            .filter_map(|id| {
                names
                    .iter()
                    .filter(|s| s.command.as_deref() == Some(id.as_str()))
                    .min_by_key(|s| s.kind != SuggestionKind::Command)
                    .cloned()
            })
            .collect();
    }
    let mut ranked: Vec<(usize, u8, &Suggestion)> = names
        .iter()
        .filter_map(|s| {
            let name = s.text.to_ascii_lowercase();
            let rank = if name.starts_with(&text) {
                0
            } else if name.contains(&text) {
                1
            } else {
                return None;
            };
            let recency = s
                .command
                .as_deref()
                .and_then(|id| recent.iter().position(|r| r == id))
                .unwrap_or(usize::MAX);
            Some((recency, rank, s))
        })
        .collect();
    ranked.sort_by(|a, b| (a.0, a.1, &a.2.text).cmp(&(b.0, b.1, &b.2.text)));
    let mut seen = BTreeSet::new();
    let mut out: Vec<Suggestion> = ranked
        .into_iter()
        .filter(|(_, _, s)| seen.insert(s.text.clone()))
        .take(MAX_SUGGESTIONS)
        .map(|(_, _, s)| s.clone())
        .collect();
    if out.is_empty() && autocorrect {
        let bound = (text.len() / 3).max(1);
        let closest = names
            .iter()
            .filter(|s| s.kind == SuggestionKind::Command)
            .filter_map(|s| {
                distance_within(&s.text.to_ascii_lowercase(), &text, bound).map(|d| (d, s))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.text.cmp(&b.1.text)));
        if let Some((_, s)) = closest {
            out.push(Suggestion {
                text: s.text.clone(),
                kind: SuggestionKind::Corrected,
                command: s.command.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{distance_within, label_words};

    #[test]
    fn the_edit_distance_stops_at_its_bound() {
        assert_eq!(distance_within("move", "mvoe", 2), Some(2));
        assert_eq!(distance_within("move", "move", 0), Some(0));
        assert_eq!(distance_within("move", "copy", 2), None);
        assert_eq!(distance_within("move", "mo", 1), None);
    }

    #[test]
    fn a_label_key_reads_as_its_words() {
        assert_eq!(label_words("db-cmd-open-drawing-label"), "open drawing");
        assert_eq!(label_words("core.draft.line"), "core.draft.line");
    }
}
