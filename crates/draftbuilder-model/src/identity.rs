//! The drawing fingerprint and the stable identifier of an object (ADR-0002).

use std::fmt;

use crate::handle::Handle;
use crate::sha1;

/// The namespace of DraftBuilder's name-based identifiers: a fixed version 4 UUID chosen once.
/// Changing it changes every stable identifier.
const NAMESPACE: [u8; 16] = [
    0x6b, 0x1e, 0x4d, 0x0a, 0x93, 0x57, 0x4c, 0x21, 0x8f, 0xd6, 0x2a, 0x70, 0xe4, 0x19, 0xb3, 0x5c,
];

/// A drawing's fingerprint: a 128-bit GUID that stays the same when the drawing is saved again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(u128);

impl Fingerprint {
    /// The fingerprint with the 128 bits `bits`. The null GUID (all zero) is not a fingerprint.
    #[must_use]
    pub fn new(bits: u128) -> Option<Self> {
        (bits != 0).then_some(Self(bits))
    }

    /// The 128 bits.
    #[must_use]
    pub fn bits(self) -> u128 {
        self.0
    }

    /// Reads the textual form files use: 32 hexadecimal digits in the groups 8-4-4-4-12,
    /// optionally inside braces. The null GUID is refused.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let inner = text.trim();
        let inner = inner
            .strip_prefix('{')
            .and_then(|t| t.strip_suffix('}'))
            .unwrap_or(inner);
        let groups: Vec<&str> = inner.split('-').collect();
        let widths = [8, 4, 4, 4, 12];
        if groups.len() != widths.len()
            || groups
                .iter()
                .zip(widths)
                .any(|(g, w)| g.len() != w || !g.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return None;
        }
        u128::from_str_radix(&groups.concat(), 16)
            .ok()
            .and_then(Self::new)
    }

    /// The form files use: uppercase, in braces.
    #[must_use]
    pub fn to_braced(self) -> String {
        format!("{{{}}}", hyphenated(self.0).to_ascii_uppercase())
    }
}

fn hyphenated(bits: u128) -> String {
    let s = format!("{bits:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &s[..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..]
    )
}

/// The stable identifier of an object: a version 5 UUID over the DraftBuilder namespace, the
/// drawing's fingerprint and the object's handle. It is computed, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableId(u128);

impl StableId {
    /// The 128 bits.
    #[must_use]
    pub fn bits(self) -> u128 {
        self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hyphenated(self.0))
    }
}

/// The version 5 UUID of `name` in `namespace` (RFC 9562, section 5.5).
fn uuid_v5(namespace: &[u8; 16], name: &[u8]) -> u128 {
    let mut input = Vec::with_capacity(16 + name.len());
    input.extend_from_slice(namespace);
    input.extend_from_slice(name);
    let hash = sha1::digest(&input);
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0F) | 0x50;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    u128::from_be_bytes(bytes)
}

/// The stable identifier of the object with `handle` in the drawing with `fingerprint`.
#[must_use]
pub fn stable_id(fingerprint: Fingerprint, handle: Handle) -> StableId {
    let mut name = [0_u8; 24];
    name[..16].copy_from_slice(&fingerprint.bits().to_be_bytes());
    name[16..].copy_from_slice(&handle.value().to_be_bytes());
    StableId(uuid_v5(&NAMESPACE, &name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_5_matches_the_published_example() {
        // RFC 9562 appendix A.4 style check: the DNS namespace and the name "python.org".
        let dns = [
            0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4,
            0x30, 0xc8,
        ];
        assert_eq!(
            hyphenated(uuid_v5(&dns, b"python.org")),
            "886313e1-3b8a-5372-9b90-0c9aee199e5d"
        );
    }

    #[test]
    fn the_stable_id_depends_on_the_fingerprint_and_the_handle_only() {
        let f = Fingerprint::parse("{0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F9}").unwrap();
        let g = Fingerprint::new(f.bits() ^ 1).unwrap();
        let h = Handle::new(0x1F).unwrap();
        let id = stable_id(f, h);
        assert_eq!(id, stable_id(f, h));
        assert_ne!(id, stable_id(g, h));
        assert_ne!(id, stable_id(f, Handle::new(0x20).unwrap()));
        let text = id.to_string();
        assert_eq!(text.as_bytes()[14], b'5', "version nibble: {text}");
        assert!(
            matches!(text.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
            "{text}"
        );
    }

    #[test]
    fn fingerprints_parse_and_print_the_file_form() {
        let f = Fingerprint::parse("0a1b2c3d-4e5f-6071-8293-a4b5c6d7e8f9").unwrap();
        assert_eq!(f.to_braced(), "{0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F9}");
        assert_eq!(Fingerprint::parse(&f.to_braced()), Some(f));
        for bad in [
            "",
            "{00000000-0000-0000-0000-000000000000}",
            "0a1b2c3d4e5f607182 93a4b5c6d7e8f9",
            "{0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F}",
            "{0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8FG}",
        ] {
            assert_eq!(Fingerprint::parse(bad), None, "{bad}");
        }
    }
}
