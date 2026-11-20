//! The interaction controller (headless): one state machine that owns the command line of a
//! document and turns pointer and keyboard events into command-line steps, selection and
//! outputs that a window renders as plain data.
//!
//! The controller is the product's answer to the command-line and cursor input rows of the
//! capability specification (DB-CMD-002, 005, 026; DB-SNAP-009, 013, 015, 017; DB-SEL-001, 002,
//! 010). It holds no window, GPU or UI type. Coordinates in events and outputs are model
//! coordinates in meters, the convention of the prompts ([`arc_command::Point`]); the controller
//! converts to drawing units where the resolution and the selection methods need them.
//!
//! Behavior, by input:
//!
//! - [`Event::Submit`]: a typed line goes to the command line (a transparent line starts with an
//!   apostrophe). An empty line accepts the prompt's default, or, with no command active, restarts
//!   the last command at its first prompt; a locked dynamic input field composes the point first.
//! - [`Event::Escape`]: clears the locked dynamic fields or a pending window first; otherwise
//!   cancels the active command; with no command, clears the selection.
//! - [`Event::Move`]: resolves the cursor (object snap, tracking, ortho, polar, snap step) and sets
//!   the pointer direction for direct distances. The object snap uses a [`SnapIndex`] kept per
//!   document revision.
//! - [`Event::Click`]: answers the active point prompt with the resolved point; answers an object
//!   prompt with the object under the pointer (through the host's [`ObjectPicker`]); with no
//!   command, adds the object under the pointer to the selection, or starts a window or crossing
//!   that the second click finishes by drag direction. At a selection prompt a click on empty space
//!   starts the same window, and a click after a `window`, `crossing` or polygon keyword takes the
//!   corner or the vertex the keyword asks for (see `gather`).
//! - [`Event::ShiftClick`]: removes the object under the pointer from the selection, or from the
//!   objects a selection prompt gathers.
//! - [`Event::Lasso`]: a free-hand outline dragged over the drawing selects by window or crossing
//!   polygon, as the drag ends right of or left of where it starts.
//! - [`Event::Option`] and a typed word: a keyword of a selection prompt (`all`, `last`, `previous`,
//!   `window`, `crossing`, `wpolygon`, `cpolygon`, `fence`, `add`, `remove`, `multiple`, `single`,
//!   `undo`) is taken by the controller; the prompt's own keywords win.
//! - [`Event::Field`] and [`Event::Tab`]: the dynamic input fields.
//! - [`Event::Redo`]: steps the document forward (a waiting command is cancelled first).
//! - [`Event::Undo`]: steps back one prompt answer of the active command, else steps the document's
//!   history.
//!
//! Every command runs through [`arc_command::Invoker`], so the rules of the registry hold for
//! typed, pointer and repeated runs alike.

mod acquire;
mod band;
mod controller;
mod fields;
mod gather;
mod lookup;
mod resolution;

use arc_base::ObjectId;
use arc_doc::Snapshot;
use arc_draft::snap::{ConstraintSource, SnapKind};

pub use acquire::ModifierStep;
pub use controller::Controller;
pub use lookup::{CommandEntry, Help, Suggestion, SuggestionKind};

use crate::precision::points::Modifier;

/// What a prompt accepts, as the window shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// A point.
    Point,
    /// One object.
    Pick,
    /// A set of objects.
    Selection,
    /// A keyword option.
    Keyword,
    /// Text.
    Text,
    /// A distance, angle, number or expression.
    Value,
}

/// The meters that one unit of the document's model space measures. The controller's events and
/// outputs are in meters; a host converts drawing units with this value.
#[must_use]
pub fn meters_per_unit(snapshot: &Snapshot) -> f64 {
    crate::draw::create::meters_per_unit(snapshot)
}

/// The history lines kept when `command.history.length` has its default; older lines are dropped.
pub const MAX_HISTORY: usize = 200;

/// The most tracking paths one resolution reports.
pub const MAX_TRACKING_PATHS: usize = 4;

/// A pointer or keyboard event of the window, in the controller's terms.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The command line was submitted (Enter) with this text, which may be empty.
    Submit(String),
    /// Escape.
    Escape,
    /// The pointer moved to `at` (meters) with a pick aperture of `aperture` (meters, the half side
    /// of the pick box).
    Move {
        /// The pointer position, in meters.
        at: [f64; 2],
        /// The pick aperture, in meters.
        aperture: f64,
    },
    /// The primary button was clicked at `at` (meters) with a pick aperture of `aperture`.
    Click {
        /// The pointer position, in meters.
        at: [f64; 2],
        /// The pick aperture, in meters.
        aperture: f64,
    },
    /// A keyword option of the active prompt was chosen by its identifier.
    Option(String),
    /// The dynamic input field `index` (0 or 1) takes `text`; a non-empty text locks it, an empty
    /// text unlocks it.
    Field {
        /// The field: 0 for the distance or X, 1 for the angle or Y.
        index: usize,
        /// The typed text.
        text: String,
    },
    /// Tab: moves the focus to the other dynamic input field.
    Tab,
    /// Shift-click at `at` (meters) with a pick aperture of `aperture`: the object under the
    /// pointer leaves the selection, or the objects a selection prompt gathers.
    ShiftClick {
        /// The pointer position, in meters.
        at: [f64; 2],
        /// The pick aperture, in meters.
        aperture: f64,
    },
    /// A free-hand outline, from the press to the release, its vertices in meters. A selection
    /// prompt or the selection without a command takes the objects inside or crossing it.
    Lasso {
        /// The vertices in drawing order, in meters.
        path: Vec<[f64; 2]>,
    },
    /// Undo: one prompt answer back in the active command, else one step of the document history.
    Undo,
    /// Redo: one step of the document history forward; a waiting command is cancelled first.
    Redo,
    /// Up: recalls the input line before the one recalled last (the newest first); the text is in
    /// [`Outputs::recalled`].
    RecallPrevious,
    /// Down: recalls the input line after the one recalled last; past the newest, the text is empty.
    RecallNext,
    /// Clears the history lines and the recall position.
    ClearHistory,
}

/// The host's object pick: the object under a point, in drawing units, with an aperture (the
/// half side of the pick box, in drawing units). The topmost object wins.
pub trait ObjectPicker {
    /// The object at `point`, `None` when none lies there.
    fn pick(&self, snapshot: &Snapshot, point: [f64; 2], aperture: f64) -> Option<ObjectId>;
}

/// The picker of the headless tests and the command-line runner: the topmost object whose curves
/// cross the pick box, through the selection methods.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrossingPicker;

impl ObjectPicker for CrossingPicker {
    fn pick(&self, snapshot: &Snapshot, point: [f64; 2], aperture: f64) -> Option<ObjectId> {
        let space = crate::model_space(snapshot)?;
        let corners = [
            arc_base::math::Point2::new(point[0] - aperture, point[1] - aperture),
            arc_base::math::Point2::new(point[0] + aperture, point[1] + aperture),
        ];
        let options = crate::select::candidates::Options::default();
        let pick =
            crate::select::methods::select_crossing(snapshot, space, corners, &options).ok()?;
        pick.ids.last().copied()
    }
}

/// The role of a line of the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRole {
    /// A line the user typed.
    Input,
    /// A command that ran; the text is its identifier.
    Command,
    /// A command that was cancelled; the text is its identifier.
    Cancelled,
    /// An undo step; the text is the identifier of the command it reversed, or empty.
    Undo,
    /// A redo step; the text is the identifier of the command it restored, or empty.
    Redo,
}

/// One line of the command history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryLine {
    /// What the line is.
    pub role: LineRole,
    /// The text: the typed line or a command identifier.
    pub text: String,
}

/// One keyword option of the active prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionOut {
    /// The identifier: what [`Event::Option`] takes.
    pub id: String,
    /// The label's message identifier.
    pub label: String,
    /// The shortcut letter, when the option has one.
    pub shortcut: Option<char>,
}

/// The active prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptOut {
    /// The identifier of the active command.
    pub command: String,
    /// The prompt's key (the parameter it fills).
    pub key: String,
    /// The prompt message's identifier.
    pub message_id: String,
    /// The message's arguments.
    pub args: Vec<(String, String)>,
    /// What the prompt accepts, one entry per accepted input kind.
    pub expects: Vec<Expect>,
    /// The keyword options, in prompt order.
    pub options: Vec<OptionOut>,
    /// The option identifier Enter takes, when the default is a keyword.
    pub default: Option<String>,
}

/// The rubber band: a rubber line of a command, or a window or crossing of a selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandOut {
    /// Where the band starts, in meters.
    pub base: [f64; 2],
    /// Where it ends now, in meters.
    pub current: [f64; 2],
    /// What the band is.
    pub mode: BandMode,
}

/// The mode of a rubber band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandMode {
    /// A rubber line from the last point of a command.
    Rubber,
    /// A window: dragged left to right, the objects fully inside.
    Window,
    /// A crossing: dragged right to left, the objects inside or touching.
    Crossing,
}

/// The object snap marker to show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapOut {
    /// The kind of the snap.
    pub kind: SnapKind,
    /// The snapped point, in meters.
    pub point: [f64; 2],
}

/// An alignment path of tracking, from an acquired point, as a point and a unit direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackOut {
    /// The acquired point the path goes through, in meters.
    pub origin: [f64; 2],
    /// The unit direction of the path.
    pub direction: [f64; 2],
}

/// The mode of the dynamic input fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicMode {
    /// Distance and angle from the last point (the angle in degrees).
    Relative,
    /// X and Y of the point (drawing units).
    Absolute,
}

/// One dynamic input field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldOut {
    /// The value shown: the locked value, else the value at the cursor. Distances and coordinates
    /// are in drawing units, angles in degrees.
    pub value: f64,
    /// Whether typing locked the field.
    pub locked: bool,
    /// Whether the field has the focus.
    pub focused: bool,
}

/// The dynamic input shown next to the cursor while a point prompt waits.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicOut {
    /// The mode.
    pub mode: DynamicMode,
    /// The two fields, in order.
    pub fields: [FieldOut; 2],
}

/// The resolved cursor: the point a pick would give and what decided it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorOut {
    /// The resolved point, in meters.
    pub point: [f64; 2],
    /// What decided it.
    pub source: ConstraintSource,
}

/// An error of an event: its code, its message identifier and the details by name.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorOut {
    /// The error code.
    pub code: &'static str,
    /// The category of the error.
    pub category: arc_base::Category,
    /// The message identifier of the error.
    pub message_id: &'static str,
    /// The arguments of the message by name.
    pub arguments: Vec<(&'static str, arc_base::error::Value)>,
    /// The details by name.
    pub details: Vec<(&'static str, arc_base::error::Value)>,
}

/// Everything a window renders after an event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outputs {
    /// The active prompt, when a command waits.
    pub prompt: Option<PromptOut>,
    /// The command history, oldest first, at most the length of `command.history.length` lines.
    pub history: Vec<HistoryLine>,
    /// The rubber band or the pending window, when one is drawn.
    pub band: Option<BandOut>,
    /// The snap marker, when an object snap decided the cursor.
    pub snap: Option<SnapOut>,
    /// The tracking paths of the cursor.
    pub tracking: Vec<TrackOut>,
    /// The dynamic input, while a point prompt waits.
    pub dynamic: Option<DynamicOut>,
    /// The selection set, in order.
    pub selection: Vec<ObjectId>,
    /// The resolved cursor, for the status display.
    pub cursor: Option<CursorOut>,
    /// The point modifier that waits for a point, when one does.
    pub modifier: Option<ModifierOut>,
    /// The errors of this event.
    pub errors: Vec<ErrorOut>,
    /// The input line a recall event put on the command line; `None` when the event recalled nothing.
    pub recalled: Option<String>,
}

/// A point modifier that waits for a point (DB-SNAP-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierOut {
    /// The modifier.
    pub modifier: Modifier,
    /// The point it waits for.
    pub step: ModifierStep,
}
