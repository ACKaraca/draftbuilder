//! Handles: the identifiers of the objects of a drawing, unique within it (ADR-0002).

use std::num::NonZeroU64;

/// A handle: a non-zero number that identifies an object within one drawing. Files write it in
/// hexadecimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle(NonZeroU64);

impl Handle {
    /// The handle with value `value`; `None` for 0, which no object has.
    #[must_use]
    pub fn new(value: u64) -> Option<Self> {
        NonZeroU64::new(value).map(Self)
    }

    /// The numeric value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0.get()
    }

    /// The uppercase hexadecimal form without prefix.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("{:X}", self.value())
    }

    /// Reads the hexadecimal form: one to sixteen hexadecimal digits, no sign, no prefix, not 0.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        if text.is_empty() || text.len() > 16 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u64::from_str_radix(text, 16).ok().and_then(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::Handle;

    #[test]
    fn hex_round_trips_and_rejects_what_is_not_a_handle() {
        let h = Handle::new(0x2AF).unwrap();
        assert_eq!(h.to_hex(), "2AF");
        assert_eq!(Handle::from_hex("2af"), Some(h));
        for bad in ["", "0", "+1", "-1", "0x1", "G", "11111111111111111"] {
            assert_eq!(Handle::from_hex(bad), None, "{bad}");
        }
        assert_eq!(Handle::new(0), None);
    }
}
