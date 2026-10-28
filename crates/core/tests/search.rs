mod common;

use common::{and, db, rule};
use hma_core::filter::FilterError;
use hma_core::search::{
    SearchPage, SearchRequest, formats, search, suggest_keywords, suggest_tags,
};
use serde_json::{Value, json};

fn run(request: Value) -> SearchPage {
    try_run(request).unwrap()
}

fn try_run(request: Value) -> Result<SearchPage, FilterError> {
    let request: SearchRequest = serde_json::from_value(request).unwrap();
    search(&db().connection().unwrap(), &request)
}

fn names(page: &SearchPage) -> Vec<&str> {
    page.cards.iter().map(|card| card.name.as_str()).collect()
}

fn commanders() -> Value {
    and(json!([rule("commander_eligible", "=", json!(true))]))
}

#[test]
fn sort_by_week_commander_rank_then_cmc_on_a_filtered_result() {
    let page = run(json!({
        "filter": commanders(),
        "sort": [
            { "field": "commander_rank_week" },
            { "field": "cmc" }
        ]
    }));

    assert_eq!(
        names(&page)[..5],
        [
            "Jace, Multiverse Architect",
            "Y'shtola, Night's Blessed",
            "Atraxa, Praetors' Voice",
            "Frodo, Adventurous Hobbit",
            "Sam, Loyal Attendant"
        ]
    );
    let unranked_cmc: Vec<f64> = page.cards[5..]
        .iter()
        .map(|card| cmc_of(&card.name))
        .collect();
    assert!(unranked_cmc.is_sorted(), "{unranked_cmc:?}");
}

fn cmc_of(name: &str) -> f64 {
    db().connection()
        .unwrap()
        .query_row(
            "SELECT cmc FROM catalog.cards WHERE name = ?",
            [name],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn cards_without_a_value_go_last_in_both_directions() {
    for direction in ["asc", "desc"] {
        let page = run(json!({
            "filter": and(json!([])),
            "sort": [{ "field": "commander_rank_week", "direction": direction }]
        }));

        let ranked: Vec<&str> = names(&page)[..6].to_vec();
        assert!(
            ranked.contains(&"Jace, Multiverse Architect"),
            "{direction}"
        );
        assert!(ranked.contains(&"Frodo, Adventurous Hobbit"), "{direction}");
        assert!(!ranked.contains(&"Sol Ring"), "{direction}");
    }
}

#[test]
fn descending_num_decks_puts_the_most_played_first() {
    let page = run(json!({
        "filter": commanders(),
        "sort": [{ "field": "num_decks_year", "direction": "desc" }]
    }));

    assert_eq!(names(&page)[0], "Jace, Multiverse Architect");
}

#[test]
fn every_sort_field_runs() {
    for field in [
        "edhrec_rank",
        "commander_rank_month",
        "commander_rank_year",
        "num_decks_week",
        "num_decks_month",
        "trend_week",
        "trend_month",
        "trend_year",
        "cmc",
        "price_eur",
        "name",
        "released_at",
    ] {
        let page = run(json!({ "filter": and(json!([])), "sort": [{ "field": field }] }));

        assert_eq!(page.cards.len(), 20, "{field}");
    }
}

#[test]
fn released_at_sorts_by_the_first_english_print() {
    let page = run(json!({
        "filter": and(json!([])),
        "sort": [{ "field": "released_at" }]
    }));

    assert!(names(&page)[..5].contains(&"Black Lotus"));
}

#[test]
fn pages_split_the_result_without_overlap() {
    let request = |offset: u32| {
        run(json!({
            "filter": and(json!([])),
            "sort": [{ "field": "cmc" }],
            "offset": offset,
            "limit": 8
        }))
    };

    let pages = [request(0), request(8), request(16)];

    let sizes: Vec<(i64, usize)> = pages
        .iter()
        .map(|page| (page.total, page.cards.len()))
        .collect();
    let mut all: Vec<&str> = pages.iter().flat_map(names).collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(sizes, [(20, 8), (20, 8), (20, 4)]);
    assert_eq!(all.len(), 20);
}

#[test]
fn limit_is_capped() {
    let request: SearchRequest =
        serde_json::from_value(json!({ "filter": and(json!([])), "limit": 100_000 })).unwrap();

    assert_eq!(request.limit, 100_000);
    assert_eq!(
        search(&db().connection().unwrap(), &request)
            .unwrap()
            .cards
            .len(),
        20
    );
}

#[test]
fn cards_carry_the_default_print_image_and_price() {
    let page =
        run(json!({ "filter": and(json!([rule("name", "contains", json!("lightning bolt"))])) }));

    let card = &page.cards[0];
    assert!(
        card.image_small
            .as_deref()
            .is_some_and(|url| url.starts_with("https://"))
    );
    assert!(card.price_eur.is_some());
}

#[test]
fn invalid_regex_fails_before_the_search() {
    let error = try_run(json!({
        "filter": and(json!([rule("name", "regex", json!("(?<name>x"))]))
    }))
    .unwrap_err();

    assert!(matches!(error, FilterError::InvalidRegex { .. }));
}

#[test]
fn tag_suggestions_match_slug_label_and_alias() {
    let conn = db().connection().unwrap();

    let suggestions = suggest_tags(&conn, "RAMP", 10).unwrap();

    assert_eq!(suggestions[0].slug, "ramp");
    assert!(suggestions[0].cards > 0);
    assert!(suggestions.len() <= 10);
}

#[test]
fn tag_suggestions_put_prefix_matches_first() {
    let conn = db().connection().unwrap();

    let suggestions = suggest_tags(&conn, "burn", 50).unwrap();

    assert_eq!(suggestions[0].slug, "burn");

    let first_non_prefix = suggestions
        .iter()
        .position(|tag| {
            !tag.slug.starts_with("burn") && !tag.label.to_lowercase().starts_with("burn")
        })
        .unwrap_or(suggestions.len());
    assert!(
        suggestions[first_non_prefix..]
            .iter()
            .all(|tag| !tag.slug.starts_with("burn"))
    );
    assert!(first_non_prefix > 0);
}

#[test]
fn keyword_suggestions_ignore_case() {
    let conn = db().connection().unwrap();

    assert_eq!(suggest_keywords(&conn, "FLY", 10).unwrap(), ["Flying"]);
}

#[test]
fn formats_come_from_the_legalities() {
    let formats = formats(&db().connection().unwrap()).unwrap();

    assert!(formats.contains(&"commander".to_owned()));
    assert!(formats.contains(&"paupercommander".to_owned()));
    assert!(formats.is_sorted());
}

#[test]
fn exact_alias_counts_as_an_exact_match() {
    let conn = db().connection().unwrap();

    assert_eq!(
        suggest_tags(&conn, "CDA", 10).unwrap()[0].slug,
        "characteristic-defining-ability"
    );
}

#[test]
fn transform_and_modal_cards_are_double_faced() {
    let page = run(json!({ "filter": and(json!([])), "sort": [{ "field": "name" }] }));

    let double_faced: Vec<&str> = page
        .cards
        .iter()
        .filter(|card| card.double_faced)
        .map(|card| card.name.as_str())
        .collect();
    assert_eq!(
        double_faced,
        [
            "Delver of Secrets // Insectile Aberration",
            "Valki, God of Lies // Tibalt, Cosmic Impostor"
        ]
    );
}
