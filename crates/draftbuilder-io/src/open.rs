//! Reading a DXF file into the drawing database: what the neutral model says about the
//! drawing's identity and unit.

use arc_draft::types::SpaceUnit;
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

/// The highest handle the file's header says it has used: the next free handle it declares,
/// less one; 0 when it declares none.
#[must_use]
pub fn seed_top(drawing: &Drawing) -> u64 {
    text_of(drawing, SEED_VARIABLE)
        .and_then(|t| Handle::from_hex(t.trim()))
        .map_or(0, |h| h.value().saturating_sub(1))
}
