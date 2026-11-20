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
    /// Object properties: changing and matching the appearance of entities.
    Property,
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
        id: "draftbuilder.modify.move",
        area: Area::Modify,
        typed: Some("MOVE"),
        aliases: &["M"],
    },
    Name {
        id: "draftbuilder.modify.copy",
        area: Area::Modify,
        typed: Some("COPY"),
        aliases: &["CO"],
    },
    Name {
        id: "draftbuilder.modify.rotate",
        area: Area::Modify,
        typed: Some("ROTATE"),
        aliases: &["RO"],
    },
    Name {
        id: "draftbuilder.modify.scale",
        area: Area::Modify,
        typed: Some("SCALE"),
        aliases: &["SC"],
    },
    Name {
        id: "draftbuilder.modify.mirror",
        area: Area::Modify,
        typed: Some("MIRROR"),
        aliases: &["MI"],
    },
    Name {
        id: "draftbuilder.modify.align",
        area: Area::Modify,
        typed: Some("ALIGN"),
        aliases: &["AL"],
    },
    Name {
        id: "draftbuilder.modify.erase",
        area: Area::Modify,
        typed: Some("ERASE"),
        aliases: &["E"],
    },
    Name {
        id: "draftbuilder.modify.explode",
        area: Area::Modify,
        typed: Some("EXPLODE"),
        aliases: &["X"],
    },
    Name {
        id: "draftbuilder.modify.stretch",
        area: Area::Modify,
        typed: Some("STRETCH"),
        aliases: &["S"],
    },
    Name {
        id: "draftbuilder.modify.trim",
        area: Area::Modify,
        typed: Some("TRIM"),
        aliases: &["TR"],
    },
    Name {
        id: "draftbuilder.modify.extend",
        area: Area::Modify,
        typed: Some("EXTEND"),
        aliases: &["EX"],
    },
    Name {
        id: "draftbuilder.modify.offset",
        area: Area::Modify,
        typed: Some("OFFSET"),
        aliases: &["O"],
    },
    Name {
        id: "draftbuilder.modify.fillet",
        area: Area::Modify,
        typed: Some("FILLET"),
        aliases: &["F"],
    },
    Name {
        id: "draftbuilder.modify.chamfer",
        area: Area::Modify,
        typed: Some("CHAMFER"),
        aliases: &["CHA"],
    },
    Name {
        id: "draftbuilder.modify.break",
        area: Area::Modify,
        typed: Some("BREAK"),
        aliases: &["BR"],
    },
    Name {
        id: "draftbuilder.modify.join",
        area: Area::Modify,
        typed: Some("JOIN"),
        aliases: &["J"],
    },
    Name {
        id: "draftbuilder.modify.array_rect",
        area: Area::Modify,
        typed: Some("ARRAYRECT"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.array_polar",
        area: Area::Modify,
        typed: Some("ARRAYPOLAR"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.array_path",
        area: Area::Modify,
        typed: Some("ARRAYPATH"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.lengthen",
        area: Area::Modify,
        typed: Some("LENGTHEN"),
        aliases: &["LEN"],
    },
    Name {
        id: "draftbuilder.draw.line",
        area: Area::Draw,
        typed: Some("LINE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.xline",
        area: Area::Draw,
        typed: Some("XLINE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.ray",
        area: Area::Draw,
        typed: Some("RAY"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.polyline",
        area: Area::Draw,
        typed: Some("PLINE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.rectangle",
        area: Area::Draw,
        typed: Some("RECTANG"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.polygon",
        area: Area::Draw,
        typed: Some("POLYGON"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.circle",
        area: Area::Draw,
        typed: Some("CIRCLE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.arc",
        area: Area::Draw,
        typed: Some("ARC"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.ellipse",
        area: Area::Draw,
        typed: Some("ELLIPSE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.donut",
        area: Area::Draw,
        typed: Some("DONUT"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.spline",
        area: Area::Draw,
        typed: Some("SPLINE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.point",
        area: Area::Draw,
        typed: Some("POINT"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.select.filter",
        area: Area::Select,
        typed: Some("FILTER"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.create",
        area: Area::Layer,
        typed: Some("LAYER"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.on",
        area: Area::Layer,
        typed: Some("LAYON"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.off",
        area: Area::Layer,
        typed: Some("LAYOFF"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.freeze",
        area: Area::Layer,
        typed: Some("LAYFRZ"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.thaw",
        area: Area::Layer,
        typed: Some("LAYTHW"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.lock",
        area: Area::Layer,
        typed: Some("LAYLCK"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.unlock",
        area: Area::Layer,
        typed: Some("LAYULK"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.match",
        area: Area::Layer,
        typed: Some("LAYMCH"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.delete_with_contents",
        area: Area::Layer,
        typed: Some("LAYDEL"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.merge",
        area: Area::Layer,
        typed: Some("LAYMRG"),
        aliases: &[],
    },
    Name {
        id: "core.draft.linetype.load",
        area: Area::Layer,
        typed: Some("LINETYPE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.property.change",
        area: Area::Property,
        typed: Some("CHPROP"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.property.change",
        area: Area::Property,
        typed: Some("CHANGE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.property.match",
        area: Area::Property,
        typed: Some("MATCHPROP"),
        aliases: &[],
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
