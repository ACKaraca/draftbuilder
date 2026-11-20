//! The controller: owns the command line of a document and the state of the pointer, selection
//! and dynamic input, and turns each [`Event`] into the next [`Outputs`].

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::{Error, ObjectId, Result};
use arc_command::history::{self, Direction};
use arc_command::{
    Accept, CommandLine, Permission, Point, PromptInput, Session, Submitted, Surface, Target,
};
use arc_doc::{Document, Snapshot};
use serde_json::Map;

use super::band;
use super::fields::Fields;
use super::resolution::{Resolved, Resolver};
use super::{
    BandMode, BandOut, CursorOut, ErrorOut, Event, Expect, HistoryLine, LineRole, MAX_HISTORY,
    ObjectPicker, OptionOut, Outputs, PromptOut, SnapOut,
};
use crate::draw::create::from_meters;
use crate::select::{Mode, combine};
use crate::settings::store::Values;

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
            seed: 1,
        })
    }

    /// The number of snap indexes built so far (the revision of the document is the only reason
    /// to build one). A test hook: it lets a host check how often the index is rebuilt.
    #[must_use]
    pub fn snap_index_builds(&self) -> u64 {
        self.resolver.builds()
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
            Event::Undo => self.undo(document, &mut errors),
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
        let snapshot = document.snapshot();
        // Typed numbers take the units of the drawing, as in the script runner.
        self.session.units = crate::script::session_units(&snapshot);
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
        if text.trim().is_empty() {
            self.empty_line(document, errors);
            return;
        }
        self.history.push(HistoryLine {
            role: LineRole::Input,
            text: text.to_owned(),
        });
        let result = self.with_line(document, |line, target| line.submit(target, text));
        self.settle(result, errors);
    }

    /// The empty line: a locked field composes the point, else the prompt takes its default; with
    /// no command, the last command starts again at its first prompt.
    fn empty_line(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if self.line.active().is_some() {
            if self.fields.any_locked() {
                if let Some(point) = self.compose(document, errors) {
                    self.answer(document, PromptInput::Point(point_of(point)), errors);
                }
                return;
            }
            let result = self.with_line(document, |line, target| line.submit(target, ""));
            self.settle(result, errors);
            return;
        }
        let Some(command) = self.line.last().map(|last| last.command.clone()) else {
            return;
        };
        let result = self.with_line(document, |line, target| {
            line.start(target, &command, Map::new())
        });
        self.settle(result, errors);
    }

    /// The point of the locked fields over the cursor, in meters; `None` without a cursor.
    fn compose(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) -> Option<[f64; 2]> {
        let (resolved, _) = self.cursor.clone()?;
        let snapshot = document.snapshot();
        let base = self.line.base_point().map(|p| [p.x, p.y]);
        let composed = self.fields.compose(&snapshot, base, resolved.point);
        push_result(errors, composed)
    }

    fn answer(&mut self, document: &Document, input: PromptInput, errors: &mut Vec<ErrorOut>) {
        let result = self.with_line(document, |line, target| line.answer(target, input));
        self.settle(result, errors);
    }

    /// Takes the outcome of a command-line step: a run or a cancel ends the command's state; a
    /// rejection is an error and keeps the prompt.
    fn settle(&mut self, result: Result<Submitted>, errors: &mut Vec<ErrorOut>) {
        match push_result(errors, result) {
            Some(Submitted::Ran(outcome)) => {
                self.history.push(HistoryLine {
                    role: LineRole::Command,
                    text: outcome.command,
                });
                self.command_ended();
            }
            Some(Submitted::Cancelled) => self.command_ended(),
            Some(Submitted::Rejected { error, .. }) => errors.push(error_out(&error)),
            Some(Submitted::Prompt(_) | Submitted::Idle) => self.fields.clear(),
            None => {}
        }
    }

    /// The state the command's end resets: the locks, the acquired points and the cursor's snap,
    /// which the document may have moved away from.
    fn command_ended(&mut self) {
        self.fields.clear();
        self.resolver.clear_acquired();
        self.cursor = None;
    }

    fn escape(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if self.fields.any_locked() {
            self.fields.clear();
            return;
        }
        if self.window.take().is_some() {
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
        let cleared = self.session.record_selection(&[], &snapshot);
        push_unit(errors, cleared);
    }

    fn option(&mut self, document: &Document, id: String, errors: &mut Vec<ErrorOut>) {
        if self.line.active().is_none() {
            return;
        }
        self.answer(document, PromptInput::Keyword(id), errors);
    }

    fn undo(&mut self, document: &Document, errors: &mut Vec<ErrorOut>) {
        if self.line.active().is_some() {
            let result = self.with_line(document, |line, target| line.back(target));
            self.settle(result, errors);
            return;
        }
        match history::step(document, &self.session, Direction::Undo) {
            Ok(stepped) => {
                self.history.push(HistoryLine {
                    role: LineRole::Command,
                    text: stepped.command.unwrap_or_else(|| "history.undo".to_owned()),
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
        let base = self.line.base_point().map(|p| [p.x, p.y]);
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
        let snapshot = document.snapshot();
        let prompt_wants = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .map(|p| Wants::of(&p.accepts));
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
                self.answer(document, PromptInput::Selection(vec![id]), errors);
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
            (None, Some(id)) => self.select_object(&snapshot, id, errors),
            (None, None) => self.start_window(at),
        }
    }

    /// Selects `id` in addition to the selection (the pick adds an object).
    fn select_object(&mut self, snapshot: &Snapshot, id: ObjectId, errors: &mut Vec<ErrorOut>) {
        let current = self.session.current_selection(snapshot);
        let next = combine(&current, &[id], Mode::Add);
        let set = self.session.record_selection(&next, snapshot);
        push_unit(errors, set);
    }

    fn start_window(&mut self, at: [f64; 2]) {
        self.window = Some(at);
    }

    /// The second click of a window: the objects it selects, as the answer of a selection prompt
    /// or added to the selection without a command.
    fn finish_window(&mut self, document: &Document, at: [f64; 2], errors: &mut Vec<ErrorOut>) {
        let Some(start) = self.window.take() else {
            return;
        };
        let snapshot = document.snapshot();
        let ids = push_result(errors, band::select(&snapshot, start, at));
        let Some(ids) = ids else {
            return;
        };
        let prompt_selects = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| Wants::of(&p.accepts).selection);
        if prompt_selects {
            self.answer(document, PromptInput::Selection(ids), errors);
            return;
        }
        let current = self.session.current_selection(&snapshot);
        let next = combine(&current, &ids, Mode::Add);
        let set = self.session.record_selection(&next, &snapshot);
        push_unit(errors, set);
    }

    fn outputs(&mut self, document: &Document, errors: Vec<ErrorOut>) -> Outputs {
        let excess = self.history.len().saturating_sub(MAX_HISTORY);
        self.history.drain(..excess);
        let snapshot = document.snapshot();
        let prompt = self.line.active().and_then(|s| {
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
                    .collect(),
                default: match &p.default {
                    Some(PromptInput::Keyword(id)) => Some(id.clone()),
                    _ => None,
                },
            })
        });
        let expects_point = prompt
            .as_ref()
            .is_some_and(|p| p.expects.contains(&Expect::Point));
        let cursor = self.cursor.as_ref().map(|(r, _)| r);
        let base = self.line.base_point().map(|p| [p.x, p.y]);
        let band = match (self.window, self.cursor.as_ref()) {
            (Some(start), Some((_, at))) => Some(BandOut {
                base: start,
                current: *at,
                mode: band::mode(start, *at),
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
            band,
            snap: cursor.and_then(|r| {
                r.snap.map(|kind| SnapOut {
                    kind,
                    point: r.point,
                })
            }),
            tracking: cursor.map(|r| r.tracking.clone()).unwrap_or_default(),
            dynamic,
            selection: self.session.current_selection(&snapshot),
            cursor: cursor.map(|r| CursorOut {
                point: r.point,
                source: r.source,
            }),
            errors,
        }
    }
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

/// The error as the window shows it.
fn error_out(error: &Error) -> ErrorOut {
    ErrorOut {
        code: error.code(),
        message_id: error.message_id(),
        details: error
            .details()
            .iter()
            .map(|(name, value)| ((*name).to_owned(), detail_text(value)))
            .collect(),
    }
}

fn detail_text(value: &arc_base::error::Value) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
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
