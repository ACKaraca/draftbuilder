//! The layer and property commands, headless: each command's exact result read back from the
//! document, the validation of its typed parameters, and undo of each change.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_doc::{ColorSource, Value};
use arc_draft::types::{LAYER, entity, layer};
use common::{Env, name_of};
use serde_json::{Value as Json, json};

/// A drawing with one line on layer 0 and its identifier.
fn drawing(env: &mut Env) -> String {
    env.run("draftbuilder.drawing.new", json!({}));
    let space = env.space().to_string();
    env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 0.0], "end": [5.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn layer_named(env: &Env, name: &str) -> arc_base::ObjectId {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .map(|(_, o)| o.id)
        .find(|id| name_of(&snapshot, *id).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no layer {name}"))
}

/// The value of one field of an object, by index.
fn field(env: &Env, id: arc_base::ObjectId, index: usize) -> Option<Value> {
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id).unwrap();
    snapshot.get(handle).unwrap().values[index].clone()
}

fn flag(env: &Env, id: arc_base::ObjectId, index: usize) -> bool {
    match field(env, id, index) {
        Some(Value::Bool(b)) => b,
        other => panic!("not a flag: {other:?}"),
    }
}

fn code_of(result: Result<Json, arc_base::Error>) -> &'static str {
    result.unwrap_err().code()
}

#[test]
fn create_sets_the_name_color_lineweight_description_and_plot_flag() {
    let mut env = Env::new();
    drawing(&mut env);
    let created = env.run(
        "draftbuilder.layer.create",
        json!({"name": "Walls", "color": 1, "lineweight": 0.5, "transparency": 0.25,
               "description": "exterior walls", "plottable": false}),
    );
    let id: arc_base::ObjectId = created["layers"][0].as_str().unwrap().parse().unwrap();
    assert_eq!(
        name_of(&env.document.snapshot(), id).as_deref(),
        Some("Walls")
    );
    match field(&env, id, layer::COLOR) {
        Some(Value::Color(color)) => {
            assert_eq!(color.source, Some(ColorSource::Indexed(1)));
            assert!(
                color.rgba.r > 0.99 && color.rgba.g < 0.01,
                "{:?}",
                color.rgba
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(field(&env, id, layer::LINEWEIGHT), Some(Value::F64(0.5)));
    assert_eq!(field(&env, id, layer::TRANSPARENCY), Some(Value::F64(0.25)));
    assert!(!flag(&env, id, layer::PLOTTABLE));
    assert_eq!(
        field(&env, id, layer::DESCRIPTION),
        Some(Value::String("exterior walls".to_owned()))
    );
    assert!(flag(&env, id, layer::ON));
}

#[test]
fn create_refuses_reserved_characters_bad_values_and_duplicates() {
    let mut env = Env::new();
    drawing(&mut env);
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.create", json!({"name": "A<B"}))),
        "db.cmd.name_invalid"
    );
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.create", json!({"name": ""}))),
        "db.cmd.name_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.create",
            json!({"name": "X", "color": 0})
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.create",
            json!({"name": "X", "transparency": 0.95})
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.create",
            json!({"name": "X", "lineweight": 11.0})
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.create",
            json!({"name": "X", "linetype": "NoSuchType"})
        )),
        "db.cmd.linetype_unknown"
    );
    env.run("draftbuilder.layer.create", json!({"name": "Dup"}));
    // Names are unique without regard to case: the product checks it on create and rename.
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.create", json!({"name": "dup"}))),
        "db.cmd.name_taken"
    );
}

#[test]
fn off_on_freeze_thaw_lock_unlock_act_on_the_layers_of_the_chosen_objects_and_undo() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Work"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Work"}),
    );
    let work = layer_named(&env, "Work");
    let cases = [
        ("draftbuilder.layer.off", layer::ON, false),
        ("draftbuilder.layer.freeze", layer::FROZEN, true),
        ("draftbuilder.layer.lock", layer::LOCKED, true),
    ];
    for (command, index, value) in cases {
        let result = env.run(command, json!({"objects": [line.clone()]}));
        assert_eq!(result["layers"], json!([work.to_string()]), "{command}");
        assert_eq!(flag(&env, work, index), value, "{command}");
    }
    env.run("draftbuilder.layer.on", json!({"objects": [line.clone()]}));
    env.run(
        "draftbuilder.layer.thaw",
        json!({"objects": [line.clone()]}),
    );
    env.run(
        "draftbuilder.layer.unlock",
        json!({"objects": [line.clone()]}),
    );
    assert!(flag(&env, work, layer::ON));
    assert!(!flag(&env, work, layer::FROZEN));
    assert!(!flag(&env, work, layer::LOCKED));

    // Six steps ran after the change to the layer. Undo runs them back, newest first: unlock,
    // then thaw, which restores the lock and the freeze.
    assert!(env.document.undo().wait().unwrap().value);
    assert!(flag(&env, work, layer::LOCKED));
    assert!(!flag(&env, work, layer::FROZEN));
    assert!(env.document.undo().wait().unwrap().value);
    assert!(flag(&env, work, layer::FROZEN));
    assert!(flag(&env, work, layer::LOCKED));
}

#[test]
fn modify_sets_the_layer_properties_and_refuses_out_of_range_values() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Plot"}));
    let plot = layer_named(&env, "Plot");
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [plot.to_string()], "plottable": false, "color": 7,
               "lineweight": 0.7, "transparency": 0.5, "description": "dimensions",
               "linetype": "Continuous", "frozen": true}),
    );
    assert!(!flag(&env, plot, layer::PLOTTABLE));
    assert!(flag(&env, plot, layer::FROZEN));
    assert_eq!(field(&env, plot, layer::LINEWEIGHT), Some(Value::F64(0.7)));
    assert_eq!(
        field(&env, plot, layer::TRANSPARENCY),
        Some(Value::F64(0.5))
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.modify",
            json!({"layers": [plot.to_string()], "transparency": 0.91})
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.modify",
            json!({"layers": [plot.to_string()]})
        )),
        "db.cmd.nothing_to_change"
    );
    // Undo restores the layer before the modify.
    assert!(env.document.undo().wait().unwrap().value);
    assert!(flag(&env, plot, layer::PLOTTABLE));
}

#[test]
fn change_and_match_move_objects_to_a_named_layer_or_the_layer_of_a_source() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Dims"}));
    let dims = layer_named(&env, "Dims");
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "dims"}),
    );
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(line.parse().unwrap()).unwrap();
    let values = snapshot.get(handle).unwrap().values.clone();
    assert_eq!(values[entity::LAYER], Some(Value::Reference(dims)));

    // A second object takes the layer of the first.
    let space = env.space().to_string();
    let other = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 1.0], "end": [5.0, 1.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.match",
        json!({"objects": [other.clone()], "source": line.clone()}),
    );
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(other.parse().unwrap()).unwrap();
    assert_eq!(
        snapshot.get(handle).unwrap().values[entity::LAYER],
        Some(Value::Reference(dims))
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.change",
            json!({"objects": [line.clone()], "layer": "Nope"})
        )),
        "db.cmd.layer_unknown"
    );
    assert!(env.document.undo().wait().unwrap().value);
}

#[test]
fn change_property_sets_the_appearance_with_its_source_and_undoes() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [line.clone()], "color": 3, "lineweight": 0.35,
               "transparency": 0.4, "linetype_scale": 2.0}),
    );
    let id: arc_base::ObjectId = line.parse().unwrap();
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id).unwrap();
    let values = snapshot.get(handle).unwrap().values.clone();
    assert_eq!(values[entity::COLOR_BY], Some(Value::Enum(2)));
    assert_eq!(values[entity::LINEWEIGHT_BY], Some(Value::Enum(2)));
    assert_eq!(values[entity::LINEWEIGHT], Some(Value::F64(0.35)));
    assert_eq!(values[entity::TRANSPARENCY], Some(Value::F64(0.4)));
    assert_eq!(values[entity::LINETYPE_SCALE], Some(Value::F64(2.0)));
    match &values[entity::COLOR] {
        Some(Value::Color(color)) => assert_eq!(color.source, Some(ColorSource::Indexed(3))),
        other => panic!("{other:?}"),
    }

    // By layer and by block set the source only.
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [line.clone()], "color": "by_layer", "lineweight": "by_block"}),
    );
    let snapshot = env.document.snapshot();
    let values = snapshot
        .get(snapshot.resolve(id).unwrap())
        .unwrap()
        .values
        .clone();
    assert_eq!(values[entity::COLOR_BY], Some(Value::Enum(0)));
    assert_eq!(values[entity::LINEWEIGHT_BY], Some(Value::Enum(1)));

    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.change",
            json!({"objects": [line.clone()], "linetype": "Missing"})
        )),
        "db.cmd.linetype_unknown"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.change",
            json!({"objects": [line.clone()], "color": 0})
        )),
        "db.cmd.value_invalid"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.change",
            json!({"objects": [line.clone()]})
        )),
        "db.cmd.nothing_to_change"
    );
    // Undo of the second change restores the object-held values.
    assert!(env.document.undo().wait().unwrap().value);
    let snapshot = env.document.snapshot();
    let values = snapshot
        .get(snapshot.resolve(id).unwrap())
        .unwrap()
        .values
        .clone();
    assert_eq!(values[entity::COLOR_BY], Some(Value::Enum(2)));
}

#[test]
fn match_property_copies_the_appearance_and_layer_of_the_source() {
    let mut env = Env::new();
    let source = drawing(&mut env);
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [source.clone()], "color": 5, "lineweight": 0.6, "transparency": 0.3}),
    );
    env.run("draftbuilder.layer.create", json!({"name": "Copy"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [source.clone()], "layer": "Copy"}),
    );
    let space = env.space().to_string();
    let target = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 2.0], "end": [5.0, 2.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.property.match",
        json!({"source": source.clone(), "objects": [target.clone()]}),
    );
    let snapshot = env.document.snapshot();
    let src = snapshot
        .get(snapshot.resolve(source.parse().unwrap()).unwrap())
        .unwrap()
        .values
        .clone();
    let dst = snapshot
        .get(snapshot.resolve(target.parse().unwrap()).unwrap())
        .unwrap()
        .values
        .clone();
    for index in [
        entity::LAYER,
        entity::COLOR_BY,
        entity::COLOR,
        entity::LINEWEIGHT,
        entity::LINEWEIGHT_BY,
        entity::TRANSPARENCY,
        entity::TRANSPARENCY_BY,
        entity::LINETYPE_SCALE,
    ] {
        assert_eq!(src[index], dst[index], "field {index}");
    }
    assert!(env.document.undo().wait().unwrap().value);
    let snapshot = env.document.snapshot();
    let dst = snapshot
        .get(snapshot.resolve(target.parse().unwrap()).unwrap())
        .unwrap()
        .values
        .clone();
    assert_ne!(dst[entity::LINEWEIGHT], src[entity::LINEWEIGHT]);
}

#[test]
fn commands_that_take_objects_refuse_a_layer_record() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Notentity"}));
    let plain = layer_named(&env, "Notentity").to_string();
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.change",
            json!({"objects": [plain.clone()], "color": 2})
        )),
        "draft.object.wrong_type"
    );
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.on", json!({"objects": [plain]}))),
        "draft.object.wrong_type"
    );
}

#[test]
fn the_typed_names_of_the_layer_and_property_commands_resolve_to_them() {
    let registry = draftbuilder_commands::registry().unwrap();
    let expected = [
        ("LAYER", "draftbuilder.layer.create"),
        ("LAYON", "draftbuilder.layer.on"),
        ("LAYOFF", "draftbuilder.layer.off"),
        ("LAYFRZ", "draftbuilder.layer.freeze"),
        ("LAYTHW", "draftbuilder.layer.thaw"),
        ("LAYLCK", "draftbuilder.layer.lock"),
        ("LAYULK", "draftbuilder.layer.unlock"),
        ("LAYMCH", "draftbuilder.layer.match"),
        ("LAYDEL", "draftbuilder.layer.delete_with_contents"),
        ("LAYMRG", "draftbuilder.layer.merge"),
        ("LINETYPE", "core.draft.linetype.load"),
        ("CHPROP", "draftbuilder.property.change"),
        ("CHANGE", "draftbuilder.property.change"),
        ("MATCHPROP", "draftbuilder.property.match"),
        ("FILTER", "draftbuilder.select.filter"),
    ];
    for (typed, id) in expected {
        let resolved = draftbuilder_commands::names::resolve(&registry, typed).unwrap();
        assert_eq!(resolved.id, id, "{typed}");
        let lower =
            draftbuilder_commands::names::resolve(&registry, &format!("_{}", typed.to_lowercase()))
                .unwrap();
        assert_eq!(lower.id, id, "_{typed}");
    }
}

/// Whether a layer named `name` exists.
fn has_layer(env: &Env, name: &str) -> bool {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .any(|(_, o)| name_of(&snapshot, o.id).as_deref() == Some(name))
}

/// The layer of an entity, as a reference.
fn layer_of_entity(env: &Env, id: &str) -> Value {
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id.parse().unwrap()).unwrap();
    snapshot.get(handle).unwrap().values[entity::LAYER]
        .clone()
        .unwrap()
}

#[test]
fn a_new_drawing_holds_layer_zero() {
    let mut env = Env::new();
    drawing(&mut env);
    assert!(has_layer(&env, "0"));
}

#[test]
fn layer_names_are_unique_without_regard_to_case_on_create_and_rename() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Walls"}));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.create", json!({"name": "walls"}))),
        "db.cmd.name_taken"
    );
    env.run("draftbuilder.layer.create", json!({"name": "Doors"}));
    let doors = layer_named(&env, "Doors").to_string();
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.rename",
            json!({"layer": doors.clone(), "name": "WALLS"})
        )),
        "db.cmd.name_taken"
    );
    // A layer may take the case of its own name.
    env.run(
        "draftbuilder.layer.rename",
        json!({"layer": doors.clone(), "name": "DOORS"}),
    );
    assert!(has_layer(&env, "DOORS"));
    assert!(env.document.undo().wait().unwrap().value);
    assert!(has_layer(&env, "Doors"));
}

#[test]
fn layer_zero_and_the_definition_point_layer_cannot_be_renamed_deleted_or_merged_away() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Defpoints"}));
    env.run("draftbuilder.layer.create", json!({"name": "Other"}));
    let zero = layer_named(&env, "0").to_string();
    let defpoints = layer_named(&env, "Defpoints").to_string();
    let other = layer_named(&env, "Other").to_string();
    for params in [
        json!({"layer": zero.clone(), "name": "Base"}),
        json!({"layer": defpoints.clone(), "name": "Base"}),
    ] {
        assert_eq!(
            code_of(env.invoke("draftbuilder.layer.rename", params)),
            "db.cmd.layer_special"
        );
    }
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.delete",
            json!({"layers": [zero.clone()]})
        )),
        "db.cmd.layer_special"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.delete_with_contents",
            json!({"layers": [defpoints.clone()]})
        )),
        "db.cmd.layer_special"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.merge",
            json!({"sources": [zero.clone()], "target": other.clone()})
        )),
        "db.cmd.layer_special"
    );
    assert!(has_layer(&env, "0") && has_layer(&env, "Defpoints"));
}

#[test]
fn delete_removes_only_unused_layers_and_undo_restores_them() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Spare"}));
    env.run("draftbuilder.layer.create", json!({"name": "Used"}));
    let used = layer_named(&env, "Used").to_string();
    let spare = layer_named(&env, "Spare").to_string();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line], "layer": "Used"}),
    );
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.delete", json!({"layers": [used]}))),
        "db.cmd.layer_in_use"
    );
    env.run("draftbuilder.layer.delete", json!({"layers": [spare]}));
    assert!(!has_layer(&env, "Spare"));
    assert!(env.document.undo().wait().unwrap().value);
    assert!(has_layer(&env, "Spare"));
}

#[test]
fn delete_with_contents_removes_the_objects_on_the_layer_inside_block_definitions_too() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Temp"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Temp"}),
    );
    let space = env.space().to_string();
    let inner = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 1.0], "end": [5.0, 1.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [inner.clone()], "layer": "Temp"}),
    );
    env.run(
        "core.draft.block.define",
        json!({"name": "HOLD", "objects": [inner.clone()]}),
    );
    let temp = layer_named(&env, "Temp").to_string();
    let removed = env.run(
        "draftbuilder.layer.delete_with_contents",
        json!({"layers": [temp]}),
    );
    let objects: Vec<String> = removed["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert!(
        objects.contains(&line) && objects.contains(&inner),
        "{removed}"
    );
    assert!(!has_layer(&env, "Temp"));
    assert!(
        env.document
            .snapshot()
            .resolve(inner.parse().unwrap())
            .is_err()
    );
    assert!(env.document.undo().wait().unwrap().value);
    assert!(has_layer(&env, "Temp"));
    assert!(
        env.document
            .snapshot()
            .resolve(inner.parse().unwrap())
            .is_ok()
    );
}

#[test]
fn merge_moves_objects_including_block_content_and_removes_the_sources() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Source"}));
    env.run("draftbuilder.layer.create", json!({"name": "Target"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Source"}),
    );
    let space = env.space().to_string();
    let inner = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 1.0], "end": [5.0, 1.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [inner.clone()], "layer": "Source"}),
    );
    env.run(
        "core.draft.block.define",
        json!({"name": "HOLD", "objects": [inner.clone()]}),
    );
    let source = layer_named(&env, "Source").to_string();
    let target = layer_named(&env, "Target").to_string();
    env.run(
        "draftbuilder.layer.merge",
        json!({"sources": [source], "target": target.clone()}),
    );
    assert!(!has_layer(&env, "Source"));
    assert_eq!(
        layer_of_entity(&env, &line),
        Value::Reference(layer_named(&env, "Target"))
    );
    assert_eq!(
        layer_of_entity(&env, &inner),
        Value::Reference(layer_named(&env, "Target"))
    );
    assert!(env.document.undo().wait().unwrap().value);
    assert!(has_layer(&env, "Source"));
    assert_eq!(
        layer_of_entity(&env, &line),
        Value::Reference(layer_named(&env, "Source"))
    );
}

#[test]
fn a_property_change_or_a_move_on_an_object_of_a_locked_layer_is_refused() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Locked"}));
    env.run("draftbuilder.layer.create", json!({"name": "Free"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Locked"}),
    );
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [line.clone()]}),
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.change",
            json!({"objects": [line.clone()], "color": 3})
        )),
        "draft.layer.locked"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.change",
            json!({"objects": [line.clone()], "layer": "Free"})
        )),
        "draft.layer.locked"
    );
    // The refused change left the object as it was; unlocking allows the change.
    env.run(
        "draftbuilder.layer.unlock",
        json!({"objects": [line.clone()]}),
    );
    env.run(
        "draftbuilder.property.change",
        json!({"objects": [line.clone()], "color": 3}),
    );
}

/// The layer of an entity, as its name.
fn layer_name_of_entity(env: &Env, id: &str) -> String {
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(id.parse().unwrap()).unwrap();
    match snapshot.get(handle).unwrap().values[entity::LAYER].clone() {
        Some(Value::Reference(layer_id)) => name_of(&snapshot, layer_id).unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn names_are_compared_by_one_fold_for_lookup_and_uniqueness() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    // Dotted and dotless i are different letters under the fold: "ÇİZGİ" is not "çizgi" (whose
    // fold is "ÇIZGI"), but "çızgı" (dotless) is the same name as "çizgi".
    env.run("draftbuilder.layer.create", json!({"name": "çizgi"}));
    env.run("draftbuilder.layer.create", json!({"name": "ÇİZGİ"}));
    for taken in ["ÇIZGI", "çızgı", "Çizgi"] {
        assert_eq!(
            code_of(env.invoke("draftbuilder.layer.create", json!({"name": taken}))),
            "db.cmd.name_taken",
            "{taken}"
        );
    }
    // Lookup by name finds the same layer that uniqueness refuses.
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "ÇIZGI"}),
    );
    assert_eq!(layer_name_of_entity(&env, &line), "çizgi");
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "çİZGİ"}),
    );
    assert_eq!(layer_name_of_entity(&env, &line), "ÇİZGİ");
}

#[test]
fn a_multi_character_uppercase_form_folds_the_same_in_uniqueness_and_lookup() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Straße"}));
    assert_eq!(
        code_of(env.invoke("draftbuilder.layer.create", json!({"name": "STRASSE"}))),
        "db.cmd.name_taken"
    );
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "strasse"}),
    );
    assert_eq!(layer_name_of_entity(&env, &line), "Straße");
}

#[test]
fn special_layers_are_recognized_in_any_letter_case() {
    let mut env = Env::new();
    drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "DefPoints"}));
    let defpoints = layer_named(&env, "DefPoints").to_string();
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.rename",
            json!({"layer": defpoints.clone(), "name": "Points"})
        )),
        "db.cmd.layer_special"
    );
    assert!(has_layer(&env, "DefPoints"));
}

/// The preserved-record fixture of the purge tests: layer SPARE is named only by an unknown entity.
const PRESERVED: &str = "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n  5\n10\n  0\nLAYER\n  5\n11\n  2\nSPARE\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n  0\nENDTAB\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nZZCUSTOM\n  5\n1A\n  8\nSPARE\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  0\nENDSEC\n  0\nEOF\n";

#[test]
fn a_layer_named_by_a_preserved_record_cannot_be_renamed_deleted_or_merged() {
    let mut env = Env::new();
    let path = env.write("preserved-layer.dxf", PRESERVED.as_bytes());
    env.run("draftbuilder.drawing.open", json!({"path": path}));
    env.run("draftbuilder.layer.create", json!({"name": "Dst"}));
    let spare = layer_named(&env, "SPARE").to_string();
    let dst = layer_named(&env, "Dst").to_string();
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.rename",
            json!({"layer": spare.clone(), "name": "Other"})
        )),
        "db.cmd.layer_preserved"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.delete",
            json!({"layers": [spare.clone()]})
        )),
        "db.cmd.layer_preserved"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.delete_with_contents",
            json!({"layers": [spare.clone()]})
        )),
        "db.cmd.layer_preserved"
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.merge",
            json!({"sources": [spare.clone()], "target": dst})
        )),
        "db.cmd.layer_preserved"
    );
    assert!(has_layer(&env, "SPARE"));
}

#[test]
fn a_merge_into_a_locked_target_or_a_delete_of_a_locked_layer_changes_nothing() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Src"}));
    env.run("draftbuilder.layer.create", json!({"name": "Dst"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Src"}),
    );
    let src = layer_named(&env, "Src").to_string();
    let dst = layer_named(&env, "Dst").to_string();
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [dst.clone()], "locked": true}),
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.merge",
            json!({"sources": [src.clone()], "target": dst.clone()})
        )),
        "draft.layer.locked"
    );
    assert!(has_layer(&env, "Src"));
    assert_eq!(layer_name_of_entity(&env, &line), "Src");
    // A locked layer with contents is not deleted with them.
    env.run(
        "draftbuilder.layer.modify",
        json!({"layers": [dst.clone()], "locked": false}),
    );
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [line.clone()]}),
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.delete_with_contents",
            json!({"layers": [src.clone()]})
        )),
        "draft.layer.locked"
    );
    assert!(has_layer(&env, "Src"));
    assert!(
        env.document
            .snapshot()
            .resolve(line.parse().unwrap())
            .is_ok()
    );
}

#[test]
fn a_property_match_from_an_object_on_a_locked_layer_is_refused() {
    let mut env = Env::new();
    let source = drawing(&mut env);
    env.run("draftbuilder.layer.create", json!({"name": "Locked"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [source.clone()], "layer": "Locked"}),
    );
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [source.clone()]}),
    );
    let space = env.space().to_string();
    let target = env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [0.0, 4.0], "end": [5.0, 4.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.property.match",
            json!({"source": source.clone(), "objects": [target.clone()]})
        )),
        "draft.layer.locked"
    );
    // The target keeps the layer it had: no layer reference was written.
    let snapshot = env.document.snapshot();
    let handle = snapshot.resolve(target.parse().unwrap()).unwrap();
    assert_eq!(snapshot.get(handle).unwrap().values[entity::LAYER], None);
}

#[test]
fn a_viewport_override_naming_a_layer_keeps_it_with_the_referrer_kind() {
    let mut env = Env::new();
    drawing(&mut env);
    let space = env.space().to_string();
    env.run("draftbuilder.layer.create", json!({"name": "Shown"}));
    let shown = layer_named(&env, "Shown").to_string();
    let layout = env.run(
        "core.draft.layout.create",
        json!({"name": "sheet", "paper_width": 297.0, "paper_height": 210.0}),
    );
    let viewport = env.run(
        "core.draft.viewport.create",
        json!({"layout": layout["space"].clone(), "model": space,
               "center": [100.0, 100.0], "width": 50.0, "height": 40.0}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "core.draft.viewport.modify",
        json!({"objects": [viewport], "field": "layer_overrides",
               "value": [[shown.clone(), false, null, null, null, null]]}),
    );
    let error = env
        .invoke(
            "draftbuilder.layer.delete",
            json!({"layers": [shown.clone()]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.layer_in_use", "{error:?}");
    assert!(
        format!("{error:?}").contains("core.draft.viewport"),
        "{error:?}"
    );
    assert!(has_layer(&env, "Shown"));
}

#[test]
fn delete_with_contents_refuses_when_a_dependent_dimension_sits_on_a_locked_layer() {
    let mut env = Env::new();
    let line = drawing(&mut env);
    let space = env.space().to_string();
    env.run("draftbuilder.layer.create", json!({"name": "Target"}));
    env.run("draftbuilder.layer.create", json!({"name": "Dims"}));
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [line.clone()], "layer": "Target"}),
    );
    let target = layer_named(&env, "Target").to_string();
    let dims = layer_named(&env, "Dims").to_string();
    let dimension = env.run(
        "core.draft.dimension.create",
        json!({"owner": space, "kind": "aligned",
               "points": [{"object": line.clone(), "key": "start"}, {"position": [0.0, 3.0]}],
               "line_position": [2.5, 4.0], "rotation": 0.0, "layer": dims}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "draftbuilder.layer.lock",
        json!({"objects": [dimension.clone()]}),
    );
    assert_eq!(
        code_of(env.invoke(
            "draftbuilder.layer.delete_with_contents",
            json!({"layers": [target]})
        )),
        "draft.layer.locked"
    );
    assert!(
        env.document
            .snapshot()
            .resolve(line.parse().unwrap())
            .is_ok()
    );
    assert!(
        env.document
            .snapshot()
            .resolve(dimension.parse().unwrap())
            .is_ok()
    );
}
