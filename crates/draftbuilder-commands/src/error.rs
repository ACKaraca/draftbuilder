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
    /// A selection geometry or filter is not usable: a polygon or fence with too few or too many
    /// vertices or no enclosed area, or a filter over its limits or with a bad test. The detail
    /// `reason` names which.
    pub const SELECTION_INVALID: ErrorCode = ErrorCode {
        code: "db.cmd.selection_invalid",
        category: Category::InvalidInput,
        message_id: "db-cmd-selection-invalid",
    };
    /// A selection or an edit would take more objects than the command allows.
    pub const LIMIT_EXCEEDED: ErrorCode = ErrorCode {
        code: "db.cmd.limit_exceeded",
        category: Category::LimitExceeded,
        message_id: "db-cmd-limit-exceeded",
    };
    /// No layer has the name given.
    pub const LAYER_UNKNOWN: ErrorCode = ErrorCode {
        code: "db.cmd.layer_unknown",
        category: Category::NotFound,
        message_id: "db-cmd-layer-unknown",
    };
    /// No linetype has the name given.
    pub const LINETYPE_UNKNOWN: ErrorCode = ErrorCode {
        code: "db.cmd.linetype_unknown",
        category: Category::NotFound,
        message_id: "db-cmd-linetype-unknown",
    };
    /// A layer name uses a character or a length that the drawing format does not allow.
    pub const NAME_INVALID: ErrorCode = ErrorCode {
        code: "db.cmd.name_invalid",
        category: Category::InvalidInput,
        message_id: "db-cmd-name-invalid",
    };
    /// A property value is outside its range.
    pub const VALUE_INVALID: ErrorCode = ErrorCode {
        code: "db.cmd.value_invalid",
        category: Category::InvalidInput,
        message_id: "db-cmd-value-invalid",
    };
    /// A property command names no property to change.
    pub const NOTHING_TO_CHANGE: ErrorCode = ErrorCode {
        code: "db.cmd.nothing_to_change",
        category: Category::InvalidInput,
        message_id: "db-cmd-nothing-to-change",
    };
    /// A layer name is already used by another layer, compared without regard to case.
    pub const NAME_TAKEN: ErrorCode = ErrorCode {
        code: "db.cmd.name_taken",
        category: Category::Conflict,
        message_id: "db-cmd-name-taken",
    };
    /// The layer is layer `0` or the definition-point layer, which cannot be renamed, deleted or
    /// merged away.
    pub const LAYER_SPECIAL: ErrorCode = ErrorCode {
        code: "db.cmd.layer_special",
        category: Category::Conflict,
        message_id: "db-cmd-layer-special",
    };
    /// The layer holds objects, so it cannot be deleted by itself.
    pub const LAYER_IN_USE: ErrorCode = ErrorCode {
        code: "db.cmd.layer_in_use",
        category: Category::Conflict,
        message_id: "db-cmd-layer-in-use",
    };
    /// Three points of an arc or circle lie on one line, or two of them coincide.
    pub const POINTS_COLLINEAR: ErrorCode = ErrorCode {
        code: "db.cmd.points_collinear",
        category: Category::InvalidInput,
        message_id: "db-cmd-points-collinear",
    };
    /// A construction has no solution for the values given, such as a tangent circle that
    /// cannot reach both objects with the radius, or an arc whose chord is longer than its
    /// diameter.
    pub const NO_SOLUTION: ErrorCode = ErrorCode {
        code: "db.cmd.no_solution",
        category: Category::InvalidInput,
        message_id: "db-cmd-no-solution",
    };
    /// An option undoes a segment, but no segment is drawn yet.
    pub const NOTHING_TO_UNDO: ErrorCode = ErrorCode {
        code: "db.cmd.nothing_to_undo",
        category: Category::Conflict,
        message_id: "db-cmd-nothing-to-undo",
    };
    /// A tangent object is not a line, segment or circle, the objects the tangent constructions
    /// accept.
    pub const TANGENT_OBJECT: ErrorCode = ErrorCode {
        code: "db.cmd.tangent_object",
        category: Category::InvalidInput,
        message_id: "db-cmd-tangent-object",
    };
    /// The layer is named by a preserved record of the file, which keeps the name (purge's rule):
    /// it cannot be renamed, deleted or merged away.
    pub const LAYER_PRESERVED: ErrorCode = ErrorCode {
        code: "db.cmd.layer_preserved",
        category: Category::Conflict,
        message_id: "db-cmd-layer-preserved",
    };
    /// A copy would refer to another entity (a dimension, a hatch boundary, a path array's path),
    /// which a copy does not carry along.
    pub const COPY_ASSOCIATED: ErrorCode = ErrorCode {
        code: "db.cmd.copy_associated",
        category: Category::Unsupported,
        message_id: "db-cmd-copy-associated",
    };
    /// The entity has no curve of its own (a point, a text, a dimension) where the command needs one.
    pub const GEOMETRY_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "db.cmd.geometry_unsupported",
        category: Category::Unsupported,
        message_id: "db-cmd-geometry-unsupported",
    };
    /// An entity that is not selected refers to an entity that the command erases.
    pub const ERASE_DEPENDENT: ErrorCode = ErrorCode {
        code: "db.cmd.erase_dependent",
        category: Category::Conflict,
        message_id: "db-cmd-erase-dependent",
    };
    /// The command cannot decompose the entity's kind (the foundation has no decomposition for it).
    pub const EXPLODE_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "db.cmd.explode_unsupported",
        category: Category::Unsupported,
        message_id: "db-cmd-explode-unsupported",
    };
    /// The command cannot stretch the entity's kind, because which of its points move is not defined
    /// for it.
    pub const STRETCH_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "db.cmd.stretch_unsupported",
        category: Category::Unsupported,
        message_id: "db-cmd-stretch-unsupported",
    };
    /// Align pairs do not describe a similarity (a uniform scale, a rotation and a translation).
    pub const ALIGN_NOT_SIMILARITY: ErrorCode = ErrorCode {
        code: "db.cmd.align_not_similarity",
        category: Category::InvalidInput,
        message_id: "db-cmd-align-not-similarity",
    };
    /// A numeric or point value of a modify command is missing, or two alternatives were given.
    pub const PARAMS_CONFLICT: ErrorCode = ErrorCode {
        code: "db.cmd.params_conflict",
        category: Category::InvalidInput,
        message_id: "db-cmd-params-conflict",
    };
    /// A pick point or edge of a modify command names no part of the entity it should change.
    pub const PICK_INVALID: ErrorCode = ErrorCode {
        code: "db.cmd.pick_invalid",
        category: Category::InvalidInput,
        message_id: "db-cmd-pick-invalid",
    };
    /// The target is read-only: a save would replace it, which its owner did not allow.
    pub const TARGET_READ_ONLY: ErrorCode = ErrorCode {
        code: "db.cmd.target_read_only",
        category: Category::PermissionDenied,
        message_id: "db-cmd-target-read-only",
    };
    /// The export would drop objects of a kind the product creates (the detail `kind` names the first
    /// kind, `count` the number of objects). Nothing is written.
    pub const EXPORT_DROPS_OBJECTS: ErrorCode = ErrorCode {
        code: "db.cmd.export_drops_objects",
        category: Category::Unsupported,
        message_id: "db-cmd-export-drops-objects",
    };
    /// Another entity outside an array's sources refers to one of them (a dimension, a hatch boundary, another
    /// array), and the array would move the source out of the drawing's reference.
    pub const ARRAY_DEPENDENT: ErrorCode = ErrorCode {
        code: "db.cmd.array_dependent",
        category: Category::Conflict,
        message_id: "db-cmd-array-dependent",
    };
    /// The sources of an array are not all owned by one space or block definition.
    pub const ARRAY_OWNER_MIXED: ErrorCode = ErrorCode {
        code: "db.cmd.array_owner_mixed",
        category: Category::InvalidInput,
        message_id: "db-cmd-array-owner-mixed",
    };
    /// A script ends while a command still waits at a prompt for an answer.
    pub const SCRIPT_PENDING: ErrorCode = ErrorCode {
        code: "db.cmd.script_pending",
        category: Category::InvalidInput,
        message_id: "db-cmd-script-pending",
    };
    /// A script line that is a history directive has a count that is not a positive whole number.
    pub const SCRIPT_DIRECTIVE: ErrorCode = ErrorCode {
        code: "db.cmd.script_directive",
        category: Category::InvalidInput,
        message_id: "db-cmd-script-directive",
    };
    /// A point typed for a pick or a selection lies on no object of the model space.
    pub const PICK_NONE: ErrorCode = ErrorCode {
        code: "db.cmd.pick_none",
        category: Category::NotFound,
        message_id: "db-cmd-pick-none",
    };
    /// A setting that may not change while a prompt waits is set with an apostrophe (DB-CMD-012).
    pub const SETTING_NOT_TRANSPARENT: ErrorCode = ErrorCode {
        code: "db.cmd.setting_not_transparent",
        category: Category::Conflict,
        message_id: "db-cmd-setting-not-transparent",
    };
    /// A script exceeds one of its limits (file size, line length, line count or tokens per line).
    pub const SCRIPT_LIMIT: ErrorCode = ErrorCode {
        code: "db.cmd.script_limit",
        category: Category::LimitExceeded,
        message_id: "db-cmd-script-limit",
    };
    /// `LAYERP` has nothing to restore: no change is kept, or every kept change no longer matches the
    /// drawing (undo took it back, or the layer changed since).
    pub const LAYER_PREVIOUS_NONE: ErrorCode = ErrorCode {
        code: "db.cmd.layer_previous_none",
        category: Category::NotFound,
        message_id: "db-cmd-layer-previous-none",
    };
    /// The command ends a layer state that is not active in this session: no isolation for
    /// `unisolate`, no walk for `walk_end`. The detail `command` names it.
    pub const LAYER_NOT_ACTIVE: ErrorCode = ErrorCode {
        code: "db.cmd.layer_not_active",
        category: Category::Conflict,
        message_id: "db-cmd-layer-not-active",
    };
    /// `LAYERP` stops at a change that was too large to keep, so it cannot step across it.
    pub const LAYER_PREVIOUS_UNKEPT: ErrorCode = ErrorCode {
        code: "db.cmd.layer_previous_unkept",
        category: Category::Conflict,
        message_id: "db-cmd-layer-previous-unkept",
    };
    /// A layer record (isolation, walk) was made on another drawing, so it is neither applied nor
    /// removed here.
    pub const LAYER_STATE_OTHER_DRAWING: ErrorCode = ErrorCode {
        code: "db.cmd.layer_state_other_drawing",
        category: Category::Conflict,
        message_id: "db-cmd-layer-state-other-drawing",
    };
    /// None of the layers an isolation or walk changed can be restored: the user changed their flags
    /// since. The record is kept.
    pub const LAYER_STATE_CHANGED: ErrorCode = ErrorCode {
        code: "db.cmd.layer_state_changed",
        category: Category::Conflict,
        message_id: "db-cmd-layer-state-changed",
    };
    /// Every code of this crate, for the registry check.
    pub const ALL: &[ErrorCode] = &[
        LAYER_PREVIOUS_NONE,
        LAYER_PREVIOUS_UNKEPT,
        LAYER_STATE_OTHER_DRAWING,
        LAYER_STATE_CHANGED,
        LAYER_NOT_ACTIVE,
        SETTING_NOT_TRANSPARENT,
        SCRIPT_LIMIT,
        SCRIPT_PENDING,
        SCRIPT_DIRECTIVE,
        PICK_NONE,
        DRAWING_NOT_EMPTY,
        NO_MODEL_SPACE,
        FORMAT_UNKNOWN,
        FORMAT_MISMATCH,
        FORMAT_UNSUPPORTED,
        TARGET_NOT_FILE,
        SELECTION_INVALID,
        LIMIT_EXCEEDED,
        LAYER_UNKNOWN,
        LINETYPE_UNKNOWN,
        NAME_INVALID,
        VALUE_INVALID,
        NOTHING_TO_CHANGE,
        NAME_TAKEN,
        LAYER_SPECIAL,
        LAYER_IN_USE,
        LAYER_PRESERVED,
        POINTS_COLLINEAR,
        NO_SOLUTION,
        NOTHING_TO_UNDO,
        TANGENT_OBJECT,
        COPY_ASSOCIATED,
        GEOMETRY_UNSUPPORTED,
        ERASE_DEPENDENT,
        EXPLODE_UNSUPPORTED,
        STRETCH_UNSUPPORTED,
        ALIGN_NOT_SIMILARITY,
        PARAMS_CONFLICT,
        PICK_INVALID,
        TARGET_READ_ONLY,
        EXPORT_DROPS_OBJECTS,
        ARRAY_DEPENDENT,
        ARRAY_OWNER_MIXED,
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
