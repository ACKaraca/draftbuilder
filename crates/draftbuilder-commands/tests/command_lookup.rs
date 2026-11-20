//! The command line's lookups, headless: input recall and history search, the history length and
//! the suggestions, the palette search and help, through the interaction controller.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use draftbuilder_commands::interaction::{
    Controller, CrossingPicker, Event, LineRole, Outputs, SuggestionKind,
};
use serde_json::json;

use common::Env;

/// A new drawing and a controller for it.
fn setup() -> (Env, Controller) {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    (env, Controller::new().unwrap())
}

fn send(env: &Env, controller: &mut Controller, event: Event) -> Outputs {
    controller.handle(&env.document, event, &CrossingPicker)
}

fn submit(env: &Env, controller: &mut Controller, text: &str) -> Outputs {
    send(env, controller, Event::Submit(text.to_owned()))
}

fn recall_previous(env: &Env, controller: &mut Controller) -> Option<String> {
    send(env, controller, Event::RecallPrevious).recalled
}

fn recall_next(env: &Env, controller: &mut Controller) -> Option<String> {
    send(env, controller, Event::RecallNext).recalled
}

#[test]
fn recall_walks_the_input_lines_newest_first_and_empties_the_line_past_the_newest() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "first");
    submit(&env, &mut c, "second");
    submit(&env, &mut c, "third");
    assert_eq!(recall_previous(&env, &mut c).as_deref(), Some("third"));
    assert_eq!(recall_previous(&env, &mut c).as_deref(), Some("second"));
    assert_eq!(recall_previous(&env, &mut c).as_deref(), Some("first"));
    assert_eq!(recall_previous(&env, &mut c), None, "no older line");
    assert_eq!(recall_next(&env, &mut c).as_deref(), Some("second"));
    assert_eq!(recall_next(&env, &mut c).as_deref(), Some("third"));
    assert_eq!(recall_next(&env, &mut c).as_deref(), Some(""));
    assert_eq!(recall_next(&env, &mut c), None, "not recalling any more");
}

#[test]
fn a_submitted_line_restarts_the_recall_from_the_newest() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "first");
    submit(&env, &mut c, "second");
    recall_previous(&env, &mut c);
    submit(&env, &mut c, "third");
    assert_eq!(recall_previous(&env, &mut c).as_deref(), Some("third"));
}

#[test]
fn clear_history_removes_the_lines_and_the_recall_position() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "first");
    recall_previous(&env, &mut c);
    let out = send(&env, &mut c, Event::ClearHistory);
    assert!(out.history.is_empty());
    assert_eq!(recall_previous(&env, &mut c), None);
    assert_eq!(recall_next(&env, &mut c), None);
}

#[test]
fn history_search_finds_input_lines_without_regard_to_case_newest_first() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "Alpha one");
    submit(&env, &mut c, "beta");
    submit(&env, &mut c, "ALPHA two");
    assert_eq!(c.search_history("alpha"), vec!["ALPHA two", "Alpha one"]);
    assert!(c.search_history("   ").is_empty());
    assert!(c.search_history(&"x".repeat(65)).is_empty());
}

#[test]
fn the_history_length_setting_keeps_only_that_many_lines() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "SETVAR name=command.history.length value=10");
    for i in 0..12 {
        submit(&env, &mut c, &format!("line {i}"));
    }
    let out = submit(&env, &mut c, "");
    assert_eq!(out.history.len(), 10);
    assert_eq!(out.history.last().unwrap().role, LineRole::Input);
    assert_eq!(out.history.last().unwrap().text, "line 11");
}

#[test]
fn a_completed_command_is_suggested_first_when_the_text_starts_the_same() {
    let (env, mut c) = setup();
    submit(&env, &mut c, "AUDIT");
    let suggestions = c.suggest("a");
    assert_eq!(suggestions[0].text, "AUDIT");
    assert_eq!(suggestions[0].kind, SuggestionKind::Command);
    assert_eq!(c.suggest("")[0].text, "AUDIT");
}

#[test]
fn a_prefix_suggests_commands_and_settings_with_their_kinds() {
    let (_env, c) = setup_no_drawing();
    let suggestions = c.suggest("MOV");
    assert!(suggestions.iter().any(|s| s.text == "MOVE"
        && s.kind == SuggestionKind::Command
        && s.command.as_deref() == Some("draftbuilder.modify.move")));
    let settings = c.suggest("draft.ort");
    assert!(settings.iter().any(|s| s.text == "draft.ortho"
        && s.kind == SuggestionKind::Setting
        && s.command.is_none()));
}

fn setup_no_drawing() -> (Env, Controller) {
    (Env::new(), Controller::new().unwrap())
}

#[test]
fn a_misspelled_name_is_corrected_to_the_closest_command_when_autocorrect_is_on() {
    let (_env, c) = setup_no_drawing();
    let suggestions = c.suggest("MOVEE");
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].kind, SuggestionKind::Corrected);
    assert_eq!(suggestions[0].text, "MOVE");
    assert_eq!(
        suggestions[0].command.as_deref(),
        Some("draftbuilder.modify.move")
    );
}

#[test]
fn autocorrect_and_autocomplete_turn_off_by_their_settings() {
    let (env, mut c) = setup();
    submit(
        &env,
        &mut c,
        "SETVAR name=command.autocorrect.on value=false",
    );
    assert!(c.suggest("MOVEE").is_empty());
    submit(
        &env,
        &mut c,
        "SETVAR name=command.autocomplete.on value=false",
    );
    assert!(c.suggest("MOV").is_empty());
}

#[test]
fn the_palette_ranks_exact_names_first_and_lists_the_aliases() {
    let (_env, c) = setup_no_drawing();
    let found = c.search_commands("move", 10);
    assert_eq!(found[0].id, "draftbuilder.modify.move");
    assert!(found[0].aliases.contains(&"M".to_owned()));
    assert!(found[0].aliases.contains(&"MOVE".to_owned()));
    let by_label = c.search_commands("open drawing", 50);
    assert!(
        by_label
            .iter()
            .any(|e| e.label_id == "db-cmd-open-drawing-label")
    );
    assert!(c.search_commands("move", 1).len() == 1);
    assert!(c.search_commands(&"m".repeat(65), 10).is_empty());
}

#[test]
fn help_gives_a_command_in_any_form_and_the_active_prompt_for_the_active_command() {
    let (env, mut c) = setup();
    let help = c.help(Some("M")).unwrap();
    assert_eq!(help.command.id, "draftbuilder.modify.move");
    assert_eq!(
        help.command.description_id,
        "db-cmd-modify-move-description"
    );
    assert!(help.prompt.is_none());
    assert!(c.help(Some("NOSUCHCOMMAND")).is_none());
    assert!(c.help(None).is_none(), "no active command");
    submit(&env, &mut c, "MOVE");
    let active = c.help(None).unwrap();
    assert_eq!(active.command.id, "draftbuilder.modify.move");
    assert!(active.prompt.is_some());
}
