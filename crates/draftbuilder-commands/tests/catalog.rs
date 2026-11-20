//! The Turkish catalog of draftbuilder-commands: every message of the English catalog has a Turkish
//! message with the same identifier and the same placeholders, and no Turkish message is unknown.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::BTreeMap;

const EN: &str = include_str!("../../../locales/en-US/draftbuilder-commands.ftl");
const TR: &str = include_str!("../../../locales/tr-TR/draftbuilder-commands.ftl");

/// Each message identifier of a catalog with its placeholders, sorted.
fn messages(catalog: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for line in catalog.lines() {
        if line.starts_with(' ') || line.starts_with('#') {
            continue;
        }
        let Some((id, value)) = line.split_once(" = ") else {
            continue;
        };
        let mut placeholders: Vec<String> = value
            .split("{$")
            .skip(1)
            .filter_map(|rest| rest.split('}').next().map(str::to_owned))
            .collect();
        placeholders.sort();
        assert!(
            out.insert(id.to_owned(), placeholders).is_none(),
            "duplicate identifier {id}"
        );
    }
    out
}

#[test]
fn every_english_message_has_a_turkish_message_with_the_same_placeholders() {
    let english = messages(EN);
    let turkish = messages(TR);
    let missing: Vec<&String> = english
        .keys()
        .filter(|id| !turkish.contains_key(*id))
        .collect();
    assert!(missing.is_empty(), "missing in tr-TR: {missing:?}");
    let unknown: Vec<&String> = turkish
        .keys()
        .filter(|id| !english.contains_key(*id))
        .collect();
    assert!(unknown.is_empty(), "unknown in tr-TR: {unknown:?}");
    for (id, placeholders) in &english {
        assert_eq!(&turkish[id], placeholders, "placeholders of {id}");
    }
}
