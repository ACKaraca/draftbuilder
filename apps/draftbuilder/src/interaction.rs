//! The interaction adapter of the DraftBuilder window: translation only. The headless controller of
//! `draftbuilder-commands` owns the command line, the prompts, the pointer resolution, the selection
//! and the dynamic input of a tab. This module turns the shell's pointer events, line actions and
//! dynamic input edits into controller events, and the controller's outputs into what the shell
//! shows. Two conversions happen here and nowhere else: the controller works in meters, the drawing
//! and the viewport in drawing units; and the host's session permissions are handed to the controller
//! before each event.

use std::collections::BTreeSet;
use std::sync::Arc;

use arc_app::commands::{EDIT_REDO, EDIT_UNDO, SELECT_CLEAR};
use arc_app::locale::Localizer;
use arc_app::shell::{DynamicOutput, Interaction, InteractionContext, InteractionOutput};
use arc_base::error::Value;
use arc_base::math::Point3;
use arc_base::{Error, ErrorCode, Result};
use arc_command::{Invocation, Permission};
use arc_doc::Document;
use arc_draft::snap::SnapKind;
use arc_ui::egui::PointerButton;
use arc_ui::{
    BandKind, DynamicEvent, DynamicField, Intent, LineAction, LineStatus, OptionControl,
    PointerEvent, PointerKind, ProductOverlay, PromptControls, SnapKind as MarkerKind,
};
use draftbuilder_commands::interaction::{
    BandMode, BandOut, Controller, CrossingPicker, DynamicMode, DynamicOut, ErrorOut, Event,
    HistoryLine, LineRole, Outputs, meters_per_unit,
};
use serde_json::Value as Json;

/// The pick aperture of a click or a hover, in pixels: the half side of the pick box.
const PICK_APERTURE_PIXELS: f64 = 5.0;

/// A message the adapter formats: its identifier in the application's catalogs.
struct Message {
    message_id: &'static str,
}

const DISTANCE: Message = Message {
    message_id: "draftbuilder-dynamic-distance",
};
const ANGLE: Message = Message {
    message_id: "draftbuilder-dynamic-angle",
};
const X: Message = Message {
    message_id: "draftbuilder-dynamic-x",
};
const Y: Message = Message {
    message_id: "draftbuilder-dynamic-y",
};
const HISTORY_RAN: Message = Message {
    message_id: "draftbuilder-history-ran",
};
const HISTORY_CANCELLED: Message = Message {
    message_id: "draftbuilder-history-cancelled",
};
const HISTORY_UNDO: Message = Message {
    message_id: "draftbuilder-history-undo",
};
const HISTORY_REDO: Message = Message {
    message_id: "draftbuilder-history-redo",
};

/// The interaction of one tab. Without a controller it consumes no pointer events and reports the
/// construction error on the command line.
pub struct DrawingInteraction {
    controller: Option<Controller>,
    /// Why there is no controller.
    unavailable: Option<ErrorOut>,
    /// What the controller showed after its last event.
    shown: Outputs,
    /// The text typed in each dynamic field; used while the field is locked.
    typed: [String; 2],
    /// The controller errors of the last click or dynamic event, reported by the next line action
    /// that has no error of its own. A pointer event has no other channel for them.
    pending: Vec<ErrorOut>,
    /// The document revision the controller last saw; a change in between came from outside.
    seen: Option<u64>,
    /// Meters per drawing unit of the document at the last event.
    scale: f64,
    /// Formats the history lines; without it the message identifiers are shown.
    messages: Option<Arc<Localizer>>,
}

impl DrawingInteraction {
    /// The interaction of a new tab.
    #[must_use]
    pub fn new(messages: Option<Arc<Localizer>>) -> Self {
        match Controller::new() {
            Ok(controller) => Self::with(Some(controller), None, messages),
            Err(error) => Self::failed(&error, messages),
        }
    }

    /// An interaction without a controller, which reports `error` on every line action.
    #[must_use]
    pub fn failed(error: &Error, messages: Option<Arc<Localizer>>) -> Self {
        Self::with(None, Some(error_out(error)), messages)
    }

    fn with(
        controller: Option<Controller>,
        unavailable: Option<ErrorOut>,
        messages: Option<Arc<Localizer>>,
    ) -> Self {
        Self {
            controller,
            unavailable,
            shown: Outputs::default(),
            typed: [String::new(), String::new()],
            pending: Vec::new(),
            seen: None,
            scale: 1.0,
            messages,
        }
    }

    /// A pointer event on `document` for a host session with `permissions`. A primary click is
    /// consumed; the other events are left to the viewport.
    pub fn pointer_on(
        &mut self,
        document: &Document,
        permissions: &BTreeSet<Permission>,
        event: &PointerEvent,
    ) -> bool {
        self.sync(document, permissions);
        let (Some(model), Some(pixels_per_unit)) = (
            event.model,
            event.pixels_per_unit.filter(|v| v.is_finite() && *v > 0.0),
        ) else {
            return false;
        };
        let at = self.to_meters([model.x, model.y]);
        let aperture = self.length_to_meters(PICK_APERTURE_PIXELS / pixels_per_unit);
        match event.kind {
            PointerKind::Move => {
                // A hover error is not kept: the next command must not report it.
                let _ = self.run(document, Event::Move { at, aperture });
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                if self.controller.is_none() {
                    return false;
                }
                self.pending = self.run(document, Event::Click { at, aperture });
                true
            }
            _ => false,
        }
    }

    /// An edit of the dynamic input on `document` for a host session with `permissions`.
    pub fn dynamic_on(
        &mut self,
        document: &Document,
        permissions: &BTreeSet<Permission>,
        event: &DynamicEvent,
    ) {
        self.sync(document, permissions);
        let event = match event {
            DynamicEvent::Edited { index, text } => {
                if let Some(typed) = self.typed.get_mut(*index) {
                    typed.clone_from(text);
                }
                Event::Field {
                    index: *index,
                    text: text.clone(),
                }
            }
            DynamicEvent::Tab { .. } => Event::Tab,
            DynamicEvent::Enter { .. } => Event::Submit(String::new()),
        };
        self.pending = self.run(document, event);
    }

    /// A command-line action on `document` for a host session with `permissions`.
    ///
    /// Returns the error of this action when it has one, else the error of the last click or dynamic
    /// event that no line action has reported yet, else the document command the controller does not
    /// own (undo and redo are the controller's). The caller runs that command and then calls
    /// [`Self::refresh_on`] and [`Self::stale_error`].
    ///
    /// # Errors
    ///
    /// The construction error when there is no controller, the action's own error, or a pending error.
    pub fn line_on(
        &mut self,
        document: &Document,
        permissions: &BTreeSet<Permission>,
        action: LineAction,
    ) -> Result<Option<Invocation>> {
        self.sync(document, permissions);
        if let Some(unavailable) = &self.unavailable {
            return Err(error_of(unavailable));
        }
        let mut errors = Vec::new();
        let request = match action {
            LineAction::ClearInput => None,
            LineAction::Submit(text) => {
                errors = self.run(document, Event::Submit(text));
                None
            }
            LineAction::Repeat => {
                errors = self.run(document, Event::Submit(String::new()));
                None
            }
            LineAction::Back => {
                errors = self.run(document, Event::Undo);
                None
            }
            LineAction::Cancel => {
                errors = self.run(document, Event::Escape);
                None
            }
            LineAction::Invoke(intent) => self.intent(document, intent, &mut errors),
            LineAction::CancelThen(intent) => {
                errors = self.run(document, Event::Escape);
                self.intent(document, intent, &mut errors)
            }
        };
        if let Some(own) = errors.first() {
            self.pending.clear();
            return Err(error_of(own));
        }
        match request {
            Some(invocation) => Ok(Some(invocation)),
            None => self.stale_error().map(|()| None),
        }
    }

    /// Shows the controller's state for `document` after the caller ran a document command.
    pub fn refresh_on(&mut self, document: &Document) {
        self.note(document);
        if let Some(controller) = self.controller.as_mut() {
            self.shown = controller.refresh(document);
        }
    }

    /// The pending error of the last click or dynamic event, taken: reported once.
    ///
    /// # Errors
    ///
    /// That error, when one is pending.
    pub fn stale_error(&mut self) -> Result<()> {
        let errors = std::mem::take(&mut self.pending);
        errors.first().map_or(Ok(()), |e| Err(error_of(e)))
    }

    /// What the shell shows now.
    #[must_use]
    pub fn output_now(&self) -> InteractionOutput {
        let prompt = self
            .controller
            .as_ref()
            .and_then(|c| c.active_prompt().cloned());
        let busy = self.controller.as_ref().is_some_and(Controller::busy);
        let shown = &self.shown;
        InteractionOutput {
            status: LineStatus {
                active: prompt.is_some(),
                answers: self.controller.as_ref().map_or(0, Controller::answers),
                has_selection: !shown.selection.is_empty() || busy,
                has_last: self.controller.as_ref().is_some_and(Controller::repeatable),
            },
            prompt,
            options: PromptControls {
                options: shown
                    .prompt
                    .as_ref()
                    .map(|p| {
                        p.options
                            .iter()
                            .map(|o| OptionControl {
                                id: o.id.clone(),
                                label: o.label.clone(),
                                shortcut: o.shortcut,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                parameters: Vec::new(),
            },
            history: shown.history.iter().map(|l| self.history_text(l)).collect(),
            overlays: self.overlays(shown),
            dynamic: shown
                .dynamic
                .as_ref()
                .map(|d| self.dynamic_output(d, shown)),
            selection: shown.selection.clone(),
        }
    }

    /// Runs one event through the controller, keeps what it shows and returns its errors.
    fn run(&mut self, document: &Document, event: Event) -> Vec<ErrorOut> {
        let Some(controller) = self.controller.as_mut() else {
            return Vec::new();
        };
        let mut out = controller.handle(document, event, &CrossingPicker);
        let errors = std::mem::take(&mut out.errors);
        self.shown = out;
        self.note(document);
        errors
    }

    /// A line-action intent: undo, redo and clear-selection are the controller's; any other command
    /// is returned for the host to run.
    fn intent(
        &mut self,
        document: &Document,
        intent: Intent,
        errors: &mut Vec<ErrorOut>,
    ) -> Option<Invocation> {
        let event = match intent.command.as_str() {
            EDIT_UNDO => Event::Undo,
            EDIT_REDO => Event::Redo,
            SELECT_CLEAR => Event::Escape,
            _ => {
                return Some(Invocation::new(intent.command, Json::Object(intent.params)));
            }
        };
        errors.extend(self.run(document, event));
        None
    }

    /// Takes the document's scale and revision after the controller ran.
    fn note(&mut self, document: &Document) {
        let snapshot = document.snapshot();
        self.scale = meters_per_unit(&snapshot);
        self.seen = Some(snapshot.revision());
    }

    /// Before an event: hands the host's permissions to the controller, and when the document
    /// changed since the last event without the controller, ends the active command.
    fn sync(&mut self, document: &Document, permissions: &BTreeSet<Permission>) {
        let snapshot = document.snapshot();
        let revision = snapshot.revision();
        let changed = self.seen.is_some_and(|seen| seen != revision);
        self.scale = meters_per_unit(&snapshot);
        self.seen = Some(revision);
        if let Some(controller) = self.controller.as_mut() {
            controller.set_permissions(permissions);
            if changed {
                self.shown = controller.external_change(document);
            }
        }
    }

    /// The conversions between drawing units and the controller's meters. They are the only
    /// conversions of this adapter.
    fn to_meters(&self, p: [f64; 2]) -> [f64; 2] {
        [p[0] * self.scale, p[1] * self.scale]
    }

    fn to_drawing(&self, p: [f64; 2]) -> [f64; 2] {
        [p[0] / self.scale, p[1] / self.scale]
    }

    fn length_to_meters(&self, length: f64) -> f64 {
        length * self.scale
    }

    fn text(&self, message: &Message, arguments: &[(&str, Value)]) -> String {
        match &self.messages {
            Some(localizer) => localizer.format(message.message_id, arguments).text,
            None => message.message_id.to_owned(),
        }
    }

    fn history_text(&self, line: &HistoryLine) -> String {
        match line.role {
            LineRole::Input => line.text.clone(),
            LineRole::Command => {
                self.text(&HISTORY_RAN, &[("command", Value::Text(line.text.clone()))])
            }
            LineRole::Cancelled => self.text(
                &HISTORY_CANCELLED,
                &[("command", Value::Text(line.text.clone()))],
            ),
            LineRole::Undo => self.text(&HISTORY_UNDO, &[]),
            LineRole::Redo => self.text(&HISTORY_REDO, &[]),
        }
    }

    fn dynamic_output(&self, out: &DynamicOut, shown: &Outputs) -> DynamicOutput {
        let labels = match out.mode {
            DynamicMode::Relative => [&DISTANCE, &ANGLE],
            DynamicMode::Absolute => [&X, &Y],
        };
        let fields = out
            .fields
            .iter()
            .zip(labels)
            .enumerate()
            .map(|(index, (field, label))| DynamicField {
                label: label.message_id.to_owned(),
                // An unlocked field is empty: typed characters go into it, and the value at the
                // cursor is what the controller reports, not text the field holds.
                text: match (field.locked, self.typed.get(index)) {
                    (true, Some(typed)) if !typed.is_empty() => typed.clone(),
                    (true, _) => format!("{:.3}", field.value),
                    (false, _) => String::new(),
                },
                locked: field.locked,
                focused: field.focused,
            })
            .collect();
        DynamicOutput {
            prompt: shown
                .prompt
                .as_ref()
                .map_or_else(String::new, |p| p.message_id.clone()),
            fields,
        }
    }

    fn overlays(&self, shown: &Outputs) -> Vec<ProductOverlay> {
        let mut overlays = Vec::new();
        if let Some(band) = shown.band {
            overlays.push(self.band_overlay(band));
        }
        if let Some(snap) = shown.snap
            && let Some(kind) = marker_of(snap.kind)
        {
            overlays.push(ProductOverlay::Snap {
                kind,
                at: point3(self.to_drawing(snap.point)),
            });
        }
        overlays
    }

    fn band_overlay(&self, band: BandOut) -> ProductOverlay {
        let (start, end) = (
            point3(self.to_drawing(band.base)),
            point3(self.to_drawing(band.current)),
        );
        match band.mode {
            BandMode::Rubber => ProductOverlay::Line {
                from: start,
                to: end,
            },
            BandMode::Window => ProductOverlay::Band {
                start,
                end,
                kind: BandKind::Window,
            },
            BandMode::Crossing => ProductOverlay::Band {
                start,
                end,
                kind: BandKind::Crossing,
            },
        }
    }
}

impl Interaction for DrawingInteraction {
    fn pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        match cx.document() {
            Ok(document) => self.pointer_on(document, &cx.session().permissions, event),
            Err(_) => false,
        }
    }

    fn dynamic(&mut self, cx: &InteractionContext<'_>, event: &DynamicEvent) {
        if let Ok(document) = cx.document() {
            self.dynamic_on(document, &cx.session().permissions, event);
        }
    }

    fn line(&mut self, cx: &InteractionContext<'_>, action: LineAction) -> Result<()> {
        let document = cx.document()?;
        match self.line_on(document, &cx.session().permissions, action) {
            Err(error) => Err(error),
            Ok(None) => Ok(()),
            Ok(Some(invocation)) => {
                let run = cx.invoke(invocation).map(|_| ());
                self.refresh_on(document);
                // The invocation's own error is reported first; a pending error does not hide it.
                run.and(self.stale_error())
            }
        }
    }

    fn output(&self) -> InteractionOutput {
        self.output_now()
    }
}

/// The marker of a snap kind; kinds without a marker in the UI kit are not drawn.
fn marker_of(kind: SnapKind) -> Option<MarkerKind> {
    Some(match kind {
        SnapKind::Endpoint => MarkerKind::Endpoint,
        SnapKind::Midpoint => MarkerKind::Midpoint,
        SnapKind::Center => MarkerKind::Center,
        SnapKind::Intersection => MarkerKind::Intersection,
        SnapKind::Perpendicular => MarkerKind::Perpendicular,
        SnapKind::Tangent => MarkerKind::Tangent,
        SnapKind::Nearest => MarkerKind::Nearest,
        SnapKind::Node => MarkerKind::Node,
        SnapKind::Quadrant => MarkerKind::Quadrant,
        _ => return None,
    })
}

/// A model point (drawing units) as a model point of the viewport.
fn point3(p: [f64; 2]) -> Point3 {
    Point3::new(p[0], p[1], 0.0)
}

fn error_out(error: &Error) -> ErrorOut {
    ErrorOut {
        code: error.code(),
        category: error.category(),
        message_id: error.message_id(),
        arguments: error
            .arguments()
            .iter()
            .map(|(name, value)| (*name, value.clone()))
            .collect(),
        details: error
            .details()
            .iter()
            .map(|(name, value)| (*name, value.clone()))
            .collect(),
    }
}

/// The error a controller error reports: its code, category, message and arguments.
fn error_of(out: &ErrorOut) -> Error {
    let mut error = Error::new(ErrorCode {
        code: out.code,
        category: out.category,
        message_id: out.message_id,
    });
    for (name, value) in out.arguments.iter().cloned() {
        error = error.with_arg(name, value);
    }
    for (name, value) in out.details.iter().cloned() {
        error = error.with_detail(name, value);
    }
    error
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, reason = "test code")]
mod tests {
    use std::collections::BTreeSet;

    use arc_app::commands::{EDIT_REDO, EDIT_UNDO, SELECT_CLEAR};
    use arc_base::Limits;
    use arc_base::error::{Category, Value};
    use arc_base::math::Point3;
    use arc_command::{Invocation, Invoker, Permission, Session, Surface};
    use arc_doc::graph::Evaluators;
    use arc_doc::{Document, DocumentConfig, Snapshot};
    use arc_draft::types::LINE;
    use arc_draft_dxf::fields::Reader;
    use arc_geom::Segment2;
    use arc_geom::encoding::GeomCodec;
    use arc_ui::egui::{Modifiers, PointerButton};
    use arc_ui::{DynamicEvent, Intent, LineAction, PointerEvent, PointerKind, ProductOverlay};
    use draftbuilder_commands::{install, model_space, registry, types};
    use serde_json::json;

    use super::*;

    fn session() -> Session {
        Session::new(
            Surface::CommandLine,
            [
                Permission::Read,
                Permission::Edit,
                Permission::FileRead,
                Permission::FileWrite,
            ],
        )
    }

    /// A document with the product's types, without a model space.
    fn bare_document() -> Document {
        let config = DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        };
        let document = Document::open(
            Snapshot::empty(Arc::new(types().unwrap())),
            Arc::new(Evaluators::new()),
            config,
        )
        .unwrap();
        install(&document);
        document
    }

    /// A new drawing with the given `draftbuilder.drawing.new` parameters.
    fn drawing(params: serde_json::Value) -> Document {
        let document = bare_document();
        Invoker::new(&registry().unwrap())
            .invoke(
                &document,
                &session(),
                Invocation::new("draftbuilder.drawing.new", params),
            )
            .unwrap();
        document
    }

    fn edit() -> BTreeSet<Permission> {
        [Permission::Read, Permission::Edit].into_iter().collect()
    }

    fn read_only() -> BTreeSet<Permission> {
        [Permission::Read].into_iter().collect()
    }

    fn adapter() -> DrawingInteraction {
        DrawingInteraction::new(Some(Arc::new(crate::localizer(&[]).unwrap())))
    }

    fn pointer(kind: PointerKind, at: [f64; 2], button: Option<PointerButton>) -> PointerEvent {
        PointerEvent {
            kind,
            button,
            modifiers: Modifiers::NONE,
            pixel: at,
            model: Some(Point3::new(at[0], at[1], 0.0)),
            pixels_per_unit: Some(1.0),
        }
    }

    fn click(
        adapter: &mut DrawingInteraction,
        document: &Document,
        permissions: &BTreeSet<Permission>,
        at: [f64; 2],
    ) -> bool {
        adapter.pointer_on(
            document,
            permissions,
            &pointer(PointerKind::Click, at, Some(PointerButton::Primary)),
        )
    }

    fn hover(
        adapter: &mut DrawingInteraction,
        document: &Document,
        permissions: &BTreeSet<Permission>,
        at: [f64; 2],
    ) -> bool {
        adapter.pointer_on(document, permissions, &pointer(PointerKind::Move, at, None))
    }

    fn typed(
        adapter: &mut DrawingInteraction,
        document: &Document,
        permissions: &BTreeSet<Permission>,
        text: &str,
    ) -> Result<()> {
        adapter
            .line_on(document, permissions, LineAction::Submit(text.to_owned()))
            .map(|request| assert!(request.is_none()))
    }

    /// The segments of the model space as (start, end) pairs, in drawing units.
    fn segments(document: &Document) -> Vec<([f64; 2], [f64; 2])> {
        let snapshot = document.snapshot();
        let mut out = Vec::new();
        for (_, object) in snapshot.iter_type(LINE).unwrap() {
            let reader = Reader::new(snapshot.registry(), LINE, &object.values);
            if let Some(arc_doc::Value::Geometry(bytes)) = reader.get("geometry") {
                let segment = Segment2::decode(&bytes.0, &Limits::DEFAULT).unwrap();
                let (a, b) = (segment.start(), segment.end());
                out.push(([a.x, a.y], [b.x, b.y]));
            }
        }
        out
    }

    fn near(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn a_line_drawn_by_two_clicks_shows_its_rubber_line_and_is_written() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        assert!(a.output_now().status.active);
        assert_eq!(a.output_now().history, vec!["LINE".to_owned()]);

        assert!(click(&mut a, &document, &p, [0.0, 0.0]));
        assert_eq!(a.output_now().status.answers, 1);
        assert!(!hover(&mut a, &document, &p, [4.0, 0.0]));
        assert!(a.output_now().overlays.iter().any(|o| matches!(
            o,
            ProductOverlay::Line { from, to }
                if near([from.x, from.y], [0.0, 0.0]) && near([to.x, to.y], [4.0, 0.0])
        )),);

        assert!(click(&mut a, &document, &p, [10.0, 0.0]));
        typed(&mut a, &document, &p, "").unwrap();
        let out = a.output_now();
        assert!(!out.status.active);
        assert!(out.status.has_last);
        assert_eq!(segments(&document), vec![([0.0, 0.0], [10.0, 0.0])]);
    }

    #[test]
    fn millimeter_coordinates_are_drawing_units_in_and_out() {
        // The default drawing is metric: its drawing unit is a millimeter.
        let document = drawing(json!({}));
        let scale = meters_per_unit(&document.snapshot());
        assert!((scale - 0.001).abs() < 1e-12, "{scale}");

        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        assert!(click(&mut a, &document, &p, [100.0, 0.0]));
        assert!(!hover(&mut a, &document, &p, [150.0, 0.0]));
        let rubber = a.output_now().overlays.into_iter().find_map(|o| match o {
            ProductOverlay::Line { from, to } => Some(([from.x, from.y], [to.x, to.y])),
            _ => None,
        });
        let (from, to) = rubber.expect("a rubber line");
        assert!(
            near(from, [100.0, 0.0]) && near(to, [150.0, 0.0]),
            "{from:?} {to:?}"
        );

        assert!(click(&mut a, &document, &p, [200.0, 0.0]));
        typed(&mut a, &document, &p, "").unwrap();
        let lines = segments(&document);
        assert_eq!(lines.len(), 1);
        assert!(near(lines[0].0, [100.0, 0.0]) && near(lines[0].1, [200.0, 0.0]));
    }

    #[test]
    fn escape_cancels_the_line_and_says_so_in_the_history() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        click(&mut a, &document, &p, [0.0, 0.0]);
        assert!(
            a.line_on(&document, &p, LineAction::Cancel)
                .unwrap()
                .is_none()
        );
        let out = a.output_now();
        assert!(out.prompt.is_none());
        assert!(!out.status.active);
        assert!(segments(&document).is_empty());
        let last = out.history.last().unwrap();
        assert!(last.starts_with("Cancelled"), "{last}");
        assert!(last.contains("draftbuilder.draw.line"), "{last}");
    }

    #[test]
    fn a_keyboard_entered_coordinate_answers_the_point() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        typed(&mut a, &document, &p, "1,2").unwrap();
        typed(&mut a, &document, &p, "@10,0").unwrap();
        typed(&mut a, &document, &p, "").unwrap();
        let lines = segments(&document);
        assert_eq!(lines.len(), 1);
        assert!(near(lines[0].0, [1.0, 2.0]) && near(lines[0].1, [11.0, 2.0]));
    }

    #[test]
    fn dynamic_fields_start_empty_and_follow_the_typed_text() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        click(&mut a, &document, &p, [0.0, 0.0]);
        let dynamic = a.output_now().dynamic.expect("point input shows the panel");
        assert_eq!(dynamic.fields[0].label, DISTANCE.message_id);
        assert_eq!(dynamic.fields[1].label, ANGLE.message_id);
        assert_eq!(dynamic.fields[0].text, "");
        assert!(!dynamic.fields[0].locked);

        let edit_field = |a: &mut DrawingInteraction, text: &str| {
            a.dynamic_on(
                &document,
                &p,
                &DynamicEvent::Edited {
                    index: 0,
                    text: text.to_owned(),
                },
            );
        };
        edit_field(&mut a, "1");
        edit_field(&mut a, "15");
        let field = &a.output_now().dynamic.unwrap().fields[0];
        assert_eq!((field.text.as_str(), field.locked), ("15", true));

        edit_field(&mut a, "");
        let field = &a.output_now().dynamic.unwrap().fields[0];
        assert_eq!((field.text.as_str(), field.locked), ("", false));
    }

    #[test]
    fn undo_then_redo_restores_the_line() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        click(&mut a, &document, &p, [0.0, 0.0]);
        click(&mut a, &document, &p, [10.0, 0.0]);
        typed(&mut a, &document, &p, "").unwrap();
        assert_eq!(segments(&document).len(), 1);

        let undo = a.line_on(&document, &p, LineAction::Invoke(Intent::new(EDIT_UNDO)));
        assert!(undo.unwrap().is_none());
        assert!(segments(&document).is_empty());

        let redo = a.line_on(&document, &p, LineAction::Invoke(Intent::new(EDIT_REDO)));
        assert!(
            redo.unwrap().is_none(),
            "redo is the controller's, not a host command"
        );
        assert_eq!(segments(&document), vec![([0.0, 0.0], [10.0, 0.0])]);
        assert_eq!(a.output_now().history.last().unwrap(), "Redo");
    }

    #[test]
    fn only_a_primary_click_is_consumed() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        let secondary = pointer(
            PointerKind::Click,
            [1.0, 1.0],
            Some(PointerButton::Secondary),
        );
        assert!(!a.pointer_on(&document, &p, &secondary));
        assert_eq!(a.output_now().status.answers, 0);
    }

    #[test]
    fn a_line_action_reports_its_own_error_with_its_arguments() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        let result = a.line_on(
            &document,
            &p,
            LineAction::Submit("NOSUCHCOMMAND".to_owned()),
        );
        let Err(error) = result else {
            panic!("an unknown command is an error");
        };
        assert_eq!(
            error.arguments().get("name"),
            Some(&Value::Text("NOSUCHCOMMAND".to_owned()))
        );
        let localizer = crate::localizer(&[]).unwrap();
        assert!(
            localizer
                .format_error(&error)
                .text
                .contains("NOSUCHCOMMAND"),
            "{}",
            localizer.format_error(&error).text
        );
    }

    #[test]
    fn a_hover_error_is_not_reported_by_a_later_command() {
        // No model space: a hover fails, and the next command-line action succeeds.
        let document = bare_document();
        let p = edit();
        let mut a = adapter();
        assert!(!hover(&mut a, &document, &p, [1.0, 1.0]));
        assert!(
            a.line_on(&document, &p, LineAction::Submit(String::new()))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_dynamic_error_is_reported_once_by_the_next_line_action() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        click(&mut a, &document, &p, [0.0, 0.0]);
        a.dynamic_on(
            &document,
            &p,
            &DynamicEvent::Edited {
                index: 0,
                text: "abc".to_owned(),
            },
        );
        a.dynamic_on(&document, &p, &DynamicEvent::Enter { index: 0 });

        assert!(a.line_on(&document, &p, LineAction::Cancel).is_err());
        assert!(
            a.line_on(&document, &p, LineAction::Cancel)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_host_without_edit_cannot_draw_the_line() {
        let document = drawing(json!({"unit": "meter"}));
        let read = read_only();
        let mut a = adapter();
        let started = a.line_on(&document, &read, LineAction::Submit("LINE".to_owned()));
        click(&mut a, &document, &read, [0.0, 0.0]);
        click(&mut a, &document, &read, [10.0, 0.0]);
        let finished = a.line_on(&document, &read, LineAction::Submit(String::new()));
        assert!(started.is_err() || finished.is_err());
        assert!(segments(&document).is_empty());
    }

    #[test]
    fn escape_clears_a_pending_window_when_no_command_is_active() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        assert!(click(&mut a, &document, &p, [0.0, 0.0]));
        let out = a.output_now();
        assert!(
            out.status.has_selection,
            "a pending window counts as a selection to clear"
        );
        assert!(
            out.overlays
                .iter()
                .any(|o| matches!(o, ProductOverlay::Band { .. }))
        );

        a.line_on(&document, &p, LineAction::Invoke(Intent::new(SELECT_CLEAR)))
            .unwrap();
        let out = a.output_now();
        assert!(!out.status.has_selection);
        assert!(
            !out.overlays
                .iter()
                .any(|o| matches!(o, ProductOverlay::Band { .. }))
        );
    }

    #[test]
    fn a_change_made_outside_the_controller_ends_the_command() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        click(&mut a, &document, &p, [0.0, 0.0]);
        assert!(a.output_now().status.active);

        let space = model_space(&document.snapshot()).unwrap().to_string();
        Invoker::new(&registry().unwrap())
            .invoke(
                &document,
                &session(),
                Invocation::new(
                    "core.draft.entity.create",
                    json!({"owner": space, "geometry": {"kind": "segment", "start": [50.0, 0.0], "end": [60.0, 0.0]}}),
                ),
            )
            .unwrap();

        hover(&mut a, &document, &p, [5.0, 5.0]);
        let out = a.output_now();
        assert!(!out.status.active);
        assert!(out.prompt.is_none());
    }

    #[test]
    fn a_zero_or_non_finite_scale_is_absent_and_consumes_nothing() {
        let document = drawing(json!({"unit": "meter"}));
        let p = edit();
        let mut a = adapter();
        typed(&mut a, &document, &p, "LINE").unwrap();
        for scale in [0.0, f64::NAN, f64::INFINITY, -1.0] {
            let event = PointerEvent {
                pixels_per_unit: Some(scale),
                ..pointer(PointerKind::Click, [0.0, 0.0], Some(PointerButton::Primary))
            };
            assert!(!a.pointer_on(&document, &p, &event), "scale {scale}");
        }
        assert_eq!(a.output_now().status.answers, 0);
    }

    #[test]
    fn an_interaction_without_a_controller_reports_its_error_on_every_line_action() {
        let document = bare_document();
        let p = edit();
        let error = Error::new(ErrorCode {
            code: "draftbuilder.test.unavailable",
            category: Category::InvalidInput,
            message_id: "draftbuilder-test-unavailable",
        });
        let mut a = DrawingInteraction::failed(&error, None);
        assert!(a.line_on(&document, &p, LineAction::ClearInput).is_err());
        assert!(
            a.line_on(&document, &p, LineAction::Submit("LINE".to_owned()))
                .is_err()
        );
        assert!(!click(&mut a, &document, &p, [0.0, 0.0]));
    }
}
