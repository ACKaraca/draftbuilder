//! The object snap table: the three-letter codes of the snap overrides and of the running set, the
//! bit encoding of the running set (the one the drawing header stores in `$OSMODE`), and the point
//! filters (DB-SNAP-011) as pure functions.

use arc_draft::snap::{SnapKind, SnapModes};

/// The running-set bit of the geometric center, which the header encoding has no bit for: a
/// product extension above the bits the header defines.
pub const GEOMETRIC_CENTER_BIT: i64 = 8192;

/// One object snap: its three-letter code, its kind and its bit in the running set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapCode {
    /// The code, in upper case.
    pub code: &'static str,
    /// The kind of the snap.
    pub kind: SnapKind,
    /// The bit of the kind in the running set.
    pub bit: i64,
}

/// Every object snap, with its code and bit. The bits 1 to 4096 follow the header encoding of the
/// running set.
pub const SNAP_CODES: &[SnapCode] = &[
    SnapCode {
        code: "END",
        kind: SnapKind::Endpoint,
        bit: 1,
    },
    SnapCode {
        code: "MID",
        kind: SnapKind::Midpoint,
        bit: 2,
    },
    SnapCode {
        code: "CEN",
        kind: SnapKind::Center,
        bit: 4,
    },
    SnapCode {
        code: "NOD",
        kind: SnapKind::Node,
        bit: 8,
    },
    SnapCode {
        code: "QUA",
        kind: SnapKind::Quadrant,
        bit: 16,
    },
    SnapCode {
        code: "INT",
        kind: SnapKind::Intersection,
        bit: 32,
    },
    SnapCode {
        code: "INS",
        kind: SnapKind::Insertion,
        bit: 64,
    },
    SnapCode {
        code: "PER",
        kind: SnapKind::Perpendicular,
        bit: 128,
    },
    SnapCode {
        code: "TAN",
        kind: SnapKind::Tangent,
        bit: 256,
    },
    SnapCode {
        code: "NEA",
        kind: SnapKind::Nearest,
        bit: 512,
    },
    SnapCode {
        code: "EXT",
        kind: SnapKind::Extension,
        bit: 2048,
    },
    SnapCode {
        code: "PAR",
        kind: SnapKind::Parallel,
        bit: 4096,
    },
    SnapCode {
        code: "GEO",
        kind: SnapKind::GeometricCenter,
        bit: GEOMETRIC_CENTER_BIT,
    },
];

/// The code that turns object snapping off for one pick.
pub const OFF_CODE: &str = "NON";

/// What a snap override asks for: one kind for one pick, or no object snap at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Override {
    /// No override: the running set applies.
    None,
    /// The one kind, in place of the running set, for one pick.
    Snap(SnapKind),
    /// No object snap for one pick (`NON`).
    Off,
}

/// The override of a typed code, without regard to case; `None` for a code the table lacks.
#[must_use]
pub fn override_of(code: &str) -> Option<Override> {
    let key = code.trim().to_ascii_uppercase();
    if key == OFF_CODE {
        return Some(Override::Off);
    }
    SNAP_CODES
        .iter()
        .find(|s| s.code == key)
        .map(|s| Override::Snap(s.kind))
}

/// The running-set bits of `modes`.
#[must_use]
pub fn bits_of(modes: SnapModes) -> i64 {
    SNAP_CODES
        .iter()
        .filter(|s| modes.contains(s.kind))
        .fold(0, |bits, s| bits | s.bit)
}

/// The running set of `bits`. Bits the table does not name are ignored.
#[must_use]
pub fn modes_of(bits: i64) -> SnapModes {
    SNAP_CODES
        .iter()
        .filter(|s| bits & s.bit != 0)
        .fold(SnapModes::NONE, |modes, s| modes.with(s.kind))
}

/// The running set named by a list of codes: `NONE` or `ALL`, or codes separated by commas or
/// spaces. Returns the bits.
///
/// # Errors
///
/// `db.cmd.snap_unknown` (detail `code`) for a code the table lacks.
pub fn bits_of_codes(text: &str) -> arc_base::Result<i64> {
    let mut bits = 0i64;
    for part in text
        .split([',', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        match part.to_ascii_uppercase().as_str() {
            "NONE" => {}
            "ALL" => bits = SNAP_CODES.iter().fold(bits, |b, s| b | s.bit),
            code => {
                let snap = SNAP_CODES.iter().find(|s| s.code == code).ok_or_else(|| {
                    arc_base::Error::new(super::codes::SNAP_UNKNOWN).with_detail("code", part)
                })?;
                bits |= snap.bit;
            }
        }
    }
    Ok(bits)
}

/// The point with the X coordinate of `x_from` and the Y coordinate of `y_from` (DB-SNAP-011, the
/// planar case). Each source point is a pick or a snapped point.
#[must_use]
pub fn filter_xy(x_from: [f64; 2], y_from: [f64; 2]) -> [f64; 2] {
    [x_from[0], y_from[1]]
}

#[cfg(test)]
mod tests {
    use arc_draft::snap::{SnapKind, SnapModes};

    use super::{Override, bits_of, bits_of_codes, filter_xy, modes_of, override_of};

    #[test]
    fn the_running_set_round_trips_through_its_bits() {
        let modes = SnapModes::of(&[
            SnapKind::Endpoint,
            SnapKind::Node,
            SnapKind::GeometricCenter,
        ]);
        assert_eq!(modes_of(bits_of(modes)), modes);
        assert_eq!(bits_of(SnapModes::NONE), 0);
        assert_eq!(bits_of_codes("end, mid").ok(), Some(3));
        assert_eq!(bits_of_codes("none").ok(), Some(0));
        assert!(bits_of_codes("END,XYZ").is_err());
    }

    #[test]
    fn overrides_read_the_three_letter_codes_without_regard_to_case() {
        assert_eq!(override_of("non"), Some(Override::Off));
        assert_eq!(override_of("Mid"), Some(Override::Snap(SnapKind::Midpoint)));
        assert_eq!(override_of("ABC"), None);
    }

    #[test]
    fn a_point_filter_takes_x_and_y_from_different_points() {
        assert_eq!(filter_xy([1.0, 2.0], [5.0, 7.0]), [1.0, 7.0]);
    }
}
