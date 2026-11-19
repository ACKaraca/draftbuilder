//! The product vocabulary and the name forms of a typed command, headless: every form of a
//! command's name resolves to that command, an unknown name gives `cmd.unknown`, the built-in
//! form ignores names added at run time, and completion lists the product's commands and
//! aliases.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::BTreeMap;

use arc_command::Registry;
use arc_command::line::Span;
use draftbuilder_commands::names::{Form, complete_line, parse, resolve, resolve_with};
use draftbuilder_commands::registry;
use draftbuilder_commands::vocabulary::NAMES;

const MOVE: &str = "core.draft.entity.transform";
const OPEN: &str = "draftbuilder.drawing.open";

fn product() -> Registry {
    registry().unwrap()
}

/// Every form of a typed name resolves to `id` and reports the form it was typed in.
fn assert_resolves(registry: &Registry, names: &[&str], id: &str) {
    for name in names {
        let resolved = resolve(registry, name).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(resolved.id, id, "{name}");
    }
}

#[test]
fn every_name_form_of_an_existing_command_resolves_to_it() {
    let registry = product();
    // The typed name, its alias, the identifier, and each prefix form of each.
    let move_forms = [
        "MOVE",
        "move",
        "Move",
        "M",
        "m",
        "_MOVE",
        "_move",
        "-MOVE",
        "-m",
        ".MOVE",
        ".move",
        ".M",
        "_core.draft.entity.transform",
        "-core.draft.entity.transform",
        ".core.draft.entity.transform",
        "core.draft.entity.transform",
    ];
    assert_resolves(&registry, &move_forms, MOVE);
    let open_forms = [
        "OPEN",
        "open",
        "_OPEN",
        "-OPEN",
        ".OPEN",
        "draftbuilder.drawing.open",
        "-draftbuilder.drawing.open",
    ];
    assert_resolves(&registry, &open_forms, OPEN);
}

#[test]
fn the_form_of_each_name_is_reported() {
    let registry = product();
    assert_eq!(resolve(&registry, "MOVE").unwrap().form, Form::Typed);
    assert_eq!(resolve(&registry, "_MOVE").unwrap().form, Form::English);
    assert_eq!(resolve(&registry, "-MOVE").unwrap().form, Form::Prompt);
    assert_eq!(resolve(&registry, ".MOVE").unwrap().form, Form::Builtin);
    assert_eq!(parse("  -MOVE  "), (Form::Prompt, "MOVE"));
    assert_eq!(parse("_"), (Form::English, ""));
}

#[test]
fn an_unknown_name_gives_cmd_unknown_in_every_form() {
    let registry = product();
    for name in [
        "FROBNICATE",
        "_FROBNICATE",
        "-FROBNICATE",
        ".FROBNICATE",
        "",
        "_",
        "-",
        ".",
    ] {
        let error = resolve(&registry, name).unwrap_err();
        assert_eq!(error.code(), "cmd.unknown", "{name:?}");
    }
}

#[test]
fn the_built_in_form_ignores_names_added_at_run_time() {
    let mut registry = product();
    registry.add_user_alias("zap", MOVE).unwrap();
    assert_eq!(resolve(&registry, "zap").unwrap().id, MOVE);
    assert_eq!(resolve(&registry, "_zap").unwrap().id, MOVE);
    assert_eq!(
        resolve(&registry, ".zap").unwrap_err().code(),
        "cmd.unknown"
    );
}

#[test]
fn every_vocabulary_row_names_a_registered_command() {
    let registry = product();
    for row in NAMES {
        assert!(registry.get(row.id).is_some(), "{}", row.id);
        for name in row.typed.iter().chain(row.aliases.iter()) {
            assert_eq!(resolve(&registry, name).unwrap().id, row.id, "{name}");
        }
    }
}

#[test]
fn completion_lists_the_product_commands_and_aliases_with_their_prefix() {
    let registry = product();
    let texts = |line: &str, cursor: usize| -> Vec<String> {
        complete_line(&registry, line, cursor)
            .candidates
            .into_iter()
            .map(|c| c.text)
            .collect()
    };
    let plain = texts("MO", 2);
    assert!(plain.contains(&"move".to_owned()), "{plain:?}");
    let prompt = texts("-mo", 3);
    assert!(prompt.contains(&"-move".to_owned()), "{prompt:?}");
    let opens = texts("op", 2);
    assert!(opens.contains(&"open".to_owned()), "{opens:?}");
    let partial = "draftbuilder.drawing.pu";
    let ids = texts(partial, partial.len());
    assert!(
        ids.contains(&"draftbuilder.drawing.purge".to_owned()),
        "{ids:?}"
    );
}

#[test]
fn a_completed_prefixed_name_replaces_the_prefix_too() {
    let registry = product();
    let completion = complete_line(&registry, "-mo", 3);
    assert_eq!(completion.span, Some(Span { start: 0, end: 3 }));
}

#[test]
fn completion_ignores_the_whitespace_that_parsing_ignores() {
    let registry = product();
    let completion = complete_line(&registry, "  mo", 4);
    assert!(
        completion.candidates.iter().any(|c| c.text == "move"),
        "{completion:?}"
    );
    assert_eq!(completion.span, Some(Span { start: 2, end: 4 }));
    let prompt = complete_line(&registry, "\t-mo", 4);
    assert!(
        prompt.candidates.iter().any(|c| c.text == "-move"),
        "{prompt:?}"
    );
}

#[test]
fn the_built_in_form_offers_only_names_it_resolves() {
    let mut registry = product();
    registry.add_user_alias("zap", MOVE).unwrap();
    let plain = complete_line(&registry, "z", 1);
    assert!(
        plain.candidates.iter().any(|c| c.text == "zap"),
        "{plain:?}"
    );
    let builtin = complete_line(&registry, ".z", 2);
    assert!(
        builtin.candidates.iter().all(|c| c.text != ".zap"),
        "{builtin:?}"
    );
    assert_eq!(
        resolve(&registry, ".zap").unwrap_err().code(),
        "cmd.unknown"
    );
    let builtin = complete_line(&registry, ".mo", 3);
    assert!(
        builtin.candidates.iter().any(|c| c.text == ".move"),
        "{builtin:?}"
    );
}

#[test]
fn a_localized_name_shared_by_two_commands_is_ambiguous() {
    let registry = product();
    let localized = BTreeMap::from([
        (MOVE.to_owned(), "Relocate".to_owned()),
        (OPEN.to_owned(), "Relocate".to_owned()),
    ]);
    let error = resolve_with(&registry, &localized, "relocate").unwrap_err();
    assert_eq!(error.code(), "cmd.name.ambiguous");
    let unique = BTreeMap::from([(MOVE.to_owned(), "Relocate".to_owned())]);
    assert_eq!(
        resolve_with(&registry, &unique, "relocate").unwrap().id,
        MOVE
    );
}
