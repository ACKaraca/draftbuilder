//! The object snap table: the three-letter codes of the snap overrides and of the running set, the
//! bit encoding of the running set (the one the drawing header stores in `$OSMODE`), and the point
//! filters (DB-SNAP-011) as pure functions.

use arc_draft::snap::{SnapKind, SnapModes};

/// The bit that suppresses the running snaps in the header encoding (`$OSMODE` 16384). A drawing whose
/// running set has it set has the running snaps off; the bits below it are kept.
pub const RUNNING_SUPPRESSED: i64 = 16384;

/// The largest running-set value: the header encoding's bits, with the suppression bit.
pub const RUNNING_MAX: i64 = 32767;

/// The code of the geometric center. It has no bit in the header encoding, so it is kept by its own
/// drawing setting (`snap.object.geometric_center`), outside the running-set mask.
pub const GEOMETRIC_CENTER_CODE: &str = "GEO";

/// One object snap: its three-letter code, its kind and its bit in the running set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapCode {
    /// The code, in upper case.
    pub code: &'static str,
    /// The kind of the snap; `None` for a snap the header encoding names but the engine does not
    /// resolve (the apparent intersection). Such a bit is read and written back, and no pick uses it.
    pub kind: Option<SnapKind>,
    /// The bit of the kind in the running set, in the header encoding.
    pub bit: i64,
}

/// Every object snap of the running set with its code and its bit, in the header encoding of
/// `$OSMODE`: 1 end, 2 mid, 4 center, 8 node, 16 quadrant, 32 intersection, 64 insertion,
/// 128 perpendicular, 256 tangent, 512 nearest, 2048 apparent intersection, 4096 extension and
/// 8192 parallel. The geometric center is not here (see [`GEOMETRIC_CENTER_CODE`]).
pub const SNAP_CODES: &[SnapCode] = &[
    SnapCode {
        code: "END",
        kind: Some(SnapKind::Endpoint),
        bit: 1,
    },
    SnapCode {
        code: "MID",
        kind: Some(SnapKind::Midpoint),
        bit: 2,
    },
    SnapCode {
        code: "CEN",
        kind: Some(SnapKind::Center),
        bit: 4,
    },
    SnapCode {
        code: "NOD",
        kind: Some(SnapKind::Node),
        bit: 8,
    },
    SnapCode {
        code: "QUA",
        kind: Some(SnapKind::Quadrant),
        bit: 16,
    },
    SnapCode {
        code: "INT",
        kind: Some(SnapKind::Intersection),
        bit: 32,
    },
    SnapCode {
        code: "INS",
        kind: Some(SnapKind::Insertion),
        bit: 64,
    },
    SnapCode {
        code: "PER",
        kind: Some(SnapKind::Perpendicular),
        bit: 128,
    },
    SnapCode {
        code: "TAN",
        kind: Some(SnapKind::Tangent),
        bit: 256,
    },
    SnapCode {
        code: "NEA",
        kind: Some(SnapKind::Nearest),
        bit: 512,
    },
    SnapCode {
        code: "APP",
        kind: None,
        bit: 2048,
    },
    SnapCode {
        code: "EXT",
        kind: Some(SnapKind::Extension),
        bit: 4096,
    },
    SnapCode {
        code: "PAR",
        kind: Some(SnapKind::Parallel),
        bit: 8192,
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

/// The override of a typed code, without regard to case; `None` for a code the table lacks or a code
/// that is no kind for one pick (the apparent intersection).
#[must_use]
pub fn override_of(code: &str) -> Option<Override> {
    let key = code.trim().to_ascii_uppercase();
    if key == OFF_CODE {
        return Some(Override::Off);
    }
    if key == GEOMETRIC_CENTER_CODE {
        return Some(Override::Snap(SnapKind::GeometricCenter));
    }
    SNAP_CODES
        .iter()
        .find(|s| s.code == key)
        .and_then(|s| s.kind)
        .map(Override::Snap)
}

/// The running-set bits of `modes`. The geometric center has no bit: see [`GEOMETRIC_CENTER_CODE`].
#[must_use]
pub fn bits_of(modes: SnapModes) -> i64 {
    SNAP_CODES
        .iter()
        .filter(|s| s.kind.is_some_and(|kind| modes.contains(kind)))
        .fold(0, |bits, s| bits | s.bit)
}

/// The kinds of the running set `bits` names. Bits the table does not name, and the suppression bit,
/// are ignored; the apparent intersection names no kind.
#[must_use]
pub fn modes_of(bits: i64) -> SnapModes {
    SNAP_CODES
        .iter()
        .filter(|s| bits & s.bit != 0)
        .fold(SnapModes::NONE, |modes, s| match s.kind {
            Some(kind) => modes.with(kind),
            None => modes,
        })
}

/// Whether `bits` suppresses the running snaps.
#[must_use]
pub fn suppressed(bits: i64) -> bool {
    bits & RUNNING_SUPPRESSED != 0
}

/// The running set named by a list of codes: `NONE` or `ALL`, or codes separated by commas or
/// spaces. Returns the header bits and whether the geometric center is in the set.
///
/// # Errors
///
/// `db.cmd.snap_unknown` (detail `code`) for a code the table lacks.
pub fn bits_of_codes(text: &str) -> arc_base::Result<(i64, bool)> {
    let mut bits = 0i64;
    let mut geometric = false;
    for part in text
        .split([',', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        match part.to_ascii_uppercase().as_str() {
            "NONE" => {}
            "ALL" => {
                bits = SNAP_CODES.iter().fold(bits, |b, s| b | s.bit);
                geometric = true;
            }
            GEOMETRIC_CENTER_CODE => geometric = true,
            code => {
                let snap = SNAP_CODES.iter().find(|s| s.code == code).ok_or_else(|| {
                    arc_base::Error::new(super::codes::SNAP_UNKNOWN).with_detail("code", part)
                })?;
                bits |= snap.bit;
            }
        }
    }
    Ok((bits, geometric))
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
    fn the_running_set_round_trips_through_its_header_bits() {
        let modes = SnapModes::of(&[
            SnapKind::Endpoint,
            SnapKind::Node,
            SnapKind::Extension,
            SnapKind::Parallel,
        ]);
        assert_eq!(bits_of(modes), 1 | 8 | 4096 | 8192);
        assert_eq!(modes_of(bits_of(modes)), modes);
        assert_eq!(bits_of(SnapModes::NONE), 0);
        assert_eq!(bits_of_codes("end, mid").ok(), Some((3, false)));
        assert_eq!(bits_of_codes("none").ok(), Some((0, false)));
        assert_eq!(
            bits_of_codes("GEO").ok(),
            Some((0, true)),
            "the geometric center is no header bit"
        );
        assert_eq!(bits_of_codes("APP").ok(), Some((2048, false)));
        assert!(bits_of_codes("END,XYZ").is_err());
        assert!(super::suppressed(16447));
        assert_eq!(
            modes_of(16447),
            SnapModes::of(&[
                SnapKind::Endpoint,
                SnapKind::Midpoint,
                SnapKind::Center,
                SnapKind::Node,
                SnapKind::Quadrant,
                SnapKind::Intersection,
            ])
        );
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
