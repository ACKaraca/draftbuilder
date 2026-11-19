//! The product command vocabulary: the one table of DraftBuilder's conventional typed names and
//! short aliases, and the areas that register the commands they name.
//!
//! A command has a stable identifier `draftbuilder.<area>.<name>` (or a core identifier it
//! reuses). Its conventional typed name is an English word in upper case; the registry takes the
//! lower-case form as a user alias, so the typed name resolves through arc-command's alias
//! mechanism. The table names only commands that are registered; [`register_names`] fails with
//! `cmd.unknown` for a row whose identifier is missing, so a bad row cannot pass the tests.
//!
//! Each area module registers its commands and applies its rows with [`register_names`]. A later
//! command adds its row here and its registration in its area module; no other shared file
//! changes.

use arc_base::Result;
use arc_command::Registry;

/// The module that registers a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// Opening, creating, saving, auditing and purging drawings.
    Drawing,
    /// Drawing entities.
    Draw,
    /// Changing entities.
    Modify,
    /// Selecting entities.
    Select,
    /// Layers, linetypes and layer states.
    Layer,
    /// Settings and profiles.
    Settings,
    /// Coordinate input, snaps and tracking.
    Precision,
}

/// One row of the vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct Name {
    /// The command identifier.
    pub id: &'static str,
    /// The area that registers the command and applies this row.
    pub area: Area,
    /// The conventional typed name, in upper case.
    pub typed: Option<&'static str>,
    /// Short aliases of the command, in upper case.
    pub aliases: &'static [&'static str],
}

/// Every name of the product: the conventional typed names and short aliases.
pub const NAMES: &[Name] = &[
    Name {
        id: "draftbuilder.drawing.open",
        area: Area::Drawing,
        typed: Some("OPEN"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.drawing.new",
        area: Area::Drawing,
        typed: Some("NEW"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.drawing.save",
        area: Area::Drawing,
        typed: Some("SAVE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.drawing.audit",
        area: Area::Drawing,
        typed: Some("AUDIT"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.drawing.purge",
        area: Area::Drawing,
        typed: Some("PURGE"),
        aliases: &[],
    },
    Name {
        id: "core.draft.entity.transform",
        area: Area::Modify,
        typed: Some("MOVE"),
        aliases: &["M"],
    },
];

/// Applies the rows of `area` to `registry`: each typed name and alias becomes a user alias of
/// its command.
///
/// # Errors
/// `cmd.unknown` when a row names an identifier that is not registered, and `cmd.duplicate`
/// when a name is taken.
pub fn register_names(registry: &mut Registry, area: Area) -> Result<()> {
    for row in NAMES.iter().filter(|row| row.area == area) {
        for name in row.typed.iter().chain(row.aliases.iter()) {
            registry.add_user_alias(&name.to_ascii_lowercase(), row.id)?;
        }
    }
    Ok(())
}
