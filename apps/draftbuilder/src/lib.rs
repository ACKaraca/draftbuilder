//! The DraftBuilder desktop application (wiring only): the DraftBuilder type set and commands
//! in the windowed shell of `arc-app`, with the viewport drawing the model space through the
//! drafting extractor of `arc-gpu`. DXF is a native format of the application: a `.dxf` file
//! opens and saves through the application's own open and save path. The binary builds this
//! product and starts the event loop; tests drive the same product headless.

use std::sync::Arc;

use arc_app::product::{Entry, Extractor, FileType, NativeFormat, Product, ProductBuilder};
use arc_app::shell::{
    self, CommandLinePanel, OutlinerPanel, PropertiesPanel, ShellConfig, ViewportPanel,
};
use arc_base::{Error, Result};
use arc_command::Clock;
use arc_doc::package::LockOwner;
use arc_draft::types::ENTITIES;
use arc_gpu::draft::{DraftOptions, extract_entity};
use arc_gpu::extract;
use arc_ui::{Axis, Binding, Dock, DockLayout};

pub mod chrome;
pub mod interaction;
mod preview;

/// The viewport panel.
pub const VIEWPORT: &str = "draftbuilder.viewport";
/// The property grid.
pub const PROPERTIES: &str = "draftbuilder.properties";
/// The outliner.
pub const OUTLINER: &str = "draftbuilder.outliner";
/// The command line.
pub const COMMAND_LINE: &str = "draftbuilder.command_line";

/// The command the property grid's edits name.
pub const EDIT_COMMAND: &str = "core.draft.entity.modify";

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const TITLE: Message = Message {
    message_id: "draftbuilder-title",
};

fn entry(id: &str, label: Message, make: fn() -> Box<dyn shell::Panel>) -> Entry {
    Entry {
        id: id.into(),
        message_id: label.message_id.into(),
        factory: Arc::new(make),
    }
}

/// The product of the application: the type set and commands of `draftbuilder-commands`, the
/// shell commands and settings, the `draftb` package and the native `dxf` file type, four panels and the drafting extractor.
///
/// # Errors
/// The registries' errors; none occur for this set.
pub fn product(clock: Arc<dyn Clock>, owner: LockOwner) -> Result<Product> {
    let mut builder = ProductBuilder::new("draftbuilder", env!("CARGO_PKG_VERSION"))
        .with_types(draftbuilder_commands::types()?)
        .with_commands(draftbuilder_commands::registry()?);
    arc_app::commands::register(builder.commands_mut())?;
    builder.last_objects(draftbuilder_commands::last_objects);
    builder.new_document_command(draftbuilder_commands::new_drawing::ID);
    let messages = localizer(&os_locales()).ok().map(Arc::new);
    builder.interaction(move || Box::new(interaction::DrawingInteraction::new(messages.clone())));
    shell::prefs::register_settings(&mut builder)?;
    builder.package_extension("draftb");
    builder.file_type(FileType {
        extension: "draftb".into(),
        message_id: "draftbuilder-file-draftb".into(),
        open: true,
        save: true,
    })?;
    builder.file_type(FileType {
        extension: "dxf".into(),
        message_id: "draftbuilder-file-dxf".into(),
        open: true,
        save: true,
    })?;
    builder.opener(
        "dxf",
        NativeFormat::new(
            draftbuilder_commands::open_document,
            draftbuilder_commands::save_document,
        ),
    )?;
    let panels = [
        entry(
            VIEWPORT,
            Message {
                message_id: "draftbuilder-panel-viewport",
            },
            || shell::panel(ViewportPanel::new()),
        ),
        entry(
            PROPERTIES,
            Message {
                message_id: "draftbuilder-panel-properties",
            },
            || shell::panel(PropertiesPanel::new(EDIT_COMMAND)),
        ),
        entry(
            OUTLINER,
            Message {
                message_id: "draftbuilder-panel-outliner",
            },
            || shell::panel(OutlinerPanel::default()),
        ),
        entry(
            COMMAND_LINE,
            Message {
                message_id: "draftbuilder-panel-command-line",
            },
            || shell::panel(CommandLinePanel),
        ),
    ];
    for panel in panels {
        builder.panel(panel)?;
    }
    let options = DraftOptions::default();
    builder.extractor(Extractor {
        id: "draftbuilder.draft".into(),
        types: ENTITIES.iter().map(|k| (*k).to_owned()).collect(),
        factory: Arc::new(move || -> extract::Extractor {
            let options = options.clone();
            shell::extractor(move |snapshot, object, _| {
                extract_entity(snapshot, object.id, &options).map(|d| d.extraction)
            })
        }),
    })?;
    builder.build(clock, owner)
}

fn tabs(panels: &[&str]) -> Dock {
    Dock::Tabs {
        tabs: panels.iter().map(|p| (*p).to_owned()).collect(),
        active: 0,
    }
}

/// The shell configuration: the viewport in the middle, the outliner and the property grid on
/// the right, the command line below; the default menu bar; and key bindings for the file
/// commands. `seed` seeds the invocations (see [`ShellConfig::seed`]).
///
/// # Errors
/// None for this layout and menu; their validation errors otherwise.
pub fn config(seed: u64) -> Result<ShellConfig> {
    let right = Dock::Split {
        axis: Axis::Vertical,
        fraction: 0.4,
        first: Box::new(tabs(&[OUTLINER])),
        second: Box::new(tabs(&[PROPERTIES])),
    };
    let top = Dock::Split {
        axis: Axis::Horizontal,
        fraction: 0.7,
        first: Box::new(tabs(&[VIEWPORT])),
        second: Box::new(right),
    };
    let root = Dock::Split {
        axis: Axis::Vertical,
        fraction: 0.7,
        first: Box::new(top),
        second: Box::new(tabs(&[COMMAND_LINE])),
    };
    let binding = |keys: &str, command: &str| Binding {
        keys: keys.into(),
        command: command.into(),
    };
    Ok(ShellConfig {
        layout: DockLayout::new(root, Vec::new())?,
        menu: shell::default_menu()?,
        bindings: vec![
            binding("Ctrl+N", arc_app::commands::FILE_NEW),
            binding("Ctrl+O", arc_app::commands::FILE_OPEN),
            binding("Ctrl+S", arc_app::commands::FILE_SAVE),
            binding("Ctrl+Shift+S", arc_app::commands::FILE_SAVE_AS),
            binding("Ctrl+W", arc_app::commands::FILE_CLOSE),
            binding("Ctrl+Q", arc_app::commands::APP_QUIT),
        ],
        // Dark, as drafting applications are drawn; a user's choice overrides it.
        appearance: arc_ui::Appearance {
            theme: arc_ui::ThemeId::Dark,
            ..arc_ui::Appearance::default()
        },
        display: Some(arc_app::shell::DisplayProbe(Arc::new(chrome::display))),
        overlays: None,
        work_plane: None,
        initial_view: None,
        toolbar: chrome::toolbar(),
        status: chrome::status(),
        seed,
    })
}

/// The languages the operating system asks for, in the order the environment names them.
#[must_use]
pub fn os_locales() -> Vec<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|n| std::env::var(n).ok())
        .collect()
}

/// The message catalogs the application formats with, for the requested languages: the shell's
/// ([`arc_app::locale::shell_catalogs`]) with the application's and the crates' English
/// messages appended.
///
/// # Errors
/// `app.locale.catalog_invalid` when a catalog does not parse.
pub fn localizer(requested: &[String]) -> Result<arc_app::locale::Localizer> {
    let mut catalogs = arc_app::locale::shell_catalogs()?;
    let english = [
        include_str!("../../../locales/en-US/draftbuilder.ftl"),
        include_str!("../../../locales/en-US/draftbuilder-commands.ftl"),
        include_str!("../../../locales/en-US/draftbuilder-io.ftl"),
    ]
    .concat();
    catalogs.append("en-US", &english)?;
    catalogs.append(
        "tr-TR",
        include_str!("../../../locales/tr-TR/draftbuilder.ftl"),
    )?;
    arc_app::locale::Localizer::new(catalogs, requested)
}

/// The window title.
#[must_use]
pub fn title(localizer: &arc_app::locale::Localizer) -> String {
    localizer.format(TITLE.message_id, &[]).text
}

/// The error of a start-up step, for the report on standard error.
#[must_use]
pub fn describe(error: &Error, localizer: Option<&arc_app::locale::Localizer>) -> String {
    match localizer {
        Some(l) => format!("{}: {}", error.code(), l.format_error(error).text),
        None => error.code().to_owned(),
    }
}
