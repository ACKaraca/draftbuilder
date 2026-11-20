//! Scripts (DB-AUTO-011, DB-AUTO-030, DB-CMD-011, DB-CMD-012): a text file of typed command lines,
//! run through arc-command's command line exactly as a user types them at the command line.
//!
//! Each line is one typed input. A line that starts a command runs it, or leaves it waiting at its
//! first prompt; a line while a prompt waits answers it. The empty line is Enter: it accepts the
//! prompt's default, or repeats the last command when no command waits. A line that starts with an
//! apostrophe runs a transparent command beside the waiting prompt. A line whose first character is
//! a semicolon is a comment. The script stops at the first line that fails and reports it.
//!
//! The text is read line by line, lazily, under named limits ([`MAX_SCRIPT_BYTES`],
//! [`MAX_LINE_BYTES`], [`MAX_LINES`] and [`MAX_TOKENS_PER_LINE`]); a script over a limit stops at the
//! line that exceeds it with `db.cmd.script_limit`. A leading byte order mark is ignored.
//!
//! Things a script needs that a user interface supplies itself:
//!
//! - the session's context units and the user coordinate system follow the drawing before each
//!   line, so a bare number is read in the drawing's unit and a typed point in its coordinate system;
//! - a point typed where a prompt asks for an object (a pick or a selection) picks the topmost object
//!   whose curves pass through it within the pick tolerance ([`PICK_TOLERANCE`], changed with the
//!   directive `PICKTOLERANCE`), as the pointer does. A selection also takes a window (`W`) or a
//!   crossing (`C`) given by two corners. A script names an object by a point on it, since it has no
//!   pointer and objects have no names;
//! - `UNDO`, `U` and `REDO`, with an optional count, step the document's history
//!   ([`arc_command::history`]), as the protocol's `history.undo` and `history.redo` do.
//!
//! A setting may be set beside a waiting prompt with an apostrophe only when its value cannot change
//! the meaning of an answer already given ([`crate::settings::TRANSPARENT_SETTINGS`]); any other
//! setting is refused with `db.cmd.setting_not_transparent`.
//!
//! Every command runs through [`arc_command::Invoker`], so the permission, the schema and the
//! predicate checks of a command apply to a script line as they do to any other surface.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use arc_base::math::Point2;
use arc_base::{Error, ObjectId, Result, TransactionId, Unit};
use arc_command::history::{self, Direction};
use arc_command::{
    Accept, CommandLine, Invocation, Invoker, LineStep, Outcome, PlaneTransform, Registry, Session,
    Submitted, Target,
};
use arc_doc::{Document, Object, Op, Snapshot};
use serde_json::{Value, json};

use crate::error::codes;
use crate::precision::ucs;
use crate::select;
use crate::select::candidates::{Candidates, Options};
use crate::settings::{self, Value as SettingValue};

/// A fixed seed for runs that must create the same identifiers every time: the tests and the
/// byte-identity checks. A run that names no seed takes one from the caller (the CLI draws it from the
/// operating system), so the identifiers of two new drawings differ. Identifiers depend on the seed and
/// on the revision a command runs on, so the same script with the same seed gives the same identifiers.
pub const SEED: u64 = 1;

/// How far, in drawing units, a picked point may lie from an object and still pick it, when the
/// script sets no other tolerance. The picks of a script are exact points on the objects they name;
/// the tolerance absorbs the rounding of a typed decimal and of the tessellated display curves.
pub const PICK_TOLERANCE: f64 = 1e-3;

/// The largest pick tolerance a `PICKTOLERANCE` directive accepts, in drawing units.
pub const MAX_PICK_TOLERANCE: f64 = 1e6;

/// The largest script, in bytes (16 MiB).
pub const MAX_SCRIPT_BYTES: usize = 16 * 1024 * 1024;

/// The longest line, in bytes (64 KiB).
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// The most lines a script has, comments and blank lines included.
pub const MAX_LINES: usize = 1_000_000;

/// The most whitespace-separated tokens a line has (the command line's own bound on arguments).
pub const MAX_TOKENS_PER_LINE: usize = 1_024;

/// The command identifiers whose results carry a preservation report of a file.
const REPORTED: &[&str] = &[
    "draftbuilder.drawing.open",
    "draftbuilder.drawing.open_dxf",
    "draftbuilder.drawing.save",
    "draftbuilder.drawing.save_dxf",
];

/// One line of a script, numbered from 1 as an editor shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLine {
    /// The line number in the script.
    pub number: usize,
    /// The typed text, without surrounding whitespace. Empty for Enter.
    pub text: String,
}

/// The typed lines of a script, read lazily: the comments are left out, the numbers keep them.
pub fn lines(text: &str) -> impl Iterator<Item = ScriptLine> + '_ {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with(';'))
        .map(|(index, line)| ScriptLine {
            number: index + 1,
            text: line.trim().to_owned(),
        })
}

/// A command that completed in a script, with what it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRun {
    /// The line the command completed on.
    pub line: usize,
    /// The command identifier, or `history.undo` and `history.redo`.
    pub command: String,
    /// Objects the line created.
    pub created: usize,
    /// Objects the line changed (their stored record was rewritten).
    pub changed: usize,
    /// Objects the line deleted.
    pub deleted: usize,
}

/// The preservation report of a command that opened or saved a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Preservation {
    /// The line of the command.
    pub line: usize,
    /// The command identifier.
    pub command: String,
    /// The report as the command returned it.
    pub report: Value,
}

/// The line that stopped a script, with the error it raised.
#[derive(Debug)]
pub struct Failure {
    /// The line number (0 for a script that is refused before its first line).
    pub line: usize,
    /// The typed text of the line.
    pub text: String,
    /// The command the line was answering, when one was waiting.
    pub command: Option<String>,
    /// The error, with the details `line` and `text` added.
    pub error: Error,
}

/// What a script did: its commands in order, the preservation reports, and the failure that stopped
/// it, if any.
#[derive(Debug, Default)]
pub struct Report {
    /// The number of lines run, comments excluded.
    pub lines: usize,
    /// The commands that completed, in order.
    pub commands: Vec<CommandRun>,
    /// The preservation reports of the opens and saves.
    pub reports: Vec<Preservation>,
    /// The failure that stopped the script.
    pub failure: Option<Failure>,
    /// The objects that a window or a crossing could not test, because their display curves could
    /// not be built. They are not in the selection, and the count is kept rather than hidden.
    pub skipped: u64,
}

impl Report {
    /// The report as JSON: the lines, the commands with their counts, the totals, the preservation
    /// reports, the skipped objects and the failure.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let created: usize = self.commands.iter().map(|c| c.created).sum();
        let changed: usize = self.commands.iter().map(|c| c.changed).sum();
        let deleted: usize = self.commands.iter().map(|c| c.deleted).sum();
        json!({
            "lines": self.lines,
            "commands": self.commands.iter().map(|c| json!({
                "line": c.line,
                "command": c.command,
                "created": c.created,
                "changed": c.changed,
                "deleted": c.deleted,
            })).collect::<Vec<_>>(),
            "totals": {
                "commands": self.commands.len(),
                "created": created,
                "changed": changed,
                "deleted": deleted,
            },
            "reports": self.reports.iter().map(|r| json!({
                "line": r.line,
                "command": r.command,
                "report": r.report,
            })).collect::<Vec<_>>(),
            "skipped": self.skipped,
            "failure": self.failure.as_ref().map(|f| json!({
                "line": f.line,
                "text": f.text,
                "command": f.command,
                "code": f.error.code(),
            })),
        })
    }
}

/// The error of a script limit: the name of the limit and its value.
fn limit(name: &str, max: usize) -> Error {
    Error::new(codes::SCRIPT_LIMIT)
        .with_detail("limit", name)
        .with_detail("max", max.to_string())
}

/// The context units a drawing gives the session: its length unit (meters for an unitless drawing)
/// and degrees for angles, the unit a bare angle is read in.
#[must_use]
pub fn session_units(snapshot: &Snapshot) -> Vec<Unit> {
    let length = crate::model_space(snapshot)
        .and_then(|space| snapshot.resolve(space).ok())
        .and_then(|handle| snapshot.get(handle).ok())
        .and_then(|object| {
            arc_draft_dxf::fields::Reader::new(
                snapshot.registry(),
                arc_draft::types::SPACE,
                &object.values,
            )
            .enum_code("unit")
        })
        .and_then(arc_draft::types::SpaceUnit::of)
        .and_then(arc_draft::types::SpaceUnit::unit)
        .copied()
        .or_else(|| Unit::by_name("meter").ok().copied());
    let mut units: Vec<Unit> = length.into_iter().collect();
    if let Ok(degree) = Unit::by_name("degree") {
        units.push(*degree);
    }
    units
}

/// Whether the command echo setting is on: each typed line is written to the echo stream.
fn echo_on(session: &Session) -> bool {
    settings::find("command.echo")
        .map(|setting| settings::store::session_current(&|key| session.setting(key), setting))
        .is_some_and(|value| matches!(value, SettingValue::Bool(true)))
}

/// A point typed as two plain numbers, `x,y`, in drawing units.
fn plain_point(text: &str) -> Option<[f64; 2]> {
    let (x, y) = text.split_once(',')?;
    let x: f64 = x.trim().parse().ok()?;
    let y: f64 = y.trim().parse().ok()?;
    (x.is_finite() && y.is_finite()).then_some([x, y])
}

/// The candidates of the model space, gathered once per document revision: the picks of a script
/// in one revision share one index.
#[derive(Default)]
struct PickCache {
    revision: Option<u64>,
    space: Option<ObjectId>,
    candidates: Option<Candidates>,
}

impl PickCache {
    /// The candidates of the model space of `snapshot`, gathered again only when the revision or
    /// the space changed.
    fn candidates(&mut self, snapshot: &Snapshot) -> Result<&Candidates> {
        let space =
            crate::model_space(snapshot).ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))?;
        let revision = snapshot.revision();
        if self.candidates.is_none() || self.revision != Some(revision) || self.space != Some(space)
        {
            self.candidates = Some(select::candidates::gather(
                snapshot,
                space,
                Options::default(),
            )?);
            self.revision = Some(revision);
            self.space = Some(space);
        }
        self.candidates
            .as_ref()
            .ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))
    }
}

/// The topmost object whose curves pass through `point` (model coordinates) within `tolerance`, and
/// the number of objects that could not be tested.
///
/// # Errors
///
/// `db.cmd.pick_none` when no object lies at the point, or when the box around the point is too
/// small to exist at the point's scale; `db.cmd.no_model_space` for a document without one.
fn pick_point(
    cache: &mut PickCache,
    snapshot: &Snapshot,
    point: [f64; 2],
    tolerance: f64,
) -> Result<(ObjectId, u64)> {
    let [x, y] = point;
    let none = || {
        Error::new(codes::PICK_NONE)
            .with_detail("x", x.to_string())
            .with_detail("y", y.to_string())
    };
    let corners = [
        Point2::new(x - tolerance, y - tolerance),
        Point2::new(x + tolerance, y + tolerance),
    ];
    // At large coordinates a tolerance below the spacing of the numbers leaves no box to pick in.
    let box_exists = corners[0].x < corners[1].x && corners[0].y < corners[1].y;
    if !box_exists || !corners.iter().all(|c| c.x.is_finite() && c.y.is_finite()) {
        return Err(none());
    }
    let candidates = cache.candidates(snapshot)?;
    let pick = select::methods::pick_among(candidates, corners, true)?;
    let id = pick.ids.last().copied().ok_or_else(none)?;
    Ok((id, pick.skipped))
}

/// The typed text of a line that answers a pick or a selection prompt with points: each point
/// becomes the identifier of the object it picks, and a selection may also take a window (`W`) or a
/// crossing (`C`) given by its two corners. `None` when the line is not such an answer.
///
/// Returns the text and the objects that the windows and crossings could not test.
fn picked_text(
    line: &CommandLine,
    cache: &mut PickCache,
    tolerance: f64,
    snapshot: &Snapshot,
    transform: &PlaneTransform,
    text: &str,
) -> Result<Option<(String, u64)>> {
    let Some(prompt) = line.active().and_then(|session| session.prompt()) else {
        return Ok(None);
    };
    let selects = prompt
        .accepts
        .iter()
        .any(|accept| matches!(accept, Accept::Selection { .. }));
    let picks = selects
        || prompt
            .accepts
            .iter()
            .any(|accept| matches!(accept, Accept::Pick { .. }));
    if !picks {
        return Ok(None);
    }
    let model = |text: &str| -> Option<[f64; 2]> {
        let [x, y] = plain_point(text)?;
        let (mx, my) = transform.apply(x, y);
        Some([mx, my])
    };
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut ids: Vec<ObjectId> = Vec::new();
    let mut seen: BTreeSet<ObjectId> = BTreeSet::new();
    let mut skipped = 0_u64;
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        let window = ["W", "C"]
            .iter()
            .find(|word| token.eq_ignore_ascii_case(word))
            .copied();
        if let Some(word) = window {
            let (Some(first), Some(second)) = (
                tokens.get(index + 1).and_then(|t| model(t)),
                tokens.get(index + 2).and_then(|t| model(t)),
            ) else {
                return Ok(None);
            };
            if !selects {
                return Ok(None);
            }
            let corners = [
                Point2::new(first[0], first[1]),
                Point2::new(second[0], second[1]),
            ];
            let candidates = cache.candidates(snapshot)?;
            let picked = select::methods::pick_among(candidates, corners, word == "C")?;
            skipped += picked.skipped;
            for id in picked.ids {
                if seen.insert(id) {
                    ids.push(id);
                }
            }
            index += 3;
        } else {
            let Some(point) = model(token) else {
                return Ok(None);
            };
            let (id, missed) = pick_point(cache, snapshot, point, tolerance)?;
            skipped += missed;
            if seen.insert(id) {
                ids.push(id);
            }
            index += 1;
        }
    }
    if ids.is_empty() || (!selects && ids.len() > 1) {
        return Ok(None);
    }
    let text = ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Some((text, skipped)))
}

/// A directive of a script: a history step or a pick tolerance. A directive is a line of its own,
/// read while no command waits.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Directive {
    /// `UNDO` (or `U`) and `REDO` with a count of steps.
    History(Direction, usize),
    /// `PICKTOLERANCE` with the tolerance in drawing units.
    PickTolerance(f64),
}

/// The directive of a line, if the line is one.
fn directive(text: &str) -> Option<Result<Directive>> {
    let mut words = text.split_whitespace();
    let word = words.next()?.to_ascii_uppercase();
    let refused = || Error::new(codes::SCRIPT_DIRECTIVE);
    match word.as_str() {
        "U" | "UNDO" | "REDO" => {
            let direction = if word == "REDO" {
                Direction::Redo
            } else {
                Direction::Undo
            };
            let count = match words.next() {
                None => 1,
                Some(text) => match text.parse::<usize>() {
                    Ok(count) if count > 0 => count,
                    _ => return Some(Err(refused())),
                },
            };
            words
                .next()
                .is_none()
                .then_some(Ok(Directive::History(direction, count)))
                .or(Some(Err(refused())))
        }
        "PICKTOLERANCE" => {
            let value = words.next().and_then(|text| text.parse::<f64>().ok());
            let valid =
                |value: f64| value.is_finite() && value > 0.0 && value <= MAX_PICK_TOLERANCE;
            match (value, words.next()) {
                (Some(value), None) if valid(value) => Some(Ok(Directive::PickTolerance(value))),
                _ => Some(Err(refused())),
            }
        }
        _ => None,
    }
}

/// The net change of one transaction's operations: the objects it created, changed and deleted. An
/// object created and deleted in the transaction is no change; one deleted and created again is
/// changed.
fn counts_of(ops: &[Op]) -> (usize, usize, usize) {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Created,
        Changed,
        Deleted,
    }
    let mut states: BTreeMap<ObjectId, State> = BTreeMap::new();
    for op in ops {
        let Some(id) = op.object_id() else {
            continue;
        };
        let previous = states.get(&id).copied();
        let next = match (op, previous) {
            (Op::Create { .. }, Some(State::Deleted)) => Some(State::Changed),
            (Op::Create { .. }, _) => Some(State::Created),
            (Op::Delete { .. }, Some(State::Created)) => None,
            (Op::Delete { .. }, _) => Some(State::Deleted),
            (_, Some(State::Deleted)) => Some(State::Deleted),
            (_, Some(State::Created)) => Some(State::Created),
            (_, _) => Some(State::Changed),
        };
        match next {
            Some(state) => {
                states.insert(id, state);
            }
            None => {
                states.remove(&id);
            }
        }
    }
    let count = |wanted: State| states.values().filter(|s| **s == wanted).count();
    (
        count(State::Created),
        count(State::Changed),
        count(State::Deleted),
    )
}

/// The objects of a snapshot by identifier, for the counts of a history step, which has no
/// transaction of its own to read.
type Objects = BTreeMap<ObjectId, std::sync::Arc<Object>>;

fn objects(snapshot: &Snapshot) -> Objects {
    snapshot
        .iter()
        .map(|(_, object)| (object.id, std::sync::Arc::clone(object)))
        .collect()
}

/// The objects created, changed and deleted between two snapshots, for a history step.
fn diff(before: &Snapshot, after: &Snapshot) -> (usize, usize, usize) {
    if before.revision() == after.revision() {
        return (0, 0, 0);
    }
    let (old, new) = (objects(before), objects(after));
    let created = new.keys().filter(|id| !old.contains_key(id)).count();
    let deleted = old.keys().filter(|id| !new.contains_key(id)).count();
    let changed = new
        .iter()
        .filter(|(id, object)| {
            old.get(id)
                .is_some_and(|prior| !std::sync::Arc::ptr_eq(prior, object))
        })
        .count();
    (created, changed, deleted)
}

/// The setting a transparent `SETVAR` or `SET` names, when that setting may not change while a
/// prompt waits: a refusal error. `None` for any other line.
fn refused_setting(registry: &Registry, text: &str) -> Option<Error> {
    let rest = text
        .trim_start()
        .strip_prefix(arc_command::TRANSPARENT_PREFIX)?;
    if rest.starts_with(arc_command::TRANSPARENT_PREFIX) {
        return None;
    }
    let mut words = rest.split_whitespace();
    let command = words.next()?;
    let resolved = registry.resolve(command, &BTreeMap::new()).ok()?;
    if !matches!(
        resolved.spec.id(),
        "draftbuilder.settings.setvar" | "draftbuilder.settings.set"
    ) {
        return None;
    }
    let words: Vec<&str> = words.collect();
    let named = words.iter().find_map(|w| w.strip_prefix("name="));
    let positional = words.iter().find(|w| !w.contains('='));
    let name = named.or(positional.copied())?.trim_matches('"');
    let setting = settings::find(name)?;
    (!settings::TRANSPARENT_SETTINGS.contains(&setting.name))
        .then(|| Error::new(codes::SETTING_NOT_TRANSPARENT).with_detail("name", setting.name))
}

/// A command that ran on a line.
struct Ran {
    command: String,
    value: Value,
    txn: Option<TransactionId>,
}

/// The state of one script run: the command line, the pick cache and the pick tolerance.
struct Engine<'a> {
    document: &'a Document,
    registry: &'a Registry,
    line: CommandLine,
    cache: PickCache,
    tolerance: f64,
    skipped: u64,
    seed: u64,
}

impl Engine<'_> {
    /// Submits one typed line: a directive, or a line to the command line. `Some` when a command
    /// ran on it.
    fn submit(&mut self, session: &Session, text: &str) -> Result<Option<Ran>> {
        let snapshot = self.document.snapshot();
        let transform = ucs::plane_transform(&snapshot)?;
        self.line.set_transform(transform);
        if self.line.active().is_none() {
            match directive(text) {
                Some(Ok(Directive::History(direction, count))) => {
                    for _ in 0..count {
                        history::step(self.document, session, direction)?;
                    }
                    let command = match direction {
                        Direction::Undo => "history.undo",
                        Direction::Redo => "history.redo",
                    };
                    return Ok(Some(Ran {
                        command: command.to_owned(),
                        value: Value::Null,
                        txn: None,
                    }));
                }
                Some(Ok(Directive::PickTolerance(tolerance))) => {
                    self.tolerance = tolerance;
                    return Ok(None);
                }
                Some(Err(error)) => return Err(error),
                None => {}
            }
        } else if let Some(error) = refused_setting(self.registry, text) {
            return Err(error);
        }
        let typed = picked_text(
            &self.line,
            &mut self.cache,
            self.tolerance,
            &snapshot,
            &transform,
            text,
        )?;
        let text = match typed {
            Some((typed, skipped)) => {
                self.skipped += skipped;
                typed
            }
            None => text.to_owned(),
        };
        let selection = session.current_selection(&snapshot);
        let localized = BTreeMap::new();
        let registry = self.registry;
        let document = self.document;
        let target = Target {
            registry,
            document,
            session,
            selection: &selection,
            localized: &localized,
            seed: self.seed,
        };
        let mut runner = |invocation: Invocation| -> Result<Outcome> {
            Invoker::new(registry).invoke(document, session, invocation)
        };
        match self
            .line
            .drive(&target, LineStep::Submit(&text), &mut runner)?
        {
            Submitted::Ran(outcome) => Ok(Some(Ran {
                command: outcome.command,
                value: outcome.value,
                txn: outcome.txn_id,
            })),
            Submitted::Rejected { error, .. } => Err(error),
            Submitted::Cancelled => Err(Error::new(arc_command::error::codes::CANCELLED)),
            Submitted::Idle | Submitted::Prompt(_) => Ok(None),
        }
    }
}

/// Runs the script `text` on `document` with the session `session`, writing the echo of each typed
/// line to `echo` while the command echo setting is on. The script stops at its first failing line.
///
/// `seed` is the seed of every invocation of the script (see [`SEED`]). `session` is updated with the
/// context units of the drawing before each line. A script that
/// leaves a command waiting at its last line fails with `db.cmd.script_pending`. A script over one
/// of its limits fails at the line that exceeds it.
#[allow(
    clippy::too_many_lines,
    reason = "one loop over the lines; its checks run in the order the limits name"
)]
pub fn run(
    document: &Document,
    registry: &Registry,
    session: &mut Session,
    text: &str,
    seed: u64,
    echo: &mut dyn Write,
) -> Report {
    let mut report = Report::default();
    if text.len() > MAX_SCRIPT_BYTES {
        report.failure = Some(failure_at(
            0,
            "",
            None,
            limit("script_bytes", MAX_SCRIPT_BYTES),
        ));
        return report;
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut engine = Engine {
        document,
        registry,
        line: CommandLine::new(),
        cache: PickCache::default(),
        tolerance: PICK_TOLERANCE,
        skipped: 0,
        seed,
    };
    let mut last = ScriptLine {
        number: 0,
        text: String::new(),
    };
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let typed = raw.trim();
        let stop = |error: Error, line: &ScriptLine| -> Report {
            Report {
                failure: Some(failure(line, waiting_command(&engine.line), error)),
                ..Report::default()
            }
        };
        let here = ScriptLine {
            number,
            text: typed.to_owned(),
        };
        if number > MAX_LINES {
            return stop(limit("line_count", MAX_LINES), &here);
        }
        if raw.len() > MAX_LINE_BYTES {
            return stop(limit("line_bytes", MAX_LINE_BYTES), &here);
        }
        if typed.starts_with(';') {
            continue;
        }
        if typed.split_whitespace().count() > MAX_TOKENS_PER_LINE {
            return stop(limit("tokens_per_line", MAX_TOKENS_PER_LINE), &here);
        }
        report.lines += 1;
        last = here.clone();
        let before = document.snapshot();
        if echo_on(session) {
            let record = json!({"level": "info", "event": "script.echo", "line": number,
                                "detail": typed});
            let _ = writeln!(echo, "{record}");
        }
        session.units = session_units(&before);
        let outcome = engine.submit(session, typed);
        match outcome {
            Ok(Some(ran)) => {
                let (created, changed, deleted) = match ran.txn {
                    Some(txn) => document.with_journal(|journal| {
                        journal
                            .get(txn)
                            .map_or((0, 0, 0), |entry| counts_of(&entry.record().ops))
                    }),
                    None if ran.command.starts_with("history.") => {
                        diff(&before, &document.snapshot())
                    }
                    None => (0, 0, 0),
                };
                let command = ran.command.clone();
                if REPORTED.contains(&command.as_str())
                    && let Some(found) = ran.value.get("report").filter(|r| !r.is_null())
                {
                    report.reports.push(Preservation {
                        line: number,
                        command: command.clone(),
                        report: found.clone(),
                    });
                }
                report.commands.push(CommandRun {
                    line: number,
                    command,
                    created,
                    changed,
                    deleted,
                });
            }
            Ok(None) => {}
            Err(error) => {
                report.failure = Some(failure(&here, waiting_command(&engine.line), error));
                report.skipped = engine.skipped;
                return report;
            }
        }
    }
    report.skipped = engine.skipped;
    if let Some(active) = engine.line.active() {
        let mut error =
            Error::new(codes::SCRIPT_PENDING).with_detail("command", active.command().id());
        if let Some(prompt) = active.prompt() {
            error = error.with_detail("prompt", prompt.key.clone());
        }
        report.failure = Some(failure(&last, waiting_command(&engine.line), error));
    }
    report
}

/// The failure of a line, with the details `line` and `text` added to its error.
fn failure(line: &ScriptLine, command: Option<String>, error: Error) -> Failure {
    failure_at(line.number, &line.text, command, error)
}

fn failure_at(number: usize, text: &str, command: Option<String>, error: Error) -> Failure {
    let error = error
        .with_detail("line", number.to_string())
        .with_detail("text", text.to_owned());
    Failure {
        line: number,
        text: text.to_owned(),
        command,
        error,
    }
}

/// The identifier of the command waiting at a prompt, when one waits.
fn waiting_command(line: &CommandLine) -> Option<String> {
    line.active().map(|active| active.command().id().to_owned())
}
