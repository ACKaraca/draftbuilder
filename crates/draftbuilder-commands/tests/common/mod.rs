//! The test environment shared by the drawing tests: a document with the product's types and
//! validators, a command registry, a session with read and write roots in a fresh directory, and
//! helpers that read geometry back from the database.

#![allow(dead_code, reason = "each test binary uses a subset")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_base::{Limits, ObjectId};
use arc_command::{Invocation, Invoker, Permission, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_draft::text::TEXT;
use arc_draft::types::{ENTITIES, LINE};
use arc_draft_dxf::fields::Reader;
use arc_geom::Segment2;
use arc_geom::encoding::GeomCodec;
use draftbuilder_commands::{install, registry, types};
use serde_json::Value;

pub const FIXTURE: &str = include_str!("../data/m0.dxf");
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A document, its commands and a session whose roots are a fresh directory.
pub struct Env {
    pub document: Document,
    pub commands: arc_command::Registry,
    pub session: Session,
    pub dir: PathBuf,
    pub seed: u64,
}

impl Env {
    pub fn new() -> Self {
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
        let dir =
            std::env::temp_dir().join(format!("draftbuilder-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
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

    pub fn invoke(&mut self, command: &str, params: Value) -> Result<Value, arc_base::Error> {
        self.seed += 1;
        let invocation = Invocation {
            seed: self.seed,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.commands)
            .invoke(&self.document, &self.session, invocation)
            .map(|outcome| outcome.value)
    }

    pub fn run(&mut self, command: &str, params: Value) -> Value {
        self.invoke(command, params).unwrap()
    }

    pub fn path(&self, name: &str) -> String {
        self.dir.join(name).to_str().unwrap().to_owned()
    }

    pub fn write(&self, name: &str, bytes: &[u8]) -> String {
        std::fs::write(self.dir.join(name), bytes).unwrap();
        self.path(name)
    }

    /// The model space of the document.
    pub fn space(&self) -> ObjectId {
        draftbuilder_commands::model_space(&self.document.snapshot()).unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The reason codes of the report items of a result.
pub fn reasons(result: &Value) -> Vec<String> {
    result["report"]["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i["reason"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The number of objects of the entity types.
pub fn entity_count(snapshot: &Snapshot) -> usize {
    ENTITIES
        .iter()
        .filter_map(|key| snapshot.iter_type(key).ok())
        .map(Iterator::count)
        .sum()
}

/// The name of a named object.
pub fn name_of(snapshot: &Snapshot, id: ObjectId) -> Option<String> {
    arc_draft_dxf::fields::name_of(snapshot, Some(id))
}

/// The owner (a space or a block record) of an object, `None` when it has none.
pub fn owner_of(snapshot: &Snapshot, id: ObjectId) -> Option<ObjectId> {
    let handle = snapshot.resolve(id).ok()?;
    snapshot.get(handle).ok()?.header.owner
}

/// The start and end of every line, with its owner: `(owner, start, end)`, sorted by coordinates.
pub fn lines(snapshot: &Snapshot) -> Vec<(Option<ObjectId>, [f64; 2], [f64; 2])> {
    let mut out = Vec::new();
    if let Ok(iter) = snapshot.iter_type(LINE) {
        for (_, object) in iter {
            let reader = Reader::new(snapshot.registry(), LINE, &object.values);
            if let Some(arc_doc::Value::Geometry(bytes)) = reader.get("geometry") {
                let segment = Segment2::decode(&bytes.0, &Limits::DEFAULT).unwrap();
                let (a, b) = (segment.start(), segment.end());
                out.push((object.header.owner, [a.x, a.y], [b.x, b.y]));
            }
        }
    }
    out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    out
}

/// The height of every text object, with its owner, sorted by height.
pub fn text_heights(snapshot: &Snapshot) -> Vec<(Option<ObjectId>, f64)> {
    let mut out = Vec::new();
    if let Ok(iter) = snapshot.iter_type(TEXT) {
        for (_, object) in iter {
            let reader = Reader::new(snapshot.registry(), TEXT, &object.values);
            out.push((object.header.owner, reader.f64("height").unwrap()));
        }
    }
    out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    out
}

/// The fixture with its unit code replaced.
pub fn with_units(code: &str) -> String {
    FIXTURE.replace("$INSUNITS\n 70\n4\n", &format!("$INSUNITS\n 70\n{code}\n"))
}

/// Every identifier of the entity types, for commands that take objects.
pub fn entity_ids(snapshot: &Snapshot) -> Vec<String> {
    let mut out = Vec::new();
    for key in ENTITIES {
        if let Ok(iter) = snapshot.iter_type(key) {
            out.extend(iter.map(|(_, o)| o.id.to_string()));
        }
    }
    out
}

/// The records of a DXF text: each record is its (code, value) pairs, the first being the `0`
/// pair that names its type. Values are trimmed.
pub fn dxf_records(text: &str) -> Vec<Vec<(i64, String)>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut records: Vec<Vec<(i64, String)>> = Vec::new();
    for pair in lines.chunks(2) {
        let [code, value] = pair else { continue };
        let Ok(code) = code.trim().parse::<i64>() else {
            continue;
        };
        let value = value.trim().to_owned();
        if code == 0 {
            records.push(vec![(0, value)]);
        } else if let Some(record) = records.last_mut() {
            record.push((code, value));
        }
    }
    records
}

/// The handle (group 5) of a record, if it has one.
pub fn handle_of(record: &[(i64, String)]) -> Option<String> {
    record
        .iter()
        .find(|(code, _)| *code == 5)
        .map(|(_, value)| value.clone())
}

/// The handle of the table record of type `kind` whose name (group 2) is `name`.
pub fn named_handle(text: &str, kind: &str, name: &str) -> Option<String> {
    dxf_records(text)
        .into_iter()
        .find(|r| r[0].1 == kind && r.iter().any(|(c, v)| *c == 2 && v == name))
        .and_then(|r| handle_of(&r))
}

/// The handles of every record of type `kind`, in file order.
pub fn handles_of_kind(text: &str, kind: &str) -> Vec<String> {
    dxf_records(text)
        .into_iter()
        .filter(|r| r[0].1 == kind)
        .filter_map(|r| handle_of(&r))
        .collect()
}

/// The handle of the line whose start x coordinate (group 10) is `x`.
pub fn line_handle_at(text: &str, x: f64) -> Option<String> {
    dxf_records(text)
        .into_iter()
        .find(|r| {
            r[0].1 == "LINE"
                && r.iter()
                    .any(|(c, v)| *c == 10 && v.parse::<f64>().is_ok_and(|value| value == x))
        })
        .and_then(|r| handle_of(&r))
}
