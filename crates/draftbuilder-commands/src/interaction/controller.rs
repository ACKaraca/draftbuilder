//! The controller: owns the command line of a document and the state of the pointer, selection
//! and dynamic input, and turns each [`Event`] into the next [`Outputs`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arc_base::{Error, ObjectId, Result, Tolerance};
use arc_command::history::{self, Direction};
use arc_command::prompt::PromptSession;
use arc_command::{
    Accept, CommandLine, Feedback, Permission, Point, PromptInput, Session, Submitted, Surface,
    Target,
};
use arc_doc::{Document, Snapshot};
use serde_json::Map;

use super::acquire::{self, Acquire, Taken};
use super::band;
use super::fields::Fields;
use super::gather::{self, Gather, Region, Shape};
use super::lookup::{self, CommandEntry, Help, Suggestion};
use super::resolution::{Resolved, Resolver};
use super::{
    BandMode, BandOut, CursorOut, ErrorOut, Event, Expect, HistoryLine, LineRole, MAX_HISTORY,
    ModifierOut, ObjectPicker, OptionOut, Outputs, PromptOut, SnapOut,
};
use crate::draw::create::from_meters;
use crate::draw::create::meters_per_unit;
use crate::error::codes;
use crate::precision::points::{self, Modifier};
use crate::select::candidates::Options;
use crate::select::{Mode, combine, methods};
use crate::settings;
use crate::settings::store::Values;

/// The command whose point prompt reads typed points for a waiting modifier: a single point, with
/// no other effect.
const PROBE_COMMAND: &str = "draftbuilder.inquiry.id";

/// What a click on a prompt can answer, from the accepted inputs of the prompt.
#[derive(Debug, Clone, Copy, Default)]
struct Wants {
    point: bool,
    pick: bool,
    selection: bool,
}

impl Wants {
    fn of(accepts: &[Accept]) -> Self {
        let mut wants = Self::default();
        for accept in accepts {
            match accept {
                Accept::Point { .. } => wants.point = true,
                Accept::Pick { .. } => wants.pick = true,
                Accept::Selection { .. } => wants.selection = true,
                _ => {}
            }
        }
        wants
    }
}

/// The state of the controller: the command line and its prompt, the cursor, the pending window,
/// the dynamic input and the history. The snap index is kept in the resolver.
///
/// A controller serves one document: the caller passes that document to every event, and the
/// snap index is keyed by its revision.
pub struct Controller {
    registry: arc_command::Registry,
    session: Session,
    line: CommandLine,
    resolver: Resolver,
    fields: Fields,
    /// The start of a pending window, in meters.
    window: Option<[f64; 2]>,
    /// The last resolved cursor and the raw pointer position it came from.
    cursor: Option<(Resolved, [f64; 2])>,
    history: Vec<HistoryLine>,
    /// How many input lines back from the newest the last recalled line is; `None` when none is recalled.
    recall: Option<usize>,
    /// The text the last recall event recalled, taken by the next outputs.
    recalled: Option<String>,
    /// The point modifier that waits for points.
    acquire: Acquire,
    /// The objects a selection prompt gathers and the shape of its next gesture.
    gather: Gather,
    /// The identifier of the command the last step ran on, for the history line of one that failed.
    running: Option<String>,
    seed: u64,
}

impl Controller {
    /// A controller with the product's commands and a command-line session.
    ///
    /// # Errors
    ///
    /// The errors of [`crate::registry`].
    pub fn new() -> Result<Self> {
        let session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit])
            .with_last_hook(Some(Arc::new(crate::last_objects)));
        Ok(Self {
            registry: crate::registry()?,
            session,
            line: CommandLine::new(),
            resolver: Resolver::default(),
            fields: Fields::default(),
            window: None,
            cursor: None,
            history: Vec::new(),
            recall: None,
            recalled: None,
            acquire: Acquire::Idle,
            gather: Gather::default(),
            running: None,
            seed: 1,
        })
    }

    /// The number of snap indexes built so far (the revision of the document is the only reason
    /// to build one). A test hook: it lets a host check how often the index is rebuilt.
    #[must_use]
    pub fn snap_index_builds(&self) -> u64 {
        self.resolver.builds()
    }

    /// The active command's prompt, when a command waits.
    #[must_use]
    pub fn active_prompt(&self) -> Option<&arc_command::prompt::Prompt> {
        self.line.active().and_then(|s| s.prompt())
    }

    /// The answers the active command has collected so far (zero without a command).
    #[must_use]
    pub fn answers(&self) -> usize {
        self.line.active().map_or(0, |s| s.answers().len())
    }

    /// Whether an empty line can repeat a completed command.
    #[must_use]
    pub fn repeatable(&self) -> bool {
        self.line.last().is_some()
    }

    /// What the window shows for `document` now, without an event: used after the host changed
    /// the document itself (a redo, for instance).
    pub fn refresh(&mut self, document: &Document) -> Outputs {
        self.outputs(document, Vec::new())
    }

    /// The permissions the controller's commands and history steps run with. The host sets them
    /// before each event from the session of the tab; a command or a step that needs `Edit` is
    /// refused without it.
    pub fn set_permissions(&mut self, permissions: &BTreeSet<Permission>) {
        self.session.permissions.clone_from(permissions);
    }

    /// Whether the controller holds state a clear (Escape without a command) removes: a pending
    /// window or a locked dynamic field.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.window.is_some()
            || self.fields.any_locked()
            || self.acquire.pending()
            || self.gather.pending()
    }

    /// The window's state for `document` after the document changed outside the controller (a
    /// menu command, a file operation): an active command is cancelled, the window and the locks
    /// are dropped, and what the window shows is returned.
    pub fn external_change(&mut self, document: &Document) -> Outputs {
        let _ = self.line.cancel();
        self.window = None;
        self.command_ended();
        self.outputs(document, Vec::new())
    }

    /// Applies one event to `document` and returns what the window shows afterwards.
    pub fn handle(
        &mut self,
        document: &Document,
        event: Event,
        picker: &dyn ObjectPicker,
    ) -> Outputs {
        let mut errors = Vec::new();
        match event {
            Event::Submit(text) => self.submit(document, &text, &mut errors),
            Event::Escape => self.escape(document, &mut errors),
            Event::Move { at, aperture } => {
                self.move_to(document, at, aperture, &mut errors);
            }
            Event::Click { at, aperture } => {
                self.click(document, at, aperture, picker, &mut errors);
            }
            Event::Option(id) => self.option(document, id, &mut errors),
            Event::Field { index, text } => self.fields.set(index, text),
            Event::Tab => self.fields.toggle(),
            Event::ShiftClick { at, aperture } => {
                self.shift_click(document, at, aperture, picker, &mut errors);
            }
            Event::Lasso { path } => self.lasso(document, &path, &mut errors),
            Event::Undo => self.undo(document, &mut errors),
            Event::Redo => self.redo(document, &mut errors),
            Event::RecallPrevious => self.recall_previous(),
            Event::RecallNext => self.recall_next(),
            Event::ClearHistory => {
                self.history.clear();
                self.recall = None;
            }
        }
        self.outputs(document, errors)
    }

    fn next_seed(&mut self) -> u64 {
        self.seed += 1;
        self.seed
    }

    /// Runs `step` on the command line with the document's snapshot as the target.
    fn with_line<T>(
        &mut self,
        document: &Document,
        step: impl FnOnce(&mut CommandLine, &Target<'_>) -> Result<T>,
    ) -> Result<T> {
        self.running = self.line.active().map(|s| s.command().id().to_owned());
        let snapshot = document.snapshot();
        // Typed numbers take the units of the drawing, as in the script runner; typed points read in
        // the current user coordinate system.
        self.session.units = crate::script::session_units(&snapshot);
        self.line
            .set_transform(crate::precision::ucs::plane_transform(&snapshot)?);
        let selection = self.session.current_selection(&snapshot);
        let localized = BTreeMap::new();
        let seed = self.next_seed();
        let target = Target {
            registry: &self.registry,
            document,
            session: &self.session,
            selection: &selection,
            localized: &localized,
            seed,
        };
        step(&mut self.line, &target)
    }

    fn submit(&mut self, document: &Document, text: &str, errors: &mut Vec<ErrorOut>) {
        self.recall = None;
        if text.trim().is_empty() {
            self.empty_line(document, errors);
            return;
        }
        self.history.push(HistoryLine {
            role: LineRole::Input,
            text: text.to_owned(),
        });
        if let Some(id) = self.selection_keyword(text) {
            self.select_keyword(document, id, errors);
            return;
        }
        if let Some(modifier) = self.typed_modifier(text) {
            self.start_modifier(modifier);
            return;
        }
        if self.acquire.pending() {
            match self.typed_point(document, text) {
                Ok(point) => self.give_point(document, point, errors),
                Err(error) => errors.push(error_out(&error)),
            }
            return;
        }
        let result = self.with_line(document, |line, target| line.submit(target, text));
        self.settle(document, result, errors);
    }

    /// The empty line: a locked field composes the point, else the prompt takes its default; with
    /// no command, the last command starts again at its first prompt.
    fn empty_line(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if self.line.active().is_some() {
            if self.fields.any_locked() {
                if let Some(point) = self.compose(document, errors) {
                    self.give_point(document, point, errors);
                }
                return;
            }
            if self.acquire.pending() {
                errors.push(error_out(&Error::new(
                    crate::precision::codes::POINT_PENDING,
                )));
                return;
            }
            if let Shape::Polygon { region, points } = &self.gather.shape {
                let (region, points) = (*region, points.clone());
                self.close_polygon(document, region, &points, errors);
                return;
            }
            if self.gather.multiple {
                let ids = self.gather.ids.clone();
                self.answer(document, PromptInput::Selection(ids), errors);
                return;
            }
            let result = self.with_line(document, |line, target| line.submit(target, ""));
            self.settle(document, result, errors);
            return;
        }
        let Some(command) = self.line.last().map(|last| last.command.clone()) else {
            return;
        };
        let result = self.with_line(document, |line, target| {
            line.start(target, &command, Map::new())
        });
        self.settle(document, result, errors);
    }

    /// The point of the locked fields over the cursor, in meters; `None` without a cursor.
    fn compose(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) -> Option<[f64; 2]> {
        let (resolved, _) = self.cursor.clone()?;
        let snapshot = document.snapshot();
        let base = self.point_base();
        let composed = self.fields.compose(&snapshot, base, resolved.point);
        push_result(errors, composed)
    }

    /// The base point of the point prompt: the base of a `FROM` offset while it waits, else the
    /// command's last point.
    /// While a modifier waits, the base is the FROM base at its offset step and none otherwise: the
    /// command's last point does not measure a modifier's points.
    fn point_base(&self) -> Option<[f64; 2]> {
        if self.acquire.pending() {
            return self.acquire.base();
        }
        self.line.base_point().map(|p| [p.x, p.y])
    }

    /// The modifier a typed line names while a point prompt waits and the prompt does not offer
    /// the same keyword itself.
    fn typed_modifier(&self, text: &str) -> Option<Modifier> {
        if !self.point_prompt() || self.prompt_claims(text) {
            return None;
        }
        points::modifier_of(text)
    }

    /// Whether the active prompt accepts a point.
    fn point_prompt(&self) -> bool {
        self.line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| Wants::of(&p.accepts).point)
    }

    /// Whether the active prompt's keyword options claim `text` (the command's word wins).
    fn prompt_claims(&self, text: &str) -> bool {
        self.line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| acquire::claimed_by(p, text))
    }

    fn start_modifier(&mut self, modifier: Modifier) {
        self.acquire = Acquire::start(modifier);
        self.fields.clear();
    }

    /// Ends a waiting modifier, if one waits: its origin, the cursor it aligned and the pointer
    /// direction it measured from are dropped at once.
    fn drop_modifier(&mut self) {
        if self.acquire.pending() {
            self.acquire = Acquire::Idle;
            self.end_state();
        }
    }

    /// The state a finished modifier leaves: no locks, no temporary origin, no cursor and no
    /// pointer direction (the next move sets the direction from the command's last point).
    fn end_state(&mut self) {
        self.fields.clear();
        self.resolver.set_only(None);
        self.cursor = None;
        let _ = self.line.set_pointer_direction(None);
    }

    /// Reads typed text as one point while a modifier waits, with the foundation's point reader:
    /// the prompt's coordinate forms, the current user coordinate system and the defaults. The last
    /// point is the command's, except at a FROM offset, where unprefixed coordinates are relative to
    /// the base. Returns the point in meters.
    fn typed_point(&self, document: &Document, text: &str) -> Result<[f64; 2]> {
        let snapshot = document.snapshot();
        let mut entry = self.line.point_entry().clone();
        let last = match self.acquire.base() {
            Some(base) => {
                entry.set_relative_default(true);
                Some(base)
            }
            None => self.line.base_point().map(|p| [p.x, p.y]),
        };
        entry.set_last_point(last.map(point_of))?;
        let spec = self
            .registry
            .get(PROBE_COMMAND)
            .cloned()
            .ok_or_else(|| Error::new(crate::error::codes::VALUE_INVALID))?;
        let (mut probe, _) =
            PromptSession::start_with_entry(spec, Map::new(), &self.session, &snapshot, entry);
        match probe.answer_text(text, &snapshot) {
            Feedback::Done(invocation) => {
                let point = invocation
                    .params
                    .get("point")
                    .and_then(|v| serde_json::from_value::<[f64; 2]>(v.clone()).ok())
                    .ok_or_else(|| Error::new(arc_command::error::codes::PROMPT_UNREADABLE))?;
                let mpu = meters_per_unit(&snapshot);
                Ok([point[0] * mpu, point[1] * mpu])
            }
            Feedback::Rejected { error, .. } => Err(error),
            _ => Err(Error::new(arc_command::error::codes::PROMPT_UNREADABLE)),
        }
    }

    /// Gives `point` (meters) to the point modifier that waits, or answers the active prompt with
    /// it when none waits.
    fn give_point(&mut self, document: &Document, point: [f64; 2], errors: &mut Vec<ErrorOut>) {
        let waiting = self.acquire.pending();
        match self.acquire.take(point) {
            Taken::Kept => self.fields.clear(),
            Taken::Answer(answer) => {
                if waiting {
                    self.end_state();
                }
                self.answer(document, PromptInput::Point(point_of(answer)), errors);
            }
        }
    }

    fn answer(&mut self, document: &Document, input: PromptInput, errors: &mut Vec<ErrorOut>) {
        let result = self.with_line(document, |line, target| line.answer(target, input));
        self.settle(document, result, errors);
    }

    /// Takes the outcome of a command-line step: a run or a cancel ends the command's state; a
    /// rejection is an error and keeps the prompt.
    fn settle(
        &mut self,
        document: &Document,
        result: Result<Submitted>,
        errors: &mut Vec<ErrorOut>,
    ) {
        match push_result(errors, result) {
            Some(Submitted::Ran(outcome)) => {
                self.history.push(HistoryLine {
                    role: LineRole::Command,
                    text: outcome.command,
                });
                self.command_ended();
            }
            Some(Submitted::Cancelled) => self.command_ended(),
            Some(Submitted::Rejected { error, .. }) => {
                errors.push(error_out(&error));
                self.drop_modifier();
            }
            Some(Submitted::Prompt(_) | Submitted::Idle) => {
                self.fields.clear();
                self.drop_modifier();
                self.gather = Gather::default();
                self.preselect(document, errors);
            }
            None => {
                // A run that failed retired the command in the foundation: its state goes too.
                if self.line.active().is_none() {
                    if let Some(name) = self.running.take() {
                        self.history.push(HistoryLine {
                            role: LineRole::Cancelled,
                            text: name,
                        });
                    }
                    self.command_ended();
                }
            }
        }
    }

    /// Noun-verb (DB-SEL-009): the first selection prompt of a command takes the session's current
    /// selection, when the prompt takes entities and the command has no answer yet. Only the objects
    /// the selection can use are offered (visible, on thawed layers; locked ones are kept, so the
    /// command refuses them with its own error). A set the prompt refuses is reported once and the
    /// prompt waits for the user; the selection stays. An accepted set is consumed when the command
    /// runs or asks its next prompt: it becomes the previous selection, which `previous` brings back
    /// after a later cancel. A command that is cancelled or fails keeps the selection.
    /// Trim and extend take their edges this way: a selection made before the command is its edges.
    fn preselect(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if !self.preselect_due() {
            return;
        }
        let snapshot = document.snapshot();
        let current = self.session.current_selection(&snapshot);
        let ids = usable(&snapshot, &current);
        if ids.is_empty() {
            return;
        }
        let result = self.with_line(document, |line, target| {
            line.answer(target, PromptInput::Selection(ids))
        });
        match result {
            Ok(Submitted::Rejected { error, .. }) => errors.push(error_out(&error)),
            Ok(outcome @ (Submitted::Ran(_) | Submitted::Prompt(_))) => {
                push_unit(errors, self.session.record_selection(&[], &snapshot));
                self.settle(document, Ok(outcome), errors);
            }
            other => self.settle(document, other, errors),
        }
    }

    /// Whether the active command waits at its first prompt, which takes entities (an entity
    /// selection, not an id list read as a list of identifiers of another kind).
    fn preselect_due(&self) -> bool {
        self.line.active().is_some_and(|session| {
            session.answers().is_empty()
                && session
                    .prompt()
                    .is_some_and(|prompt| takes_entities(&prompt.accepts))
        })
    }

    /// The state the command's end resets: the locks, the acquired points and the cursor's snap,
    /// which the document may have moved away from.
    fn command_ended(&mut self) {
        self.fields.clear();
        self.resolver.clear_acquired();
        self.acquire = Acquire::Idle;
        self.cursor = None;
        self.gather = Gather::default();
    }

    fn escape(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if self.fields.any_locked() {
            self.fields.clear();
            return;
        }
        if self.acquire.pending() {
            self.drop_modifier();
            return;
        }
        if self.window.take().is_some() {
            return;
        }
        if self.gather.pending() {
            // A selection prompt drops its gesture and its gathered objects, and keeps waiting.
            self.gather = Gather::default();
            return;
        }
        if let Some(name) = self.line.active().map(|s| s.command().id().to_owned()) {
            let cancelled = self.line.cancel();
            if matches!(cancelled, Submitted::Cancelled) {
                self.history.push(HistoryLine {
                    role: LineRole::Cancelled,
                    text: name,
                });
            }
            self.command_ended();
            return;
        }
        let snapshot = document.snapshot();
        if !self.session.current_selection(&snapshot).is_empty() {
            let cleared = self.session.record_selection(&[], &snapshot);
            push_unit(errors, cleared);
        }
    }

    fn option(&mut self, document: &Document, id: String, errors: &mut Vec<ErrorOut>) {
        if self.line.active().is_none() {
            return;
        }
        if let Some(keyword) = self.selection_keyword(&id) {
            self.select_keyword(document, keyword, errors);
            return;
        }
        let modifier = points::modifier_of(&id).filter(|_| self.point_prompt());
        if let Some(modifier) = modifier.filter(|_| !self.prompt_claims(&id)) {
            self.start_modifier(modifier);
            return;
        }
        // Another option answers the prompt: a waiting modifier ends first.
        self.drop_modifier();
        self.answer(document, PromptInput::Keyword(id), errors);
    }

    fn undo(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if self.acquire.pending() {
            self.drop_modifier();
            return;
        }
        if self.line.active().is_some() {
            let result = self.with_line(document, |line, target| line.back(target));
            self.settle(document, result, errors);
            return;
        }
        self.step_history(document, Direction::Undo, errors);
    }

    /// Redo steps the document history forward. A waiting command is cancelled first, since a
    /// redo has no prompt answer to step back from.
    fn redo(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if let Some(name) = self.line.active().map(|s| s.command().id().to_owned()) {
            let cancelled = self.line.cancel();
            if matches!(cancelled, Submitted::Cancelled) {
                self.history.push(HistoryLine {
                    role: LineRole::Cancelled,
                    text: name,
                });
            }
            self.command_ended();
        }
        self.step_history(document, Direction::Redo, errors);
    }

    fn step_history(
        &mut self,
        document: &Document,
        direction: Direction,
        errors: &mut Vec<ErrorOut>,
    ) {
        match history::step(document, &self.session, direction) {
            Ok(stepped) => {
                let role = match direction {
                    Direction::Undo => LineRole::Undo,
                    Direction::Redo => LineRole::Redo,
                };
                self.history.push(HistoryLine {
                    role,
                    text: stepped.command.unwrap_or_default(),
                });
                self.command_ended();
            }
            Err(error) => errors.push(error_out(&error)),
        }
    }

    /// Resolves the pointer and sets the pointer direction for direct distances.
    fn move_to(
        &mut self,
        document: &Document,
        at: [f64; 2],
        aperture: f64,
        errors: &mut Vec<ErrorOut>,
    ) -> Option<Resolved> {
        let snapshot = document.snapshot();
        let space = crate::model_space(&snapshot);
        let Some(space) = space else {
            errors.push(error_out(&Error::new(crate::error::codes::NO_MODEL_SPACE)));
            return None;
        };
        let values = Values::of(&snapshot, &|key: &str| self.session.setting(key));
        let base = self.point_base();
        self.resolver.set_only(self.acquire.origin());
        let resolved = self
            .resolver
            .resolve(&snapshot, space, &values, at, aperture, base);
        let resolved = push_result(errors, resolved)?;
        if let Some(base) = base {
            let direction = unit(base, resolved.point);
            let set = self.line.set_pointer_direction(direction);
            push_unit(errors, set);
        }
        self.cursor = Some((resolved.clone(), at));
        Some(resolved)
    }

    fn click(
        &mut self,
        document: &Document,
        at: [f64; 2],
        aperture: f64,
        picker: &dyn ObjectPicker,
        errors: &mut Vec<ErrorOut>,
    ) {
        let Some(resolved) = self.move_to(document, at, aperture, errors) else {
            return;
        };
        if self.window.is_some() {
            self.finish_window(document, at, errors);
            return;
        }
        if self.selection_prompt() {
            match self.gather.shape.clone() {
                Shape::Default => {}
                Shape::Box(_) => {
                    self.start_window(at);
                    return;
                }
                Shape::Polygon { .. } => {
                    self.add_vertex(resolved.point, errors);
                    return;
                }
            }
        }
        let snapshot = document.snapshot();
        let prompt_wants = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .map(|p| Wants::of(&p.accepts));
        if self.acquire.pending() && prompt_wants.is_some_and(|w| w.point) {
            let point = if self.fields.any_locked() {
                match self.compose(document, errors) {
                    Some(p) => p,
                    None => return,
                }
            } else {
                resolved.point
            };
            self.give_point(document, point, errors);
            return;
        }
        let object = match prompt_wants {
            Some(wants) if wants.pick || wants.selection => {
                pick_at(picker, &snapshot, at, aperture)
            }
            Some(_) => None,
            None => pick_at(picker, &snapshot, at, aperture),
        };
        match (prompt_wants, object) {
            (Some(wants), Some(id)) if wants.pick => {
                self.answer(document, PromptInput::Pick(id), errors);
            }
            (Some(wants), Some(id)) if wants.selection => {
                self.gesture(document, vec![id], self.gather.mode, errors);
            }
            (Some(wants), _) if wants.point => {
                let point = if self.fields.any_locked() {
                    let composed = self.compose(document, errors);
                    match composed {
                        Some(p) => p,
                        None => return,
                    }
                } else {
                    resolved.point
                };
                self.answer(document, PromptInput::Point(point_of(point)), errors);
            }
            (Some(wants), None) if wants.selection => self.start_window(at),
            (Some(_), _) => {}
            (None, Some(id)) => self.gesture(document, vec![id], self.gather.mode, errors),
            (None, None) => self.start_window(at),
        }
    }

    fn start_window(&mut self, at: [f64; 2]) {
        self.window = Some(at);
    }

    /// The second click of a window: the objects it selects, taken as a gesture.
    fn finish_window(&mut self, document: &Document, at: [f64; 2], errors: &mut Vec<ErrorOut>) {
        let Some(start) = self.window.take() else {
            return;
        };
        let snapshot = document.snapshot();
        let mode = match &self.gather.shape {
            Shape::Box(mode) => *mode,
            _ => band::mode(start, at),
        };
        let ids = push_result(errors, band::select(&snapshot, start, at, mode));
        let Some(ids) = ids else {
            return;
        };
        self.gesture(document, ids, self.gather.mode, errors);
    }

    /// Shift-click: the object under the pointer leaves the selection, or the objects a selection
    /// prompt gathers (which then takes the next gestures in multiple mode).
    fn shift_click(
        &mut self,
        document: &Document,
        at: [f64; 2],
        aperture: f64,
        picker: &dyn ObjectPicker,
        errors: &mut Vec<ErrorOut>,
    ) {
        if self.move_to(document, at, aperture, errors).is_none() {
            return;
        }
        if self.window.is_some() || self.gather.shape != Shape::Default {
            return;
        }
        let snapshot = document.snapshot();
        let Some(id) = pick_at(picker, &snapshot, at, aperture) else {
            return;
        };
        if self.selection_prompt() {
            self.gather.set_multiple(true, None);
            self.gesture(document, vec![id], Mode::Remove, errors);
        } else if self.line.active().is_none() {
            self.gesture(document, vec![id], Mode::Remove, errors);
        }
    }

    /// A lasso: the free-hand outline selects by window polygon when the drag ends right of where it
    /// starts, else by crossing polygon, as a gesture of the selection prompt or of no command.
    fn lasso(&mut self, document: &Document, path: &[[f64; 2]], errors: &mut Vec<ErrorOut>) {
        if self.line.active().is_some() && !self.selection_prompt() {
            return;
        }
        // A pending window or polygon keeps its corners and vertices: the lasso is refused instead.
        if self.window.is_some() || self.gather.shape != Shape::Default {
            errors.push(error_out(
                &Error::new(codes::SELECTION_INVALID).with_detail("reason", "gesture_pending"),
            ));
            return;
        }
        if path.len() > methods::MAX_VERTICES {
            errors.push(error_out(
                &Error::new(codes::LIMIT_EXCEEDED)
                    .with_detail("limit", methods::MAX_VERTICES.to_string()),
            ));
            return;
        }
        // Vertices within the linear tolerance of the one before make no edge (a resting pointer,
        // jitter), and a drag that returns to its start closes the outline: both are dropped.
        let linear = Tolerance::DEFAULT.linear;
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < linear;
        let mut outline: Vec<[f64; 2]> = Vec::with_capacity(path.len());
        for point in path {
            if outline.last().is_none_or(|last| !near(*last, *point)) {
                outline.push(*point);
            }
        }
        if outline.len() > 1
            && outline
                .first()
                .zip(outline.last())
                .is_some_and(|(a, b)| near(*a, *b))
        {
            outline.pop();
        }
        let (Some(first), Some(last)) = (outline.first(), outline.last()) else {
            return;
        };
        let region = if last[0] >= first[0] {
            Region::Window
        } else {
            Region::Crossing
        };
        let snapshot = document.snapshot();
        let ids = push_result(errors, gather::region_ids(&snapshot, region, &outline));
        if let Some(ids) = ids {
            self.gesture(document, ids, self.gather.mode, errors);
        }
    }

    /// A gesture of the pointer or a keyword: at a selection prompt it answers the prompt, or in
    /// multiple mode combines with the gathered objects; with no command it changes the selection.
    /// A prompt that takes no selection ignores it.
    fn gesture(
        &mut self,
        document: &Document,
        ids: Vec<ObjectId>,
        mode: Mode,
        errors: &mut Vec<ErrorOut>,
    ) {
        if self.selection_prompt() {
            self.gather.shape = Shape::Default;
            if self.gather.multiple {
                push_unit(errors, self.gather.apply(&ids, mode));
            } else {
                self.answer(document, PromptInput::Selection(ids), errors);
            }
            return;
        }
        if self.line.active().is_some() {
            return;
        }
        let snapshot = document.snapshot();
        let current = self.session.current_selection(&snapshot);
        let next = combine(&current, &ids, mode);
        // A gesture that changes nothing keeps the previous selection as it was.
        if next != current {
            let set = self.session.record_selection(&next, &snapshot);
            push_unit(errors, set);
        }
    }

    /// Whether the active prompt takes a selection.
    fn selection_prompt(&self) -> bool {
        self.line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| Wants::of(&p.accepts).selection)
    }

    /// The keyword of a selection prompt that `text` names, when the prompt's own keywords do not
    /// claim the word.
    fn selection_keyword(&self, text: &str) -> Option<&'static str> {
        if !self.selection_prompt() {
            return None;
        }
        if self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| acquire::claimed_by(p, text))
        {
            return None;
        }
        gather::keyword(text).or_else(|| {
            text.trim()
                .eq_ignore_ascii_case("selection")
                .then_some("selection")
        })
    }

    /// Applies a keyword of a selection prompt. Shape keywords set the shape of the next gesture;
    /// the others are gestures (`all`, `last`, `previous`) or change the gathering.
    fn select_keyword(&mut self, document: &Document, id: &str, errors: &mut Vec<ErrorOut>) {
        let shape = match id {
            "window" => Some(Shape::Box(BandMode::Window)),
            "crossing" => Some(Shape::Box(BandMode::Crossing)),
            "wpolygon" => Some(Shape::Polygon {
                region: Region::Window,
                points: Vec::new(),
            }),
            "cpolygon" => Some(Shape::Polygon {
                region: Region::Crossing,
                points: Vec::new(),
            }),
            "fence" => Some(Shape::Polygon {
                region: Region::Fence,
                points: Vec::new(),
            }),
            _ => None,
        };
        if let Some(shape) = shape {
            self.gather.shape = shape;
            return;
        }
        match id {
            "add" => self.gather.set_multiple(true, Some(Mode::Add)),
            "remove" => self.gather.set_multiple(true, Some(Mode::Remove)),
            "multiple" => self.gather.set_multiple(true, None),
            "single" => self.gather.single(),
            "undo" if self.gather.has_steps() => self.gather.undo(),
            // Nothing gathered: the prompt's previous answer steps back, as undo does.
            "undo" => self.undo(document, errors),
            "all" | "last" | "previous" | "selection" => {
                self.gather.shape = Shape::Default;
                self.window = None;
                let snapshot = document.snapshot();
                let ids = match id {
                    "all" => crate::model_space(&snapshot)
                        .ok_or_else(|| Error::new(codes::NO_MODEL_SPACE))
                        .and_then(|space| {
                            methods::select_all(&snapshot, space, &Options::default())
                        })
                        .map(|pick| pick.ids),
                    "last" => Ok(crate::last_objects(&snapshot)),
                    "previous" => Ok(usable(
                        &snapshot,
                        &self.session.previous_selection(&snapshot),
                    )),
                    _ => Ok(usable(
                        &snapshot,
                        &self.session.current_selection(&snapshot),
                    )),
                };
                match ids {
                    Ok(ids) => self.gesture(document, ids, self.gather.mode, errors),
                    Err(error) => errors.push(error_out(&error)),
                }
            }
            _ => {}
        }
    }

    /// Adds a polygon or fence vertex, bounded by the vertex limit of the selection methods.
    fn add_vertex(&mut self, point: [f64; 2], errors: &mut Vec<ErrorOut>) {
        let Shape::Polygon { points, .. } = &mut self.gather.shape else {
            return;
        };
        if points.len() >= methods::MAX_VERTICES {
            errors.push(error_out(
                &Error::new(codes::LIMIT_EXCEEDED)
                    .with_detail("limit", methods::MAX_VERTICES.to_string()),
            ));
            return;
        }
        points.push(point);
    }

    /// Closes the polygon or fence of the selection prompt: its objects are a gesture, and the shape
    /// is dropped. Too few vertices are refused and the shape keeps waiting.
    fn close_polygon(
        &mut self,
        document: &Document,
        region: Region,
        points: &[[f64; 2]],
        errors: &mut Vec<ErrorOut>,
    ) {
        let snapshot = document.snapshot();
        match gather::region_ids(&snapshot, region, points) {
            Ok(ids) => self.gesture(document, ids, self.gather.mode, errors),
            Err(error) => errors.push(error_out(&error)),
        }
    }

    /// The active prompt as the window shows it, when a command waits.
    fn prompt_out(&self) -> Option<PromptOut> {
        self.line.active().and_then(|s| {
            s.prompt().map(|p| PromptOut {
                command: s.command().id().to_owned(),
                key: p.key.clone(),
                message_id: p.message.id.clone(),
                args: p
                    .message
                    .args
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                expects: p.accepts.iter().map(expect_of).collect(),
                options: p
                    .keyword_options()
                    .map(|o| OptionOut {
                        id: o.id.clone(),
                        label: o.label.clone(),
                        shortcut: o.shortcut,
                    })
                    .chain(if Wants::of(&p.accepts).selection {
                        gather::options()
                    } else {
                        Vec::new()
                    })
                    .collect(),
                default: match &p.default {
                    Some(PromptInput::Keyword(id)) => Some(id.clone()),
                    _ => None,
                },
            })
        })
    }

    /// Recalls the input line before the one recalled last, the newest first.
    fn recall_previous(&mut self) {
        let back = self.recall.map_or(0, |b| b + 1);
        if let Some(text) = self.input_back(back) {
            self.recall = Some(back);
            self.recalled = Some(text);
        }
    }

    /// Recalls the input line after the one recalled last; past the newest, the line is emptied.
    fn recall_next(&mut self) {
        match self.recall {
            None => {}
            Some(0) => {
                self.recall = None;
                self.recalled = Some(String::new());
            }
            Some(back) => {
                self.recall = Some(back - 1);
                self.recalled = self.input_back(back - 1);
            }
        }
    }

    /// The input line `back` input lines before the newest one (0 is the newest).
    fn input_back(&self, back: usize) -> Option<String> {
        self.history
            .iter()
            .rev()
            .filter(|line| line.role == LineRole::Input)
            .nth(back)
            .map(|line| line.text.clone())
    }

    /// The history length the session setting `command.history.length` gives.
    fn history_limit(&self) -> usize {
        match self.session_value("command.history.length") {
            Some(settings::Value::Int(n)) => usize::try_from(n).unwrap_or(MAX_HISTORY),
            _ => MAX_HISTORY,
        }
    }

    /// The value of the session setting `name` in effect (its default when unset).
    fn session_value(&self, name: &str) -> Option<settings::Value> {
        let setting = settings::find(name)?;
        Some(settings::store::session_current(
            &|key| self.session.setting(key),
            setting,
        ))
    }

    /// The flag the session setting `name` holds.
    fn session_flag(&self, name: &str) -> bool {
        matches!(self.session_value(name), Some(settings::Value::Bool(true)))
    }

    /// The identifiers of the commands run most recently, the newest first, each once.
    fn recent_commands(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        self.history
            .iter()
            .rev()
            .filter(|line| line.role == LineRole::Command)
            .filter(|line| seen.insert(line.text.clone()))
            .take(lookup::RECENT_COMMANDS)
            .map(|line| line.text.clone())
            .collect()
    }

    /// The input lines that contain `query` (without regard to case), the newest first, at most the
    /// history length. An empty query, or one longer than [`lookup::MAX_QUERY_BYTES`], matches nothing.
    #[must_use]
    pub fn search_history(&self, query: &str) -> Vec<String> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || query.len() > lookup::MAX_QUERY_BYTES {
            return Vec::new();
        }
        self.history
            .iter()
            .rev()
            .filter(|line| line.role == LineRole::Input)
            .filter(|line| line.text.to_lowercase().contains(&query))
            .take(self.history_limit())
            .map(|line| line.text.clone())
            .collect()
    }

    /// The commands whose names or labels match `query`, the best matches first (see
    /// [`lookup::search`]).
    #[must_use]
    pub fn search_commands(&self, query: &str, limit: usize) -> Vec<CommandEntry> {
        lookup::search(&self.registry, query, limit)
    }

    /// The suggestions for the text typed on the command line (see [`lookup::suggest`]). Nothing is
    /// suggested while the setting `command.autocomplete.on` is off; the correction follows
    /// `command.autocorrect.on`.
    #[must_use]
    pub fn suggest(&self, text: &str) -> Vec<Suggestion> {
        if !self.session_flag("command.autocomplete.on") {
            return Vec::new();
        }
        lookup::suggest(
            &self.registry,
            text,
            &self.recent_commands(),
            self.session_flag("command.autocorrect.on"),
        )
    }

    /// Help for the command `name` in any form the command line reads. With `None`, help for the
    /// active command and its prompt. `None` when there is no such command.
    #[must_use]
    pub fn help(&self, name: Option<&str>) -> Option<Help> {
        match name {
            Some(name) => Some(Help {
                command: lookup::entry_named(&self.registry, name)?,
                prompt: None,
            }),
            None => {
                let active = self.line.active()?.command().id().to_owned();
                Some(Help {
                    command: lookup::entry_named(&self.registry, &active)?,
                    prompt: self.prompt_out(),
                })
            }
        }
    }

    /// The selection the window shows: the gathered objects in multiple mode, else the session's.
    fn shown_selection(&self, snapshot: &Snapshot) -> Vec<ObjectId> {
        if self.gather.multiple {
            self.gather
                .ids
                .iter()
                .copied()
                .filter(|id| snapshot.resolve(*id).is_ok())
                .collect()
        } else {
            self.session.current_selection(snapshot)
        }
    }

    fn outputs(&mut self, document: &Document, errors: Vec<ErrorOut>) -> Outputs {
        let excess = self.history.len().saturating_sub(self.history_limit());
        self.history.drain(..excess);
        let snapshot = document.snapshot();
        let prompt = self.prompt_out();
        let expects_point = prompt
            .as_ref()
            .is_some_and(|p| p.expects.contains(&Expect::Point));
        let cursor = self.cursor.as_ref().map(|(r, _)| r);
        let base = self.point_base();
        let band = match (self.window, self.cursor.as_ref()) {
            (Some(start), Some((_, at))) => Some(BandOut {
                base: start,
                current: *at,
                mode: match &self.gather.shape {
                    Shape::Box(mode) => *mode,
                    _ => band::mode(start, *at),
                },
            }),
            (None, Some((r, _))) if expects_point && base.is_some() => Some(BandOut {
                base: base.unwrap_or_default(),
                current: r.point,
                mode: BandMode::Rubber,
            }),
            _ => None,
        };
        let dynamic = match (&cursor, expects_point) {
            (Some(r), true) => Some(self.fields.output(&snapshot, base, r.point)),
            _ => None,
        };
        Outputs {
            prompt,
            history: self.history.clone(),
            recalled: self.recalled.take(),
            band,
            snap: cursor.and_then(|r| {
                r.snap.map(|kind| SnapOut {
                    kind,
                    point: r.point,
                })
            }),
            tracking: cursor.map(|r| r.tracking.clone()).unwrap_or_default(),
            dynamic,
            selection: self.shown_selection(&snapshot),
            cursor: cursor.map(|r| CursorOut {
                point: r.point,
                source: r.source,
            }),
            modifier: self
                .acquire
                .waiting()
                .map(|(modifier, step)| ModifierOut { modifier, step }),
            errors,
        }
    }
}

/// The objects of `ids` a selection can use in the model space: the filter of the selection methods
/// for visible objects on thawed layers, with locked objects kept, so that the command itself
/// refuses them with its own error. Empty without a model space.
fn usable(snapshot: &Snapshot, ids: &[ObjectId]) -> Vec<ObjectId> {
    let Some(space) = crate::model_space(snapshot) else {
        return Vec::new();
    };
    let options = Options {
        include_locked: true,
    };
    methods::select_previous(snapshot, space, ids, &options)
        .map(|pick| pick.ids)
        .unwrap_or_default()
}

/// Whether a prompt takes entities: a selection whose types name them. A selection without types
/// is an identifier list of another kind (layers, for instance), which a selection never fills.
fn takes_entities(accepts: &[Accept]) -> bool {
    accepts
        .iter()
        .any(|accept| matches!(accept, Accept::Selection { types, .. } if !types.is_empty()))
}

/// The unit direction from `from` to `to` (meters), `None` when they coincide.
fn unit(from: [f64; 2], to: [f64; 2]) -> Option<[f64; 2]> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy);
    (length > 0.0 && length.is_finite()).then_some([dx / length, dy / length])
}

/// The prompt point of a model point in meters.
fn point_of(p: [f64; 2]) -> Point {
    Point {
        x: p[0],
        y: p[1],
        z: None,
    }
}

/// The object under `at` (meters) through the host's picker, in drawing units.
fn pick_at(
    picker: &dyn ObjectPicker,
    snapshot: &Snapshot,
    at: [f64; 2],
    aperture: f64,
) -> Option<ObjectId> {
    let point = [from_meters(snapshot, at[0]), from_meters(snapshot, at[1])];
    picker.pick(snapshot, point, from_meters(snapshot, aperture))
}

/// The kind of input an accept takes, as the window shows it.
fn expect_of(accept: &Accept) -> Expect {
    match accept {
        Accept::Point { .. } => Expect::Point,
        Accept::Pick { .. } => Expect::Pick,
        Accept::Selection { .. } => Expect::Selection,
        Accept::Keyword { .. } => Expect::Keyword,
        Accept::Text { .. } => Expect::Text,
        _ => Expect::Value,
    }
}

/// The error as the window shows it: its code, category, message and its arguments and details.
fn error_out(error: &Error) -> ErrorOut {
    ErrorOut {
        code: error.code(),
        category: error.category(),
        message_id: error.message_id(),
        arguments: error
            .arguments()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect(),
        details: error
            .details()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect(),
    }
}

/// The value of `result`, or its error pushed to `errors`.
fn push_result<T>(errors: &mut Vec<ErrorOut>, result: Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            errors.push(error_out(&error));
            None
        }
    }
}

/// Records the error of a unit result, if any.
fn push_unit(errors: &mut Vec<ErrorOut>, result: Result<()>) {
    if let Err(error) = result {
        errors.push(error_out(&error));
    }
}

#[cfg(test)]
mod tests {
    use arc_command::Accept;

    use super::takes_entities;

    #[test]
    fn an_id_list_without_entity_types_is_never_preselected() {
        let layers = Accept::Selection {
            min: 1,
            max: None,
            types: Vec::new(),
        };
        let entities = Accept::Selection {
            min: 1,
            max: None,
            types: vec!["LINE".to_owned()],
        };
        assert!(!takes_entities(&[layers]));
        assert!(takes_entities(&[entities]));
    }
}
