//! Point acquisition modifiers (DB-SNAP-010): one-shot overrides a point prompt takes by keyword.
//! `FROM` takes a base point and then an offset from it; `TEMPTRACK` takes a temporary alignment
//! origin for the next point only; `MIDBETWEEN` takes the midpoint of two points. The state of a
//! modifier that waits for points lives in the interaction controller; this module holds the
//! keywords and the pure point arithmetic.

/// The point modifiers, in the order the keyword table lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    /// A base point, then a point offset from it.
    From,
    /// A temporary tracking origin, used once for the next point.
    TempTrack,
    /// The midpoint of two points.
    MidBetween,
}

/// The modifier a typed keyword names, without regard to case; `None` for any other text.
#[must_use]
pub fn modifier_of(text: &str) -> Option<Modifier> {
    match text.trim().to_ascii_uppercase().as_str() {
        "FROM" => Some(Modifier::From),
        "TEMPTRACK" => Some(Modifier::TempTrack),
        "MIDBETWEEN" => Some(Modifier::MidBetween),
        _ => None,
    }
}

/// The midpoint of two points, in drawing units or meters alike.
#[must_use]
pub fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

#[cfg(test)]
mod tests {
    use super::{Modifier, midpoint, modifier_of};

    #[test]
    fn the_keywords_read_without_regard_to_case_and_nothing_else() {
        assert_eq!(modifier_of(" from "), Some(Modifier::From));
        assert_eq!(modifier_of("TempTrack"), Some(Modifier::TempTrack));
        assert_eq!(modifier_of("MIDBETWEEN"), Some(Modifier::MidBetween));
        assert_eq!(modifier_of("MID"), None, "MID is an object snap code");
        assert_eq!(modifier_of("5,5"), None);
    }

    #[test]
    fn the_midpoint_is_the_average_of_both_coordinates() {
        assert_eq!(midpoint([0.0, 0.0], [10.0, 4.0]), [5.0, 2.0]);
        assert_eq!(midpoint([-3.0, 1.0], [3.0, -1.0]), [0.0, 0.0]);
    }
}
