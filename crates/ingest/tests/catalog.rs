use std::path::{Path, PathBuf};

use hma_core::db::sql_string_literal;
use hma_core::duckdb::Connection;
use hma_ingest::catalog::{self, CatalogCounts, CatalogError, RawLayout, SCRYFALL_BULKS};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

struct Built {
    _dir: tempfile::TempDir,
    counts: CatalogCounts,
    conn: Connection,
}

fn build_fixture_catalog() -> Built {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("catalog.duckdb");
    let counts = catalog::build(&RawLayout::new(FIXTURES), &out).unwrap();
    let conn = Connection::open(&out).unwrap();
    Built {
        _dir: dir,
        counts,
        conn,
    }
}

fn query_one<T: hma_core::duckdb::types::FromSql>(conn: &Connection, sql: &str, name: &str) -> T {
    conn.query_row(sql, [name], |row| row.get(0)).unwrap()
}

#[test]
fn builds_every_table_from_raw_files() {
    let built = build_fixture_catalog();

    assert_eq!(
        built.counts,
        CatalogCounts {
            cards: 20,
            commanders: 10,
            cards_without_print: 0,
            prints: 438,
            rulings: 115,
            oracle_tags: 218,
            art_tags: 856,
            card_tags: 1685,
        }
    );
}

#[test]
fn excludes_tokens_that_share_a_card_name() {
    let built = build_fixture_catalog();

    let count: i64 = query_one(
        &built.conn,
        "SELECT count(*) FROM cards WHERE name = ?",
        "Tarmogoyf",
    );

    assert_eq!(count, 1);
}

#[test]
fn multi_face_text_covers_every_face() {
    let built = build_fixture_catalog();

    let text: String = query_one(
        &built.conn,
        "SELECT oracle_text FROM cards WHERE name = ?",
        "Delver of Secrets // Insectile Aberration",
    );

    assert!(text.contains("transform this creature"));
    assert!(text.ends_with("Flying"));
}

#[test]
fn front_name_is_the_first_face() {
    let built = build_fixture_catalog();

    let front: String = query_one(
        &built.conn,
        "SELECT front_name FROM cards WHERE name = ?",
        "Valki, God of Lies // Tibalt, Cosmic Impostor",
    );

    assert_eq!(front, "Valki, God of Lies");
}

#[test]
fn non_numeric_power_keeps_display_value_and_null_number() {
    let built = build_fixture_catalog();

    let (power, power_num, toughness): (String, Option<f64>, String) = built
        .conn
        .query_row(
            "SELECT power, power_num, toughness FROM cards WHERE name = 'Tarmogoyf'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(
        (power.as_str(), power_num, toughness.as_str()),
        ("*", None, "1+*")
    );
}

#[test]
fn commander_eligibility_covers_creatures_backgrounds_and_walkers() {
    let built = build_fixture_catalog();

    let eligible = |name: &str| -> bool {
        query_one(
            &built.conn,
            "SELECT is_commander_eligible FROM cards WHERE name = ?",
            name,
        )
    };

    assert!(eligible("Atraxa, Praetors' Voice"));
    assert!(eligible("Raised by Giants"));
    assert!(eligible("Teferi, Temporal Archmage"));
    assert!(eligible("Valki, God of Lies // Tibalt, Cosmic Impostor"));
    assert!(!eligible("Llanowar Elves"));
    assert!(!eligible("Black Lotus"));
}

#[test]
fn legality_filter_takes_the_format_as_a_parameter() {
    let built = build_fixture_catalog();

    let banned: String = query_one(
        &built.conn,
        "SELECT string_agg(name, ',') FROM cards WHERE legalities[?] = 'banned'",
        "commander",
    );

    assert_eq!(banned, "Black Lotus");
}

#[test]
fn default_print_is_a_recent_classic_frame() {
    let built = build_fixture_catalog();

    let (set_type, border, frame_effects, collector_number): (String, String, String, String) =
        built
            .conn
            .query_row(
                "SELECT p.set_type, p.border_color, p.frame_effects::VARCHAR, p.collector_number
                 FROM cards c JOIN prints p ON p.print_id = c.default_print_id
                 WHERE c.name = 'Lightning Bolt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

    assert!(
        [
            "core",
            "expansion",
            "commander",
            "masters",
            "draft_innovation"
        ]
        .contains(&set_type.as_str())
    );
    assert_eq!(border, "black");
    assert_eq!(frame_effects, "[]");
    assert!(collector_number.parse::<u32>().is_ok());
}

#[test]
fn art_tags_reach_cards_through_their_prints() {
    let built = build_fixture_catalog();

    let art_tags: i64 = query_one(
        &built.conn,
        "SELECT count(*) FROM card_tag_flat f
         JOIN tags t USING (tag_id) JOIN cards c USING (oracle_id)
         WHERE t.kind = 'art' AND c.name = ?",
        "Lightning Bolt",
    );

    assert!(art_tags > 0);
}

#[test]
fn tag_ancestors_include_self_and_every_parent_level() {
    let built = build_fixture_catalog();

    let (missing_self, deeper_than_parent): (i64, i64) = built
        .conn
        .query_row(
            "SELECT
                (SELECT count(*) FROM tags t WHERE NOT EXISTS (
                    SELECT 1 FROM tag_ancestors a WHERE a.tag_id = t.tag_id AND a.ancestor_id = t.tag_id)),
                (SELECT count(*) FROM tag_ancestors a WHERE a.tag_id <> a.ancestor_id AND NOT EXISTS (
                    SELECT 1 FROM tag_edges e WHERE e.child_id = a.tag_id AND e.parent_id = a.ancestor_id))",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(missing_self, 0);
    assert!(deeper_than_parent > 0, "closure must reach grandparents");
}

#[test]
fn missing_raw_file_is_reported_by_path() {
    let dir = tempfile::tempdir().unwrap();

    let err =
        catalog::build(&RawLayout::new(dir.path()), &dir.path().join("c.duckdb")).unwrap_err();

    assert!(
        matches!(err, CatalogError::MissingRaw(path) if path.ends_with("scryfall/oracle_cards.jsonl.gz"))
    );
}

#[test]
fn changed_bulk_format_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let raw = copy_fixtures_to(dir.path());
    write_renamed_fields_jsonl_gz(&raw.scryfall_file("oracle_cards"));

    let err = catalog::build(&raw, &dir.path().join("c.duckdb")).unwrap_err();

    assert!(
        matches!(
            err,
            CatalogError::Step {
                step: "validate",
                ..
            }
        ),
        "{err}"
    );
}

fn copy_fixtures_to(dir: &Path) -> RawLayout {
    let raw = RawLayout::new(dir.join("raw"));
    std::fs::create_dir_all(raw.scryfall_dir()).unwrap();
    for bulk in SCRYFALL_BULKS {
        let source: PathBuf = RawLayout::new(FIXTURES).scryfall_file(bulk);
        std::fs::copy(source, raw.scryfall_file(bulk)).unwrap();
    }
    raw
}

fn write_renamed_fields_jsonl_gz(path: &Path) {
    Connection::open_in_memory()
        .unwrap()
        .execute_batch(&format!(
            "COPY (SELECT 'x' AS id, 'renamed fields' AS title) TO {} (FORMAT json, COMPRESSION gzip)",
            sql_string_literal(&path.to_string_lossy())
        ))
        .unwrap();
}
