//! The running object snap set in the header encoding of `$OSMODE` (review item 1): the bits of the
//! codes, the geometric center kept outside the header, the suppression bit, and a file's values
//! round-tripping through open and save.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use common::Env;
use serde_json::json;

/// A DXF file whose header holds `$OSMODE` with `value`.
fn drawing_with_osmode(value: i64) -> String {
    format!(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$OSMODE\n 70\n{value}\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

fn osmode(path: &std::path::Path) -> i64 {
    let text = std::fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = lines.iter().position(|l| *l == "$OSMODE").unwrap();
    lines[at + 2].parse().unwrap()
}

#[test]
fn the_codes_of_extension_parallel_and_apparent_intersection_have_their_header_bits() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run("draftbuilder.precision.osnap", json!({"modes": "EXT"}));
    assert_eq!(setting(&mut env), 4096, "extension is bit 4096");
    env.run("draftbuilder.precision.osnap", json!({"modes": "PAR"}));
    assert_eq!(setting(&mut env), 8192, "parallel is bit 8192");
    env.run("draftbuilder.precision.osnap", json!({"modes": "APP,END"}));
    assert_eq!(
        setting(&mut env),
        2049,
        "apparent intersection is bit 2048 and names no kind"
    );
}

fn setting(env: &mut Env) -> i64 {
    env.run(
        "draftbuilder.settings.get",
        json!({"name": "snap.object.running"}),
    )["value"]
        .as_i64()
        .unwrap()
}

#[test]
fn the_geometric_center_is_a_setting_outside_the_header_bits() {
    let mut env = Env::new();
    env.run("draftbuilder.drawing.new", json!({"unit": "meter"}));
    env.run("draftbuilder.precision.osnap", json!({"modes": "GEO,END"}));
    assert_eq!(setting(&mut env), 1, "only the end snap is in the mask");
    let geo = env.run(
        "draftbuilder.settings.get",
        json!({"name": "snap.object.geometric_center"}),
    );
    assert_eq!(geo["value"], json!(true));
    let path = env.path("geo.dxf");
    env.run("draftbuilder.drawing.save", json!({"path": path.clone()}));
    assert_eq!(osmode(std::path::Path::new(&path)), 1);
}

#[test]
fn header_values_with_the_suppression_bit_round_trip_through_open_and_save() {
    for value in [4133_i64, 16447] {
        let mut env = Env::new();
        let source = env.write(
            &format!("in{value}.dxf"),
            drawing_with_osmode(value).as_bytes(),
        );
        let opened = env.run(
            "draftbuilder.drawing.open_dxf",
            json!({"path": source.clone()}),
        );
        assert!(opened.is_object(), "{opened}");
        assert_eq!(setting(&mut env), value, "the value as the file has it");
        let out = env.path(&format!("out{value}.dxf"));
        env.run("draftbuilder.drawing.save", json!({"path": out.clone()}));
        assert_eq!(osmode(std::path::Path::new(&out)), value);
        env.document = draftbuilder_commands::open_document(
            std::path::Path::new(&out),
            arc_doc::DocumentConfig {
                workers: 0,
                ..arc_doc::DocumentConfig::default()
            },
            &env.session,
        )
        .unwrap();
        assert_eq!(setting(&mut env), value, "and after a reopen");
    }
}
