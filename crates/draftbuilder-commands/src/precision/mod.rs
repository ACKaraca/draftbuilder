//! Precision (area `precision`): the object snap table and the snap overrides, the resolution of a
//! pick under the running snap set, ortho, polar and the snap step ([`resolve`]), point filters
//! ([`snaps`]), the inquiry commands with their exact results in the drawing's display formats
//! ([`inquiry`]), and the planar user coordinate systems ([`ucs`]).

pub mod inquiry;
pub mod resolve;
pub mod snaps;
pub mod ucs;

use arc_base::Result;
use arc_command::Registry;

use crate::vocabulary::{Area, register_names};

/// The error codes of the precision area.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// No coordinate system has the name given.
    pub const UCS_UNKNOWN: ErrorCode = ErrorCode {
        code: "db.cmd.ucs_unknown",
        category: Category::NotFound,
        message_id: "db-cmd-ucs-unknown",
    };
    /// A coordinate system name is empty, too long or contains a wildcard or a control character.
    pub const UCS_NAME: ErrorCode = ErrorCode {
        code: "db.cmd.ucs_name",
        category: Category::InvalidInput,
        message_id: "db-cmd-ucs-name",
    };
    /// A snap override or running-set code is not one of the codes of the table.
    pub const SNAP_UNKNOWN: ErrorCode = ErrorCode {
        code: "db.cmd.snap_unknown",
        category: Category::InvalidInput,
        message_id: "db-cmd-snap-unknown",
    };
    /// An area or a length command takes a closed object or a closed set of points and the object
    /// given is open.
    pub const NOT_CLOSED: ErrorCode = ErrorCode {
        code: "db.cmd.not_closed",
        category: Category::InvalidInput,
        message_id: "db-cmd-not-closed",
    };
    /// An area command takes fewer than three points.
    pub const POINTS_TOO_FEW: ErrorCode = ErrorCode {
        code: "db.cmd.points_too_few",
        category: Category::InvalidInput,
        message_id: "db-cmd-points-too-few",
    };
    /// The object has no curve geometry the command measures.
    pub const NO_CURVE: ErrorCode = ErrorCode {
        code: "db.cmd.no_curve",
        category: Category::InvalidInput,
        message_id: "db-cmd-no-curve",
    };
    /// Every code of this module, for the registry check.
    pub const ALL: &[ErrorCode] = &[
        UCS_UNKNOWN,
        UCS_NAME,
        SNAP_UNKNOWN,
        NOT_CLOSED,
        POINTS_TOO_FEW,
        NO_CURVE,
    ];
}

/// Registers the commands of the area and applies its names.
///
/// # Errors
///
/// `cmd.duplicate` when an identifier is already registered, and the errors of
/// [`register_names`].
pub fn register(registry: &mut Registry) -> Result<()> {
    resolve::register(registry)?;
    inquiry::register(registry)?;
    ucs::register(registry)?;
    register_names(registry, Area::Precision)
}

#[cfg(test)]
mod tests {
    use super::codes;

    const CATALOG: &str = include_str!("../../../../locales/en-US/draftbuilder-commands.ftl");

    #[test]
    fn the_precision_codes_are_well_formed_and_every_message_is_defined() {
        assert_eq!(arc_base::error::registry_problems(codes::ALL), vec![]);
        for code in codes::ALL {
            assert!(
                CATALOG.contains(&format!("{} =", code.message_id)),
                "{}",
                code.message_id
            );
        }
    }
}
