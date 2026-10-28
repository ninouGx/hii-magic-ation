use std::collections::BTreeSet;

use hma_core::duckdb::Connection;
use hma_ingest::catalog::{self, RawLayout};
use hma_ingest::verify::diff_commanders;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

#[test]
fn commander_diff_names_extra_and_missing_cards() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("catalog.duckdb");
    catalog::build(&RawLayout::new(FIXTURES), &out).unwrap();
    let conn = Connection::open(&out).unwrap();
    let atraxa: String = conn
        .query_row(
            "SELECT oracle_id FROM cards WHERE name = 'Atraxa, Praetors'' Voice'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut scryfall: BTreeSet<String> = conn
        .prepare("SELECT oracle_id FROM cards WHERE is_commander_eligible")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    scryfall.remove(&atraxa);
    scryfall.insert("00000000-new-commander".to_owned());

    let diff = diff_commanders(&conn, "main", &scryfall).unwrap();

    assert_eq!(diff.extra, ["Atraxa, Praetors' Voice"]);
    assert_eq!(diff.missing, ["unknown card 00000000-new-commander"]);
    assert!(!diff.is_exact());
}
