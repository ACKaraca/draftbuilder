//! The application product, headless: it builds, registers its panels and file type, opens a
//! DXF drawing into a tab, and its extractor draws every entity of the drawing.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{Cancel, IdGenerator, Limits};
use arc_command::FixedClock;
use arc_doc::package::LockOwner;
use arc_gpu::extract::Extractor;

const FIXTURE: &str = include_str!("../../../crates/draftbuilder-commands/tests/data/m0.dxf");

fn owner() -> LockOwner {
    LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "draftbuilder tests".into(),
        time_unix_ms: 0,
    }
}

#[test]
fn the_product_opens_a_dxf_drawing_and_draws_every_entity() {
    let dir = std::env::temp_dir().join(format!("draftbuilder-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m0.dxf"), FIXTURE).unwrap();

    let product = draftbuilder::product(Arc::new(FixedClock(0)), owner()).unwrap();
    assert_eq!(product.panels().len(), 4);
    assert_eq!(product.file_types().len(), 2);
    assert!(product.opener(&dir.join("x.dxf")).is_some());
    let factory = &product.extractors()[0].factory;
    let extractor = *factory.create().downcast::<Extractor>().unwrap();

    let mut seed = 0_u64;
    let config = AppConfig {
        data_dir: dir.clone(),
        limits: Limits::DEFAULT,
        id_generators: Box::new(move || {
            seed += 1;
            IdGenerator::seeded(seed)
        }),
        recent: RecentFiles::new(4),
    };
    let mut app = Application::new(product, config);
    let tab = app.open(&dir.join("m0.dxf")).unwrap();
    let snapshot = app.tab(tab).unwrap().document().snapshot();
    let mut drawn = 0;
    for (_, object) in snapshot.iter() {
        if arc_draft::types::ENTITIES.contains(
            &snapshot
                .type_key(snapshot.resolve(object.id).unwrap())
                .unwrap()
                .as_str(),
        ) {
            let extraction = extractor(&snapshot, object, &Cancel::new()).unwrap();
            // Entities of a block definition draw through their references.
            drawn += usize::from(!extraction.items.is_empty());
        }
    }
    assert_eq!(
        drawn, 7,
        "the model space entities draw, the block's two do not"
    );

    // The save goes through the application's own path and keeps the handles of the source.
    app.save(tab).unwrap();
    let text = std::fs::read_to_string(dir.join("m0.dxf")).unwrap();
    for handle in ["100", "101", "102", "103", "104", "109", "10A", "12", "30"] {
        assert!(text.lines().any(|l| l.trim() == handle), "{handle}");
    }
    assert!(!app.tab(tab).unwrap().is_dirty());

    let localizer = draftbuilder::localizer(&[]).unwrap();
    assert_eq!(draftbuilder::title(&localizer), "DraftBuilder");
    // Shell messages are no longer raw identifiers.
    assert_ne!(
        localizer.format("app-command-undo", &[]).text,
        "app-command-undo"
    );
    draftbuilder::config(1).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_document_starts_with_a_model_space_that_is_not_an_undo_step() {
    let dir = std::env::temp_dir().join(format!("draftbuilder-new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let product = draftbuilder::product(Arc::new(FixedClock(0)), owner()).unwrap();
    let mut seed = 100_u64;
    let config = AppConfig {
        data_dir: dir,
        limits: Limits::DEFAULT,
        id_generators: Box::new(move || {
            seed += 1;
            IdGenerator::seeded(seed)
        }),
        recent: RecentFiles::new(4),
    };
    let mut app = Application::new(product, config);
    let tab = app.new_document().unwrap();
    let document = app.tab(tab).unwrap().document();
    // Without a model space no drawing command can run; the window used to open without one.
    assert!(draftbuilder_commands::model_space(&document.snapshot()).is_some());
    assert!(!document.can_undo(), "undo would remove the model space");
    assert!(!app.tab(tab).unwrap().is_dirty());
}
