#![allow(
    dead_code,
    reason = "each test binary uses a different part of this module"
)]

use std::path::Path;
use std::sync::OnceLock;

use hma_core::db::{CATALOG_NEW_FILE, Db};
use hma_ingest::catalog::{self, RawLayout};
use serde_json::{Value, json};

const INGEST_FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ingest/tests/fixtures");

/// The fixture catalog with EDHREC pages, built once per test binary.
pub fn db() -> &'static Db {
    static DB: OnceLock<Db> = OnceLock::new();
    DB.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        let raw = RawLayout::new(dir.join("raw"));
        std::fs::create_dir_all(dir.join("raw")).unwrap();
        std::os::unix::fs::symlink(
            Path::new(INGEST_FIXTURES).join("scryfall"),
            raw.scryfall_dir(),
        )
        .unwrap();
        for (window, pages) in [
            ("week", &["week.json", "week-pastweek-1.json"][..]),
            ("month", &["month.json"]),
            ("year", &["year.json"]),
        ] {
            let window_dir = raw.edhrec_commanders_dir().join(window);
            std::fs::create_dir_all(&window_dir).unwrap();
            for (number, page) in pages.iter().enumerate() {
                std::fs::copy(
                    Path::new(INGEST_FIXTURES)
                        .join("edhrec/commanders")
                        .join(page),
                    window_dir.join(format!("{number:03}.json")),
                )
                .unwrap();
            }
        }
        catalog::build(&raw, &dir.join(CATALOG_NEW_FILE)).unwrap();
        Db::open(&dir).unwrap()
    })
}

pub fn and(rules: Value) -> Value {
    let mut group = json!({ "combinator": "and" });
    group["rules"] = rules;
    group
}

pub fn rule(field: &str, operator: &str, value: Value) -> Value {
    let mut rule = json!({ "field": field, "operator": operator });
    rule["value"] = value;
    rule
}
