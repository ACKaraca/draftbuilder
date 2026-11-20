//! The foundation's DWG fixture opened through the script's `OPEN` (`draftbuilder.drawing.open`): its
//! entities, its layer and its block come through, and the fingerprint and the handle seed of the file
//! are kept.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use arc_base::Limits;
use arc_io_dxf::{Group, ReadOptions};
use common::{Harness, dxf_records, value_of};
use draftbuilder_model::Handle;
use draftbuilder_model::database::Database;

#[test]
fn the_dwg_fixture_opens_with_its_entities_layer_and_block_and_keeps_its_identity() {
    let mut h = Harness::new();
    let bytes = arc_io_dwg::testing::sample_dwg_bytes().unwrap();
    let dwg = h.write("sample.dwg", &bytes);
    let output = h.path("sample.dxf");
    let report = h.run(&format!(
        "OPEN path={}\nSAVE path={}\n",
        common::quoted(&dwg),
        common::quoted(&output)
    ));
    assert!(
        report.failure.is_none(),
        "{:?}",
        report.failure.as_ref().map(|f| f.error.code())
    );

    // The entities, the layer and the block of the file, as the saved drawing holds them.
    let saved = std::fs::read_to_string(&output).unwrap();
    let records = dxf_records(&saved);
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "LAYER" && value_of(r, 2) == Some("WALLS"))
    );
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "LINE" && value_of(r, 8) == Some("WALLS"))
    );
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "INSERT" && value_of(r, 2) == Some("DOOR"))
    );
    assert!(
        records
            .iter()
            .any(|r| r[0].1 == "BLOCK" && value_of(r, 2) == Some("DOOR"))
    );
    assert_eq!(
        records.iter().filter(|r| r[0].1 == "CIRCLE").count(),
        1,
        "the circle of the block"
    );

    // The fingerprint of the file is the drawing's, and the handle high-water mark is at least the
    // handle seed the file declares (its next free handle, less one).
    let snapshot = h.document.snapshot();
    let database = Database::of(&snapshot);
    let fingerprint = database.fingerprint().unwrap();
    assert!(
        fingerprint
            .to_braced()
            .eq_ignore_ascii_case(arc_io_dwg::testing::FINGERPRINT_GUID),
        "{}",
        fingerprint.to_braced()
    );
    let read =
        arc_io_dwg::read(&bytes, &mut ReadOptions::with_limits(Limits::CONSTRAINED)).unwrap();
    let seed = read
        .drawing
        .header_variable("$HANDSEED")
        .and_then(|groups| groups.first())
        .and_then(Group::as_text)
        .and_then(|text| Handle::from_hex(text.trim()))
        .unwrap();
    assert!(
        database.top() >= seed.value() - 1,
        "{} is below the file's seed {}",
        database.top(),
        seed.value()
    );
}
