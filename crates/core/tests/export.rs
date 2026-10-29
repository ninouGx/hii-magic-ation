mod common;

use common::{fresh_db, oracle_id};
use hma_core::db::Db;
use hma_core::export::{self, Export};
use hma_core::selection::{self, CardChange, Selection, SelectionMeta};

fn add(db: &Db, deck: &Selection, card: &str) {
    selection::add_card(
        &db.connection().unwrap(),
        &deck.selection_id,
        &oracle_id(db, card),
    )
    .unwrap();
}

fn deck_with(
    db: &Db,
    commander: &str,
    commander_print: Option<String>,
    cards: &[&str],
) -> Selection {
    let conn = db.connection().unwrap();
    let created = selection::create(&conn, "Deck").unwrap();
    let meta = SelectionMeta {
        name: "Deck".to_owned(),
        notes: String::new(),
        commander_oracle_id: Some(oracle_id(db, commander)),
        partner_oracle_id: None,
        commander_print_id: commander_print,
        partner_print_id: None,
    };
    let deck = selection::update(&conn, &created.selection_id, &meta).unwrap();
    for card in cards {
        add(db, &deck, card);
    }
    deck
}

/// `(print_id, "(SET) number")` of a print of the card, the default one or another.
fn print_label(db: &Db, card: &str, default: bool) -> (String, String) {
    db.connection()
        .unwrap()
        .query_row(
            "SELECT p.print_id, '(' || upper(p.set_code) || ') ' || p.collector_number
             FROM catalog.prints AS p JOIN catalog.cards AS c USING (oracle_id)
             WHERE c.name = ? AND (p.print_id = c.default_print_id) = ?
             ORDER BY p.print_id LIMIT 1",
            duckdb::params![card, default],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn export(db: &Db, deck: &Selection) -> Export {
    export::moxfield(&db.connection().unwrap(), &deck.selection_id).unwrap()
}

#[test]
fn commander_comes_first_with_its_chosen_print() {
    let (_dir, db) = fresh_db();
    let (print_id, label) = print_label(&db, "Dragonlord Atarka", false);
    let deck = deck_with(
        &db,
        "Dragonlord Atarka",
        Some(print_id),
        &["Lightning Bolt"],
    );

    let text = export(&db, &deck).text;

    assert_eq!(
        text.lines().next(),
        Some(format!("1 Dragonlord Atarka {label}").as_str())
    );
}

#[test]
fn a_card_without_a_chosen_print_uses_its_default_print() {
    let (_dir, db) = fresh_db();
    let (_, label) = print_label(&db, "Lightning Bolt", true);
    let deck = deck_with(&db, "Dragonlord Atarka", None, &["Lightning Bolt"]);

    let text = export(&db, &deck).text;

    assert!(
        text.lines()
            .any(|line| line == format!("1 Lightning Bolt {label}"))
    );
}

#[test]
fn a_chosen_print_of_a_list_card_is_kept() {
    let (_dir, db) = fresh_db();
    let (print_id, label) = print_label(&db, "Sol Ring", false);
    let deck = deck_with(&db, "Dragonlord Atarka", None, &["Sol Ring"]);
    let change = CardChange {
        quantity: 1,
        print_id: Some(print_id),
        category: None,
    };
    selection::set_card(
        &db.connection().unwrap(),
        &deck.selection_id,
        &oracle_id(&db, "Sol Ring"),
        &change,
    )
    .unwrap();

    let text = export(&db, &deck).text;

    assert!(
        text.lines()
            .any(|line| line == format!("1 Sol Ring {label}"))
    );
}

#[test]
fn double_faced_and_adventure_cards_use_the_front_name_and_split_cards_keep_both() {
    let (_dir, db) = fresh_db();
    let deck = deck_with(
        &db,
        "Dragonlord Atarka",
        None,
        &[
            "Delver of Secrets // Insectile Aberration",
            "Bonecrusher Giant // Stomp",
            "Fire // Ice",
        ],
    );

    let names: Vec<String> = export(&db, &deck)
        .text
        .lines()
        .skip(1)
        .map(|line| line.split(" (").next().unwrap().to_owned())
        .collect();

    assert_eq!(
        names,
        [
            "1 Bonecrusher Giant",
            "1 Delver of Secrets",
            "1 Fire // Ice"
        ]
    );
}

#[test]
fn quantities_are_written_and_a_commander_in_the_list_appears_once() {
    let (_dir, db) = fresh_db();
    let deck = deck_with(
        &db,
        "Dragonlord Atarka",
        None,
        &["Sol Ring", "Sol Ring", "Sol Ring", "Dragonlord Atarka"],
    );

    let text = export(&db, &deck).text;

    assert_eq!(
        text.lines()
            .filter(|line| line.contains("Dragonlord Atarka"))
            .count(),
        1
    );
    assert!(text.lines().any(|line| line.starts_with("3 Sol Ring (")));
}

#[test]
fn a_card_without_any_print_keeps_its_name_alone() {
    assert_eq!(export::line(1, "Lightning Bolt", None), "1 Lightning Bolt");
    assert_eq!(
        export::line(12, "Forest", Some(("ltr", "272"))),
        "12 Forest (LTR) 272"
    );
}

#[test]
fn a_card_removed_by_a_sync_is_skipped_and_reported() {
    let (_dir, db) = fresh_db();
    let deck = deck_with(&db, "Dragonlord Atarka", None, &["Lightning Bolt"]);
    db.connection()
        .unwrap()
        .execute(
            "INSERT INTO selection_cards (selection_id, oracle_id) VALUES (?, 'gone')",
            [&deck.selection_id],
        )
        .unwrap();

    let result = export(&db, &deck);

    assert_eq!(result.skipped, ["gone"]);
    assert!(!result.text.contains("gone"));
}
