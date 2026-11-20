//! Undo directives (DB-DBASE-038, DB-DBASE-039): the typed forms of the document's history
//! operations, read by the script runner and by the interaction controller alike.
//!
//! The forms, with their words from [`crate::vocabulary::history_words`]:
//!
//! - `UNDO [count]`, `U [count]` and `REDO [count]` step the undo or redo stack. A count of
//!   [`MAX_STEP_COUNT`] or less steps that many steps in one transaction: all or nothing.
//! - `UNDO MARK name` sets a named mark at the top of the undo stack; `UNDO BACK name` undoes every
//!   step above that mark.
//! - `UNDO GROUP name` opens a group, and every command committed while it is open forms one
//!   undo step; `UNDO ENDGROUP` closes the innermost open group.
//! - `OOPS` restores the objects that the most recent erase removed, and reports how many came back.
//!
//! The history operations are the document's own, not commands: a command's context cannot reach
//! the undo stack, so the forms are read before a command line, as the script runner and the
//! controller read their other directives. Every form changes the document or its history and
//! needs the `edit` permission. Nothing here touches the automation protocol.

use arc_base::{Error, Result};
use arc_command::history::{self, Direction};
use arc_command::{Permission, Session};
use arc_doc::Document;
use serde_json::json;

use crate::error::codes;
use crate::vocabulary::history_words as words;

/// The most steps one count may name. A larger count is refused as a malformed directive.
pub const MAX_STEP_COUNT: usize = 1_000;

/// The longest mark or group name, in bytes: the document's own limit on marks.
pub const MAX_NAME_BYTES: usize = 128;

/// A label of a history operation: the message identifier its catalog entry is under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label {
    /// The message identifier (in `locales/en-US/draftbuilder-commands.ftl`).
    pub message_id: &'static str,
}

/// The label a restore of erased objects records in the journal.
pub const RESTORE_LABEL: Label = Label {
    message_id: "db-hist-restore-erased",
};

/// One typed history directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Steps the undo or redo stack by `count` steps (1 to [`MAX_STEP_COUNT`]).
    Step {
        /// Undo or redo.
        direction: Direction,
        /// The number of steps.
        count: usize,
    },
    /// Sets the mark `name` at the top of the undo stack.
    SetMark(String),
    /// Undoes every step above the mark `name`.
    BackToMark(String),
    /// Opens the group `name`.
    BeginGroup(String),
    /// Closes the innermost open group.
    EndGroup,
    /// Restores the objects removed by the most recent erase.
    RestoreErased,
}

impl Request {
    /// The name a script report gives the operation, in the `history.` namespace of the protocol's
    /// history methods. Only a restore records this name in the journal; steps, marks and groups
    /// leave no record of their own.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Step {
                direction: Direction::Undo,
                ..
            } => "history.undo",
            Self::Step {
                direction: Direction::Redo,
                ..
            } => "history.redo",
            Self::SetMark(_) => "history.set_mark",
            Self::BackToMark(_) => "history.undo_to_mark",
            Self::BeginGroup(_) => "history.begin_group",
            Self::EndGroup => "history.end_group",
            Self::RestoreErased => "history.restore_erased",
        }
    }
}

/// What a directive did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// The operation's name ([`Request::name`]).
    pub name: &'static str,
    /// The steps stepped (undo, redo or back-to-mark); zero for the others.
    pub steps: usize,
    /// The objects restored by a restore; zero for the others.
    pub restored: usize,
    /// For a single step: the command identifier of the record it reversed or restored, if named.
    pub command: Option<String>,
    /// For a single step: the message identifier of that command's label, if named.
    pub label: Option<String>,
}

/// The first whitespace-separated word of `text` and the trimmed rest.
fn split_first(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    match text.find(char::is_whitespace) {
        Some(at) => (&text[..at], text[at..].trim()),
        None => (text, ""),
    }
}

/// Reads `text` as a history directive. `None` when its first word is not a history word, so the
/// line belongs to the command line; `Some(Err)` for a history word with a malformed rest.
#[must_use]
pub fn parse(text: &str) -> Option<Result<Request>> {
    let (word, rest) = split_first(text);
    let refused = || Error::new(codes::HISTORY_FORM);
    let word = word.to_ascii_uppercase();
    let direction = if word == words::UNDO || word == words::UNDO_SHORT {
        Direction::Undo
    } else if word == words::REDO {
        Direction::Redo
    } else if word == words::OOPS {
        return Some(if rest.is_empty() {
            Ok(Request::RestoreErased)
        } else {
            Err(refused())
        });
    } else {
        return None;
    };
    let (form, after) = split_first(rest);
    if form.is_empty() {
        return Some(Ok(Request::Step {
            direction,
            count: 1,
        }));
    }
    if form.starts_with('+') {
        return Some(Err(refused()));
    }
    if let Ok(count) = form.parse::<usize>() {
        return Some(
            if (1..=MAX_STEP_COUNT).contains(&count) && after.is_empty() {
                Ok(Request::Step { direction, count })
            } else {
                Err(refused())
            },
        );
    }
    let form = form.to_ascii_uppercase();
    if direction != Direction::Undo {
        return Some(Err(refused()));
    }
    let named =
        |name: &str| (!name.is_empty() && name.len() <= MAX_NAME_BYTES).then(|| name.to_owned());
    let request = if form == words::ENDGROUP {
        after.is_empty().then_some(Request::EndGroup)
    } else if form == words::MARK {
        named(after).map(Request::SetMark)
    } else if form == words::BACK {
        named(after).map(Request::BackToMark)
    } else if form == words::GROUP {
        named(after).map(Request::BeginGroup)
    } else {
        None
    };
    Some(request.ok_or_else(refused))
}

/// Refuses a directive without the `edit` permission, before anything is stepped or restored.
///
/// # Errors
///
/// `cmd.permission_denied` (detail `permission` `edit`) when the session lacks `edit`.
pub fn require_edit(session: &Session, request: &Request) -> Result<()> {
    if session.permissions.contains(&Permission::Edit) {
        return Ok(());
    }
    Err(Error::new(arc_command::error::codes::PERMISSION_DENIED)
        .with_arg("command", request.name())
        .with_detail("permission", "edit"))
}

/// Runs `request` on `document` for `session`. Each form is one document operation: a count is
/// one transaction, and a refused operation changes nothing.
///
/// # Errors
///
/// `cmd.permission_denied` without `edit`; `cmd.history.empty` for a single step with nothing on
/// its stack; `doc.undo.sequence_invalid` for a count above the stack, a missing or stale mark, or
/// a group that is not open or is nested in the wrong order; `db.cmd.nothing_to_restore` when
/// the most recent erase has no object to bring back.
pub fn apply(document: &Document, session: &Session, request: &Request) -> Result<Applied> {
    require_edit(session, request)?;
    let mut applied = Applied {
        name: request.name(),
        ..Applied::default()
    };
    match request {
        Request::Step {
            direction,
            count: 1,
        } => {
            let stepped = history::step(document, session, *direction)?;
            applied.steps = 1;
            applied.command = stepped.command;
            applied.label = stepped.label;
        }
        Request::Step { direction, count } => {
            history::step_many(document, session, *direction, *count)?;
            applied.steps = *count;
        }
        Request::SetMark(mark) => {
            document.set_mark(mark.clone()).wait()?;
        }
        Request::BackToMark(mark) => {
            let before = document.undo_step_list(usize::MAX).len();
            if document.undo_to_mark(mark.clone()).wait()?.value {
                let after = document.undo_step_list(usize::MAX).len();
                applied.steps = before.saturating_sub(after);
            }
        }
        Request::BeginGroup(group) => {
            document.begin_group(group.clone()).wait()?;
        }
        Request::EndGroup => {
            document.end_group().wait()?;
        }
        Request::RestoreErased => {
            let command = json!({ "command": applied.name, "params": {} }).to_string();
            let label = json!({ "message_id": RESTORE_LABEL.message_id }).to_string();
            let restored = document.restore_erased(command, label).wait()?.value;
            if restored == 0 {
                return Err(Error::new(codes::NOTHING_TO_RESTORE));
            }
            applied.restored = restored;
        }
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_forms_are_read_and_malformed_ones_refused() {
        let undo = |count| Request::Step {
            direction: Direction::Undo,
            count,
        };
        assert_eq!(parse("u").unwrap().unwrap(), undo(1));
        assert_eq!(parse("UNDO 3").unwrap().unwrap(), undo(3));
        assert_eq!(
            parse("redo 2").unwrap().unwrap(),
            Request::Step {
                direction: Direction::Redo,
                count: 2
            }
        );
        assert_eq!(
            parse("UNDO MARK  before  wall").unwrap().unwrap(),
            Request::SetMark("before  wall".into())
        );
        assert_eq!(
            parse("UNDO BACK a").unwrap().unwrap(),
            Request::BackToMark("a".into())
        );
        assert_eq!(
            parse("UNDO GROUP g").unwrap().unwrap(),
            Request::BeginGroup("g".into())
        );
        assert_eq!(parse("UNDO ENDGROUP").unwrap().unwrap(), Request::EndGroup);
        assert_eq!(parse("OOPS").unwrap().unwrap(), Request::RestoreErased);
        let long = "n".repeat(MAX_NAME_BYTES + 1);
        assert!(parse(&format!("UNDO MARK {long}")).unwrap().is_err());
        assert!(parse(&format!("UNDO GROUP {long}")).unwrap().is_err());
        let exact = "n".repeat(MAX_NAME_BYTES);
        assert!(parse(&format!("UNDO GROUP {exact}")).unwrap().is_ok());
        assert!(parse("LINE").is_none());
        assert!(parse("OOPSIE").is_none());
        for bad in [
            "UNDO 0",
            "UNDO 1001",
            "UNDO -1",
            "UNDO 1.5",
            "UNDO 2 3",
            "UNDO MARK",
            "UNDO BACK",
            "UNDO GROUP",
            "UNDO ENDGROUP x",
            "UNDO JUNK",
            "REDO MARK m",
            "REDO ENDGROUP",
            "OOPS now",
            "UNDO +3",
        ] {
            let parsed = parse(bad).unwrap_or_else(|| panic!("{bad} is a history word"));
            assert_eq!(parsed.unwrap_err().code(), "db.cmd.history_form", "{bad}");
        }
    }
}
