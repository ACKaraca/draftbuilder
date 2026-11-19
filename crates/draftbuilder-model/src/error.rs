//! The error codes owned by `draftbuilder-model`. Messages are in
//! `locales/en-US/draftbuilder-model.ftl`.

/// The error codes owned by `draftbuilder-model`.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// A handle is already bound to another object, or an object already has another handle.
    pub const HANDLE_TAKEN: ErrorCode = ErrorCode {
        code: "db.model.handle_taken",
        category: Category::Conflict,
        message_id: "db-model-handle-taken",
    };

    /// Every code of this crate, for the registry check.
    pub const ALL: &[ErrorCode] = &[HANDLE_TAKEN];
}

#[cfg(test)]
mod tests {
    use super::codes;

    const CATALOG: &str = include_str!("../../../locales/en-US/draftbuilder-model.ftl");

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
