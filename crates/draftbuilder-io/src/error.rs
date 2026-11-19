//! The loss reasons owned by `draftbuilder-io`, added to the report of `arc-io-dxf` and
//! `arc-draft-dxf` when a drawing is saved. Messages are in
//! `locales/en-US/draftbuilder-io.ftl`.

use arc_io_dxf::loss::{Outcome, Reason, Severity};

/// Entities were written with handles that DraftBuilder assigned instead of their source
/// handles: they were created in this session, or their source handle is reserved by the
/// export's own structure, or the entities could not be matched to the objects.
pub const HANDLE_ASSIGNED: Reason = Reason {
    code: "db.io.handle_assigned",
    outcome: Outcome::Approximated,
    severity: Severity::Warning,
    message_id: "db-io-loss-handle-assigned",
};

/// The drawing had no fingerprint; one was assigned and written, and later saves keep it.
pub const FINGERPRINT_ASSIGNED: Reason = Reason {
    code: "db.io.fingerprint_assigned",
    outcome: Outcome::Mapped,
    severity: Severity::Info,
    message_id: "db-io-loss-fingerprint-assigned",
};

/// Two entities of the file had the same handle; the first keeps it and the later one is
/// written with an assigned handle.
pub const HANDLE_DUPLICATE: Reason = Reason {
    code: "db.io.handle_duplicate",
    outcome: Outcome::Approximated,
    severity: Severity::Warning,
    message_id: "db-io-loss-handle-duplicate",
};

/// Every reason of this crate.
pub const ALL: &[Reason] = &[HANDLE_ASSIGNED, FINGERPRINT_ASSIGNED, HANDLE_DUPLICATE];

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
