use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hma_core::duckdb::Connection;
use hma_ingest::catalog::{self, CatalogError, RawLayout, SCRYFALL_BULKS};
use hma_ingest::edhrec::{self, EdhrecError, PopularitySource, WINDOWS};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const NOW: u64 = 1_800_000_000;

struct FixturePages {
    root: PathBuf,
    requests: Mutex<Vec<String>>,
}

impl FixturePages {
    fn new() -> Self {
        Self {
            root: Path::new(FIXTURES).join("edhrec"),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn take_requests(&self) -> Vec<String> {
        std::mem::take(&mut self.requests.lock().unwrap())
    }
}

impl PopularitySource for FixturePages {
    async fn fetch(&self, path: &str) -> Result<Vec<u8>, EdhrecError> {
        self.requests.lock().unwrap().push(path.to_owned());
        Ok(std::fs::read(self.root.join(path))?)
    }
}

struct BrokenPages;

impl PopularitySource for BrokenPages {
    async fn fetch(&self, _path: &str) -> Result<Vec<u8>, EdhrecError> {
        Ok(br#"{"container": {"renamed": []}}"#.to_vec())
    }
}

fn raw_with_scryfall_fixtures(dir: &Path) -> RawLayout {
    let raw = RawLayout::new(dir.join("raw"));
    std::fs::create_dir_all(raw.scryfall_dir()).unwrap();
    for bulk in SCRYFALL_BULKS {
        std::fs::copy(
            RawLayout::new(FIXTURES).scryfall_file(bulk),
            raw.scryfall_file(bulk),
        )
        .unwrap();
    }
    raw
}

#[tokio::test(start_paused = true)]
async fn sync_follows_more_links_across_both_page_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let source = FixturePages::new();

    let outcome = edhrec::sync_commander_pages(&source, &RawLayout::new(dir.path()), NOW)
        .await
        .unwrap();

    assert_eq!(
        source.take_requests(),
        [
            "commanders/week.json",
            "commanders/week-pastweek-1.json",
            "commanders/month.json",
            "commanders/year.json",
        ]
    );
    assert_eq!(outcome.fetched.values().sum::<usize>(), 4);
}

#[tokio::test(start_paused = true)]
async fn sync_waits_one_second_between_requests() {
    let dir = tempfile::tempdir().unwrap();
    let started = tokio::time::Instant::now();

    edhrec::sync_commander_pages(&FixturePages::new(), &RawLayout::new(dir.path()), NOW)
        .await
        .unwrap();

    assert_eq!(started.elapsed().as_secs(), 3);
}

#[tokio::test(start_paused = true)]
async fn windows_fetched_within_24_hours_are_not_fetched_again() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawLayout::new(dir.path());
    let source = FixturePages::new();
    edhrec::sync_commander_pages(&source, &raw, NOW)
        .await
        .unwrap();
    source.take_requests();

    let outcome = edhrec::sync_commander_pages(&source, &raw, NOW + 23 * 3600)
        .await
        .unwrap();

    assert!(source.take_requests().is_empty());
    assert_eq!(outcome.cached, WINDOWS);

    edhrec::sync_commander_pages(&source, &raw, NOW + 25 * 3600)
        .await
        .unwrap();
    assert_eq!(source.take_requests().len(), 4);
}

#[tokio::test(start_paused = true)]
async fn unknown_page_shape_fails_and_keeps_previous_pages() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawLayout::new(dir.path());
    edhrec::sync_commander_pages(&FixturePages::new(), &raw, NOW)
        .await
        .unwrap();

    let err = edhrec::sync_commander_pages(&BrokenPages, &raw, NOW + 25 * 3600)
        .await
        .unwrap_err();

    assert!(matches!(err, EdhrecError::UnknownShape(path) if path == "commanders/week.json"));
    let kept = std::fs::read_dir(raw.edhrec_commanders_dir().join("week"))
        .unwrap()
        .count();
    assert_eq!(kept, 2);
}

#[tokio::test(start_paused = true)]
async fn catalog_maps_commanders_by_print_id_and_partner_names() {
    let dir = tempfile::tempdir().unwrap();
    let raw = raw_with_scryfall_fixtures(dir.path());
    edhrec::sync_commander_pages(&FixturePages::new(), &raw, NOW)
        .await
        .unwrap();
    let out = dir.path().join("catalog.duckdb");

    let counts = catalog::build(&raw, &out).unwrap();

    assert_eq!((counts.edhrec_ranks, counts.edhrec_unresolved), (15, 4));
    let conn = Connection::open(&out).unwrap();
    let week: Vec<(String, String, i32)> = conn
        .prepare(
            "SELECT c.name, coalesce(p.name, ''), r.rank
             FROM edhrec_commander_ranks r
             JOIN cards c ON c.oracle_id = r.oracle_id
             LEFT JOIN cards p ON p.oracle_id = r.partner_oracle_id
             WHERE r.period = 'week'
             ORDER BY r.rank, c.name",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        week,
        [
            ("Jace, Multiverse Architect".into(), String::new(), 1),
            ("Y'shtola, Night's Blessed".into(), String::new(), 2),
            ("Atraxa, Praetors' Voice".into(), String::new(), 17),
            (
                "Frodo, Adventurous Hobbit".into(),
                "Sam, Loyal Attendant".into(),
                25
            ),
            (
                "Sam, Loyal Attendant".into(),
                "Frodo, Adventurous Hobbit".into(),
                25
            ),
        ]
    );
    let unresolved: Vec<String> = conn
        .prepare("SELECT name FROM edhrec_unresolved WHERE period = 'week' ORDER BY rank")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        unresolved,
        ["Unknown Commander One", "Unknown Commander Two"]
    );
}

#[test]
fn catalog_without_edhrec_pages_has_empty_rank_tables() {
    let dir = tempfile::tempdir().unwrap();
    let raw = raw_with_scryfall_fixtures(dir.path());

    let counts = catalog::build(&raw, &dir.path().join("catalog.duckdb")).unwrap();

    assert_eq!((counts.edhrec_ranks, counts.edhrec_unresolved), (0, 0));
}

#[test]
fn interrupted_window_download_is_ignored_by_the_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let raw = raw_with_scryfall_fixtures(dir.path());
    let partial = raw.edhrec_commanders_dir().join("week.part");
    std::fs::create_dir_all(&partial).unwrap();
    std::fs::write(partial.join("001.json"), "{\"truncated").unwrap();

    let result = catalog::build(&raw, &dir.path().join("catalog.duckdb"));

    assert!(!matches!(result, Err(CatalogError::Step { .. })));
}
