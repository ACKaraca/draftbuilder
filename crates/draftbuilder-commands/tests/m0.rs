//! The M0 exit test, headless: a DXF drawing with lines, an arc, polylines and text opens, a
//! move command runs, undo restores the geometry, and the saved DXF carries the handles of the
//! source (docs/roadmap.md, M0). The viewport half runs in `draftbuilder-cli`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Permission, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot, Value as DocValue};
use arc_draft::types::ENTITIES;
use arc_io_dxf::{Drawing, EntityKind, ReadOptions, read};
use draftbuilder_commands::{install, registry, types};
use draftbuilder_model::Database;
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("data/m0.dxf");
static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Env {
    document: Document,
    commands: arc_command::Registry,
    session: Session,
    dir: PathBuf,
    seed: u64,
}

impl Env {
    fn new() -> Self {
        let config = DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        };
        let document = Document::open(
            Snapshot::empty(Arc::new(types().unwrap())),
            Arc::new(Evaluators::new()),
            config,
        )
        .unwrap();
        install(&document);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("draftbuilder-m0-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut session = Session::new(
            Surface::Automation,
            [
                Permission::Edit,
                Permission::Read,
                Permission::FileRead,
                Permission::FileWrite,
            ],
        );
        session.roots = Roots::new([&dir], [&dir]).unwrap();
        Self {
            document,
            commands: registry().unwrap(),
            session,
            dir,
            seed: 1,
        }
    }

    fn run(&mut self, command: &str, params: Value) -> Value {
        self.seed += 1;
        let invocation = Invocation {
            seed: self.seed,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.commands)
            .invoke(&self.document, &self.session, invocation)
            .unwrap()
            .value
    }

    fn path(&self, name: &str) -> String {
        self.dir.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn parse(bytes: &[u8]) -> Drawing {
    read(bytes, &mut ReadOptions::default()).unwrap().drawing
}

/// The handles of the top-level entities, in file order.
fn handles(drawing: &Drawing) -> Vec<u64> {
    drawing
        .entities
        .iter()
        .filter(|e| !matches!(e.kind, EntityKind::Preserved(_)))
        .map(|e| e.source_id().unwrap().0)
        .collect()
}

/// Every handle written in the sections after the header, in file order.
fn all_handles(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut in_header = false;
    let mut out = Vec::new();
    for pair in lines.chunks(2) {
        let [code, value] = pair else { break };
        match (*code, *value) {
            ("2", "HEADER") => in_header = true,
            ("0", "ENDSEC") => in_header = false,
            ("5" | "105", v) if !in_header => out.push(v.to_owned()),
            _ => {}
        }
    }
    out
}

/// Every entity object of the snapshot with its values, by identifier.
fn entities(snapshot: &Snapshot) -> Vec<(ObjectId, Vec<Option<DocValue>>)> {
    let mut out = Vec::new();
    for key in ENTITIES {
        if let Ok(iter) = snapshot.iter_type(key) {
            out.extend(iter.map(|(_, o)| (o.id, o.values.clone())));
        }
    }
    out.sort_by_key(|(id, _)| *id);
    out
}

#[test]
fn open_move_undo_save_keeps_the_handles_of_the_source() {
    let mut env = Env::new();
    let source = FIXTURE.as_bytes();
    std::fs::write(env.dir.join("m0.dxf"), source).unwrap();
    let source_handles = handles(&parse(source));
    assert_eq!(source_handles, [0x100, 0x101, 0x102, 0x103, 0x104, 0x109]);

    // Open: every entity is bound to its source handle; the file's fingerprint is kept.
    let opened = env.run(
        "draftbuilder.drawing.open_dxf",
        json!({"path": env.path("m0.dxf")}),
    );
    assert_eq!(opened["handles"], 6, "{opened}");
    assert_eq!(
        opened["fingerprint"],
        "{3E7A9C14-5B2D-4F68-A1C3-90D2E8B47F15}"
    );
    assert_eq!(opened["fingerprint_assigned"], false);
    assert_eq!(
        opened["handle_top"], "1FF",
        "the file's next handle was 0x200"
    );
    let before = env.document.snapshot();
    let database = Database::of(&before);
    assert_eq!(database.len(), 6);
    let objects = entities(&before);
    assert_eq!(objects.len(), 6);
    for (id, _) in &objects {
        let handle = database.handle_of(*id).unwrap();
        assert!(source_handles.contains(&handle.value()));
        assert_eq!(database.object_of(handle), Some(*id));
        assert!(database.stable_id_of(*id).is_some());
    }

    // Move every entity, then undo: the geometry is restored exactly.
    let ids: Vec<String> = objects.iter().map(|(id, _)| id.to_string()).collect();
    env.run(
        "draftbuilder.modify.move",
        json!({"objects": ids, "offset": [25.0, -10.0]}),
    );
    let moved = env.document.snapshot();
    assert_ne!(entities(&moved), objects, "the move changed the geometry");
    assert!(env.document.undo().wait().unwrap().value);
    let restored = env.document.snapshot();
    assert_eq!(entities(&restored), objects, "undo restores the geometry");
    assert_eq!(
        Database::of(&restored).len(),
        6,
        "undoing the move keeps the handle table"
    );

    // Save: the same handles, the same fingerprint, a new version GUID, unique handles.
    let saved = env.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": env.path("saved.dxf")}),
    );
    assert_eq!(saved["handles_carried"], 6, "{saved}");
    assert_eq!(saved["handles_assigned"], 0);
    let bytes = std::fs::read(env.dir.join("saved.dxf")).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    let drawing = parse(&bytes);
    assert_eq!(handles(&drawing), source_handles);
    let var = |name: &str| {
        drawing
            .header_variable(name)
            .and_then(|g| g.first())
            .and_then(|g| g.as_text())
            .map(str::to_owned)
    };
    assert_eq!(
        var("$FINGERPRINTGUID").as_deref(),
        Some("{3E7A9C14-5B2D-4F68-A1C3-90D2E8B47F15}")
    );
    assert_ne!(
        var("$VERSIONGUID").as_deref(),
        Some("{B8D41F07-6C93-4E1A-9A52-7D0E3C6A18F4}")
    );
    let seed = u64::from_str_radix(&var_seed(&drawing), 16).unwrap();
    assert!(
        seed >= 0x200,
        "the next handle stays above the file's: {seed:X}"
    );
    let written = all_handles(&text);
    let unique: BTreeSet<&String> = written.iter().collect();
    assert_eq!(unique.len(), written.len(), "every handle is written once");

    // The preservation report is surfaced, and nothing the fixture holds is dropped.
    assert_eq!(opened["report"]["summary"]["dropped"], 0, "{opened}");
    assert_eq!(saved["report"]["summary"]["dropped"], 0, "{saved}");
}

fn var_seed(drawing: &Drawing) -> String {
    drawing
        .header_variable("$HANDSEED")
        .and_then(|g| g.first())
        .and_then(|g| g.as_text())
        .unwrap()
        .to_owned()
}

#[test]
fn a_saved_move_is_in_the_file_and_the_handles_stay() {
    let mut env = Env::new();
    std::fs::write(env.dir.join("m0.dxf"), FIXTURE).unwrap();
    env.run(
        "draftbuilder.drawing.open_dxf",
        json!({"path": env.path("m0.dxf")}),
    );
    let ids: Vec<String> = entities(&env.document.snapshot())
        .iter()
        .map(|(id, _)| id.to_string())
        .collect();
    env.run(
        "draftbuilder.modify.move",
        json!({"objects": ids, "offset": [25.0, -10.0]}),
    );
    env.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": env.path("moved.dxf")}),
    );
    let before = parse(FIXTURE.as_bytes());
    let after = parse(&std::fs::read(env.dir.join("moved.dxf")).unwrap());
    assert_eq!(handles(&after), handles(&before));
    let (EntityKind::Line(a), EntityKind::Line(b)) =
        (&before.entities[0].kind, &after.entities[0].kind)
    else {
        panic!("the first entity is a line");
    };
    assert_eq!(b.start[0], a.start[0] + 25.0);
    assert_eq!(b.start[1], a.start[1] - 10.0);
}

#[test]
fn a_drawing_without_a_fingerprint_gets_one_that_the_report_names() {
    let mut env = Env::new();
    let text: String = FIXTURE.replace(
        "$FINGERPRINTGUID\n  2\n{3E7A9C14-5B2D-4F68-A1C3-90D2E8B47F15}",
        "$UNKNOWNVAR\n  2\nx",
    );
    assert_ne!(text, FIXTURE);
    std::fs::write(env.dir.join("plain.dxf"), text).unwrap();
    let opened = env.run(
        "draftbuilder.drawing.open_dxf",
        json!({"path": env.path("plain.dxf")}),
    );
    assert_eq!(opened["fingerprint_assigned"], true);
    let saved = env.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": env.path("plain-saved.dxf")}),
    );
    assert_eq!(saved["fingerprint"], opened["fingerprint"]);
    let reasons: Vec<&str> = saved["report"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["reason"].as_str())
        .collect();
    assert!(
        reasons.contains(&"db.io.fingerprint_assigned"),
        "{reasons:?}"
    );
}

#[test]
fn an_independent_copy_has_a_new_fingerprint_and_a_second_open_is_refused() {
    let mut env = Env::new();
    std::fs::write(env.dir.join("m0.dxf"), FIXTURE).unwrap();
    let path = env.path("m0.dxf");
    env.run("draftbuilder.drawing.open_dxf", json!({"path": path}));
    let copy = env.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": env.path("copy.dxf"), "independent_copy": true}),
    );
    assert_ne!(
        copy["fingerprint"],
        "{3E7A9C14-5B2D-4F68-A1C3-90D2E8B47F15}"
    );
    let again = Invoker::new(&env.commands).invoke(
        &env.document,
        &env.session,
        Invocation::new("draftbuilder.drawing.open_dxf", json!({"path": path})),
    );
    assert_eq!(again.unwrap_err().code(), "db.cmd.drawing_not_empty");
}

#[test]
fn a_file_that_repeats_a_handle_still_opens_and_the_report_says_so() {
    let mut env = Env::new();
    let text = FIXTURE.replace("  5\n101\n", "  5\n100\n");
    assert_ne!(text, FIXTURE);
    std::fs::write(env.dir.join("dup.dxf"), text).unwrap();
    let opened = env.run(
        "draftbuilder.drawing.open_dxf",
        json!({"path": env.path("dup.dxf")}),
    );
    assert_eq!(opened["handles"], 5, "{opened}");
    let reasons = |v: &Value| -> Vec<String> {
        v["report"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|i| i["reason"].as_str().map(str::to_owned))
            .collect()
    };
    assert!(reasons(&opened).contains(&"db.io.handle_duplicate".to_owned()));
    let saved = env.run(
        "draftbuilder.drawing.save_dxf",
        json!({"path": env.path("dup-saved.dxf")}),
    );
    assert_eq!(saved["handles_assigned"], 1, "{saved}");
    assert!(reasons(&saved).contains(&"db.io.handle_assigned".to_owned()));
    let written = all_handles(&std::fs::read_to_string(env.dir.join("dup-saved.dxf")).unwrap());
    let unique: BTreeSet<&String> = written.iter().collect();
    assert_eq!(unique.len(), written.len(), "every handle is written once");
}

#[test]
fn objects_the_move_does_not_handle_are_refused_and_nothing_changes() {
    let mut env = Env::new();
    let created = env.run(
        "core.draft.space.create",
        json!({"name": "m", "kind": "model", "unit": "millimeter"}),
    );
    let space = created["id"].as_str().unwrap().to_owned();
    let revision = env.document.snapshot().revision();
    env.seed += 1;
    let invocation = Invocation {
        seed: env.seed,
        ..Invocation::new(
            "draftbuilder.modify.move",
            json!({"objects": [space], "offset": [1.0, 1.0]}),
        )
    };
    let error = Invoker::new(&env.commands)
        .invoke(&env.document, &env.session, invocation)
        .unwrap_err();
    assert_eq!(error.code(), "db.cmd.move_unsupported");
    assert_eq!(env.document.snapshot().revision(), revision);
}
