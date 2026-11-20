//! The toolbar strip and the status bar of the DraftBuilder window: which commands the buttons
//! start and which precision settings the toggles show and change. The shell draws both; this
//! module only names their items and reads the drawing's settings.

use std::sync::Arc;

use arc_app::shell::{ChoiceState, StatusToggle, ToolItem};
use arc_draft::types::{LAYER, layer};
use arc_ui::{Intent, ViewportDisplay};
use draftbuilder_commands::precision::snaps::suppressed;
use draftbuilder_commands::settings::Value;
use draftbuilder_commands::settings::store::{Values, row_value};
use draftbuilder_commands::view::TOGGLES;
use serde_json::{Map, Value as Json};

/// The settings command the toggles run.
const TOGGLE: &str = "draftbuilder.view.toggle";
/// The command that makes a layer current.
const CURRENT_LAYER: &str = "draftbuilder.settings.current_layer";

const GRID: &str = "draft.grid.on";
const SNAP: &str = "draft.snap.on";
const ORTHO: &str = "draft.ortho";
const POLAR: &str = "draft.polar.on";
const OBJECT_SNAP: &str = "snap.object.running";
const LAYER_CURRENT: &str = "draft.layer.current";

/// The most layers the list shows.
const MAX_LAYERS: usize = 512;

/// A message of the toolbar or the status bar, written literally so the catalog check finds it.
struct Label {
    message_id: &'static str,
}

const DRAWING: &[(Label, &str)] = &[
    (
        Label {
            message_id: "draftbuilder-tool-line",
        },
        "LINE",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-polyline",
        },
        "PLINE",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-circle",
        },
        "CIRCLE",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-arc",
        },
        "ARC",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-rectangle",
        },
        "RECTANG",
    ),
];
const MODIFYING: &[(Label, &str)] = &[
    (
        Label {
            message_id: "draftbuilder-tool-move",
        },
        "MOVE",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-copy",
        },
        "COPY",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-rotate",
        },
        "ROTATE",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-scale",
        },
        "SCALE",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-trim",
        },
        "TRIM",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-extend",
        },
        "EXTEND",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-offset",
        },
        "OFFSET",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-fillet",
        },
        "FILLET",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-erase",
        },
        "ERASE",
    ),
];
const HISTORY: &[(Label, &str)] = &[
    (
        Label {
            message_id: "draftbuilder-tool-undo",
        },
        "UNDO",
    ),
    (
        Label {
            message_id: "draftbuilder-tool-redo",
        },
        "REDO",
    ),
];
const LAYER_HINT: Label = Label {
    message_id: "draftbuilder-tool-layer-hint",
};
const ZOOM_EXTENTS: Label = Label {
    message_id: "draftbuilder-tool-zoom-extents",
};

fn buttons<'a>(group: &'a [(Label, &'a str)]) -> impl Iterator<Item = ToolItem> + 'a {
    group.iter().map(|(label, line)| ToolItem::Button {
        label: label.message_id.to_owned(),
        hint: None,
        line: (*line).to_owned(),
    })
}

/// The toolbar: drawing, modifying and history commands as text buttons that submit the same text
/// a user types, the layer list with the current layer, and zoom to the extents.
#[must_use]
pub fn toolbar() -> Vec<ToolItem> {
    let mut items: Vec<ToolItem> = buttons(DRAWING).collect();
    items.push(ToolItem::Separator);
    items.extend(buttons(MODIFYING));
    items.push(ToolItem::Separator);
    items.extend(buttons(HISTORY));
    items.push(ToolItem::Separator);
    items.push(ToolItem::Choice {
        hint: Some(LAYER_HINT.message_id.into()),
        probe: Arc::new(|app, tab| {
            let Ok(tab) = app.tab(tab) else {
                return ChoiceState::default();
            };
            let snapshot = tab.document().snapshot();
            let mut options: Vec<String> = snapshot
                .iter_type(LAYER)
                .map(|layers| {
                    layers
                        .filter_map(|(_, object)| match object.values.get(layer::NAME)? {
                            Some(arc_doc::Value::String(name)) => Some(name.clone()),
                            _ => None,
                        })
                        .take(MAX_LAYERS)
                        .collect()
                })
                .unwrap_or_default();
            options.sort_by_key(|n| n.to_lowercase());
            let values = Values::of(&snapshot, &|_: &str| None);
            ChoiceState {
                options,
                current: Some(values.text(LAYER_CURRENT)),
            }
        }),
        pick: Arc::new(|name| {
            let mut params = Map::new();
            params.insert("layer".into(), Json::String(name.to_owned()));
            Intent {
                command: CURRENT_LAYER.into(),
                params,
            }
        }),
    });
    items.push(ToolItem::Separator);
    items.push(ToolItem::ZoomExtents {
        label: ZOOM_EXTENTS.message_id.into(),
    });
    items
}

/// The drawing setting `name` as the effective values of the active drawing give it. The grid
/// is shown in a drawing that stores no choice for it (a new drawing): its absence is not "off".
fn flag(
    app: &arc_app::application::Application,
    tab: arc_app::application::TabId,
    name: &str,
) -> Option<bool> {
    let snapshot = app.tab(tab).ok()?.document().snapshot();
    if let Some(on) = switched(tab, name) {
        return Some(on);
    }
    if name == GRID {
        return Some(row_value(&snapshot, GRID).is_none_or(|v| v == Value::Bool(true)));
    }
    Some(Values::of(&snapshot, &|_: &str| None).flag(name))
}

/// What the viewport draws: the axes, and the grid unless the drawing turned it off, at the
/// drawing's grid spacing when it stores one (else a spacing that fits the zoom).
#[must_use]
pub fn display(
    app: &arc_app::application::Application,
    tab: arc_app::application::TabId,
) -> ViewportDisplay {
    let grid = flag(app, tab, GRID).unwrap_or(true);
    let spacing = app.tab(tab).ok().and_then(|t| {
        let snapshot = t.document().snapshot();
        row_value(&snapshot, GRID)?;
        match row_value(&snapshot, "draft.grid.spacing")? {
            Value::Point(p) => Some(p[0]),
            _ => None,
        }
    });
    ViewportDisplay {
        grid,
        axes: true,
        spacing,
    }
}

/// The state the status bar gave the toggle of the setting `name` of `tab`, when it switched it.
fn switched(tab: arc_app::application::TabId, name: &str) -> Option<bool> {
    let (key, _) = TOGGLES.iter().find(|(_, setting)| *setting == name)?;
    draftbuilder_commands::view::switched(&format!("{tab:?}"), key)
}

/// The intent that switches the status bar toggle `key` of `tab`: it changes the open window,
/// never the drawing, so it is no undo step.
fn toggle_intent(tab: arc_app::application::TabId, key: &str, on: bool) -> Intent {
    let mut params = Map::new();
    params.insert("tab".into(), Json::String(format!("{tab:?}")));
    params.insert("name".into(), Json::String(key.to_owned()));
    params.insert("on".into(), Json::Bool(on));
    Intent {
        command: TOGGLE.into(),
        params,
    }
}

const GRID_LABEL: Label = Label {
    message_id: "draftbuilder-status-grid",
};
const GRID_HINT: Label = Label {
    message_id: "draftbuilder-status-grid-hint",
};
const SNAP_LABEL: Label = Label {
    message_id: "draftbuilder-status-snap",
};
const SNAP_HINT: Label = Label {
    message_id: "draftbuilder-status-snap-hint",
};
const ORTHO_LABEL: Label = Label {
    message_id: "draftbuilder-status-ortho",
};
const ORTHO_HINT: Label = Label {
    message_id: "draftbuilder-status-ortho-hint",
};
const POLAR_LABEL: Label = Label {
    message_id: "draftbuilder-status-polar",
};
const POLAR_HINT: Label = Label {
    message_id: "draftbuilder-status-polar-hint",
};
const OSNAP_LABEL: Label = Label {
    message_id: "draftbuilder-status-osnap",
};
const OSNAP_HINT: Label = Label {
    message_id: "draftbuilder-status-osnap-hint",
};

fn toggle(name: &'static str, label: &Label, hint: &Label, key: arc_ui::egui::Key) -> StatusToggle {
    StatusToggle {
        label: label.message_id.to_owned(),
        hint: Some(hint.message_id.to_owned()),
        state: Arc::new(move |app, tab| flag(app, tab, name)),
        set: Arc::new(move |_, tab, on| {
            let (key, _) = TOGGLES.iter().find(|(_, setting)| *setting == name)?;
            Some(toggle_intent(tab, key, on))
        }),
        key: Some(key),
    }
}

/// The status bar toggles: grid, snap, ortho and polar tracking are boolean drawing settings;
/// the object snap is the bit of the running object snap setting that suppresses the running
/// set, so a toggle off leaves the chosen set in place.
#[must_use]
pub fn status() -> Vec<StatusToggle> {
    vec![
        toggle(GRID, &GRID_LABEL, &GRID_HINT, arc_ui::egui::Key::F7),
        toggle(SNAP, &SNAP_LABEL, &SNAP_HINT, arc_ui::egui::Key::F9),
        toggle(ORTHO, &ORTHO_LABEL, &ORTHO_HINT, arc_ui::egui::Key::F8),
        toggle(POLAR, &POLAR_LABEL, &POLAR_HINT, arc_ui::egui::Key::F10),
        StatusToggle {
            label: OSNAP_LABEL.message_id.into(),
            hint: Some(OSNAP_HINT.message_id.into()),
            state: Arc::new(|app, tab| {
                if let Some(on) = switched(tab, OBJECT_SNAP) {
                    return Some(on);
                }
                let snapshot = app.tab(tab).ok()?.document().snapshot();
                let values = Values::of(&snapshot, &|_: &str| None);
                Some(!suppressed(values.int(OBJECT_SNAP)))
            }),
            set: Arc::new(|_, tab, on| Some(toggle_intent(tab, "osnap", on))),
            key: Some(arc_ui::egui::Key::F3),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use draftbuilder_commands::vocabulary::{NAMES, history_words};

    #[test]
    fn every_toolbar_button_starts_a_typed_command_and_every_label_is_in_both_catalogs() {
        let mut ids: Vec<String> = Vec::new();
        for item in toolbar() {
            match item {
                ToolItem::Button { label, line, .. } => {
                    let known = NAMES.iter().any(|n| n.typed == Some(line.as_str()))
                        || [history_words::UNDO, history_words::REDO].contains(&line.as_str());
                    assert!(known, "{line} is not a typed command");
                    ids.push(label);
                }
                ToolItem::ZoomExtents { label } => ids.push(label),
                ToolItem::Choice { hint, .. } => ids.extend(hint),
                ToolItem::Separator => {}
            }
        }
        for toggle in status() {
            ids.push(toggle.label);
            ids.extend(toggle.hint);
        }
        for language in ["en-US", "tr-TR"] {
            let localizer = crate::localizer(&[language.to_owned()]).unwrap();
            for id in &ids {
                let text = localizer.format(id, &[]);
                assert_eq!(
                    text.locale.as_ref().map(ToString::to_string).as_deref(),
                    Some(language),
                    "{id} in {language}"
                );
            }
        }
    }
}
