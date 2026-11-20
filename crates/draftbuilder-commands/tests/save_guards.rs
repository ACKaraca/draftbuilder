//! The guards of the save command, headless: a read-only target and a symbolic link are refused and left
//! as they were; a save keeps the permissions of the file it replaces (the last three are Unix only). The
//! refusal of an export that drops a product object is unit-tested in `save.rs`; arrays are written.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

#[cfg(unix)]
use common::Env;
#[cfg(unix)]
use serde_json::json;

#[cfg(unix)]
#[test]
fn a_read_only_target_is_refused_and_keeps_its_content() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let path = env.write("locked.dxf", b"original");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let error = env
        .invoke("draftbuilder.drawing.save", json!({"path": path.clone()}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.target_read_only", "{error:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_target_is_refused_and_the_link_and_its_file_are_left_alone() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let real = env.write("real.dxf", b"real");
    let link = env.path("link.dxf");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let error = env
        .invoke("draftbuilder.drawing.save", json!({"path": link.clone()}))
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.target_not_file", "{error:?}");
    assert_eq!(std::fs::read(&real).unwrap(), b"real");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn a_save_keeps_the_permissions_of_the_file_it_replaces() {
    use std::os::unix::fs::PermissionsExt;
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    let path = env.write("private.dxf", b"old");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    env.run("draftbuilder.drawing.save", json!({"path": path.clone()}));
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(
        std::fs::read(&path).unwrap().starts_with(b"  0"),
        "a DXF was written"
    );
}
