//! `draftbuilder.drawing.audit`: checks the drawing database and reports what it finds. Nothing
//! is repaired.
//!
//! The objects checked are the entities and the layers, linetypes, block definitions, styles and
//! hatch patterns. The checks are: the owner of each object is a live object; each handle is bound
//! to one object (the handle table keeps the first binding of a handle, the later ones are
//! reported); and each curve entity's geometry decodes as the curve kind of its type. The commit
//! validators of the document already refuse these states, so a finding is rare. Dictionaries and
//! application names are not in the model and are not checked.

use arc_base::{Limits, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use arc_doc::Snapshot;
use arc_doc::source_map::RECORD_KIND;
use arc_draft::types::{ARC, CIRCLE, CONSTRUCTION_LINE, ELLIPSE, ELLIPTICAL_ARC, POLYLINE, RAY};
use arc_draft_dxf::fields::Reader;
use arc_geom::encoding::GeomCodec;
use arc_geom::{Arc2, Circle2, Ellipse2, EllipticalArc2, Line2, Polyline2, Ray2};
use draftbuilder_model::database::{FORMAT, SOURCE_DOCUMENT};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Message;

/// The identifier of the command.
pub const ID: &str = "draftbuilder.drawing.audit";
const LABEL: Message = Message {
    message_id: "db-cmd-audit-label",
};
const DESCRIPTION: Message = Message {
    message_id: "db-cmd-audit-description",
};

/// Parameters of `draftbuilder.drawing.audit`: none.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditParams {}

/// One finding of the audit.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Finding {
    /// The check that found it: `owner.missing`, `handle.duplicate` or `geometry.invalid`.
    pub check: String,
    /// The object, as its identifier, when the finding is about one.
    pub object: Option<String>,
    /// What was found.
    pub detail: String,
}

/// What `draftbuilder.drawing.audit` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AuditResult {
    /// The objects checked.
    pub objects: u64,
    /// The handle bindings of the drawing.
    pub bindings: u64,
    /// The findings, in the order the checks ran; at most the session's `record_count` limit.
    pub findings: Vec<Finding>,
    /// The findings beyond the limit, which are counted and not listed.
    pub omitted: u64,
}

/// Keeps the first `limit` findings and counts the rest.
fn cap_findings(mut findings: Vec<Finding>, limit: usize) -> (Vec<Finding>, u64) {
    let omitted = findings.len().saturating_sub(limit);
    findings.truncate(limit);
    (findings, omitted as u64)
}

fn finding(check: &str, object: Option<String>, detail: impl Into<String>) -> Finding {
    Finding {
        check: check.to_owned(),
        object,
        detail: detail.into(),
    }
}

/// Whether `bytes` decode as a `T` under the default limits.
fn decodes<T: GeomCodec>(bytes: &[u8]) -> bool {
    T::decode(bytes, &Limits::DEFAULT).is_ok()
}

/// The geometry check of each curve type: the decoder of its geometry.
fn geometry_decodes(key: &str, bytes: &[u8]) -> Option<bool> {
    Some(match key {
        RAY => decodes::<Ray2>(bytes),
        CONSTRUCTION_LINE => decodes::<Line2>(bytes),
        CIRCLE => decodes::<Circle2>(bytes),
        ARC => decodes::<Arc2>(bytes),
        ELLIPSE => decodes::<Ellipse2>(bytes),
        ELLIPTICAL_ARC => decodes::<EllipticalArc2>(bytes),
        POLYLINE => decodes::<Polyline2>(bytes),
        _ => return None,
    })
}

/// The owner and geometry checks of every object, and the handle check of the bindings. The
/// session's cancellation is checked before each type.
fn audit(ctx: &Context<'_>) -> Result<AuditResult> {
    let snapshot = ctx.snapshot();
    let mut findings = Vec::new();
    let mut objects = 0u64;
    let keys = arc_draft::types::ENTITIES
        .iter()
        .copied()
        .chain(arc_draft::purge::purgeable());
    for key in keys {
        ctx.checkpoint()?;
        let Ok(iter) = snapshot.iter_type(key) else {
            continue;
        };
        for (_, object) in iter {
            objects += 1;
            let id = object.id.to_string();
            if let Some(owner) = object.header.owner
                && snapshot.resolve(owner).is_err()
            {
                findings.push(finding(
                    "owner.missing",
                    Some(id.clone()),
                    "the owner is not a live object",
                ));
            }
            let reader = Reader::new(snapshot.registry(), key, &object.values);
            if let Some(arc_doc::Value::Geometry(bytes)) = reader.get("geometry")
                && geometry_decodes(key, &bytes.0) == Some(false)
            {
                findings.push(finding(
                    "geometry.invalid",
                    Some(id.clone()),
                    "the geometry does not decode as the curve kind of its type",
                ));
            }
        }
    }
    let bindings = bindings(snapshot, &mut findings);
    let limit = usize::try_from(ctx.limits().record_count).unwrap_or(usize::MAX);
    let (findings, omitted) = cap_findings(findings, limit);
    Ok(AuditResult {
        objects,
        bindings,
        findings,
        omitted,
    })
}

/// Checks the handle bindings of the drawing: a handle bound to two objects, the first one kept.
/// Returns the number of bindings.
fn bindings(snapshot: &Snapshot, findings: &mut Vec<Finding>) -> u64 {
    let mut seen = std::collections::BTreeSet::new();
    let mut count = 0u64;
    for row in snapshot.foreign_rows().filter(|r| {
        r.source_format == FORMAT
            && r.record_kind == RECORD_KIND
            && r.data == SOURCE_DOCUMENT.as_bytes()
    }) {
        count += 1;
        let handle = row.source_id.clone().unwrap_or_default();
        if !seen.insert(handle.clone()) {
            findings.push(finding(
                "handle.duplicate",
                None,
                format!("the handle {handle} is bound to more than one object"),
            ));
        }
    }
    count
}

fn run(ctx: &mut Context<'_>, _: AuditParams) -> Result<AuditResult> {
    audit(ctx)
}

/// Registers `draftbuilder.drawing.audit` (permission class `read`, not undoable: it changes
/// nothing).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(ID, LABEL.message_id, DESCRIPTION.message_id, run)
            .with_undo(UndoBehavior::NotUndoable)
            .with_permissions([Permission::Read]),
    )
}

#[cfg(test)]
mod tests {
    use super::{Finding, cap_findings};

    fn finding(n: usize) -> Finding {
        Finding {
            check: "owner.missing".to_owned(),
            object: Some(n.to_string()),
            detail: String::new(),
        }
    }

    #[test]
    fn findings_beyond_the_limit_are_counted_and_not_listed() {
        let (kept, omitted) = cap_findings((0..5).map(finding).collect(), 2);
        assert_eq!(kept.len(), 2);
        assert_eq!(omitted, 3);
        assert_eq!(kept[1].object.as_deref(), Some("1"));
        let (kept, omitted) = cap_findings((0..2).map(finding).collect(), 2);
        assert_eq!((kept.len(), omitted), (2, 0));
    }
}
