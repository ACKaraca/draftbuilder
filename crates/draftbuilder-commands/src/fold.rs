//! The fold of names: the one comparison that every name of the product uses, for layer names,
//! linetype names, the names purge protects, the names a preserved record contains and the values
//! of selection filters.
//!
//! The fold maps each character to its uppercase form, one character at a time (the standard
//! library's `char::to_uppercase`, which may give more than one character, as `ß` gives `SS`). The
//! result does not depend on where a character stands in the name, so the same letter folds the
//! same way everywhere. There is no trimming and no normalization: a leading space is part of the
//! name, and a composed letter differs from its decomposed form.
//!
//! Turkish letters follow from the rule, without a locale: `i` folds to `I` and dotted `İ` stays
//! `İ`, so `çizgi` and `ÇIZGI` are one name, `ÇİZGİ` is another name, `çızgı` (dotless `ı`) is the
//! same name as `çizgi`, and `Straße` is the same name as `STRASSE`. Lookup by name and the
//! uniqueness of names both use this fold, so they agree.

/// The fold of `text`: each character replaced by its uppercase form, in order.
#[must_use]
pub fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_uppercase).collect()
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn turkish_i_follows_the_one_rule_in_both_directions() {
        assert_eq!(fold("çizgi"), fold("ÇIZGI"));
        assert_ne!(fold("ÇİZGİ"), fold("çizgi"));
        assert_eq!(fold("çızgı"), fold("çizgi"));
    }

    #[test]
    fn a_letter_with_a_multi_character_uppercase_form_folds_the_same_everywhere() {
        assert_eq!(fold("Straße"), fold("STRASSE"));
        assert_eq!(fold("xSSx"), fold("xßx"));
    }

    #[test]
    fn nothing_is_trimmed_or_normalized() {
        assert_ne!(fold(" Walls"), fold("Walls"));
        assert_ne!(fold("e\u{301}"), fold("\u{e9}"));
    }
}
