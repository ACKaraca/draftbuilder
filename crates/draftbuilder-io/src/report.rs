//! Product items in a loss report.

use std::collections::BTreeMap;

use arc_io_dxf::Outcome;
use arc_io_dxf::loss::{Argument, LossItem, LossReport, Reason};

/// Adds an item for `count` occurrences of `reason` on records of `kind`, and counts them in the
/// summary. Nothing is added for a count of 0.
pub fn note(report: &mut LossReport, reason: Reason, kind: &str, count: u64) {
    if count == 0 {
        return;
    }
    report.items.push(LossItem {
        outcome: reason.outcome,
        severity: reason.severity,
        source_id: None,
        record_kind: Some(kind.to_owned()),
        object_id: None,
        reason: reason.code.to_owned(),
        message_id: reason.message_id.to_owned(),
        arguments: BTreeMap::from([(
            "count".to_owned(),
            Argument::Int(i64::try_from(count).unwrap_or(i64::MAX)),
        )]),
    });
    match reason.outcome {
        Outcome::Mapped => report.summary.mapped += count,
        Outcome::Approximated => report.summary.approximated += count,
        Outcome::Preserved => report.summary.preserved += count,
        Outcome::Dropped => report.summary.dropped += count,
        Outcome::Failed => report.summary.failed += count,
    }
}

/// Adds the counts and items of `other` (the report of writing) to `report`.
pub fn merge(report: &mut LossReport, other: &LossReport) {
    report.items.extend(other.items.iter().cloned());
    report.truncated |= other.truncated;
    report.summary.mapped += other.summary.mapped;
    report.summary.approximated += other.summary.approximated;
    report.summary.preserved += other.summary.preserved;
    report.summary.dropped += other.summary.dropped;
    report.summary.failed += other.summary.failed;
}
