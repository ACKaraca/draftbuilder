//! The point acquisition state (DB-SNAP-010): a point modifier that waits for points while a point
//! prompt waits. Each modifier takes one or two points the pointer or the dynamic fields give; the
//! last one answers the prompt and the state ends. The prompt is never answered by a point that a
//! modifier keeps.

use arc_command::Prompt;

use crate::precision::points::{Modifier, midpoint};

/// Whether the keyword options of `prompt` claim `text`, as the command line matches a typed word:
/// an option whose identifier equals it (ignoring ASCII case), or whose one-letter shortcut it is.
/// Labels are not matched. A claimed word is the command's keyword and never a point modifier.
#[must_use]
pub(super) fn claimed_by(prompt: &Prompt, text: &str) -> bool {
    let text = text.trim();
    let mut chars = text.chars();
    let single = match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    prompt.keyword_options().any(|option| {
        option.id.eq_ignore_ascii_case(text)
            || (single.is_some() && option.shortcut.map(|c| c.to_ascii_lowercase()) == single)
    })
}

/// Which point of a modifier the next point is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierStep {
    /// The base point, the tracking origin or the first point.
    First,
    /// The offset from the base, the point for the tracking origin, or the second point.
    Second,
}

/// The state of a modifier that waits for points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Acquire {
    /// No modifier waits.
    Idle,
    /// `FROM`: the base point, then the point the offset from it gives.
    From { base: Option<[f64; 2]> },
    /// `TEMPTRACK`: the origin, then the point aligned with it.
    TempTrack { origin: Option<[f64; 2]> },
    /// `MIDBETWEEN`: the first point, then the second.
    Between { first: Option<[f64; 2]> },
}

/// What a point given to the state does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Taken {
    /// The modifier kept the point; the prompt waits.
    Kept,
    /// The modifier ends with this point, which answers the prompt.
    Answer([f64; 2]),
}

impl Acquire {
    /// The state a modifier starts in.
    pub(super) fn start(modifier: Modifier) -> Self {
        match modifier {
            Modifier::From => Self::From { base: None },
            Modifier::TempTrack => Self::TempTrack { origin: None },
            Modifier::MidBetween => Self::Between { first: None },
        }
    }

    /// Whether a modifier waits for a point.
    pub(super) fn pending(self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// The modifier and the point it waits for, when one waits.
    pub(super) fn waiting(self) -> Option<(Modifier, ModifierStep)> {
        match self {
            Self::Idle => None,
            Self::From { base: None } => Some((Modifier::From, ModifierStep::First)),
            Self::From { base: Some(_) } => Some((Modifier::From, ModifierStep::Second)),
            Self::TempTrack { origin: None } => Some((Modifier::TempTrack, ModifierStep::First)),
            Self::TempTrack { origin: Some(_) } => {
                Some((Modifier::TempTrack, ModifierStep::Second))
            }
            Self::Between { first: None } => Some((Modifier::MidBetween, ModifierStep::First)),
            Self::Between { first: Some(_) } => Some((Modifier::MidBetween, ModifierStep::Second)),
        }
    }

    /// The base point a `FROM` offset is measured from, while the offset waits.
    pub(super) fn base(self) -> Option<[f64; 2]> {
        match self {
            Self::From { base } => base,
            _ => None,
        }
    }

    /// The temporary tracking origin, while the point it aligns waits: the only origin the
    /// resolution offers tracking paths from.
    pub(super) fn origin(self) -> Option<[f64; 2]> {
        match self {
            Self::TempTrack { origin } => origin,
            _ => None,
        }
    }

    /// Takes `point` (meters or drawing units, as the caller uses them) and returns what it does.
    /// A modifier that has its last point ends; the state is then idle.
    pub(super) fn take(&mut self, point: [f64; 2]) -> Taken {
        match *self {
            Self::Idle => Taken::Answer(point),
            Self::From { base: None } => {
                *self = Self::From { base: Some(point) };
                Taken::Kept
            }
            Self::TempTrack { origin: None } => {
                *self = Self::TempTrack {
                    origin: Some(point),
                };
                Taken::Kept
            }
            Self::Between { first: None } => {
                *self = Self::Between { first: Some(point) };
                Taken::Kept
            }
            Self::Between { first: Some(first) } => {
                *self = Self::Idle;
                Taken::Answer(midpoint(first, point))
            }
            Self::From { base: Some(_) } | Self::TempTrack { origin: Some(_) } => {
                *self = Self::Idle;
                Taken::Answer(point)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use arc_command::{Accept, KeywordOption, Prompt};

    use super::{Acquire, ModifierStep, Taken, claimed_by};
    use crate::precision::points::Modifier;

    #[test]
    fn a_keyword_option_claims_its_identifier_or_shortcut_and_never_a_label() {
        let prompt = Prompt::new(
            "first",
            "first",
            Accept::keywords([
                KeywordOption::new("two", "label_two").with_shortcut('t'),
                KeywordOption::new("from", "label_from"),
            ]),
        );
        assert!(claimed_by(&prompt, "TWO"), "identifier, any case");
        assert!(claimed_by(&prompt, " t "), "one-letter shortcut");
        assert!(
            claimed_by(&prompt, "from"),
            "an identifier that is a modifier keyword"
        );
        assert!(!claimed_by(&prompt, "label_two"), "labels are not matched");
        assert!(!claimed_by(&prompt, "mid"));
        let plain = Prompt::new(
            "p",
            "p",
            Accept::Point {
                dims: arc_command::PointDims::Two,
            },
        );
        assert!(!claimed_by(&plain, "from"));
    }

    #[test]
    fn from_takes_a_base_then_answers_with_the_offset_point() {
        let mut state = Acquire::start(Modifier::From);
        assert_eq!(state.waiting(), Some((Modifier::From, ModifierStep::First)));
        assert_eq!(state.take([10.0, 0.0]), Taken::Kept);
        assert_eq!(state.base(), Some([10.0, 0.0]));
        assert_eq!(
            state.waiting(),
            Some((Modifier::From, ModifierStep::Second))
        );
        assert_eq!(state.take([10.0, 5.0]), Taken::Answer([10.0, 5.0]));
        assert!(!state.pending());
    }

    #[test]
    fn a_temporary_track_origin_is_kept_for_one_point_only() {
        let mut state = Acquire::start(Modifier::TempTrack);
        assert_eq!(state.take([3.0, 4.0]), Taken::Kept);
        assert_eq!(state.origin(), Some([3.0, 4.0]));
        assert_eq!(state.take([8.0, 4.0]), Taken::Answer([8.0, 4.0]));
        assert_eq!(state.origin(), None);
        assert!(!state.pending());
    }

    #[test]
    fn mid_between_answers_with_the_midpoint_of_two_points() {
        let mut state = Acquire::start(Modifier::MidBetween);
        assert_eq!(state.take([0.0, 0.0]), Taken::Kept);
        assert_eq!(state.take([10.0, 4.0]), Taken::Answer([5.0, 2.0]));
        assert!(!state.pending());
    }
}
