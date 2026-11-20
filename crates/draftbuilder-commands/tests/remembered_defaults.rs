//! Remembered defaults (DB-CMD-004): a command stores the value it ran with, and the prompt that asks
//! the same parameter next offers it. A stored value never fails a command, and a command that fails
//! stores nothing.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code"
)]

mod common;

use std::collections::BTreeMap;

use arc_base::Unit;
use arc_command::{CommandLine, Prompt, PromptInput, Submitted, Target};
use common::Env;
use draftbuilder_commands::settings::remember::{CIRCLE_RADIUS, OFFSET_DISTANCE, POLYGON_SIDES};
use serde_json::{Value as Json, json};

/// A drawing in `unit`; typed numbers read in its length unit, as the script runner has them.
fn new_drawing(unit: &str) -> Env {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": unit}));
    env.session.units = vec![
        *Unit::by_name(unit).unwrap(),
        *Unit::by_name("degree").unwrap(),
    ];
    env
}

/// Typed lines over the environment, as the command line receives them.
struct Typed {
    line: CommandLine,
    seed: u64,
}

impl Typed {
    fn new() -> Self {
        Self {
            line: CommandLine::new(),
            seed: 2_000,
        }
    }

    fn submit(&mut self, env: &Env, text: &str) -> Submitted {
        self.seed += 1;
        let localized = BTreeMap::new();
        let target = Target {
            registry: &env.commands,
            document: &env.document,
            session: &env.session,
            selection: &[],
            localized: &localized,
            seed: self.seed,
        };
        self.line.submit(&target, text).unwrap()
    }
}

/// The prompt a submission asks; any other result is a failure of the test.
fn asked(submitted: Submitted) -> Prompt {
    match submitted {
        Submitted::Prompt(prompt) => prompt,
        other => panic!("no prompt was asked: {other:?}"),
    }
}

fn setting_of(env: &Env, key: &str) -> Option<Json> {
    env.session.setting(key)
}

#[test]
fn a_circle_radius_is_offered_the_next_time_in_meters() {
    let env = new_drawing("meter");
    let mut typed = Typed::new();
    asked(typed.submit(&env, "circle"));
    asked(typed.submit(&env, "0,0"));
    assert!(matches!(typed.submit(&env, "2"), Submitted::Ran(_)));
    assert_eq!(setting_of(&env, CIRCLE_RADIUS), Some(json!(2.0)));

    let mut again = Typed::new();
    asked(again.submit(&env, "circle"));
    let radius = asked(again.submit(&env, "0,0"));
    assert_eq!(radius.key, "radius");
    assert_eq!(radius.default, Some(PromptInput::Distance(2.0)));
    env.session.set_setting(CIRCLE_RADIUS, json!(0.5)).unwrap();
    let mut third = Typed::new();
    asked(third.submit(&env, "circle"));
    let radius = asked(third.submit(&env, "0,0"));
    assert_eq!(radius.default, Some(PromptInput::Distance(0.5)));
}

#[test]
fn a_remembered_length_is_stored_in_meters_whatever_the_drawing_unit() {
    // A millimeter drawing: a radius of 1000 units is 1 meter.
    let mut env = new_drawing("millimeter");
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 1000.0}}),
    );
    let stored = setting_of(&env, CIRCLE_RADIUS).unwrap().as_f64().unwrap();
    assert!((stored - 1.0).abs() < 1e-12, "{stored}");
}

#[test]
fn a_value_outside_the_range_of_the_prompt_is_not_offered() {
    // The sides prompt admits whole numbers from 3 to 1024; the default stays 4 otherwise.
    let env = new_drawing("meter");
    for (stored, offered) in [
        (json!(2), Some(4.0)),
        (json!(5.5), Some(4.0)),
        (json!(1025), Some(4.0)),
        (json!(6), Some(6.0)),
    ] {
        env.session
            .set_setting(POLYGON_SIDES, stored.clone())
            .unwrap();
        let mut typed = Typed::new();
        let prompt = asked(typed.submit(&env, "polygon"));
        assert_eq!(prompt.key, "sides");
        let default = match prompt.default {
            Some(PromptInput::Number(quantity)) => Some(quantity.value()),
            _ => None,
        };
        assert_eq!(default, offered, "stored {stored}");
    }
}

#[test]
fn a_command_that_fails_stores_nothing() {
    let mut env = new_drawing("meter");
    let missing = "0190a1b2-0000-7000-8000-000000000001";
    let result = env.invoke(
        "draftbuilder.modify.offset",
        json!({"objects": [missing], "distance": 7.0}),
    );
    assert!(result.is_err());
    assert!(setting_of(&env, OFFSET_DISTANCE).is_none());
}

#[test]
fn a_full_settings_store_does_not_fail_the_command() {
    // The session holds the most settings it may; the run still succeeds and stores nothing.
    let mut env = new_drawing("meter");
    for n in 0..arc_command::settings::MAX_SETTINGS {
        env.session
            .set_setting(&format!("draftbuilder.test.key_{n}"), json!(n))
            .unwrap();
    }
    env.run(
        "draftbuilder.draw.circle",
        json!({"shape": {"mode": "center", "center": [0.0, 0.0], "radius": 3.0}}),
    );
    assert!(setting_of(&env, CIRCLE_RADIUS).is_none());
}
