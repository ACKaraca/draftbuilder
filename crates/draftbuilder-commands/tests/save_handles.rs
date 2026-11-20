//! Handles across saves: the layer 0 of a new drawing keeps its handle when the file is saved again,
//! the handles of deleted or merged-away objects are not given to later objects, and each saved
//! file opens again.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_draft::types::LAYER;
use common::{Env, line_handle_at, named_handle};
use serde_json::json;

/// Saves the document to `name` in the environment's directory and returns the file's text.
fn save(env: &mut Env, name: &str) -> String {
    let path = env.path(name);
    env.run("draftbuilder.drawing.save_dxf", json!({"path": path}));
    std::fs::read_to_string(env.dir.join(name)).unwrap()
}

/// Opens a saved file in a fresh environment: the file must be valid.
fn reopens(file: &str, text: &str) {
    let mut other = Env::new();
    std::fs::write(other.dir.join(file), text).unwrap();
    let path = other.path(file);
    other.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
}

/// The identifier of the layer named `name`.
fn layer_id(env: &Env, name: &str) -> String {
    let snapshot = env.document.snapshot();
    snapshot
        .iter_type(LAYER)
        .unwrap()
        .find(|(_, o)| {
            arc_draft_dxf::fields::name_of(&snapshot, Some(o.id)).as_deref() == Some(name)
        })
        .map(|(_, o)| o.id.to_string())
        .unwrap_or_else(|| panic!("no layer {name}"))
}

/// Creates a line from `(x, 0)` to `(x + 1, 0)` and returns its identifier.
fn line(env: &mut Env, x: f64) -> String {
    let space = env.space().to_string();
    env.run(
        "core.draft.entity.create",
        json!({"owner": space, "geometry": {"kind": "segment", "start": [x, 0.0], "end": [x + 1.0, 0.0]}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn layer_zero_of_a_new_drawing_keeps_its_handle_across_saves() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let first = save(&mut env, "first.dxf");
    let handle =
        named_handle(&first, "LAYER", "0").expect("layer 0 has a handle in the first save");
    reopens("first.dxf", &first);

    let mut again = Env::new();
    std::fs::copy(env.dir.join("first.dxf"), again.dir.join("first.dxf")).unwrap();
    let path = again.path("first.dxf");
    again.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    let second = save(&mut again, "second.dxf");
    assert_eq!(
        named_handle(&second, "LAYER", "0"),
        Some(handle),
        "{second}"
    );
}

#[test]
fn the_handle_of_a_deleted_entity_is_not_given_to_a_later_entity() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let gone = line(&mut env, 1.0);
    let first = save(&mut env, "first.dxf");
    let deleted = line_handle_at(&first, 1.0).expect("the first line has a handle");
    env.run("core.draft.entity.delete", json!({"objects": [gone]}));
    line(&mut env, 7.0);
    let second = save(&mut env, "second.dxf");
    let later = line_handle_at(&second, 7.0).expect("the later line has a handle");
    assert_ne!(later, deleted, "{second}");
    assert_eq!(
        line_handle_at(&second, 1.0),
        None,
        "the deleted line is not written"
    );
    reopens("second.dxf", &second);
}

#[test]
fn the_handle_of_a_merged_away_layer_is_not_given_to_a_later_layer() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    env.run("draftbuilder.layer.create", json!({"name": "Src"}));
    env.run("draftbuilder.layer.create", json!({"name": "Dst"}));
    let first = save(&mut env, "first.dxf");
    let merged_away = named_handle(&first, "LAYER", "Src").expect("Src has a handle");
    let (src, dst) = (layer_id(&env, "Src"), layer_id(&env, "Dst"));
    env.run(
        "draftbuilder.layer.merge",
        json!({"sources": [src], "target": dst}),
    );
    env.run("draftbuilder.layer.create", json!({"name": "Later"}));
    let second = save(&mut env, "second.dxf");
    let later = named_handle(&second, "LAYER", "Later").expect("Later has a handle");
    assert_ne!(later, merged_away, "{second}");
    assert_eq!(named_handle(&second, "LAYER", "Src"), None, "{second}");
    reopens("second.dxf", &second);
}

#[test]
fn a_save_after_a_delete_with_contents_is_a_valid_file() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let kept = line(&mut env, 1.0);
    env.run("draftbuilder.layer.create", json!({"name": "Temp"}));
    let doomed = line(&mut env, 5.0);
    env.run(
        "draftbuilder.layer.change",
        json!({"objects": [doomed.clone()], "layer": "Temp"}),
    );
    let temp = layer_id(&env, "Temp");
    env.run(
        "draftbuilder.layer.delete_with_contents",
        json!({"layers": [temp]}),
    );
    let text = save(&mut env, "after.dxf");
    assert!(named_handle(&text, "LAYER", "Temp").is_none(), "{text}");
    assert!(line_handle_at(&text, 1.0).is_some(), "{text}");
    assert!(
        env.document
            .snapshot()
            .resolve(kept.parse().unwrap())
            .is_ok()
    );
    reopens("after.dxf", &text);
}

/// The commands of the undo steps, newest first.
fn step_commands(env: &Env) -> Vec<String> {
    env.document
        .undo_step_list(100)
        .into_iter()
        .map(|step| step.command)
        .collect()
}

#[test]
fn a_save_is_no_undo_step_and_the_next_undo_takes_the_last_edit() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let first = line(&mut env, 1.0);
    let second: arc_base::ObjectId = line(&mut env, 7.0).parse().unwrap();
    let steps = step_commands(&env).len();
    save(&mut env, "one.dxf");
    // The save recorded handles, yet leaves the step list as it was.
    assert_eq!(step_commands(&env).len(), steps);
    assert!(step_commands(&env).iter().all(|c| !c.contains("save")));
    assert!(env.document.undo().wait().unwrap().value);
    let snapshot = env.document.snapshot();
    assert!(snapshot.resolve(second).is_err(), "the last edit is undone");
    assert!(snapshot.resolve(first.parse().unwrap()).is_ok());
    // A second save leaves the redo stack alone: the undone edit comes back.
    save(&mut env, "two.dxf");
    assert!(env.document.redo().wait().unwrap().value);
    assert!(env.document.snapshot().resolve(second).is_ok());
}

#[test]
fn a_handle_stays_with_its_object_through_undo_redo_and_saves_and_is_never_reused() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    line(&mut env, 1.0);
    line(&mut env, 7.0);
    let first = save(&mut env, "one.dxf");
    let (a, b) = (
        line_handle_at(&first, 1.0).unwrap(),
        line_handle_at(&first, 7.0).unwrap(),
    );
    // Undo the creation of the second line: its row refers to an object that is not live, and
    // the export neither writes it nor gives its handle away.
    assert!(env.document.undo().wait().unwrap().value);
    let second = save(&mut env, "two.dxf");
    assert_eq!(line_handle_at(&second, 1.0), Some(a.clone()));
    assert_eq!(line_handle_at(&second, 7.0), None);
    // Redo creates it again under the same identifier: the row applies again.
    assert!(env.document.redo().wait().unwrap().value);
    let third = save(&mut env, "three.dxf");
    assert_eq!(line_handle_at(&third, 1.0), Some(a.clone()));
    assert_eq!(line_handle_at(&third, 7.0), Some(b.clone()));
    // Undo it again and draw another line: it gets a handle nobody had.
    assert!(env.document.undo().wait().unwrap().value);
    line(&mut env, 13.0);
    let fourth = save(&mut env, "four.dxf");
    let c = line_handle_at(&fourth, 13.0).unwrap();
    assert!(c != a && c != b, "{c} is new");
    assert_eq!(line_handle_at(&fourth, 7.0), None);
}

#[test]
fn an_erased_object_that_is_restored_keeps_its_handle() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let id = line(&mut env, 1.0);
    let first = save(&mut env, "one.dxf");
    let handle = line_handle_at(&first, 1.0).unwrap();
    env.run("core.draft.entity.delete", json!({"objects": [id]}));
    let erased = save(&mut env, "two.dxf");
    assert_eq!(line_handle_at(&erased, 1.0), None);
    let restored = env
        .document
        .restore_erased("{}".to_owned(), "{}".to_owned())
        .wait()
        .unwrap()
        .value;
    assert_eq!(restored, 1);
    let third = save(&mut env, "three.dxf");
    assert_eq!(line_handle_at(&third, 1.0), Some(handle));
}

fn top(env: &Env) -> u64 {
    draftbuilder_model::Database::of(&env.document.snapshot()).top()
}

#[test]
fn a_save_whose_write_fails_after_the_commit_keeps_the_recorded_handles_for_the_next_save() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let id: arc_base::ObjectId = line(&mut env, 1.0).parse().unwrap();
    // The target directory goes away between the command's check and the write: the validator
    // runs when the bindings commit, after the check and before the write.
    let directory = env.dir.join("sub");
    std::fs::create_dir(&directory).unwrap();
    let armed = Arc::new(AtomicBool::new(true));
    let (flag, gone) = (Arc::clone(&armed), directory.clone());
    env.document
        .add_validator(Arc::new(move |base, candidate, _| {
            if candidate.foreign_len() > base.foreign_len() && flag.swap(false, Ordering::SeqCst) {
                std::fs::remove_dir_all(&gone).unwrap();
            }
            Vec::new()
        }));
    let target = env.path("sub/x.dxf");
    let error = env
        .invoke("draftbuilder.drawing.save_dxf", json!({"path": target}))
        .unwrap_err();
    assert!(error.code().starts_with("cmd.path"), "{}", error.code());
    let recorded = draftbuilder_model::Database::of(&env.document.snapshot())
        .handle_of(id)
        .expect("the handle was recorded before the write");
    let text = save(&mut env, "ok.dxf");
    assert_eq!(
        line_handle_at(&text, 1.0),
        Some(recorded.to_hex()),
        "the next save writes the recorded handle"
    );
}

#[test]
fn a_refused_target_fails_before_anything_is_recorded() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    line(&mut env, 1.0);
    let directory = env.dir.join("dir.dxf");
    std::fs::create_dir(&directory).unwrap();
    let read_only = env.dir.join("ro.dxf");
    std::fs::write(&read_only, "x").unwrap();
    let mut permissions = std::fs::metadata(&read_only).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&read_only, permissions).unwrap();
    let outside = std::env::temp_dir().join(format!("db-outside-{}.dxf", std::process::id()));
    let mut targets = vec![
        directory.to_str().unwrap().to_owned(),
        read_only.to_str().unwrap().to_owned(),
        outside.to_str().unwrap().to_owned(),
    ];
    #[cfg(unix)]
    {
        let link = env.dir.join("link.dxf");
        std::os::unix::fs::symlink(env.dir.join("real.dxf"), &link).unwrap();
        targets.push(link.to_str().unwrap().to_owned());
    }
    let (revision, mark, rows) = (
        env.document.snapshot().revision(),
        top(&env),
        env.document.snapshot().foreign_len(),
    );
    for target in targets {
        let result = env.invoke("draftbuilder.drawing.save_dxf", json!({"path": target}));
        assert!(result.is_err(), "{target}");
        let snapshot = env.document.snapshot();
        assert_eq!(snapshot.revision(), revision, "{target}");
        assert_eq!(snapshot.foreign_len(), rows, "{target}");
        assert_eq!(top(&env), mark, "{target}");
    }
    assert!(!outside.exists());
}

#[test]
fn a_save_whose_binding_commit_is_refused_writes_no_file_and_records_nothing() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let id: arc_base::ObjectId = line(&mut env, 1.0).parse().unwrap();
    let refuse = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&refuse);
    env.document
        .add_validator(Arc::new(move |base, candidate, _| {
            if flag.load(Ordering::SeqCst) && candidate.foreign_len() > base.foreign_len() {
                vec![arc_doc::Finding {
                    severity: arc_doc::Severity::Error,
                    error: arc_base::Error::new(arc_command::error::codes::PATH_IO),
                }]
            } else {
                Vec::new()
            }
        }));
    let path = env.path("never.dxf");
    assert!(
        env.invoke("draftbuilder.drawing.save_dxf", json!({"path": path}))
            .is_err()
    );
    assert!(
        !env.dir.join("never.dxf").exists(),
        "no file without the record"
    );
    let db = draftbuilder_model::Database::of(&env.document.snapshot());
    assert!(db.handle_of(id).is_none());
    refuse.store(false, Ordering::SeqCst);
    let text = save(&mut env, "later.dxf");
    assert!(line_handle_at(&text, 1.0).is_some());
}

#[test]
fn the_handle_high_water_mark_never_falls() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({}));
    let mut marks = vec![top(&env)];
    let a = line(&mut env, 1.0);
    line(&mut env, 7.0);
    save(&mut env, "one.dxf");
    marks.push(top(&env));
    assert!(env.document.undo().wait().unwrap().value);
    marks.push(top(&env));
    save(&mut env, "two.dxf");
    assert!(env.document.redo().wait().unwrap().value);
    marks.push(top(&env));
    env.run("core.draft.entity.delete", json!({"objects": [a]}));
    save(&mut env, "three.dxf");
    marks.push(top(&env));
    let restored = env
        .document
        .restore_erased("{}".to_owned(), "{}".to_owned())
        .wait()
        .unwrap()
        .value;
    assert_eq!(restored, 1);
    save(&mut env, "four.dxf");
    marks.push(top(&env));
    assert!(marks.windows(2).all(|w| w[0] <= w[1]), "{marks:?}");
    assert!(marks[1] > marks[0]);
}

fn osmode(env: &mut Env) -> serde_json::Value {
    env.run("draftbuilder.settings.get", json!({"name": "OSMODE"}))["value"].clone()
}

fn ucs_names(env: &mut Env) -> serde_json::Value {
    let listed = env.run("draftbuilder.precision.ucs_list", json!({}));
    listed["systems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].clone())
        .collect()
}

#[test]
fn a_save_between_an_undone_setting_or_coordinate_system_and_its_redo_loses_neither() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    line(&mut env, 1.0);
    // Steps that add rows (a setting, a coordinate system), undone, with a save in between.
    env.run("draftbuilder.precision.osnap", json!({"modes": "END,MID"}));
    env.run("draftbuilder.precision.ucs_save", json!({"name": "Plate"}));
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.document.undo().wait().unwrap().value);
    let before = osmode(&mut env);
    save(&mut env, "one.dxf");
    assert!(env.document.redo().wait().unwrap().value);
    assert!(env.document.redo().wait().unwrap().value);
    assert_eq!(osmode(&mut env), json!(3));
    assert_eq!(ucs_names(&mut env), json!(["Plate"]));
    // And the undo of both, around the recorded bindings.
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(osmode(&mut env), before);
    assert_eq!(ucs_names(&mut env), json!([]));
}

#[test]
fn a_save_between_a_deleted_coordinate_system_and_the_undo_of_the_delete_loses_nothing() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    line(&mut env, 1.0);
    env.run("draftbuilder.precision.ucs_save", json!({"name": "Plate"}));
    env.run(
        "draftbuilder.precision.ucs_delete",
        json!({"name": "Plate"}),
    );
    save(&mut env, "one.dxf");
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(ucs_names(&mut env), json!(["Plate"]));
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(ucs_names(&mut env), json!([]));
}
