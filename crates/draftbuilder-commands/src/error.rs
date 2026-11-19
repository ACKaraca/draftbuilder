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
    /// The entities the import created cannot be matched to the handles of the file.
    pub const HANDLES_UNALIGNED: ErrorCode = ErrorCode {
        code: "db.cmd.handles_unaligned",
        category: Category::FailedOperation,
        message_id: "db-cmd-handles-unaligned",
    };
    /// The move command does not handle the type of an object.
    pub const MOVE_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "db.cmd.move_unsupported",
        category: Category::Unsupported,
        message_id: "db-cmd-move-unsupported",
    };
    /// The move offset is not a pair of finite numbers.
    pub const OFFSET_INVALID: ErrorCode = ErrorCode {
        code: "db.cmd.offset_invalid",
        category: Category::InvalidInput,
        message_id: "db-cmd-offset-invalid",
    };

    /// Every code of this crate, for the registry check.
    pub const ALL: &[ErrorCode] = &[
        DRAWING_NOT_EMPTY,
        NO_MODEL_SPACE,
        HANDLES_UNALIGNED,
        MOVE_UNSUPPORTED,
        OFFSET_INVALID,
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
