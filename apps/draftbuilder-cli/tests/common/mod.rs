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
            session: session.with_last_hook(Some(std::sync::Arc::new(
                draftbuilder_commands::last_objects,
            ))),
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
            script::SEED,
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
    cli_script_seeded(dir, package, script, None)
}

/// [`cli_script`] with `--seed` when `seed` is given; without one the CLI draws its own seed.
pub fn cli_script_seeded(
    dir: &Path,
    package: &Path,
    script: &Path,
    seed: Option<&str>,
) -> std::process::Output {
    let root = dir.to_str().unwrap();
    let mut args = vec![
        "script".to_owned(),
        package.to_str().unwrap().to_owned(),
        "--script".to_owned(),
        script.to_str().unwrap().to_owned(),
        "--create".to_owned(),
        "--save".to_owned(),
        "--allow-edit".to_owned(),
        "--read-root".to_owned(),
        root.to_owned(),
        "--write-root".to_owned(),
        root.to_owned(),
    ];
    if let Some(seed) = seed {
        args.extend(["--seed".to_owned(), seed.to_owned()]);
    }
    std::process::Command::new(env!("CARGO_BIN_EXE_draftbuilder-cli"))
        .args(&args)
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

/// An anonymous block of a DXF text: its name, its base point and the centers of its circles.
type BlockCircles = (String, (f64, f64), Vec<(f64, f64)>);

/// Where the displayed copies of the anonymous blocks (names starting `*A`) of a saved DXF text are: for
/// each block reference of such a block, the centers of the block's circles moved by the reference's
/// insertion point relative to the block's base point. Sorted.
pub fn displayed_circle_centers(text: &str) -> Vec<(f64, f64)> {
    let records = dxf_records(text);
    let point = |record: &[(i64, String)], code: i64| -> Option<(f64, f64)> {
        let value = |c: i64| {
            record
                .iter()
                .find(|(k, _)| *k == c)
                .and_then(|(_, v)| v.parse::<f64>().ok())
        };
        Some((value(code)?, value(code + 10)?))
    };
    let name_of = |record: &[(i64, String)]| {
        record
            .iter()
            .find(|(k, _)| *k == 2)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let mut blocks: Vec<BlockCircles> = Vec::new();
    let mut open: Option<usize> = None;
    for record in &records {
        match record[0].1.as_str() {
            "BLOCK" => {
                let name = name_of(record);
                if name.starts_with("*A") {
                    blocks.push((name, point(record, 10).unwrap_or_default(), Vec::new()));
                    open = Some(blocks.len() - 1);
                }
            }
            "ENDBLK" => open = None,
            "CIRCLE" => {
                if let (Some(index), Some(center)) = (open, point(record, 10)) {
                    blocks[index].2.push(center);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for record in records.iter().filter(|r| r[0].1 == "INSERT") {
        let name = name_of(record);
        let Some((_, base, centers)) = blocks.iter().find(|(n, _, _)| *n == name) else {
            continue;
        };
        let Some(insert) = point(record, 10) else {
            continue;
        };
        for center in centers {
            out.push((insert.0 + center.0 - base.0, insert.1 + center.1 - base.1));
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}
