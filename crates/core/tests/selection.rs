mod common;

use common::{and, fresh_db, oracle_id, rule};
use hma_core::db::Db;
use hma_core::filter::Group;
use hma_core::selection::{self, Alert, CardChange, Selection, SelectionError, SelectionMeta};
use serde_json::json;

fn meta(db: &Db, name: &str, commander: Option<&str>, partner: Option<&str>) -> SelectionMeta {
    SelectionMeta {
        name: name.to_owned(),
        notes: String::new(),
        commander_oracle_id: commander.map(|card| oracle_id(db, card)),
        partner_oracle_id: partner.map(|card| oracle_id(db, card)),
    }
}

fn add(db: &Db, selection: &Selection, card: &str) -> Selection {
    selection::add_card(
        &db.connection().unwrap(),
        &selection.selection_id,
        &oracle_id(db, card),
    )
    .unwrap()
}

fn with_commander(db: &Db, commander: &str) -> Selection {
    let conn = db.connection().unwrap();
    let created = selection::create(&conn, "Deck").unwrap();
    selection::update(
        &conn,
        &created.selection_id,
        &meta(db, "Deck", Some(commander), None),
    )
    .unwrap()
}

#[test]
fn card_outside_the_commander_identity_is_flagged_but_added() {
    let (_dir, db) = fresh_db();
    let deck = with_commander(&db, "Dragonlord Atarka");

    let deck = add(&db, &deck, "Delver of Secrets // Insectile Aberration");

    assert_eq!(deck.cards.len(), 1);
    assert!(deck.alerts.iter().any(|alert| matches!(
        alert,
        Alert::OutsideIdentity { name, .. } if name == "Delver of Secrets // Insectile Aberration"
    )));
}

#[test]
fn colorless_and_inside_identity_cards_raise_no_identity_alert() {
    let (_dir, db) = fresh_db();
    let deck = with_commander(&db, "Dragonlord Atarka");

    add(&db, &deck, "Sol Ring");
    let deck = add(&db, &deck, "Lightning Bolt");

    assert!(
        !deck
            .alerts
            .iter()
            .any(|alert| matches!(alert, Alert::OutsideIdentity { .. }))
    );
}

#[test]
fn partner_identity_joins_the_commander_identity() {
    let (_dir, db) = fresh_db();
    let conn = db.connection().unwrap();
    let deck = selection::create(&conn, "Hobbits").unwrap();
    let deck = selection::update(
        &conn,
        &deck.selection_id,
        &meta(
            &db,
            "Hobbits",
            Some("Frodo, Adventurous Hobbit"),
            Some("Sam, Loyal Attendant"),
        ),
    )
    .unwrap();

    let deck = add(&db, &deck, "Llanowar Elves");

    assert!(deck.alerts.is_empty(), "{:?}", deck.alerts);
    assert_eq!(deck.count, 3);
}

#[test]
fn selection_survives_a_restart() {
    let dir = {
        let (dir, db) = fresh_db();
        let deck = with_commander(&db, "Dragonlord Atarka");
        add(&db, &deck, "Lightning Bolt");
        dir
    };

    let db = Db::open(dir.path()).unwrap();
    let conn = db.connection().unwrap();
    let summaries = selection::list(&conn).unwrap();
    let deck = selection::get(&conn, &summaries[0].selection_id).unwrap();

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].commander_name.as_deref(),
        Some("Dragonlord Atarka")
    );
    assert_eq!(deck.cards[0].name.as_deref(), Some("Lightning Bolt"));
}

#[test]
fn count_includes_commanders_and_quantities() {
    let (_dir, db) = fresh_db();
    let deck = with_commander(&db, "Dragonlord Atarka");

    add(&db, &deck, "Lightning Bolt");
    let deck = add(&db, &deck, "Lightning Bolt");

    assert_eq!(deck.count, 3);
    assert_eq!(
        selection::list(&db.connection().unwrap()).unwrap()[0].count,
        3
    );
}

#[test]
fn second_copy_raises_a_singleton_alert() {
    let (_dir, db) = fresh_db();
    let deck = with_commander(&db, "Dragonlord Atarka");

    add(&db, &deck, "Shivan Dragon");
    let deck = add(&db, &deck, "Shivan Dragon");

    assert!(deck.alerts.contains(&Alert::Singleton {
        oracle_id: oracle_id(&db, "Shivan Dragon"),
        name: "Shivan Dragon".to_owned(),
        quantity: 2,
    }));
}

#[test]
fn commander_in_the_list_counts_once_and_is_flagged() {
    let (_dir, db) = fresh_db();
    let deck = with_commander(&db, "Dragonlord Atarka");

    let deck = add(&db, &deck, "Dragonlord Atarka");

    assert_eq!(deck.count, 1);
    assert!(
        deck.alerts
            .iter()
            .any(|alert| matches!(alert, Alert::CommanderInList { .. }))
    );
}

#[test]
fn banned_card_and_ineligible_commander_are_flagged() {
    let (_dir, db) = fresh_db();
    let deck = with_commander(&db, "Sol Ring");

    let deck = add(&db, &deck, "Black Lotus");

    assert!(deck.alerts.contains(&Alert::NotLegal {
        oracle_id: oracle_id(&db, "Black Lotus"),
        name: "Black Lotus".to_owned(),
        status: "banned".to_owned(),
    }));
    assert!(
        deck.alerts
            .iter()
            .any(|alert| matches!(alert, Alert::NotCommander { name, .. } if name == "Sol Ring"))
    );
}

#[test]
fn chosen_print_must_belong_to_the_card() {
    let (_dir, db) = fresh_db();
    let conn = db.connection().unwrap();
    let deck = with_commander(&db, "Dragonlord Atarka");
    let bolt = oracle_id(&db, "Lightning Bolt");
    let (bolt_print, bolt_image, ring_print): (String, String, String) = conn
        .query_row(
            "SELECT b.print_id, b.image_small, r.print_id
             FROM catalog.prints AS b, catalog.prints AS r, catalog.cards AS bc, catalog.cards AS rc
             WHERE b.oracle_id = bc.oracle_id AND bc.name = 'Lightning Bolt' AND b.set_code = 'lea'
               AND r.oracle_id = rc.oracle_id AND rc.name = 'Sol Ring'
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let change = |print_id: &str| CardChange {
        quantity: 1,
        print_id: Some(print_id.to_owned()),
        category: Some("Burn".to_owned()),
    };

    let foreign = selection::set_card(&conn, &deck.selection_id, &bolt, &change(&ring_print));
    let deck = selection::set_card(&conn, &deck.selection_id, &bolt, &change(&bolt_print)).unwrap();

    assert!(matches!(foreign, Err(SelectionError::ForeignPrint { .. })));
    assert_eq!(deck.cards[0].print_id.as_deref(), Some(bolt_print.as_str()));
    assert_eq!(
        deck.cards[0].image_small.as_deref(),
        Some(bolt_image.as_str())
    );
    assert_eq!(deck.cards[0].category.as_deref(), Some("Burn"));
}

#[test]
fn quantity_below_one_is_refused() {
    let (_dir, db) = fresh_db();
    let conn = db.connection().unwrap();
    let deck = with_commander(&db, "Dragonlord Atarka");

    let result = selection::set_card(
        &conn,
        &deck.selection_id,
        &oracle_id(&db, "Lightning Bolt"),
        &CardChange {
            quantity: 0,
            print_id: None,
            category: None,
        },
    );

    assert!(matches!(result, Err(SelectionError::EmptyQuantity)));
}

#[test]
fn filter_adds_every_match_once() {
    let (_dir, db) = fresh_db();
    let conn = db.connection().unwrap();
    let deck = with_commander(&db, "Dragonlord Atarka");
    let red: Group =
        serde_json::from_value(and(json!([rule("colors", "includes", json!(["R"]))]))).unwrap();

    let first = selection::add_filtered(&conn, &deck.selection_id, &red).unwrap();
    let second = selection::add_filtered(&conn, &deck.selection_id, &red).unwrap();

    let deck = selection::get(&conn, &deck.selection_id).unwrap();
    assert!(first > 3);
    assert_eq!(second, 0);
    assert_eq!(deck.cards.len(), first);
}

#[test]
fn removed_card_and_deleted_selection_are_gone() {
    let (_dir, db) = fresh_db();
    let conn = db.connection().unwrap();
    let deck = with_commander(&db, "Dragonlord Atarka");
    add(&db, &deck, "Lightning Bolt");

    let after =
        selection::remove_card(&conn, &deck.selection_id, &oracle_id(&db, "Lightning Bolt"))
            .unwrap();
    selection::delete(&conn, &deck.selection_id).unwrap();

    assert!(after.cards.is_empty());
    assert!(matches!(
        selection::get(&conn, &deck.selection_id),
        Err(SelectionError::NotFound)
    ));
    assert!(selection::list(&conn).unwrap().is_empty());
}

#[test]
fn unknown_card_is_refused() {
    let (_dir, db) = fresh_db();
    let conn = db.connection().unwrap();
    let deck = selection::create(&conn, "Deck").unwrap();

    let result = selection::add_card(&conn, &deck.selection_id, "nope");

    assert!(matches!(result, Err(SelectionError::UnknownCard(_))));
}
