//! The loss reasons owned by `draftbuilder-io`, added to the report of `arc-io-dxf` and
//! `arc-draft-dxf` when a drawing is saved. Messages are in
//! `locales/en-US/draftbuilder-io.ftl`.

use arc_io_dxf::loss::{Outcome, Reason, Severity};

/// The drawing had no fingerprint; one was assigned and written, and later saves keep it.
pub const FINGERPRINT_ASSIGNED: Reason = Reason {
    code: "db.io.fingerprint_assigned",
    outcome: Outcome::Mapped,
    severity: Severity::Info,
    message_id: "db-io-loss-fingerprint-assigned",
};

/// The drawing had no layer `0`; the reserved layer was created when the drawing was opened.
pub const LAYER_ZERO_CREATED: Reason = Reason {
    code: "db.io.layer_zero_created",
    outcome: Outcome::Mapped,
    severity: Severity::Info,
    message_id: "db-io-loss-layer-zero-created",
};

/// Layer names of the file that differ only by case. The product compares layer names without regard
/// to case, so each such group is one name in the drawing; the report counts the groups.
pub const LAYER_NAME_CASE_DUPLICATE: Reason = Reason {
    code: "db.io.layer_name_case_duplicate",
    outcome: Outcome::Mapped,
    severity: Severity::Warning,
    message_id: "db-io-loss-layer-name-case-duplicate",
};

/// A header variable of the file holds a value that its setting cannot take (out of range, a name
/// the drawing lacks, or another type); the setting keeps its default.
pub const HEADER_VALUE_IGNORED: Reason = Reason {
    code: "db.io.header_value_ignored",
    outcome: Outcome::Dropped,
    severity: Severity::Warning,
    message_id: "db-io-loss-header-value-ignored",
};

/// A header variable holds a number outside its setting's range; the value is set to the nearest bound.
pub const HEADER_VALUE_CLAMPED: Reason = Reason {
    code: "db.io.header_value_clamped",
    outcome: Outcome::Approximated,
    severity: Severity::Info,
    message_id: "db-io-loss-header-value-clamped",
};

/// The file's current layer is frozen or missing; the current layer is layer 0.
pub const CURRENT_LAYER_FALLBACK: Reason = Reason {
    code: "db.io.current_layer_fallback",
    outcome: Outcome::Mapped,
    severity: Severity::Warning,
    message_id: "db-io-loss-current-layer-fallback",
};

/// Header variables of the file that the save does not write back: the product keeps only its
/// settings and the coordinate system, and the export writes its own variables.
pub const HEADER_NOT_WRITTEN: Reason = Reason {
    code: "db.io.header_not_written",
    outcome: Outcome::Dropped,
    severity: Severity::Warning,
    message_id: "db-io-loss-header-not-written",
};

/// Every reason of this crate.
pub const ALL: &[Reason] = &[
    FINGERPRINT_ASSIGNED,
    LAYER_ZERO_CREATED,
    LAYER_NAME_CASE_DUPLICATE,
    HEADER_VALUE_IGNORED,
    HEADER_VALUE_CLAMPED,
    CURRENT_LAYER_FALLBACK,
    HEADER_NOT_WRITTEN,
];

#[cfg(test)]
mod tests {
    use super::ALL;

    const CATALOG: &str = include_str!("../../../locales/en-US/draftbuilder-io.ftl");

    #[test]
    fn every_reason_has_a_message_and_a_unique_code() {
        for (n, reason) in ALL.iter().enumerate() {
            assert!(CATALOG.contains(&format!("{} =", reason.message_id)));
            assert!(reason.code.starts_with("db.io."));
            assert!(ALL.iter().skip(n + 1).all(|r| r.code != reason.code));
        }
    }
}
