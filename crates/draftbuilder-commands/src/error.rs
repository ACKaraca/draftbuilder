//! The error codes owned by `draftbuilder-commands`. Messages are in
//! `locales/en-US/draftbuilder-commands.ftl`.

/// The error codes owned by `draftbuilder-commands`.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// A drawing is opened into a document that already holds a drawing.
    pub const DRAWING_NOT_EMPTY: ErrorCode = ErrorCode {
        code: "db.cmd.drawing_not_empty",
        category: Category::Conflict,
        message_id: "db-cmd-drawing-not-empty",
    };
    /// The document has no model space.
    pub const NO_MODEL_SPACE: ErrorCode = ErrorCode {
        code: "db.cmd.no_model_space",
        category: Category::NotFound,
        message_id: "db-cmd-no-model-space",
    };
    /// The file name has an extension that is neither `.dwg` nor `.dxf`.
    pub const FORMAT_UNKNOWN: ErrorCode = ErrorCode {
        code: "db.cmd.format_unknown",
        category: Category::InvalidInput,
        message_id: "db-cmd-format-unknown",
    };
    /// The command does not read or write the file's format: a DXF-only command was given a DWG
    /// file, or a DWG file was named as a save target.
    pub const FORMAT_MISMATCH: ErrorCode = ErrorCode {
        code: "db.cmd.format_mismatch",
        category: Category::InvalidInput,
        message_id: "db-cmd-format-mismatch",
    };
    /// The format is known but cannot be written by this product yet.
    pub const FORMAT_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "db.cmd.format_unsupported",
        category: Category::Unsupported,
        message_id: "db-cmd-format-unsupported",
    };
    /// The save target exists and is not a regular file (a directory, a link or a device).
    pub const TARGET_NOT_FILE: ErrorCode = ErrorCode {
        code: "db.cmd.target_not_file",
        category: Category::Conflict,
        message_id: "db-cmd-target-not-file",
    };
    /// Every code of this crate, for the registry check.
    pub const ALL: &[ErrorCode] = &[
        DRAWING_NOT_EMPTY,
        NO_MODEL_SPACE,
        FORMAT_UNKNOWN,
        FORMAT_MISMATCH,
        FORMAT_UNSUPPORTED,
        TARGET_NOT_FILE,
    ];
}

/// A message identifier of this crate that is not an error code.
pub struct Message {
    /// The identifier in `locales/en-US/draftbuilder-commands.ftl`.
    pub message_id: &'static str,
}

/// The progress message of an open.
pub const OPEN_PROGRESS: Message = Message {
    message_id: "db-cmd-progress-open",
};
/// The progress message of a save.
pub const SAVE_PROGRESS: Message = Message {
    message_id: "db-cmd-progress-save",
};

#[cfg(test)]
mod tests {
    use super::codes;

    const CATALOG: &str = include_str!("../../../locales/en-US/draftbuilder-commands.ftl");

    #[test]
    fn the_registry_is_well_formed_and_every_message_is_defined() {
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
