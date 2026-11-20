//! The identifiers of a run: a run without `--seed` draws its own seed, so two new drawings made by two
//! runs get different fingerprints; with the same `--seed` the runs give the same fingerprints.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use common::{Harness, cli_script_seeded, header_value};

const FINGERPRINT: &str = "$FINGERPRINTGUID";

/// A script that makes a new drawing and saves it as `out` (a file in the harness directory).
fn new_drawing_script(h: &Harness, name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let output = h.path(&format!("{name}.dxf"));
    let script = h.write(
        &format!("{name}.scr"),
        format!(
            "NEW unit=meter\nSAVE path=\"{}\"\n",
            output.to_str().unwrap().replace('\\', "\\\\")
        )
        .as_bytes(),
    );
    (output, script)
}

/// The fingerprint of the drawing that the run `seed` saved as `name`.
fn fingerprint_of_run(h: &Harness, name: &str, seed: Option<&str>) -> String {
    let (output, script) = new_drawing_script(h, name);
    let package = h.path(&format!("{name}.draftb"));
    let run = cli_script_seeded(&h.dir, &package, &script, seed);
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = std::fs::read_to_string(&output).unwrap();
    header_value(&text, FINGERPRINT).unwrap()
}

#[test]
fn two_runs_without_a_seed_give_two_new_drawings_different_fingerprints() {
    let h = Harness::new();
    let first = fingerprint_of_run(&h, "first", None);
    let second = fingerprint_of_run(&h, "second", None);
    assert_ne!(first, second);
}

#[test]
fn two_runs_with_the_same_seed_give_the_same_fingerprint() {
    let h = Harness::new();
    let first = fingerprint_of_run(&h, "first", Some("5"));
    let second = fingerprint_of_run(&h, "second", Some("5"));
    assert_eq!(first, second);
}

#[test]
fn a_seed_takes_the_same_number_syntax_in_script_and_run() {
    // The foundation's `run` refuses a seed with a leading `+`; `script` takes the same syntax.
    let h = Harness::new();
    let (_, script) = new_drawing_script(&h, "plus");
    let refused = cli_script_seeded(&h.dir, &h.path("plus.draftb"), &script, Some("+5"));
    assert_eq!(
        refused.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    let run = |seed: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_draftbuilder-cli"))
            .args([
                "run",
                &h.path("none.draftb").to_string_lossy(),
                "--command",
                "draftbuilder.drawing.new",
                "--seed",
                seed,
            ])
            .output()
            .unwrap()
            .status
            .code()
    };
    assert_eq!(run("+5"), Some(2));
    assert_ne!(run("5"), Some(2));
}
