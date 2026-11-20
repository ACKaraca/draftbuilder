//! The settings model: typed and compatibility names, validation with typed errors, the listing,
//! drawing-scoped values that undo and read back from a DXF header, session values, the current
//! layer and the appearance new entities take from the settings.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_doc::{ColorSource, Value};
use arc_draft::types::entity;
use arc_io_dxf::Value as Dxf;
use common::{Env, FIXTURE, name_of};
use draftbuilder_commands::precision::ucs;
use draftbuilder_commands::settings::header::header_values;
use serde_json::{Value as Json, json};

/// A new drawing in meters with one line on layer 0 and no other content.
fn drawing(env: &mut Env) {
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.session.units = vec![
        *arc_base::Unit::by_name("meter").unwrap(),
        *arc_base::Unit::by_name("degree").unwrap(),
    ];
}

/// The layer identifier named `name`.
fn layer_named(env: &Env, name: &str) -> arc_base::ObjectId {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(arc_draft::types::LAYER)
        .unwrap()
        .map(|(_, o)| o.id)
        .find(|id| name_of(&snapshot, *id).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no layer {name}"))
}

/// The value of setting `name` through `draftbuilder.settings.get`.
fn get(env: &mut Env, name: &str) -> Json {
    env.run("draftbuilder.settings.get", json!({"name": name}))["value"].clone()
}

/// The first line the command draws, its layer reference.
fn layer_of_last_line(env: &Env) -> Option<arc_base::ObjectId> {
    let snapshot = env.document.snapshot();
    let mut found = None;
    for (_, object) in snapshot.iter_type(arc_draft::types::LINE).unwrap() {
        if let Some(Some(Value::Reference(layer))) = object.values.get(entity::LAYER) {
            found = Some(*layer);
        }
    }
    found
}

#[test]
fn a_typed_name_and_its_compatibility_name_read_and_write_one_setting_and_undo() {
    let mut env = Env::new();
    drawing(&mut env);
    assert_eq!(get(&mut env, "draft.linetype.global_scale"), json!(1.0));
    let set = env.run(
        "draftbuilder.settings.setvar",
        json!({"name": "LTSCALE", "value": 2.5}),
    );
    assert_eq!(set["name"], "draft.linetype.global_scale");
    assert_eq!(set["compat"], "LTSCALE");
    assert_eq!(set["scope"], "drawing");
    assert_eq!(get(&mut env, "draft.linetype.global_scale"), json!(2.5));
    assert_eq!(get(&mut env, "ltscale"), json!(2.5));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(get(&mut env, "draft.linetype.global_scale"), json!(1.0));
}

#[test]
fn values_that_do_not_fit_the_setting_fail_with_typed_errors() {
    let mut env = Env::new();
    drawing(&mut env);
    let code = |env: &mut Env, command: &str, params: Json| {
        env.invoke(command, params).unwrap_err().code()
    };
    assert_eq!(
        code(
            &mut env,
            "draftbuilder.settings.set",
            json!({"name": "LUNITS", "value": 9})
        ),
        "db.cmd.setting_value"
    );
    assert_eq!(
        code(
            &mut env,
            "draftbuilder.settings.set",
            json!({"name": "NOSUCH", "value": 1})
        ),
        "db.cmd.setting_unknown"
    );
    assert_eq!(
        code(
            &mut env,
            "draftbuilder.settings.set",
            json!({"name": "INSUNITS", "value": 4})
        ),
        "db.cmd.setting_read_only"
    );
    assert_eq!(
        code(
            &mut env,
            "draftbuilder.settings.set",
            json!({"name": "LIMMIN", "value": [1, 2, 3]})
        ),
        "db.cmd.setting_value"
    );
    assert_eq!(
        code(
            &mut env,
            "draftbuilder.settings.set",
            json!({"name": "ORTHOMODE", "value": "maybe"})
        ),
        "db.cmd.setting_value"
    );
    assert_eq!(
        code(
            &mut env,
            "draftbuilder.settings.set",
            json!({"name": "LTSCALE", "value": f64::NAN})
        ),
        "db.cmd.setting_value"
    );
    assert_eq!(get(&mut env, "LUNITS"), json!(2));
}

#[test]
fn the_listing_filters_by_a_wildcard_over_both_names() {
    let mut env = Env::new();
    drawing(&mut env);
    let listed = env.run(
        "draftbuilder.settings.list",
        json!({"filter": "draft.units.*"}),
    );
    let names: Vec<&str> = listed["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "draft.units.length",
            "draft.units.length_precision",
            "draft.units.angle",
            "draft.units.angle_precision",
            "draft.units.insertion",
        ]
    );
    let by_compat = env.run("draftbuilder.settings.list", json!({"filter": "LUP*"}));
    assert_eq!(by_compat["settings"].as_array().unwrap().len(), 1);
    let all = env.run("draftbuilder.settings.list", json!({}));
    assert!(all["settings"].as_array().unwrap().len() > 30);
}

#[test]
fn the_current_layer_must_exist_follows_a_rename_and_cannot_be_frozen_or_deleted() {
    let mut env = Env::new();
    drawing(&mut env);
    let walls = env.run("draftbuilder.layer.create", json!({"name": "Walls"}))["layers"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let walls_id: arc_base::ObjectId = walls.parse().unwrap();
    let missing = env
        .invoke(
            "draftbuilder.settings.current_layer",
            json!({"layer": "Nowhere"}),
        )
        .unwrap_err();
    assert_eq!(missing.code(), "db.cmd.setting_value");
    env.run(
        "draftbuilder.settings.current_layer",
        json!({"layer": "Walls"}),
    );
    assert_eq!(get(&mut env, "CLAYER"), json!("Walls"));
    env.run(
        "draftbuilder.layer.rename",
        json!({"layer": walls, "name": "Partition"}),
    );
    assert_eq!(get(&mut env, "CLAYER"), json!("Partition"));
    let frozen = env
        .invoke(
            "draftbuilder.layer.modify",
            json!({"layers": [walls], "frozen": true}),
        )
        .unwrap_err();
    assert_eq!(frozen.code(), "db.cmd.current_layer");
    let deleted = env
        .invoke("draftbuilder.layer.delete", json!({"layers": [walls]}))
        .unwrap_err();
    assert_eq!(deleted.code(), "db.cmd.current_layer");
    assert_eq!(layer_named(&env, "Partition"), walls_id);
}

#[test]
fn a_frozen_layer_cannot_be_made_current() {
    let mut env = Env::new();
    drawing(&mut env);
    let other = env.run("draftbuilder.layer.create", json!({"name": "Notes"}))["layers"][0]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [other], "frozen": true}),
    );
    let error = env
        .invoke(
            "draftbuilder.settings.current_layer",
            json!({"layer": "Notes"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.layer_frozen");
}

#[test]
fn a_line_drawn_after_set_current_layer_lands_on_that_layer() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    env.run(
        "draftbuilder.settings.current_layer",
        json!({"layer": "Walls"}),
    );
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [4.0, 0.0]]}),
    );
    assert_eq!(layer_of_last_line(&env), Some(layer_named(&env, "Walls")));
}

#[test]
fn the_appearance_settings_give_new_entities_their_color_and_transparency_sources() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "CECOLOR", "value": 1}),
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "draft.transparency.current", "value": 25}),
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "CELWEIGHT", "value": 50}),
    );
    env.run(
        "draftbuilder.draw.line",
        json!({"points": [[0.0, 0.0], [4.0, 0.0]]}),
    );
    let snapshot = env.document.snapshot();
    let object = snapshot
        .iter_type(arc_draft::types::LINE)
        .unwrap()
        .map(|(_, o)| o.clone())
        .last()
        .unwrap();
    assert_eq!(
        object.values[entity::COLOR_BY],
        Some(Value::Enum(arc_draft::types::By::Object as u32))
    );
    match &object.values[entity::COLOR] {
        Some(Value::Color(color)) => assert_eq!(color.source, Some(ColorSource::Indexed(1))),
        other => panic!("color {other:?}"),
    }
    assert_eq!(object.values[entity::TRANSPARENCY], Some(Value::F64(0.25)));
    assert_eq!(object.values[entity::LINEWEIGHT], Some(Value::F64(0.5)));
}

#[test]
fn a_session_value_is_set_and_read_but_never_undone_or_stored_in_the_drawing() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "CMDECHO", "value": false}),
    );
    let report = env.run("draftbuilder.settings.get", json!({"name": "command.echo"}));
    assert_eq!(report["value"], json!(false));
    assert_eq!(report["scope"], "session");
    let snapshot = env.document.snapshot();
    assert!(
        snapshot
            .foreign_rows()
            .all(|row| row.source_id.as_deref() != Some("command.echo"))
    );
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "CMDECHO", "value": true}),
    );
}

/// A DXF file of the fixture whose header carries drawing settings and a coordinate system.
fn fixture_with_header() -> String {
    FIXTURE.replacen(
        "  9\n$INSUNITS\n",
        "  9\n$LTSCALE\n 40\n2.5\n  9\n$CLAYER\n  8\nWALLS\n  9\n$LUPREC\n 70\n2\n  9\n$UCSORG\n 10\n1.0\n 20\n2.0\n 30\n0.0\n  9\n$UCSXDIR\n 10\n0.0\n 20\n1.0\n 30\n0.0\n  9\n$INSUNITS\n",
        1,
    )
}

#[test]
fn drawing_settings_and_the_coordinate_system_read_back_from_a_dxf_header() {
    let mut env = Env::new();
    let path = env.write("header.dxf", fixture_with_header().as_bytes());
    env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    assert_eq!(get(&mut env, "LTSCALE"), json!(2.5));
    assert_eq!(get(&mut env, "LUPREC"), json!(2));
    assert_eq!(get(&mut env, "CLAYER"), json!("WALLS"));
    let system = ucs::current(&env.document.snapshot());
    assert_eq!(system.origin, [1.0, 2.0]);
    assert!((system.x_axis[0]).abs() < 1e-12 && (system.x_axis[1] - 1.0).abs() < 1e-12);
    let header = header_values(&env.document.snapshot());
    let ltscale = header.iter().find(|v| v.name == "$LTSCALE").unwrap();
    assert_eq!(ltscale.values[0].value, Dxf::Double(2.5));
    let clayer = header.iter().find(|v| v.name == "$CLAYER").unwrap();
    assert_eq!(clayer.values[0].value, Dxf::Text("WALLS".to_owned()));
    let origin = header.iter().find(|v| v.name == "$UCSORG").unwrap();
    assert_eq!(origin.values[0].value, Dxf::Double(1.0));
}

#[test]
fn a_header_value_changed_in_the_session_is_undone_with_its_command() {
    let mut env = Env::new();
    let path = env.write("undo.dxf", fixture_with_header().as_bytes());
    env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    env.run(
        "draftbuilder.settings.set",
        json!({"name": "LTSCALE", "value": 3.0}),
    );
    let header = header_values(&env.document.snapshot());
    let ltscale = header.iter().find(|v| v.name == "$LTSCALE").unwrap();
    assert_eq!(ltscale.values[0].value, Dxf::Double(3.0));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(get(&mut env, "LTSCALE"), json!(2.5));
}

/// A command line over the environment, for the typed and prompt forms.
struct Typed {
    line: arc_command::CommandLine,
    seed: u64,
}

impl Typed {
    fn new() -> Self {
        Self {
            line: arc_command::CommandLine::new(),
            seed: 500,
        }
    }

    fn submit(&mut self, env: &Env, text: &str) -> arc_command::Submitted {
        self.seed += 1;
        let localized = std::collections::BTreeMap::new();
        let target = arc_command::Target {
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

/// The value of a command that ran from the line.
fn ran(submitted: arc_command::Submitted) -> Json {
    match submitted {
        arc_command::Submitted::Ran(outcome) => outcome.value,
        other => panic!("the command did not run: {other:?}"),
    }
}

#[test]
fn setvar_prompt_form_asks_the_name_then_the_value_and_undoes() {
    let mut env = Env::new();
    drawing(&mut env);
    let mut line = Typed::new();
    assert!(matches!(
        line.submit(&env, "setvar"),
        arc_command::Submitted::Prompt(_)
    ));
    assert!(matches!(
        line.submit(&env, "LTSCALE"),
        arc_command::Submitted::Prompt(_)
    ));
    let result = ran(line.submit(&env, "2.5"));
    assert_eq!(result["value"], json!(2.5));
    assert_eq!(get(&mut env, "draft.linetype.global_scale"), json!(2.5));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(get(&mut env, "draft.linetype.global_scale"), json!(1.0));
}

#[test]
fn clayer_prompt_form_makes_the_named_layer_current() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    let mut line = Typed::new();
    assert!(matches!(
        line.submit(&env, "clayer"),
        arc_command::Submitted::Prompt(_)
    ));
    let result = ran(line.submit(&env, "Walls"));
    assert_eq!(result["value"], json!("Walls"));
    assert_eq!(get(&mut env, "CLAYER"), json!("Walls"));
}
