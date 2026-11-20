//! Workflow W01 (`docs/product/workflows.md`) run as a script: the typed lines of
//! `tests/data/w01.scr` and `tests/data/w01_no_array.scr` run on an empty drawing, from the team's
//! metric template written by this test. The checks are the geometry the workflow implies (read
//! back from the saved DXF text, exact to a small tolerance), the identity of two replays, the
//! states of the trim and of the undo and redo, the refused save of the drawing with the array,
//! and the save of the drawing without it: byte-identical files, the same handles after a reopen.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use arc_doc::Snapshot;
use common::{Harness, fill, handle_of, section, type_of, value_of, values_of};
use draftbuilder_commands::script;
use draftbuilder_model::database::Database;

const W01: &str = include_str!("data/w01.scr");
const W01_NO_ARRAY: &str = include_str!("data/w01_no_array.scr");
const TEMPLATE: &[u8] = include_bytes!("data/team_metric_template.dxf");

/// The commands of the workflow with the array, in the order they complete.
const WITH_ARRAY: &[&str] = &[
    "draftbuilder.drawing.new",
    "draftbuilder.settings.setvar",
    "draftbuilder.precision.osnap",
    "draftbuilder.settings.setvar",
    "draftbuilder.settings.setvar",
    "draftbuilder.settings.setvar",
    "draftbuilder.settings.setvar",
    "draftbuilder.draw.polyline",
    "draftbuilder.settings.setvar",
    "draftbuilder.draw.circle",
    "draftbuilder.modify.array_rect",
    "draftbuilder.modify.fillet",
    "draftbuilder.modify.offset",
    "draftbuilder.settings.setvar",
    "draftbuilder.draw.line",
    "draftbuilder.draw.line",
    "draftbuilder.modify.trim",
    "history.undo",
    "history.redo",
];

/// The tangent of a quarter of a right angle: the bulge of a quarter circle.
fn quarter_bulge() -> f64 {
    (std::f64::consts::PI / 8.0).tan()
}

/// The template in the harness directory, and the output of the save.
fn paths(h: &Harness, output: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        h.write("team_metric_template.dxf", TEMPLATE),
        h.path(output),
    )
}

/// The number of objects of each type.
fn counts(snapshot: &Snapshot) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for (handle, _) in snapshot.iter() {
        *out.entry(type_of(snapshot, handle)).or_insert(0) += 1;
    }
    out
}

/// The objects of a snapshot and its foreign rows as text, sorted: two identical databases give
/// identical text.
fn dump(snapshot: &Snapshot) -> Vec<String> {
    let mut out: Vec<String> = snapshot
        .iter()
        .map(|(handle, object)| {
            format!(
                "{} {:?} {:?}",
                object.id,
                snapshot.type_key(handle),
                object.values
            )
        })
        .collect();
    out.extend(snapshot.foreign_rows().map(|row| format!("{row:?}")));
    out.sort();
    out
}

/// The vertices of a polyline record: `(x, y, bulge)` in file order.
fn vertices(record: &[(i64, String)]) -> Vec<(f64, f64, f64)> {
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    for (code, value) in record {
        let number = || value.parse::<f64>().unwrap();
        match code {
            10 => out.push((number(), 0.0, 0.0)),
            20 => out.last_mut().unwrap().1 = number(),
            42 => out.last_mut().unwrap().2 = number(),
            _ => {}
        }
    }
    out
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "{actual} is not {expected}"
    );
}

fn assert_vertices(actual: &[(f64, f64, f64)], expected: &[(f64, f64, f64)]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert_close(a.0, e.0);
        assert_close(a.1, e.1);
        assert_close(a.2, e.2);
    }
}

/// The records of the entities of a saved DXF text of the kind `kind`.
fn entities(text: &str, kind: &str) -> Vec<Vec<(i64, String)>> {
    section(text, "ENTITIES")
        .into_iter()
        .filter(|r| r[0].1 == kind)
        .collect()
}

#[test]
fn w01_draws_the_part_and_the_save_is_refused_because_of_the_array() {
    let mut h = Harness::new();
    let (template, output) = paths(&h, "w01.dxf");
    let text = fill(W01, &[("template", &template), ("output", &output)]);
    let report = h.run(&text);

    let last = script::lines(&text).last().unwrap().number;
    let failure = report.failure.as_ref().expect("the save is refused");
    assert_eq!(failure.line, last, "the save is the last line");
    assert_eq!(failure.error.code(), "db.cmd.export_drops_objects");
    assert!(!output.exists(), "nothing is written");
    let ran: Vec<&str> = report.commands.iter().map(|c| c.command.as_str()).collect();
    assert_eq!(ran, WITH_ARRAY);

    let snapshot = h.document.snapshot();
    let counts = counts(&snapshot);
    let count = |key: &str| counts.get(key).copied().unwrap_or(0);
    assert_eq!(
        count("core.draft.polyline"),
        2,
        "outline and its offset: {counts:?}"
    );
    assert_eq!(
        count("core.draft.line"),
        2,
        "centerline and wall: {counts:?}"
    );
    assert_eq!(
        count("core.draft.circle"),
        1,
        "the hole the array copies: {counts:?}"
    );
    assert_eq!(count("core.draft.array"), 1, "the array: {counts:?}");
}

#[test]
fn w01_replayed_as_a_token_script_gives_an_identical_database() {
    let mut first = Harness::new();
    let mut second = Harness::new();
    let (template_a, output_a) = paths(&first, "w01.dxf");
    let (template_b, output_b) = paths(&second, "w01.dxf");
    let run_a = first.run(&fill(
        W01,
        &[("template", &template_a), ("output", &output_a)],
    ));
    let run_b = second.run(&fill(
        W01,
        &[("template", &template_b), ("output", &output_b)],
    ));

    assert_eq!(
        dump(&first.document.snapshot()),
        dump(&second.document.snapshot())
    );
    let commands_a: Vec<_> = run_a
        .commands
        .iter()
        .map(|c| (&c.command, c.created, c.changed, c.deleted))
        .collect();
    let commands_b: Vec<_> = run_b
        .commands
        .iter()
        .map(|c| (&c.command, c.created, c.changed, c.deleted))
        .collect();
    assert_eq!(commands_a, commands_b);
}

#[test]
fn w01_without_the_array_draws_the_exact_geometry() {
    let mut h = Harness::new();
    let (template, output) = paths(&h, "w01.dxf");
    let text = fill(
        W01_NO_ARRAY,
        &[("template", &template), ("output", &output)],
    );
    let report = h.run(&text);
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );
    assert!(output.exists());
    let saved = std::fs::read_to_string(&output).unwrap();

    let b = quarter_bulge();
    let outline = entities(&saved, "LWPOLYLINE")
        .into_iter()
        .find(|r| value_of(r, 8) == Some("OUTLINE") && values_of(r, 10).len() == 8)
        .unwrap();
    assert_eq!(value_of(&outline, 70), Some("1"), "closed");
    assert_vertices(
        &vertices(&outline),
        &[
            (0.0, 2.0, b),
            (2.0, 0.0, 0.0),
            (18.0, 0.0, b),
            (20.0, 2.0, 0.0),
            (20.0, 8.0, b),
            (18.0, 10.0, 0.0),
            (2.0, 10.0, b),
            (0.0, 8.0, 0.0),
        ],
    );

    let offset = entities(&saved, "LWPOLYLINE")
        .into_iter()
        .find(|r| r != &outline)
        .unwrap();
    assert_vertices(
        &vertices(&offset),
        &[
            (1.0, 2.0, b),
            (2.0, 1.0, 0.0),
            (18.0, 1.0, b),
            (19.0, 2.0, 0.0),
            (19.0, 8.0, b),
            (18.0, 9.0, 0.0),
            (2.0, 9.0, b),
            (1.0, 8.0, 0.0),
        ],
    );

    let mut holes: Vec<(f64, f64, f64, String)> = entities(&saved, "CIRCLE")
        .iter()
        .map(|r| {
            (
                values_of(r, 10)[0],
                values_of(r, 20)[0],
                values_of(r, 40)[0],
                value_of(r, 8).unwrap().to_owned(),
            )
        })
        .collect();
    holes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert_eq!(holes.len(), 2);
    assert_eq!(holes[0].0, 5.0);
    assert_eq!(holes[0].1, 5.0);
    assert_eq!(holes[0].2, 1.0);
    assert_eq!(holes[0].3, "HOLES");
    assert_eq!(holes[1].0, 15.0);
    assert_eq!(holes[1].1, 5.0);
    assert_eq!(holes[1].2, 1.0);

    let mut lines: Vec<Vec<f64>> = entities(&saved, "LINE")
        .iter()
        .map(|r| {
            vec![
                values_of(r, 10)[0],
                values_of(r, 20)[0],
                values_of(r, 11)[0],
                values_of(r, 21)[0],
            ]
        })
        .collect();
    lines.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
    assert_eq!(
        lines,
        vec![vec![0.0, 5.0, 20.0, 5.0], vec![12.0, -1.0, 12.0, 11.0]]
    );
    for record in entities(&saved, "LINE") {
        assert_eq!(value_of(&record, 8), Some("OUTLINE"));
    }
}

#[test]
fn w01_without_the_array_saves_identical_files_with_the_same_handles_after_reopen() {
    // Two processes give two files with the same bytes. Within one process the handles a save
    // writes depend on the saves before it (the save keeps a mark of the handles it wrote), so the
    // byte comparison runs the CLI twice.
    let dir = Harness::new();
    let template = dir.write("team_metric_template.dxf", TEMPLATE);
    for name in ["first", "second"] {
        let output = dir.path(&format!("{name}.dxf"));
        let script = dir.write(
            &format!("{name}.scr"),
            fill(
                W01_NO_ARRAY,
                &[("template", &template), ("output", &output)],
            )
            .as_bytes(),
        );
        let package = dir.path(&format!("{name}.draftb"));
        let run = common::cli_script(&dir.dir, &package, &script);
        assert_eq!(
            run.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let bytes_first = std::fs::read(dir.path("first.dxf")).unwrap();
    let bytes_second = std::fs::read(dir.path("second.dxf")).unwrap();
    assert_eq!(bytes_first, bytes_second, "the saved DXF files differ");

    // Two replays in this process give the same database before the save. After a save the rows
    // of the handles it wrote depend on the saves before it in the process, so the database is
    // compared up to the save line. The handles of the file are the handles the database binds to
    // the entities it wrote.
    let mut first = Harness::new();
    let mut second = Harness::new();
    let (template_a, output_a) = paths(&first, "w01.dxf");
    let (template_b, output_b) = paths(&second, "w01.dxf");
    let before_save = |text: &str| -> String {
        let lines: Vec<&str> = text.lines().collect();
        let save = lines.iter().rposition(|l| l.starts_with("SAVE ")).unwrap();
        lines[..save].join("\n")
    };
    let script_a = fill(
        W01_NO_ARRAY,
        &[("template", &template_a), ("output", &output_a)],
    );
    let script_b = fill(
        W01_NO_ARRAY,
        &[("template", &template_b), ("output", &output_b)],
    );
    assert!(first.run(&before_save(&script_a)).failure.is_none());
    assert!(second.run(&before_save(&script_b)).failure.is_none());
    common::assert_same_lines(
        &dump(&first.document.snapshot()),
        &dump(&second.document.snapshot()),
    );
    let save = format!("SAVE path={}", common::quoted(&output_a));
    assert!(first.run(&save).failure.is_none());

    let text = String::from_utf8(std::fs::read(&output_a).unwrap()).unwrap();
    let file_handles: BTreeSet<String> = section(&text, "ENTITIES")
        .iter()
        .filter_map(|r| handle_of(r))
        .collect();
    let bound = entity_handles(&first.document.snapshot());
    assert_eq!(
        file_handles.len(),
        6,
        "two polylines, two circles, two lines"
    );
    assert_eq!(file_handles, bound);

    // Reopened, the drawing has the same entities and the same handles.
    let reopened = first.reopen(&output_a);
    assert_eq!(entity_handles(&reopened.snapshot()), file_handles);
}

/// The handles the database binds to the entities (lines, polylines and circles) of `snapshot`.
fn entity_handles(snapshot: &Snapshot) -> BTreeSet<String> {
    let database = Database::of(snapshot);
    let mut out = BTreeSet::new();
    for (handle, object) in snapshot.iter() {
        if [
            "core.draft.line",
            "core.draft.polyline",
            "core.draft.circle",
        ]
        .contains(&type_of(snapshot, handle).as_str())
        {
            out.insert(database.handle_of(object.id).unwrap().to_hex());
        }
    }
    out
}

#[test]
fn w01_trim_undo_and_redo_restore_the_intermediate_states() {
    // The state after the trim, after the two undos and after the redo: each is the script up to
    // that line, saved to a file that is read back.
    let script_lines: Vec<script::ScriptLine> = script::lines(W01_NO_ARRAY).collect();
    let stop = |word: &str| script_lines.iter().position(|l| l.text == word).unwrap();
    let states = [
        ("end", stop("end")),
        ("UNDO 2", stop("UNDO 2")),
        ("REDO", stop("REDO")),
    ];
    let mut lines_per_state = Vec::new();
    for (name, index) in states {
        let mut h = Harness::new();
        let (template, output) = paths(&h, "state.dxf");
        let head: String = script_lines[..=index]
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!(
            "{}\nSAVE path=\"{}\"\n",
            fill(&head, &[("template", &template)]),
            output.to_str().unwrap().replace('\\', "\\\\")
        );
        let report = h.run(&text);
        assert!(
            report.failure.is_none(),
            "{name}: {:?}",
            report.failure.as_ref().map(|f| f.error.code())
        );
        let saved = std::fs::read_to_string(&output).unwrap();
        let mut found: Vec<(String, Vec<f64>)> = entities(&saved, "LINE")
            .iter()
            .map(|r| {
                (
                    "LINE".to_owned(),
                    vec![
                        values_of(r, 10)[0],
                        values_of(r, 20)[0],
                        values_of(r, 11)[0],
                        values_of(r, 21)[0],
                    ],
                )
            })
            .collect();
        found.sort_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap());
        lines_per_state.push((name, found));
    }
    let (end, after_trim) = &lines_per_state[0];
    assert_eq!(*end, "end");
    assert_eq!(
        after_trim,
        &vec![
            ("LINE".to_owned(), vec![12.0, 5.0, 20.0, 5.0]),
            ("LINE".to_owned(), vec![12.0, -1.0, 12.0, 11.0]),
        ],
        "the centerline keeps the part past the wall, the picked part is cut"
    );
    let (_, after_undo) = &lines_per_state[1];
    assert_eq!(
        after_undo,
        &vec![("LINE".to_owned(), vec![0.0, 5.0, 20.0, 5.0])],
        "the trim and the wall are undone: the centerline is whole and the wall is gone"
    );
    let (_, after_redo) = &lines_per_state[2];
    assert_eq!(
        after_redo,
        &vec![
            ("LINE".to_owned(), vec![0.0, 5.0, 20.0, 5.0]),
            ("LINE".to_owned(), vec![12.0, -1.0, 12.0, 11.0]),
        ],
        "the redo gives the wall back and the trim stays undone"
    );
}
