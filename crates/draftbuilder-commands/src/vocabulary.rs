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
        id: "draftbuilder.modify.reverse",
        area: Area::Modify,
        typed: Some("REVERSE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.pedit",
        area: Area::Modify,
        typed: Some("PEDIT"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.change",
        area: Area::Modify,
        typed: None,
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.flatten",
        area: Area::Modify,
        typed: Some("FLATTEN"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.draw_order",
        area: Area::Modify,
        typed: Some("DRAWORDER"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.modify.overkill",
        area: Area::Modify,
        typed: Some("OVERKILL"),
        aliases: &[],
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
        aliases: &["PL"],
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
        aliases: &["C"],
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
        id: "draftbuilder.draw.divide",
        area: Area::Draw,
        typed: Some("DIVIDE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.measure",
        area: Area::Draw,
        typed: Some("MEASURE"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.draw.wipeout",
        area: Area::Draw,
        typed: Some("WIPEOUT"),
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
        id: "draftbuilder.layer.previous",
        area: Area::Layer,
        typed: Some("LAYERP"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.isolate",
        area: Area::Layer,
        typed: Some("LAYISO"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.unisolate",
        area: Area::Layer,
        typed: Some("LAYUNISO"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.walk",
        area: Area::Layer,
        typed: Some("LAYWALK"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.walk_end",
        area: Area::Layer,
        typed: None,
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.viewport_isolate",
        area: Area::Layer,
        typed: Some("LAYVPI"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.viewport_freeze",
        area: Area::Layer,
        typed: Some("VPLAYER"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.viewport_override",
        area: Area::Layer,
        typed: None,
        aliases: &[],
    },
    Name {
        id: "draftbuilder.layer.viewport_clear",
        area: Area::Layer,
        typed: None,
        aliases: &[],
    },
    Name {
        id: "draftbuilder.property.summary",
        area: Area::Property,
        typed: Some("PROPERTIES"),
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
    Name {
        id: "draftbuilder.settings.setvar",
        area: Area::Settings,
        typed: Some("SETVAR"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.settings.current_layer",
        area: Area::Settings,
        typed: Some("CLAYER"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.precision.osnap",
        area: Area::Precision,
        typed: Some("OSNAP"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.precision.ucs",
        area: Area::Precision,
        typed: Some("UCS"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.inquiry.distance",
        area: Area::Precision,
        typed: Some("DIST"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.inquiry.id",
        area: Area::Precision,
        typed: Some("ID"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.inquiry.area",
        area: Area::Precision,
        typed: Some("AREA"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.precision.units",
        area: Area::Precision,
        typed: Some("UNITS"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.inquiry.list",
        area: Area::Precision,
        typed: Some("LIST"),
        aliases: &[],
    },
    Name {
        id: "draftbuilder.inquiry.mass",
        area: Area::Precision,
        typed: Some("MASSPROP"),
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

/// The typed words of the history directives (DB-DBASE-038, DB-DBASE-039). They are not commands:
/// a history step runs on the document's undo stack, outside any transaction, so they are read
/// before a command line (see [`crate::undo`]).
pub mod history_words {
    /// Undo, with an optional count, or a mark, back-to-mark or group form.
    pub const UNDO: &str = "UNDO";
    /// The short form of [`UNDO`].
    pub const UNDO_SHORT: &str = "U";
    /// Redo, with an optional count.
    pub const REDO: &str = "REDO";
    /// Restores the objects removed by the most recent erase.
    pub const OOPS: &str = "OOPS";
    /// After [`UNDO`]: sets a named mark at the top of the undo stack.
    pub const MARK: &str = "MARK";
    /// After [`UNDO`]: undoes back to a named mark.
    pub const BACK: &str = "BACK";
    /// After [`UNDO`]: opens a named group whose later commands form one undo step.
    pub const GROUP: &str = "GROUP";
    /// After [`UNDO`]: closes the innermost open group.
    pub const ENDGROUP: &str = "ENDGROUP";
}

/// One keyword a selection prompt takes: its identifier, which is also what the typed word names,
/// and the message identifier of its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionOption {
    /// The identifier, typed in lower or upper case.
    pub id: &'static str,
    /// The label's message identifier.
    pub message_id: &'static str,
}

/// The keywords of a selection prompt, in the order the prompt offers them (DB-SEL-005, 003, 007,
/// 010). `window`, `crossing`, `wpolygon`, `cpolygon` and `fence` define the shape of the next
/// gestures; `add`, `remove`, `multiple`, `single` and `undo` change how gestures are gathered.
pub const SELECTION_OPTIONS: &[SelectionOption] = &[
    SelectionOption {
        id: "all",
        message_id: "db-sel-option-all",
    },
    SelectionOption {
        id: "last",
        message_id: "db-sel-option-last",
    },
    SelectionOption {
        id: "previous",
        message_id: "db-sel-option-previous",
    },
    SelectionOption {
        id: "window",
        message_id: "db-sel-option-window",
    },
    SelectionOption {
        id: "crossing",
        message_id: "db-sel-option-crossing",
    },
    SelectionOption {
        id: "wpolygon",
        message_id: "db-sel-option-wpolygon",
    },
    SelectionOption {
        id: "cpolygon",
        message_id: "db-sel-option-cpolygon",
    },
    SelectionOption {
        id: "fence",
        message_id: "db-sel-option-fence",
    },
    SelectionOption {
        id: "add",
        message_id: "db-sel-option-add",
    },
    SelectionOption {
        id: "remove",
        message_id: "db-sel-option-remove",
    },
    SelectionOption {
        id: "multiple",
        message_id: "db-sel-option-multiple",
    },
    SelectionOption {
        id: "single",
        message_id: "db-sel-option-single",
    },
    SelectionOption {
        id: "undo",
        message_id: "db-sel-option-undo",
    },
];
