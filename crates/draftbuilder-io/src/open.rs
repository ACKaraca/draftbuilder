//! Reading a DXF file into the drawing database: what the neutral model says about the
//! drawing's identity, and the source handles of the entities the import plan creates.

use arc_draft::types::SpaceUnit;
use arc_draft_dxf::plan::{Container, Op, Plan};
use arc_draft_dxf::units::unit_of_code;
use arc_io_dxf::Drawing;
use draftbuilder_model::{Fingerprint, Handle};

/// The header variable that holds the fingerprint.
pub const FINGERPRINT_VARIABLE: &str = "$FINGERPRINTGUID";
/// The header variable that holds the version GUID.
pub const VERSION_VARIABLE: &str = "$VERSIONGUID";
/// The header variable that holds the next free handle.
pub const SEED_VARIABLE: &str = "$HANDSEED";

fn text_of(drawing: &Drawing, name: &str) -> Option<String> {
    drawing
        .header_variable(name)?
        .iter()
        .find_map(|g| g.as_text())
        .map(str::to_owned)
}

/// The fingerprint the file carries, `None` when it has none, or the null GUID, or text that is
/// not a GUID.
#[must_use]
pub fn fingerprint_of(drawing: &Drawing) -> Option<Fingerprint> {
    Fingerprint::parse(&text_of(drawing, FINGERPRINT_VARIABLE)?)
}

/// The unit of the file's coordinates; unitless when the file declares none or an unknown one.
#[must_use]
pub fn unit_of(drawing: &Drawing) -> SpaceUnit {
    drawing
        .header_variable("$INSUNITS")
        .and_then(|g| g.first())
        .and_then(|g| match g.value {
            arc_io_dxf::Value::Int(code) => Some(code),
            _ => None,
        })
        .and_then(unit_of_code)
        .unwrap_or(SpaceUnit::Unitless)
}

/// The highest handle the file has ever used, as far as the neutral model shows it: the next
/// free handle the header declares, less one, and every entity handle of the plan.
#[must_use]
pub fn handle_top(drawing: &Drawing, plan: &Plan) -> u64 {
    let seed = text_of(drawing, SEED_VARIABLE)
        .and_then(|t| Handle::from_hex(t.trim()))
        .map_or(0, |h| h.value().saturating_sub(1));
    plan.ops
        .iter()
        .filter_map(|op| match op {
            Op::Entity { spec, .. } => spec.source.map(|h| h.0),
            _ => None,
        })
        .fold(seed, u64::max)
}

/// The source handle of each model space entity the plan creates, in creation order; `None`
/// for an entity the file gave no handle. Entities of blocks are not listed: their handles are
/// not carried yet.
#[must_use]
pub fn space_handles(plan: &Plan) -> Vec<Option<Handle>> {
    plan.ops
        .iter()
        .filter_map(|op| match op {
            Op::Entity {
                container: Container::Space,
                spec,
            } => Some(spec.source.and_then(|h| Handle::new(h.0))),
            _ => None,
        })
        .collect()
}
