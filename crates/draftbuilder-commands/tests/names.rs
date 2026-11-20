//! The product's name resolver, headless: a command registered after the resolver is installed resolves
//! by its prefixed identifier, the built-in and English forms do not read user aliases, and an alias cannot
//! shadow a vocabulary name.

#![allow(clippy::unwrap_used, reason = "test code")]

use arc_command::{CommandSpec, Context, Permission, UndoBehavior};
use draftbuilder_commands::names::resolve;
use draftbuilder_commands::registry;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoParams {}

fn later(_: &mut Context<'_>, _: NoParams) -> arc_base::Result<bool> {
    Ok(true)
}

#[test]
fn a_command_registered_after_the_resolver_is_installed_resolves_by_its_prefixed_identifier() {
    let mut registry = registry().unwrap();
    registry
        .register(
            CommandSpec::new(
                "draftbuilder.test.later",
                "db-cmd-draw-line-label",
                "db-cmd-draw-line-description",
                later,
            )
            .with_undo(UndoBehavior::NotUndoable)
            .with_permissions([Permission::Read]),
        )
        .unwrap();
    assert_eq!(
        resolve(&registry, ".draftbuilder.test.later").unwrap().id,
        "draftbuilder.test.later"
    );
    assert_eq!(
        resolve(&registry, "_draftbuilder.test.later").unwrap().id,
        "draftbuilder.test.later"
    );
    assert_eq!(
        resolve(&registry, "-draftbuilder.test.later").unwrap().id,
        "draftbuilder.test.later"
    );
    // The unprefixed identifier still resolves through the registry's own rule.
    assert_eq!(
        resolve(&registry, "draftbuilder.test.later").unwrap().id,
        "draftbuilder.test.later"
    );
}

#[test]
fn a_prefixed_name_of_a_command_the_registry_does_not_hold_is_unknown() {
    let registry = registry().unwrap();
    assert_eq!(
        resolve(&registry, ".draftbuilder.test.absent")
            .unwrap_err()
            .code(),
        "cmd.unknown"
    );
    assert_eq!(
        resolve(&registry, "_NOSUCHNAME").unwrap_err().code(),
        "cmd.unknown"
    );
}

#[test]
fn a_user_alias_cannot_shadow_a_vocabulary_name_and_the_built_in_form_ignores_user_aliases() {
    let mut registry = registry().unwrap();
    // `M` is the vocabulary alias of MOVE, so a user alias of the same text is refused.
    assert_eq!(
        registry
            .add_user_alias("m", "draftbuilder.drawing.open")
            .unwrap_err()
            .code(),
        "cmd.duplicate"
    );
    assert_eq!(
        resolve(&registry, "M").unwrap().id,
        "draftbuilder.modify.move"
    );
    registry
        .add_user_alias("zap", "draftbuilder.modify.move")
        .unwrap();
    assert_eq!(
        resolve(&registry, "zap").unwrap().id,
        "draftbuilder.modify.move"
    );
    assert_eq!(
        resolve(&registry, "_zap").unwrap_err().code(),
        "cmd.unknown"
    );
    assert_eq!(
        resolve(&registry, ".zap").unwrap_err().code(),
        "cmd.unknown"
    );
}
