//! The test environment of the script tests: a document with the product's types and validators,
//! the product's command set, a session whose roots are a fresh directory, the placeholders of the
//! workflow scripts and the reader of the DXF text the tests compare.

#![allow(dead_code, reason = "each test binary uses a subset")]
#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_command::{Permission, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use draftbuilder_commands::script::{self, Report};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A document, its commands and a session whose read and write roots are a fresh directory that
/// is removed when the harness is dropped.
pub struct Harness {
    /// The directory of the scripts' files.
    pub dir: PathBuf,
    /// The drawing the script runs on.
    pub document: Document,
    /// The session of the script.
    pub session: Session,
    /// The command set.
    pub registry: arc_command::Registry,
}

impl Harness {
    /// An empty document in a fresh directory.
    pub fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("draftbuilder-script-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        };
        let types = draftbuilder_commands::types().unwrap();
        let document = Document::open(
            Snapshot::empty(Arc::new(types)),
            Arc::new(Evaluators::new()),
            config,
        )
        .unwrap();
        draftbuilder_commands::install(&document);
        let mut session = Session::new(
            Surface::CommandLine,
            [
                Permission::Edit,
                Permission::Read,
                Permission::FileRead,
                Permission::FileWrite,
            ],
        );
        session.roots = Roots::new([&dir], [&dir]).unwrap();
        Self {
            dir,
            document,
            session: draftbuilder_commands::with_hooks(session),
            registry: draftbuilder_commands::registry().unwrap(),
        }
    }

    /// Runs `text` on the document; the echo is discarded.
    pub fn run(&mut self, text: &str) -> Report {
        let mut echo = Vec::new();
        script::run(
            &self.document,
            &self.registry,
            &mut self.session,
            text,
            &mut echo,
        )
    }

    /// The absolute path of `name` in the directory.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Writes `bytes` to `name` in the directory and returns its path.
    pub fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// Reopens the DXF file `path` as a new document with the product's types and validators.
    pub fn reopen(&self, path: &Path) -> Document {
        draftbuilder_commands::open_document(
            path,
            DocumentConfig {
                workers: 0,
                ..DocumentConfig::default()
            },
            &self.session,
        )
        .unwrap()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A path as a quoted script value: the backslashes and quotes of the path are escaped, so a path
/// with spaces or a Windows separator is one value.
pub fn quoted(path: &Path) -> String {
    let text = path
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{text}\"")
}

/// The script with each `{name}` placeholder replaced by the quoted path it names.
pub fn fill(script: &str, paths: &[(&str, &Path)]) -> String {
    let mut out = script.to_owned();
    for (name, path) in paths {
        out = out.replace(&format!("{{{name}}}"), &quoted(path).replace('"', ""));
    }
    out
}

/// The records of a DXF text: each record is its `(code, value)` pairs, the first pair being the
/// `0` that names its type. Values are trimmed.
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

/// The records of the section `name` of a DXF text, without the section's own records.
pub fn section(text: &str, name: &str) -> Vec<Vec<(i64, String)>> {
    let records = dxf_records(text);
    let mut out = Vec::new();
    let mut inside = false;
    for record in records {
        match (record[0].0, record[0].1.as_str()) {
            (0, "SECTION") => inside = record.iter().any(|(c, v)| *c == 2 && v == name),
            (0, "ENDSEC") => inside = false,
            _ if inside => out.push(record),
            _ => {}
        }
    }
    out
}

/// The handle (group 5) of a record, if it has one.
pub fn handle_of(record: &[(i64, String)]) -> Option<String> {
    record
        .iter()
        .find(|(code, _)| *code == 5)
        .map(|(_, value)| value.clone())
}

/// The value of the first group with `code` in a record.
pub fn value_of(record: &[(i64, String)], code: i64) -> Option<&str> {
    record
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, v)| v.as_str())
}

/// Every group with `code` in a record, in order.
pub fn values_of(record: &[(i64, String)], code: i64) -> Vec<f64> {
    record
        .iter()
        .filter(|(c, _)| *c == code)
        .map(|(_, v)| v.parse::<f64>().unwrap())
        .collect()
}

/// The identifier of the type of an object of `snapshot`, by its handle.
pub fn type_of(snapshot: &Snapshot, handle: arc_doc::Handle) -> String {
    snapshot
        .type_key(handle)
        .map(|key| key.as_str().to_owned())
        .unwrap_or_default()
}

/// Runs the product's CLI in a new process: `script` on the package `package`, with the script
/// file `script` and the directory `dir` as both roots. Creates the package and saves it.
pub fn cli_script(dir: &Path, package: &Path, script: &Path) -> std::process::Output {
    let root = dir.to_str().unwrap();
    std::process::Command::new(env!("CARGO_BIN_EXE_draftbuilder-cli"))
        .args([
            "script",
            package.to_str().unwrap(),
            "--script",
            script.to_str().unwrap(),
            "--create",
            "--save",
            "--allow-edit",
            "--read-root",
            root,
            "--write-root",
            root,
        ])
        .output()
        .unwrap()
}

/// Asserts that two lists of text are equal, and on a difference names the first line that differs.
pub fn assert_same_lines(left: &[String], right: &[String]) {
    let first = left.iter().zip(right).position(|(a, b)| a != b);
    if let Some(index) = first {
        panic!(
            "first difference at {index}:\n  left: {}\n right: {}",
            left[index], right[index]
        );
    }
    assert_eq!(left.len(), right.len(), "the lists have different lengths");
}

/// The value of the header variable `name` of a DXF text, when the header holds it.
pub fn header_value(text: &str, name: &str) -> Option<String> {
    let records = dxf_records(text);
    let header = records
        .iter()
        .find(|r| r.iter().any(|(c, v)| *c == 2 && v == "HEADER"))?;
    header
        .windows(2)
        .find(|pair| pair[0].0 == 9 && pair[0].1 == name)
        .map(|pair| pair[1].1.clone())
}
