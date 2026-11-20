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
        let session = session.with_last_hook(Some(Arc::new(draftbuilder_commands::last_objects)));
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

/// The curve of an entity of the drawing, decoded from its stored geometry.
pub fn curve_of_entity(snapshot: &Snapshot, id: ObjectId) -> arc_geom::Curve2 {
    let handle = snapshot.resolve(id).unwrap();
    let object = snapshot.get(handle).unwrap();
    let key = snapshot.type_key(handle).unwrap().as_str().to_owned();
    match Reader::new(snapshot.registry(), &key, &object.values).get("geometry") {
        Some(arc_doc::Value::Geometry(bytes)) => {
            arc_geom::Curve2::decode(&bytes.0, &Limits::DEFAULT).unwrap()
        }
        _ => panic!("no geometry"),
    }
}

/// The `(x, y)` value of group `code` of a record, when it has one.
fn point_of(record: &[(i64, String)], code: i64) -> Option<(f64, f64)> {
    let value = |c: i64| {
        record
            .iter()
            .find(|(k, _)| *k == c)
            .and_then(|(_, v)| v.parse::<f64>().ok())
    };
    Some((value(code)?, value(code + 10)?))
}

/// An anonymous block of a DXF text: its name, its base point and the centers of its circles.
type BlockCircles = (String, (f64, f64), Vec<(f64, f64)>);

/// Where the displayed copies of the anonymous blocks (names starting `*A`) of a saved DXF text are:
/// for each block reference (INSERT) of such a block, the centers of the circles of its block moved by
/// the reference's insertion point relative to the block's base point. Sorted.
pub fn displayed_circle_centers(text: &str) -> Vec<(f64, f64)> {
    let records = dxf_records(text);
    let mut blocks: Vec<BlockCircles> = Vec::new();
    let mut open: Option<usize> = None;
    for record in &records {
        match record[0].1.as_str() {
            "BLOCK" => {
                let name = record
                    .iter()
                    .find(|(c, _)| *c == 2)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                if name.starts_with("*A") {
                    blocks.push((name, point_of(record, 10).unwrap_or_default(), Vec::new()));
                    open = Some(blocks.len() - 1);
                }
            }
            "ENDBLK" => open = None,
            "CIRCLE" => {
                if let (Some(index), Some(center)) = (open, point_of(record, 10)) {
                    blocks[index].2.push(center);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for record in records.iter().filter(|r| r[0].1 == "INSERT") {
        let name = record
            .iter()
            .find(|(c, _)| *c == 2)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default();
        let Some((_, base, centers)) = blocks.iter().find(|(n, _, _)| n == name) else {
            continue;
        };
        let Some(insert) = point_of(record, 10) else {
            continue;
        };
        for center in centers {
            out.push((insert.0 + center.0 - base.0, insert.1 + center.1 - base.1));
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}
