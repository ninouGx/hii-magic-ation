use std::path::Path;
use std::sync::OnceLock;

use hma_core::db::{CATALOG_NEW_FILE, Db};
use hma_core::duckdb::params_from_iter;
use hma_core::filter::{CARDS_FROM, FilterError, Group, check_regexes, compile};
use hma_ingest::catalog::{self, RawLayout};
use serde_json::{Value, json};

const INGEST_FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ingest/tests/fixtures");

fn db() -> &'static Db {
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

fn group(filter: Value) -> Group {
    serde_json::from_value(filter).unwrap()
}

fn and(rules: Value) -> Value {
    let mut group = json!({ "combinator": "and" });
    group["rules"] = rules;
    group
}

fn rule(field: &str, operator: &str, value: Value) -> Value {
    let mut rule = json!({ "field": field, "operator": operator });
    rule["value"] = value;
    rule
}

fn names(filter: Value) -> Vec<String> {
    let compiled = compile(&group(filter));
    let conn = db().connection().unwrap();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT c.name FROM {CARDS_FROM} WHERE {} ORDER BY c.name",
            compiled.sql
        ))
        .unwrap();
    stmt.query_map(params_from_iter(compiled.params), |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn names_where(field: &str, operator: &str, value: Value) -> Vec<String> {
    names(and(json!([rule(field, operator, value)])))
}

#[test]
fn empty_group_matches_every_card() {
    assert_eq!(names(and(json!([]))).len(), 20);
}

#[test]
fn legality_matches_the_status_in_the_given_format() {
    let legal = names_where(
        "legality",
        "=",
        json!({ "format": "commander", "status": "legal" }),
    );
    let banned = names_where(
        "legality",
        "=",
        json!({ "format": "commander", "status": "banned" }),
    );

    assert!(legal.contains(&"Sol Ring".to_owned()));
    assert_eq!(banned, ["Black Lotus"]);
}

#[test]
fn name_contains_ignores_case() {
    assert_eq!(
        names_where("name", "contains", json!("DRAGON")),
        ["Dragonlord Atarka", "Shivan Dragon"]
    );
}

#[test]
fn name_not_contains_excludes_matches() {
    let names = names_where("name", "not_contains", json!("dragon"));

    assert_eq!(names.len(), 18);
    assert!(!names.iter().any(|name| name.contains("Dragon")));
}

#[test]
fn name_regex_is_case_insensitive_and_anchors_on_each_face() {
    assert_eq!(
        names_where("name", "regex", json!("^dragon")),
        ["Dragonlord Atarka"]
    );
    assert_eq!(
        names_where("name", "regex", json!("^ice$")),
        ["Fire // Ice"]
    );
}

#[test]
fn oracle_text_searches_every_face() {
    assert_eq!(
        names_where("oracle_text", "contains", json!("tap target permanent")),
        ["Fire // Ice"]
    );
}

#[test]
fn oracle_text_regex_anchors_on_each_line() {
    assert!(
        names_where("oracle_text", "regex", json!("^flying$"))
            .contains(&"Shivan Dragon".to_owned())
    );
}

#[test]
fn type_line_matches_back_faces() {
    assert_eq!(
        names_where("type_line", "regex", json!("^legendary planeswalker")),
        [
            "Jace, Multiverse Architect",
            "Teferi, Temporal Archmage",
            "Valki, God of Lies // Tibalt, Cosmic Impostor"
        ]
    );
}

#[test]
fn color_identity_at_most_keeps_subsets_and_colorless() {
    assert_eq!(
        names_where("color_identity", "at_most", json!(["U", "R"])),
        [
            "Black Lotus",
            "Bonecrusher Giant // Stomp",
            "Delver of Secrets // Insectile Aberration",
            "Fire // Ice",
            "Lightning Bolt",
            "Shivan Dragon",
            "Sol Ring",
            "Teferi, Temporal Archmage"
        ]
    );
}

#[test]
fn color_identity_includes_requires_every_color() {
    assert_eq!(
        names_where("color_identity", "includes", json!(["B", "W"])),
        [
            "Atraxa, Praetors' Voice",
            "Frodo, Adventurous Hobbit",
            "Jace, Multiverse Architect",
            "Y'shtola, Night's Blessed"
        ]
    );
}

#[test]
fn color_identity_exactly_matches_the_same_set() {
    assert_eq!(
        names_where("color_identity", "exactly", json!(["R", "G"])),
        ["Dragonlord Atarka"]
    );
}

#[test]
fn empty_color_list_means_colorless() {
    assert_eq!(
        names_where("colors", "exactly", json!([])),
        ["Black Lotus", "Sol Ring"]
    );
    assert_eq!(
        names_where("color_identity", "at_most", json!([])),
        ["Black Lotus", "Sol Ring"]
    );
}

#[test]
fn colors_use_both_faces_of_a_split_card() {
    assert!(
        names_where("colors", "includes", json!(["U", "R"])).contains(&"Fire // Ice".to_owned())
    );
}

#[test]
fn cmc_supports_every_comparison() {
    assert_eq!(names_where("cmc", "=", json!(0)), ["Black Lotus"]);
    assert_eq!(
        names_where("cmc", ">=", json!(6)),
        [
            "Dragonlord Atarka",
            "Raised by Giants",
            "Shivan Dragon",
            "Teferi, Temporal Archmage"
        ]
    );
    assert_eq!(names_where("cmc", ">", json!(6)), ["Dragonlord Atarka"]);
    assert_eq!(names_where("cmc", "<", json!(1)), ["Black Lotus"]);
    assert_eq!(
        names_where("cmc", "<=", json!(1)),
        [
            "Black Lotus",
            "Bushi Tenderfoot // Kenzo the Hardhearted",
            "Delver of Secrets // Insectile Aberration",
            "Lightning Bolt",
            "Llanowar Elves",
            "Sol Ring"
        ]
    );
    assert_eq!(
        names_where("cmc", "between", json!([5, 6])),
        [
            "Jace, Multiverse Architect",
            "Raised by Giants",
            "Shivan Dragon",
            "Teferi, Temporal Archmage"
        ]
    );
}

#[test]
fn power_compares_numbers_and_skips_star_values() {
    assert_eq!(
        names_where("power", ">=", json!(5)),
        ["Dragonlord Atarka", "Shivan Dragon"]
    );
    assert!(!names_where("power", ">=", json!(0)).contains(&"Tarmogoyf".to_owned()));
}

#[test]
fn toughness_and_loyalty_compare_numbers() {
    assert_eq!(
        names_where("toughness", "=", json!(8)),
        ["Dragonlord Atarka"]
    );
    assert_eq!(
        names_where("loyalty", ">=", json!(5)),
        ["Teferi, Temporal Archmage"]
    );
}

#[test]
fn keywords_contain_any_or_all_ignoring_case() {
    assert_eq!(
        names_where("keywords", "contains_all", json!(["flying", "TRAMPLE"])),
        ["Dragonlord Atarka"]
    );
    assert_eq!(
        names_where("keywords", "contains_any", json!(["trample", "deathtouch"])),
        [
            "Atraxa, Praetors' Voice",
            "Dragonlord Atarka",
            "Wilson, Refined Grizzly"
        ]
    );
}

#[test]
fn commander_eligible_is_a_boolean() {
    let commanders = names_where("commander_eligible", "=", json!(true));
    let others = names_where("commander_eligible", "=", json!(false));

    assert!(commanders.contains(&"Raised by Giants".to_owned()));
    assert!(others.contains(&"Sol Ring".to_owned()));
    assert_eq!(commanders.len() + others.len(), 20);
}

#[test]
fn edhrec_rank_compares_numbers() {
    let ranked = names_where("edhrec_rank", "<=", json!(1));

    assert_eq!(ranked, ["Sol Ring"]);
}

#[test]
fn or_group_unions_and_not_group_negates() {
    let dragons = json!({
        "combinator": "or",
        "rules": [
            rule("name", "regex", json!("^dragon")),
            rule("type_line", "contains", json!("Dragon")),
        ]
    });
    let not_red = json!({
        "combinator": "and",
        "not": true,
        "rules": [rule("colors", "includes", json!(["R"]))]
    });

    assert_eq!(
        names(json!({ "combinator": "or", "rules": [dragons.clone()] })),
        ["Dragonlord Atarka", "Shivan Dragon"]
    );
    assert_eq!(names(and(json!([dragons, not_red]))), Vec::<String>::new());
}

#[test]
fn not_group_keeps_cards_without_a_value() {
    let not_big = json!({
        "combinator": "and",
        "not": true,
        "rules": [rule("power", ">=", json!(5))]
    });

    let names = names(and(json!([not_big])));

    assert_eq!(names.len(), 18);
    assert!(names.contains(&"Sol Ring".to_owned()));
}

#[test]
fn groups_nest_three_levels_deep() {
    let inner = json!({
        "combinator": "or",
        "rules": [rule("name", "contains", json!("bolt")), rule("name", "contains", json!("ring"))]
    });
    let middle = json!({ "combinator": "and", "rules": [inner, rule("cmc", "<=", json!(1))] });

    assert_eq!(names(and(json!([middle]))), ["Lightning Bolt", "Sol Ring"]);
}

#[test]
fn user_values_travel_as_parameters() {
    let compiled = compile(&group(and(json!([rule(
        "name",
        "contains",
        json!("'; DROP TABLE cards; --")
    )]))));

    assert!(!compiled.sql.contains("DROP"));
    assert_eq!(compiled.params.len(), 1);
    assert!(names_where("name", "contains", json!("'; DROP TABLE cards; --")).is_empty());
}

#[test]
fn query_builder_ids_are_accepted() {
    let filter = json!({
        "id": "root",
        "combinator": "and",
        "rules": [{ "id": "r1", "field": "cmc", "operator": "=", "value": 0, "valueSource": "value" }]
    });

    assert_eq!(names(filter), ["Black Lotus"]);
}

#[test]
fn unknown_field_and_wrong_value_type_are_rejected() {
    let unknown: Result<Group, _> =
        serde_json::from_value(and(json!([rule("nope", "=", json!(1))])));
    let wrong: Result<Group, _> =
        serde_json::from_value(and(json!([rule("cmc", "=", json!("three"))])));

    assert!(unknown.unwrap_err().to_string().contains("nope"));
    assert!(wrong.is_err());
}

#[test]
fn invalid_regex_reports_the_re2_message() {
    let conn = db().connection().unwrap();
    let filter = group(and(json!([rule("name", "regex", json!("(?=dragon)"))])));

    let error = check_regexes(&conn, &filter).unwrap_err();

    match error {
        FilterError::InvalidRegex { pattern, message } => {
            assert_eq!(pattern, "(?=dragon)");
            assert!(message.contains("invalid perl operator"), "{message}");
        }
        FilterError::Db(error) => panic!("{error}"),
    }
}

#[test]
fn valid_regexes_pass_the_check() {
    let conn = db().connection().unwrap();
    let filter = group(and(json!([rule("oracle_text", "regex", json!("^{T}:"))])));

    assert!(check_regexes(&conn, &filter).is_ok());
}

fn tag_id(slug: &str) -> String {
    db().connection()
        .unwrap()
        .query_row(
            "SELECT tag_id FROM catalog.tags WHERE slug = ? AND kind = 'oracle'",
            [slug],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn has_tag_matches_direct_taggings_only() {
    let burn = tag_id("burn");

    assert!(names_where("tag", "has", json!({ "tag_id": burn })).is_empty());
}

#[test]
fn has_tag_with_children_follows_the_hierarchy() {
    let burn = tag_id("burn");
    let burn_player = tag_id("burn-player");

    let with_children = names_where(
        "tag",
        "has",
        json!({ "tag_id": burn, "include_children": true }),
    );

    let direct_child = names_where("tag", "has", json!({ "tag_id": burn_player }));

    assert!(!direct_child.is_empty());
    assert!(direct_child.iter().all(|name| with_children.contains(name)));
}

#[test]
fn has_not_tag_keeps_every_other_card() {
    let ramp = tag_id("ramp");

    let tagged = names_where("tag", "has", json!({ "tag_id": ramp }));
    let untagged = names_where("tag", "has_not", json!({ "tag_id": ramp }));

    assert!(!tagged.is_empty());
    assert_eq!(tagged.len() + untagged.len(), 20);
}

#[test]
fn art_tags_reach_the_card_through_its_prints() {
    let conn = db().connection().unwrap();
    let art_tag: String = conn
        .query_row(
            "SELECT tag_id FROM catalog.tags WHERE kind = 'art' AND tag_id IN (SELECT tag_id FROM catalog.card_tag_flat) LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(!names_where("tag", "has", json!({ "tag_id": art_tag })).is_empty());
}

#[test]
fn commander_rank_uses_the_period_and_the_best_entry() {
    assert_eq!(
        names_where("commander_rank_week", "<=", json!(2)),
        ["Jace, Multiverse Architect", "Y'shtola, Night's Blessed"]
    );
    assert_eq!(
        names_where("commander_rank_month", "between", json!([17, 25])),
        [
            "Atraxa, Praetors' Voice",
            "Frodo, Adventurous Hobbit",
            "Sam, Loyal Attendant"
        ]
    );
    assert!(names_where("commander_rank_year", ">", json!(25)).is_empty());
}

#[test]
fn num_decks_compares_per_period() {
    assert_eq!(
        names_where("num_decks_week", ">=", json!(2019)),
        ["Jace, Multiverse Architect", "Y'shtola, Night's Blessed"]
    );
    assert_eq!(names_where("num_decks_month", "=", json!(943)).len(), 2);
    assert_eq!(names_where("num_decks_year", ">", json!(3000)).len(), 1);
}

#[test]
fn flavor_text_matches_any_print() {
    assert_eq!(
        names_where("flavor_text", "contains", json!("sparkmage")),
        ["Lightning Bolt"]
    );
    assert!(
        !names_where("flavor_text", "not_contains", json!("sparkmage"))
            .contains(&"Lightning Bolt".to_owned())
    );
    assert_eq!(
        names_where("flavor_text", "regex", json!("^the sparkmage")),
        ["Lightning Bolt"]
    );
}

#[test]
fn rulings_match_any_ruling() {
    assert_eq!(
        names_where("rulings", "contains", json!("\"partner with\" keyword")),
        ["Frodo, Adventurous Hobbit", "Sam, Loyal Attendant"]
    );
}

#[test]
fn artist_matches_any_print_ignoring_case() {
    let rush = names_where("artist", "=", json!("christopher rush"));

    assert!(rush.contains(&"Lightning Bolt".to_owned()));
    assert!(rush.contains(&"Black Lotus".to_owned()));
    assert!(!rush.contains(&"Sol Ring".to_owned()));
}

#[test]
fn set_and_year_accept_one_value_or_a_list() {
    let alpha = [
        "Black Lotus",
        "Lightning Bolt",
        "Llanowar Elves",
        "Shivan Dragon",
        "Sol Ring",
    ];

    assert_eq!(names_where("set", "=", json!("LEA")), alpha);
    assert_eq!(names_where("set", "in", json!(["lea", "nope"])), alpha);
    assert!(names_where("set", "in", json!([])).is_empty());
    assert_eq!(names_where("year", "in", json!([1993])), alpha);
}

#[test]
fn price_uses_the_default_print() {
    let conn = db().connection().unwrap();
    let price: f64 = conn
        .query_row(
            "SELECT dp.price_eur::DOUBLE FROM catalog.cards AS c JOIN catalog.prints AS dp ON dp.print_id = c.default_print_id WHERE c.name = 'Lightning Bolt'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(names_where("price_eur", "=", json!(price)).contains(&"Lightning Bolt".to_owned()));
    assert!(!names_where("price_eur", ">", json!(price)).contains(&"Lightning Bolt".to_owned()));
}

#[test]
fn print_and_ruling_regexes_are_checked() {
    let conn = db().connection().unwrap();
    for field in ["flavor_text", "rulings"] {
        let filter = group(and(json!([rule(field, "regex", json!("(a"))])));

        assert!(matches!(
            check_regexes(&conn, &filter),
            Err(FilterError::InvalidRegex { .. })
        ));
    }
}
