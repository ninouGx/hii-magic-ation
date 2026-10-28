use hma_core::db::{CATALOG_NEW_FILE, Db, DbError, wal_path};

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

fn write_catalog(path: &std::path::Path, marker: &str) {
    let conn = hma_core::duckdb::Connection::open(path).unwrap();
    conn.execute_batch(&format!("CREATE TABLE cards AS SELECT '{marker}' AS name"))
        .unwrap();
    conn.close().unwrap();
}

fn catalog_marker(db: &Db) -> String {
    db.connection()
        .unwrap()
        .query_row("SELECT name FROM catalog.cards", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn pending_catalog_is_promoted_at_open() {
    let dir = tempfile::tempdir().unwrap();
    write_catalog(&dir.path().join(CATALOG_NEW_FILE), "pending");

    let db = Db::open(dir.path()).unwrap();

    assert_eq!(catalog_marker(&db), "pending");
    assert!(!dir.path().join(CATALOG_NEW_FILE).exists());
}

#[test]
fn swap_replaces_the_attached_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();
    assert!(!db.catalog_ready());
    write_catalog(&dir.path().join(CATALOG_NEW_FILE), "first");
    assert!(db.swap_catalog().unwrap());
    write_catalog(&dir.path().join(CATALOG_NEW_FILE), "second");

    let swapped = db.swap_catalog().unwrap();

    assert!(swapped);
    assert!(db.catalog_ready());
    assert_eq!(catalog_marker(&db), "second");
    assert!(!db.swap_catalog().unwrap(), "nothing pending any more");
}

#[test]
fn catalog_still_open_by_its_writer_is_not_promoted() {
    let dir = tempfile::tempdir().unwrap();
    let pending = dir.path().join(CATALOG_NEW_FILE);
    write_catalog(&pending, "pending");
    std::fs::write(wal_path(&pending), b"").unwrap();

    let result = Db::open(dir.path());

    assert!(matches!(result, Err(DbError::UnclosedCatalog(_))));
    assert!(pending.exists());
}
