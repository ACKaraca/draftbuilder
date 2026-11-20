//! ZOOM (`draftbuilder.view.zoom`, typed `ZOOM`, alias `Z`): changes the view, never the document.
//! The command writes its request as the session setting [`REQUEST`]; the interaction controller
//! takes it after the event and the window applies it to the camera. A zoom is neither a document
//! change nor an undo step.
//!
//! The forms: `Extents`, `Previous`, `Window` (two corners, or a first point picked at once) and a
//! number, the scale factor (above one enlarges, below one reduces).

use std::collections::BTreeMap;

use arc_base::{Dimension, Error, Result};
use arc_command::{
    Accept, CommandSpec, Context, KeywordOption, Permission, Prompt, PromptInput, PromptState,
    Registry, Step, UndoBehavior,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::draw::create::{done, drawing_point, machine};
use crate::error::{Message, codes};
use crate::vocabulary::{Area, register_names};

/// The session setting that carries the request from the command to the controller.
pub const REQUEST: &str = "draftbuilder.view.request";

/// The status bar toggles: the short name and the drawing setting each one shadows.
pub const TOGGLES: &[(&str, &str)] = &[
    ("grid", "draft.grid.on"),
    ("snap", "draft.snap.on"),
    ("ortho", "draft.ortho"),
    ("polar", "draft.polar.on"),
    ("osnap", "snap.object.running"),
];

/// The session setting that carries the toggle `name` to the controller.
#[must_use]
pub fn override_key(name: &str) -> String {
    format!("draftbuilder.override.{name}")
}

/// The toggles the window's status bar switched, per tab (its debug name) and toggle. They are
/// state of the open window, not of the drawing: switching one writes no row, journal record or
/// undo step.
static SWITCHED: std::sync::Mutex<BTreeMap<(String, String), bool>> =
    std::sync::Mutex::new(BTreeMap::new());

/// The state the status bar gave the toggle `name` of `tab`, when it switched it.
#[must_use]
pub fn switched(tab: &str, name: &str) -> Option<bool> {
    let map = SWITCHED.lock().ok()?;
    map.get(&(tab.to_owned(), name.to_owned())).copied()
}

/// What a zoom asks of the view.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ZoomMode {
    /// Frame the whole drawing.
    Extents,
    /// Return to the view before the last zoom.
    Previous,
    /// Frame the rectangle between two corners.
    Window,
    /// Scale the view about its center.
    Scale,
}

/// Parameters of `draftbuilder.view.zoom`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomParams {
    /// The form of the zoom.
    pub mode: ZoomMode,
    /// The two corners of a window, in drawing units.
    #[serde(default)]
    pub corners: Option<[[f64; 2]; 2]>,
    /// The scale factor, above zero.
    #[serde(default)]
    pub factor: Option<f64>,
}

/// The result of a zoom: the request handed to the window.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Zoomed {
    /// The form of the zoom.
    pub mode: ZoomMode,
}

/// Parameters of `draftbuilder.view.toggle`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToggleParams {
    /// The tab, as the window names it.
    pub tab: String,
    /// The toggle: `grid`, `snap`, `ortho`, `polar` or `osnap`.
    pub name: String,
    /// The new state.
    pub on: bool,
}

/// The result of a toggle.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Toggled {
    /// The new state.
    pub on: bool,
}

fn toggle(_: &mut Context<'_>, p: ToggleParams) -> Result<Toggled> {
    if !TOGGLES.iter().any(|(name, _)| *name == p.name) || p.tab.len() > 64 {
        return Err(Error::new(codes::VALUE_INVALID).with_detail("field", "name"));
    }
    let mut map = SWITCHED
        .lock()
        .map_err(|_| Error::new(codes::VALUE_INVALID).with_detail("field", "tab"))?;
    // A window has a handful of tabs and five toggles each.
    if map.len() >= 1024 {
        map.clear();
    }
    map.insert((p.tab, p.name), p.on);
    Ok(Toggled { on: p.on })
}

const LABEL: Message = Message {
    message_id: "db-cmd-view-zoom-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-view-zoom-description",
};
const TOGGLE_LABEL: Message = Message {
    message_id: "db-cmd-view-toggle-label",
};
const TOGGLE_DESCRIPTION: Message = Message {
    message_id: "db-cmd-view-toggle-description",
};
const PROMPT: Message = Message {
    message_id: "db-cmd-view-zoom-prompt",
};
const CORNER: Message = Message {
    message_id: "db-cmd-view-zoom-corner",
};
const OTHER_CORNER: Message = Message {
    message_id: "db-cmd-view-zoom-other-corner",
};
const OPTION_EXTENTS: Message = Message {
    message_id: "db-cmd-view-option-extents",
};
const OPTION_PREVIOUS: Message = Message {
    message_id: "db-cmd-view-option-previous",
};
const OPTION_WINDOW: Message = Message {
    message_id: "db-cmd-view-option-window",
};

fn run(ctx: &mut Context<'_>, p: ZoomParams) -> Result<Zoomed> {
    let bad = |field: &'static str| Error::new(codes::VALUE_INVALID).with_detail("field", field);
    match p.mode {
        ZoomMode::Window => {
            let ok = p.corners.is_some_and(|c| {
                c.iter().flatten().all(|v| v.is_finite())
                    && c[0][0] != c[1][0]
                    && c[0][1] != c[1][1]
            });
            if !ok {
                return Err(bad("corners"));
            }
        }
        ZoomMode::Scale if !p.factor.is_some_and(|f| f.is_finite() && f > 0.0) => {
            return Err(bad("factor"));
        }
        _ => {}
    }
    let request = serde_json::json!({"mode": p.mode, "corners": p.corners, "factor": p.factor});
    ctx.set_setting(REQUEST, request)?;
    Ok(Zoomed { mode: p.mode })
}

fn corner(state: &PromptState<'_>, key: &str) -> Option<[f64; 2]> {
    match state.answer(key)? {
        PromptInput::Point(p) => {
            let q = drawing_point(state.snapshot, *p);
            Some([q.x, q.y])
        }
        _ => None,
    }
}

fn point() -> Accept {
    Accept::Point {
        dims: arc_command::PointDims::Two,
    }
}

fn ask(state: &PromptState<'_>) -> Result<Step> {
    let window = |first: [f64; 2]| match corner(state, "corner2") {
        Some(second) => done(serde_json::json!({"mode": "window", "corners": [first, second]})),
        None => Ok(Step::Ask(Prompt::new(
            "corner2",
            OTHER_CORNER.message_id,
            point(),
        ))),
    };
    match state.answer("option") {
        None => Ok(Step::Ask(
            Prompt::new(
                "option",
                PROMPT.message_id,
                Accept::keywords([
                    KeywordOption::new("extents", OPTION_EXTENTS.message_id).with_shortcut('e'),
                    KeywordOption::new("previous", OPTION_PREVIOUS.message_id).with_shortcut('p'),
                    KeywordOption::new("window", OPTION_WINDOW.message_id).with_shortcut('w'),
                ]),
            )
            .or(Accept::number(Dimension::NONE))
            .or(point())
            .with_default(PromptInput::Keyword("extents".to_owned())),
        )),
        Some(PromptInput::Keyword(k)) if k == "extents" || k == "previous" => {
            done(serde_json::json!({"mode": k}))
        }
        Some(PromptInput::Keyword(_)) => match corner(state, "corner1") {
            Some(first) => window(first),
            None => Ok(Step::Ask(Prompt::new(
                "corner1",
                CORNER.message_id,
                point(),
            ))),
        },
        Some(PromptInput::Number(q)) => {
            done(serde_json::json!({"mode": "scale", "factor": q.value()}))
        }
        Some(PromptInput::Point(p)) => {
            let q = drawing_point(state.snapshot, *p);
            window([q.x, q.y])
        }
        Some(_) => Err(Error::new(codes::VALUE_INVALID).with_detail("field", "option")),
    }
}

/// Registers ZOOM and its names.
///
/// # Errors
///
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            "draftbuilder.view.zoom",
            LABEL.message_id,
            DESCRIPTION.message_id,
            run,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read])
        .with_prompt(machine(ask, crate::draw::create::accept_all)),
    )?;
    registry.register(
        CommandSpec::new(
            "draftbuilder.view.toggle",
            TOGGLE_LABEL.message_id,
            TOGGLE_DESCRIPTION.message_id,
            toggle,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )?;
    register_names(registry, Area::View)
}
