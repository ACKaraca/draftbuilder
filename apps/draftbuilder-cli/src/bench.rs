//! `bench`: a diagnostic timing of the operations that the performance targets of
//! `docs/architecture/overview.md` name. It is hidden from the usage text, it is not a gate (`cargo
//! xtask ci` does not run it) and it asserts no time.
//!
//! The arguments are `bench --entities <N> --work <dir> [--seed <S>] [--reps <R>] [--build-reps <B>]
//! [--selection <K>] [--format text|json]`. The drawing of N entities (a multiple of 100) is built
//! through the product's drawing commands: up to ten seeded prototypes of 100 entities each (40 lines,
//! 20 arcs, 20 circles and 20 polylines with bulges, on four layers), copied with
//! `draftbuilder.modify.copy` to seeded target points. The drawing is saved as DXF in `<dir>`, opened
//! again, and then each operation runs on the opened drawing: move, copy, rotate and erase of K
//! selected entities (K at most 10,000), and a move of one entity. Each operation is followed by its
//! undo and its redo. The build (generate, save, open) runs B times and the modify operations R times;
//! both default to 5. Times are wall-clock times of the calling
//! thread: `commit` ends when the command returns, `settled` also waits until regeneration is idle.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_base::{Error, ObjectId, Result};
use arc_command::error::codes;
use arc_command::headless::{Exit, Product};
use arc_command::{IdParam, Invocation, Invoker, Permission, Roots, Session, Surface};
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_draft::types::{ARC, CIRCLE, LINE, POLYLINE};
use serde_json::{Value, json};

/// The entities of one prototype; a drawing has a multiple of this many entities.
const GROUP: u64 = 100;
/// The most prototypes; the copies of a drawing are spread over them.
const PROTOTYPES: u64 = 10;
/// The most selected entities of a modify command.
const MAX_SELECTION: usize = 10_000;
/// The most target points of one copy command.
const MAX_TARGETS: usize = 10_000;
/// The layers of the drawing, one per kind of prototype entity.
const LAYERS: [&str; 4] = ["WALLS", "DOORS", "FURNITURE", "ANNOTATION"];
/// The longest wait for regeneration to become idle after a command.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(600);

/// The arguments of `bench`.
#[derive(Debug)]
pub struct Options {
    /// The number of entities of the drawing; a multiple of 100.
    pub entities: u64,
    /// The seed of the generator and of the identifiers.
    pub seed: u64,
    /// The runs of each operation.
    pub reps: u32,
    /// The runs of the build (generate, save, open).
    pub build_reps: u32,
    /// The entities of each modify operation.
    pub selection: usize,
    /// The directory the DXF file is written to and read from.
    pub work: PathBuf,
    /// Whether the report is JSON (`--format json`) instead of text.
    pub json: bool,
}

fn usage(reason: impl Into<String>) -> Error {
    Error::new(codes::CLI_USAGE).with_detail("reason", reason.into())
}

fn number(name: &str, value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|_| usage(format!("value:{name}")))
}

/// Parses the arguments of `bench` (without the subcommand name).
///
/// # Errors
/// `cmd.cli.usage` for an unknown or repeated flag, a missing or malformed value, a size that is not
/// a positive multiple of 100, a selection above 10,000, or a missing `--entities` or `--work`.
pub fn parse(args: Vec<OsString>) -> Result<Options> {
    let mut entities = None;
    let mut seed = 1;
    let mut reps = 5;
    let mut build_reps = None;
    let mut selection = MAX_SELECTION;
    let mut work = None;
    let mut json = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let flag = arg.into_string().map_err(|_| usage("arguments"))?;
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (flag, None),
        };
        let value = match inline {
            Some(value) => value,
            None => args
                .next()
                .and_then(|v| v.into_string().ok())
                .ok_or_else(|| usage(format!("missing_value:{name}")))?,
        };
        match name.as_str() {
            "--entities" if entities.is_none() => entities = Some(number(&name, &value)?),
            "--seed" => seed = number(&name, &value)?,
            "--reps" => reps = number(&name, &value)?,
            "--build-reps" if build_reps.is_none() => {
                build_reps = Some(number(&name, &value)?);
            }
            "--selection" => {
                selection = usize::try_from(number(&name, &value)?)
                    .map_err(|_| usage("value:--selection"))?;
            }
            "--work" if work.is_none() => work = Some(PathBuf::from(value)),
            "--format" => match value.as_str() {
                "json" => json = true,
                "text" => json = false,
                _ => return Err(usage("value:--format")),
            },
            "--entities" | "--work" => return Err(usage(format!("repeated:{name}"))),
            _ => return Err(usage(format!("unknown_flag:{name}"))),
        }
    }
    let entities = entities.ok_or_else(|| usage("missing_flag:--entities"))?;
    let work = work.ok_or_else(|| usage("missing_flag:--work"))?;
    if entities == 0 || entities % GROUP != 0 {
        return Err(usage("value:--entities"));
    }
    let build_reps = build_reps.unwrap_or(reps);
    if reps == 0 || build_reps == 0 {
        return Err(usage("value:--reps"));
    }
    let reps = u32::try_from(reps).map_err(|_| usage("value:--reps"))?;
    let build_reps = u32::try_from(build_reps).map_err(|_| usage("value:--build-reps"))?;
    if selection == 0 || selection > MAX_SELECTION {
        return Err(usage("value:--selection"));
    }
    Ok(Options {
        entities,
        seed,
        reps,
        build_reps,
        selection,
        work,
        json,
    })
}

/// SplitMix64: a seeded generator with no operating-system randomness.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `[0, 1)` with 53 random bits.
    fn unit(&mut self) -> f64 {
        #[allow(
            clippy::cast_precision_loss,
            reason = "53 bits fit an f64 mantissa exactly"
        )]
        let bits = (self.next_u64() >> 11) as f64;
        bits / 9_007_199_254_740_992.0
    }

    fn point(&mut self, side: f64) -> [f64; 2] {
        [self.unit() * side, self.unit() * side]
    }
}

/// One named series of samples, in milliseconds.
struct Series {
    name: String,
    samples: Vec<f64>,
}

fn record(series: &mut Vec<Series>, name: String, sample: f64) {
    match series.iter_mut().find(|s| s.name == name) {
        Some(s) => s.samples.push(sample),
        None => series.push(Series {
            name,
            samples: vec![sample],
        }),
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn timed<T>(f: impl FnOnce() -> Result<T>) -> Result<(T, f64)> {
    let start = Instant::now();
    let value = f()?;
    Ok((value, ms(start.elapsed())))
}

/// The state of one run: the product, the session with the work directory as both roots, and the
/// seed of the next invocation.
struct Bench<'p> {
    product: &'p Product,
    session: Session,
    seed: u64,
    invocations: u64,
}

impl Bench<'_> {
    fn invoke(&mut self, document: &Document, command: &str, params: Value) -> Result<Value> {
        self.invocations += 1;
        let invocation = Invocation {
            seed: self.seed.wrapping_add(self.invocations),
            ..Invocation::new(command, params)
        };
        Ok(Invoker::new(&self.product.registry)
            .invoke(document, &self.session, invocation)?
            .value)
    }

    /// The identifiers a drawing command returns.
    fn created(
        &mut self,
        document: &Document,
        command: &str,
        params: Value,
    ) -> Result<Vec<IdParam>> {
        let value = self.invoke(document, command, params)?;
        serde_json::from_value(value.get("ids").cloned().unwrap_or(Value::Null))
            .map_err(|_| Error::new(codes::FAULT))
    }

    fn layer(&mut self, document: &Document, name: &str) -> Result<()> {
        self.invoke(
            document,
            "draftbuilder.settings.current_layer",
            json!({"layer": name}),
        )
        .map(|_| ())
    }

    fn new_document(&self) -> Result<Document> {
        let document = Document::open(
            Snapshot::empty(Arc::clone(&self.product.types)),
            Arc::clone(&self.product.evaluators),
            DocumentConfig::default(),
        )?;
        draftbuilder_commands::install(&document);
        Ok(document)
    }

    /// Builds the drawing of `entities` entities in a new document.
    fn generate(&mut self, entities: u64, seed: u64) -> Result<Document> {
        let document = self.new_document()?;
        self.invoke(&document, "draftbuilder.drawing.new", json!({}))?;
        for name in LAYERS {
            self.invoke(
                &document,
                "draftbuilder.layer.create",
                json!({"name": name}),
            )?;
        }
        let groups = entities / GROUP;
        let prototypes = groups.min(PROTOTYPES);
        let copies = groups - prototypes;
        // The prototypes are drawn before any copy: each draw command scans the model space, so the
        // drawing stays small while they are made.
        let mut sources = Vec::new();
        for p in 0..prototypes {
            sources.push(self.prototype(&document, seed, p)?);
        }
        let side = 2000.0 * u64_to_f64(copies.max(1)).sqrt();
        for (p, ids) in sources.iter().enumerate() {
            let p = u64::try_from(p).unwrap_or(u64::MAX);
            let count = copies / prototypes + u64::from(p < copies % prototypes);
            let mut rng = Rng::new(seed ^ salt(p, 2));
            let targets: Vec<[f64; 2]> = (0..count).map(|_| rng.point(side)).collect();
            for chunk in targets.chunks(MAX_TARGETS) {
                self.invoke(
                    &document,
                    "draftbuilder.modify.copy",
                    json!({"objects": ids, "base": [0.0, 0.0], "targets": chunk}),
                )?;
            }
        }
        Ok(document)
    }

    /// One prototype of 100 entities, drawn with the drawing commands.
    fn prototype(&mut self, document: &Document, seed: u64, p: u64) -> Result<Vec<IdParam>> {
        let mut rng = Rng::new(seed ^ salt(p, 1));
        let mut ids = Vec::new();
        self.layer(document, LAYERS[0])?;
        for _ in 0..8 {
            let points: Vec<[f64; 2]> = (0..6).map(|_| rng.point(1000.0)).collect();
            ids.extend(self.created(
                document,
                "draftbuilder.draw.line",
                json!({"points": points}),
            )?);
        }
        self.layer(document, LAYERS[1])?;
        for _ in 0..20 {
            let center = rng.point(1000.0);
            let radius = 5.0 + rng.unit() * 45.0;
            let a0 = rng.unit() * std::f64::consts::TAU;
            let a1 = a0 + 0.2 + rng.unit() * 1.4;
            let a2 = a1 + 0.2 + rng.unit() * 1.4;
            let on = |a: f64| [center[0] + radius * a.cos(), center[1] + radius * a.sin()];
            let shape =
                json!({"method": "three_points", "start": on(a0), "second": on(a1), "end": on(a2)});
            ids.extend(self.created(document, "draftbuilder.draw.arc", json!({"shape": shape}))?);
        }
        self.layer(document, LAYERS[2])?;
        for _ in 0..20 {
            let center = rng.point(1000.0);
            let radius = 2.0 + rng.unit() * 30.0;
            let shape = json!({"mode": "center", "center": center, "radius": radius});
            ids.extend(self.created(
                document,
                "draftbuilder.draw.circle",
                json!({"shape": shape}),
            )?);
        }
        self.layer(document, LAYERS[3])?;
        for _ in 0..20 {
            let count = 4 + usize::try_from(rng.next_u64() % 3).unwrap_or(0);
            let vertices: Vec<Value> = (0..count)
                .map(|_| {
                    let bulge = if rng.unit() < 0.5 {
                        0.0
                    } else {
                        rng.unit() * 1.2 - 0.6
                    };
                    json!({"point": rng.point(1000.0), "bulge": bulge})
                })
                .collect();
            let close = rng.unit() < 0.3;
            let params = json!({"vertices": vertices, "close": close});
            ids.extend(self.created(document, "draftbuilder.draw.polyline", params)?);
        }
        Ok(ids)
    }
}

/// A distinct seed for each prototype (`p`) and purpose (`kind`).
fn salt(p: u64, kind: u64) -> u64 {
    (p + 1)
        .wrapping_mul(0x2545_F491_4F6C_DD1D)
        .wrapping_add(kind.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

#[allow(clippy::cast_precision_loss, reason = "counts below 2^53 are exact")]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

/// The lines, arcs, circles and polylines of `document` by identifier (the order of creation), and
/// the first `count` of them. The first value is the number of entities in the drawing.
fn selection(document: &Document, count: usize) -> (usize, Vec<ObjectId>) {
    let snapshot = document.snapshot();
    let mut ids = Vec::new();
    for key in [LINE, ARC, CIRCLE, POLYLINE] {
        if let Ok(entities) = snapshot.iter_type(key) {
            ids.extend(entities.map(|(_, object)| object.id));
        }
    }
    ids.sort();
    let total = ids.len();
    ids.truncate(count);
    (total, ids)
}

/// Runs one undo step (`redo` false) or one redo step, and fails when there was none.
fn step(document: &Document, redo: bool) -> Result<()> {
    let pending = if redo {
        document.redo()
    } else {
        document.undo()
    };
    let done = pending.wait()?;
    if done.value {
        Ok(())
    } else {
        Err(Error::new(codes::FAULT))
    }
}

/// Runs `command` once on the drawing, then its undo and its redo, recording commit and settled times
/// of each. The drawing is left as it was before the command.
fn measure_operation(
    bench: &mut Bench<'_>,
    document: &Document,
    label: &str,
    command: &str,
    params: &Value,
    series: &mut Vec<Series>,
) -> Result<()> {
    let start = Instant::now();
    bench.invoke(document, command, params.clone())?;
    let commit = ms(start.elapsed());
    settle(document)?;
    record(series, format!("{label}.commit"), commit);
    record(series, format!("{label}.settled"), ms(start.elapsed()));

    for (redo, name) in [(false, "undo"), (true, "redo")] {
        let start = Instant::now();
        step(document, redo)?;
        let commit = ms(start.elapsed());
        settle(document)?;
        record(series, format!("{label}.{name}.commit"), commit);
        record(
            series,
            format!("{label}.{name}.settled"),
            ms(start.elapsed()),
        );
    }
    step(document, false)
}

fn settle(document: &Document) -> Result<()> {
    if document.wait_idle(SETTLE_TIMEOUT) {
        Ok(())
    } else {
        Err(Error::new(codes::FAULT))
    }
}

/// The report: every series with its samples, minimum and median, and what this tool does not measure.
fn report(
    options: &Options,
    entities_present: usize,
    selection: usize,
    dxf_bytes: u64,
    series: &[Series],
) -> Value {
    let measurements: Vec<Value> = series
        .iter()
        .map(|s| {
            let mut sorted = s.samples.clone();
            sorted.sort_by(f64::total_cmp);
            let min = sorted.first().copied().unwrap_or(0.0);
            let median = match sorted.len() {
                0 => 0.0,
                n if n % 2 == 1 => sorted[n / 2],
                n => (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0,
            };
            json!({"name": s.name, "unit": "ms", "samples": s.samples,
                   "min": min, "median": median})
        })
        .collect();
    json!({
        "tool": "draftbuilder-cli bench",
        "product_version": env!("CARGO_PKG_VERSION"),
        "build": if cfg!(debug_assertions) { "debug" } else { "release" },
        "logical_cores": std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get),
        "entities": options.entities,
        "entities_in_opened_drawing": entities_present,
        "seed": options.seed,
        "reps": options.reps,
        "build_reps": options.build_reps,
        "selection": selection,
        "dxf_bytes": dxf_bytes,
        "measurements": measurements,
        "not_measured": [
            {"target": "pan and zoom", "reason": "the render path draws one default view per call on a newly created graphics device and takes no viewport or zoom; a redraw at two zoom levels needs an interactive viewport"},
            {"target": "object snap and selection preview", "reason": "driven by pointer input in the application; no headless timing exists"},
            {"target": "Lisp entity access", "reason": "the Lisp host is not built"}
        ]
    })
}

fn text(report: &Value, out: &mut impl Write) -> Result<()> {
    let io = |e: std::io::Error| Error::new(codes::PATH_IO).with_source(e);
    let field = |name: &str| report[name].to_string();
    writeln!(
        out,
        "draftbuilder-cli bench ({} build)",
        report["build"].as_str().unwrap_or("?")
    )
    .map_err(io)?;
    writeln!(
        out,
        "entities {} (opened {}), seed {}, reps {}, build reps {}, selection {}, dxf bytes {}, logical cores {}",
        field("entities"),
        field("entities_in_opened_drawing"),
        field("seed"),
        field("reps"),
        field("build_reps"),
        field("selection"),
        field("dxf_bytes"),
        field("logical_cores")
    )
    .map_err(io)?;
    for m in report["measurements"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "{:<40} min {:>12.3} ms   median {:>12.3} ms   n {}",
            m["name"].as_str().unwrap_or("?"),
            m["min"].as_f64().unwrap_or(0.0),
            m["median"].as_f64().unwrap_or(0.0),
            m["samples"].as_array().map_or(0, Vec::len)
        )
        .map_err(io)?;
    }
    for n in report["not_measured"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "not measured: {} ({})",
            n["target"].as_str().unwrap_or("?"),
            n["reason"].as_str().unwrap_or("?")
        )
        .map_err(io)?;
    }
    Ok(())
}

/// Runs the subcommand: builds, saves, opens and measures; writes the report to `stdout`.
///
/// # Errors
/// `cmd.cli.usage` for the arguments; the errors of the commands, the file and the roots otherwise.
pub fn execute(args: Vec<OsString>, product: &Product, stdout: &mut impl Write) -> Result<Exit> {
    let options = parse(args)?;
    std::fs::create_dir_all(&options.work)
        .map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    let mut session = Session::new(
        Surface::Automation,
        [
            Permission::Read,
            Permission::Edit,
            Permission::FileRead,
            Permission::FileWrite,
        ],
    );
    session.roots = Roots::new([&options.work], [&options.work])?;
    let mut bench = Bench {
        product,
        session,
        seed: options.seed,
        invocations: 0,
    };
    let dxf: PathBuf = options
        .work
        .join(format!("bench-{}-{}.dxf", options.entities, options.seed));
    let mut series = Vec::new();

    for _ in 0..options.build_reps {
        let (document, generate) = timed(|| bench.generate(options.entities, options.seed))?;
        record(&mut series, "generate".into(), generate);
        let (_, save) =
            timed(|| draftbuilder_commands::save_document(&document, &dxf, &bench.session))?;
        record(&mut series, "save_dxf".into(), save);
    }
    let mut opened = None;
    for _ in 0..options.build_reps {
        let (document, open) = timed(|| {
            draftbuilder_commands::open_document(&dxf, DocumentConfig::default(), &bench.session)
        })?;
        record(&mut series, "open_dxf".into(), open);
        opened = Some(document);
    }
    let dxf_bytes = std::fs::metadata(&dxf).map_or(0, |m| m.len());
    let document = opened.ok_or_else(|| Error::new(codes::FAULT))?;

    let (entities_present, ids) = selection(&document, options.selection);
    let selected: Vec<IdParam> = ids.iter().copied().map(IdParam).collect();
    let single: Vec<IdParam> = ids.iter().take(1).copied().map(IdParam).collect();
    let operations = [
        (
            "move_selection",
            "draftbuilder.modify.move",
            json!({"objects": selected, "displacement": [25.0, 10.0]}),
        ),
        (
            "copy_selection",
            "draftbuilder.modify.copy",
            json!({"objects": selected, "base": [0.0, 0.0], "targets": [[25.0, 10.0]]}),
        ),
        (
            "rotate_selection",
            "draftbuilder.modify.rotate",
            json!({"objects": selected, "base": [0.0, 0.0], "angle": 0.3}),
        ),
        (
            "erase_selection",
            "draftbuilder.modify.erase",
            json!({"objects": selected}),
        ),
        (
            "move_one_entity",
            "draftbuilder.modify.move",
            json!({"objects": single, "displacement": [25.0, 10.0]}),
        ),
    ];
    for _ in 0..options.reps {
        for (label, command, params) in &operations {
            measure_operation(&mut bench, &document, label, command, params, &mut series)?;
        }
    }
    let _ = std::fs::remove_file(&dxf);
    let report = report(&options, entities_present, ids.len(), dxf_bytes, &series);
    if options.json {
        writeln!(stdout, "{report}").map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    } else {
        text(&report, stdout)?;
    }
    Ok(Exit::Success)
}

/// Runs `bench`; errors are reported as the other subcommands report them.
pub fn run<W: Write, E: Write>(
    args: Vec<OsString>,
    product: &Product,
    stdout: &mut W,
    stderr: &mut E,
) -> Exit {
    match execute(args, product, stdout) {
        Ok(exit) => exit,
        Err(error) => arc_command::headless::report_error(error, stderr),
    }
}
