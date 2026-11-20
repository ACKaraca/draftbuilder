//! The drawing-scoped settings survive a save and a reopen, and an unknown header variable of an
//! opened file survives a save (twice). The settings are read back with `draftbuilder.settings.get`
//! on the reopened drawing.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_command::{Invocation, Invoker};
use common::{Harness, quoted};
use serde_json::{Value, json};

/// The settings the round trip sets, with the value each reads back as.
fn expected() -> Vec<(&'static str, Value)> {
    vec![
        ("draft.layer.current", json!("PLAN")),
        ("draft.linetype.global_scale", json!(2.5)),
        ("draft.units.length_precision", json!(3)),
        ("draft.point.size", json!(1.5)),
        // The running snap set is stored as its bit mask: END is 1 and MID is 2.
        ("snap.object.running", json!(3)),
        ("draft.grid.on", json!(true)),
        ("draft.grid.spacing", json!([2.0, 3.0])),
    ]
}

/// The value a setting reads as on `document`, through the command of the product.
fn read(h: &Harness, document: &arc_doc::Document, name: &str) -> Value {
    let outcome = Invoker::new(&h.registry)
        .invoke(
            document,
            &h.session,
            Invocation::new("draftbuilder.settings.get", json!({"name": name})),
        )
        .unwrap();
    outcome.value["value"].clone()
}

#[test]
fn drawing_settings_survive_a_save_and_a_reopen() {
    let mut h = Harness::new();
    let output = h.path("settings.dxf");
    let text = format!(
        "NEW unit=meter\n\
         LAYER name=PLAN\n\
         SETVAR name=CLAYER value=PLAN\n\
         SETVAR name=LTSCALE value=2.5\n\
         SETVAR name=LUPREC value=3\n\
         SETVAR name=PDSIZE value=1.5\n\
         OSNAP END,MID\n\
         SETVAR name=GRIDMODE value=on\n\
         SETVAR name=GRIDUNIT value=2,3\n\
         SAVE path={}\n",
        quoted(&output)
    );
    let report = h.run(&text);
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    let reopened = h.reopen(&output);
    for (name, value) in expected() {
        assert_eq!(read(&h, &reopened, name), value, "{name}");
    }
}

#[test]
fn an_unknown_header_variable_survives_two_saves() {
    // The partner drawing is opened and saved; the saved file is opened in a new process-independent
    // document (a document holds one drawing) and saved again.
    let mut first = Harness::new();
    let partner = first.write(
        "partner_drawing.dxf",
        include_bytes!("data/partner_drawing.dxf"),
    );
    let saved_once = first.path("once.dxf");
    let report = first.run(&format!(
        "OPEN path={}\nSAVE path={}\n",
        quoted(&partner),
        quoted(&saved_once)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );

    let mut second = Harness::new();
    let copy = second.write("once.dxf", &std::fs::read(&saved_once).unwrap());
    let saved_twice = second.path("twice.dxf");
    let report = second.run(&format!(
        "OPEN path={}\nSAVE path={}\n",
        quoted(&copy),
        quoted(&saved_twice)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    for path in [&saved_once, &saved_twice] {
        let text = std::fs::read_to_string(path).unwrap();
        assert_eq!(
            common::header_value(&text, "$ARCTEST_NOTE").as_deref(),
            Some("partner-owned header value"),
            "{}",
            path.display()
        );
    }
}
