//! The cycling order of overlapping candidates (DB-SEL-008) as pure functions.
//!
//! The candidates under a pick are ordered topmost first: the larger draw-order key first, and
//! ties by identifier. Cycling moves from the current candidate to the next one in that order and
//! wraps around to the first.

use arc_base::ObjectId;

/// The candidates in cycling order: the topmost first (descending draw order), ties by ascending
/// identifier. A candidate listed twice keeps its first position.
#[must_use]
pub fn cycle_order(candidates: &[(ObjectId, i64)]) -> Vec<ObjectId> {
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut seen = std::collections::BTreeSet::new();
    sorted
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| seen.insert(*id))
        .collect()
}

/// The candidate after `current` in `order`, wrapping to the first. Without a current candidate,
/// or when it is not in `order`, the first candidate. `None` for an empty order.
#[must_use]
pub fn cycle_next(order: &[ObjectId], current: Option<ObjectId>) -> Option<ObjectId> {
    let first = order.first().copied()?;
    let Some(at) = current.and_then(|c| order.iter().position(|id| *id == c)) else {
        return Some(first);
    };
    Some(order.get(at + 1).copied().unwrap_or(first))
}

#[cfg(test)]
mod tests {
    use arc_base::id::Identifier;

    use super::*;

    fn id(n: u128) -> ObjectId {
        ObjectId::from_u128(n)
    }

    #[test]
    fn topmost_first_then_identifier() {
        let (a, b, c) = (id(1), id(2), id(3));
        let order = cycle_order(&[(a, 5), (b, 9), (c, 5)]);
        assert_eq!(order, vec![b, a, c]);
    }

    #[test]
    fn cycling_wraps_and_starts_at_the_first_candidate() {
        let (a, b, c) = (id(1), id(2), id(3));
        let order = [a, b, c];
        assert_eq!(cycle_next(&order, None), Some(a));
        assert_eq!(cycle_next(&order, Some(a)), Some(b));
        assert_eq!(cycle_next(&order, Some(c)), Some(a));
        assert_eq!(cycle_next(&[], Some(a)), None);
    }
}
