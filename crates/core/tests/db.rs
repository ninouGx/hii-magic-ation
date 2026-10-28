use hma_core::db::Db;

const ORACLE_TAGS_SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/oracle_tags_sample.jsonl.gz"
);

#[test]
fn open_attaches_user_and_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();

    let info = db.info().unwrap();

    assert_eq!(info.databases, ["catalog", "user"]);
    assert!(info.duckdb_version.starts_with('v'));
}

#[test]
fn catalog_rejects_writes() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();

    let result = db
        .connection()
        .unwrap()
        .execute_batch("CREATE TABLE catalog.probe (id INTEGER)");

    assert!(result.is_err());
}

#[test]
fn user_data_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let db = Db::open(dir.path()).unwrap();
        db.connection()
            .unwrap()
            .execute_batch("CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('kept')")
            .unwrap();
    }

    let db = Db::open(dir.path()).unwrap();
    let body: String = db
        .connection()
        .unwrap()
        .query_row("SELECT body FROM notes", [], |row| row.get(0))
        .unwrap();

    assert_eq!(body, "kept");
}

#[test]
fn connections_run_on_separate_threads() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let conn = db.connection().unwrap();
            std::thread::spawn(move || -> i64 {
                conn.query_row("SELECT ?::BIGINT * 2", [i], |row| row.get(0))
                    .unwrap()
            })
        })
        .collect();
    let results: Vec<i64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(results, [0, 2, 4, 6]);
}

#[test]
fn reads_scryfall_jsonl_gz_with_nested_taggings() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();

    let (tags, taggings): (i64, i64) = db
        .connection()
        .unwrap()
        .query_row(
            "SELECT count(DISTINCT id), count(*)
             FROM (SELECT id, unnest(taggings) FROM read_json(?, format = 'newline_delimited'))",
            [ORACLE_TAGS_SAMPLE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!((tags, taggings), (3, 9));
}
