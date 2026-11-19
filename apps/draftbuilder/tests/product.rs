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
    assert_eq!(product.file_types()[0].extension, "draftb");
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
    let tab = draftbuilder::open_dxf(&mut app, &dir.join("m0.dxf")).unwrap();
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
            assert!(!extraction.items.is_empty());
            drawn += 1;
        }
    }
    assert_eq!(drawn, 6);

    let localizer = draftbuilder::localizer(&[]).unwrap();
    assert_eq!(draftbuilder::title(&localizer), "DraftBuilder");
    draftbuilder::config(1).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
