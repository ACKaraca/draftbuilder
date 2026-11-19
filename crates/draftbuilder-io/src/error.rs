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

/// Every reason of this crate.
pub const ALL: &[Reason] = &[FINGERPRINT_ASSIGNED, LAYER_ZERO_CREATED];

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
