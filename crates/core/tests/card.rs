mod common;

use common::db;
use hma_core::card::{CardDetail, card_detail};

fn detail(name: &str) -> CardDetail {
    let conn = db().connection().unwrap();
    let oracle_id: String = conn
        .query_row(
            "SELECT oracle_id FROM catalog.cards WHERE name = ?",
            [name],
            |row| row.get(0),
        )
        .unwrap();
    card_detail(&conn, &oracle_id).unwrap().unwrap()
}

#[test]
fn unknown_card_is_none() {
    let conn = db().connection().unwrap();

    assert!(card_detail(&conn, "nope").unwrap().is_none());
}

#[test]
fn double_faced_card_lists_both_faces() {
    let card = detail("Delver of Secrets // Insectile Aberration");

    let faces: Vec<_> = card.faces.iter().map(|face| face.name.as_deref()).collect();
    assert!(card.double_faced);
    assert_eq!(
        faces,
        [Some("Delver of Secrets"), Some("Insectile Aberration")]
    );
    assert_eq!(card.faces[1].power.as_deref(), Some("3"));
}

#[test]
fn single_faced_card_has_no_faces_and_its_lists() {
    let card = detail("Dragonlord Atarka");

    assert!(card.faces.is_empty());
    assert!(!card.double_faced);
    assert_eq!(card.color_identity, ["G", "R"]);
    assert_eq!(card.keywords, ["Flying", "Trample"]);
}

#[test]
fn legalities_cover_every_format() {
    let card = detail("Black Lotus");

    let commander = card
        .legalities
        .iter()
        .find(|legality| legality.format == "commander")
        .unwrap();
    assert_eq!(commander.status, "banned");
    assert!(card.legalities.len() > 10);
}

#[test]
fn commander_ranks_merge_partner_entries_per_period() {
    let card = detail("Frodo, Adventurous Hobbit");

    let periods: Vec<(&str, i32)> = card
        .commander_ranks
        .iter()
        .map(|rank| (rank.period.as_str(), rank.rank))
        .collect();
    assert_eq!(periods, [("week", 25), ("month", 25), ("year", 25)]);
    assert_eq!(card.commander_ranks[0].num_decks, Some(943));
}

#[test]
fn tags_rulings_and_prints_come_along() {
    let card = detail("Lightning Bolt");

    assert!(card.tags.iter().any(|tag| tag.kind == "oracle"));
    assert!(card.tags.iter().any(|tag| tag.kind == "art"));
    assert!(card.prints.len() > 20);
    assert!(
        card.prints
            .windows(2)
            .all(|pair| pair[0].released_at >= pair[1].released_at)
    );
    assert!(
        card.prints
            .iter()
            .any(|print| Some(&print.print_id) == card.default_print_id.as_ref())
    );
}

#[test]
fn rulings_are_listed_in_date_order() {
    let card = detail("Frodo, Adventurous Hobbit");

    assert!(!card.rulings.is_empty());
    assert!(
        card.rulings
            .windows(2)
            .all(|pair| pair[0].published_at <= pair[1].published_at)
    );
}

#[test]
fn each_print_lists_the_art_tags_of_its_own_illustration() {
    let card = detail("Lightning Bolt");

    let art: std::collections::BTreeSet<&str> = card
        .tags
        .iter()
        .filter(|tag| tag.kind == "art")
        .map(|tag| tag.tag_id.as_str())
        .collect();
    let per_print: std::collections::BTreeSet<&str> = card
        .prints
        .iter()
        .flat_map(|print| print.art_tag_ids.iter().map(String::as_str))
        .collect();
    let distinct_sets: std::collections::BTreeSet<&Vec<String>> =
        card.prints.iter().map(|print| &print.art_tag_ids).collect();
    assert_eq!(per_print, art);
    assert!(distinct_sets.len() > 1, "prints with other art differ");
    assert!(
        card.prints
            .iter()
            .all(|print| print.art_tag_ids.windows(2).all(|pair| pair[0] < pair[1]))
    );
}
