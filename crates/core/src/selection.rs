//! Named card selections, the draft decks a user builds and exports.

use duckdb::types::Value;
use duckdb::{Connection, OptionalExt, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::filter::{CARDS_FROM, FilterError, Group, check_regexes, compile};

/// A Commander deck holds 100 cards, commanders included.
pub const DECK_SIZE: i64 = 100;

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("no selection with this id")]
    NotFound,
    #[error("no card with oracle_id {0}")]
    UnknownCard(String),
    #[error("print {print_id} is not a print of {oracle_id}")]
    ForeignPrint { print_id: String, oracle_id: String },
    #[error("print {0} has no commander or partner to belong to")]
    PrintWithoutCard(String),
    #[error("quantity must be at least 1")]
    EmptyQuantity,
    #[error("a selection needs a name")]
    EmptyName,
    #[error("the partner must differ from the commander")]
    PartnerIsCommander,
    #[error(transparent)]
    Filter(#[from] FilterError),
    #[error(transparent)]
    Db(#[from] duckdb::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectionSummary {
    pub selection_id: String,
    pub name: String,
    pub commander_name: Option<String>,
    pub partner_name: Option<String>,
    pub count: i64,
    pub updated_at: String,
}

/// The editable fields of a selection, all sent on each update.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SelectionMeta {
    pub name: String,
    #[serde(default)]
    pub notes: String,
    pub commander_oracle_id: Option<String>,
    pub partner_oracle_id: Option<String>,
    /// `None` keeps the default print of the card.
    #[serde(default)]
    pub commander_print_id: Option<String>,
    #[serde(default)]
    pub partner_print_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Selection {
    pub selection_id: String,
    pub name: String,
    pub notes: String,
    pub commander: Option<SelectionCard>,
    pub partner: Option<SelectionCard>,
    pub updated_at: String,
    /// Cards counted toward [`DECK_SIZE`]: commanders plus card quantities.
    pub count: i64,
    pub cards: Vec<SelectionCard>,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SelectionCard {
    pub oracle_id: String,
    /// `None` when a sync removed the card from the catalog.
    pub name: Option<String>,
    pub mana_cost: Option<String>,
    pub type_line: Option<String>,
    pub cmc: Option<f64>,
    pub quantity: i32,
    /// `None` means the default print.
    pub print_id: Option<String>,
    pub image_small: Option<String>,
    pub double_faced: bool,
    pub category: Option<String>,
}

/// Alerts flag a card, they never block a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Alert {
    /// More copies than the rules allow, `limit` is 1 for most cards.
    Singleton {
        oracle_id: String,
        name: String,
        quantity: i32,
        limit: i32,
    },
    OutsideIdentity {
        oracle_id: String,
        name: String,
    },
    NotLegal {
        oracle_id: String,
        name: String,
        status: String,
    },
    NotCommander {
        oracle_id: String,
        name: String,
    },
    CommanderInList {
        oracle_id: String,
        name: String,
    },
    MissingCard {
        oracle_id: String,
    },
}

/// What a card change sets. Every field is replaced.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CardChange {
    pub quantity: i32,
    pub print_id: Option<String>,
    pub category: Option<String>,
}

pub fn list(conn: &Connection) -> Result<Vec<SelectionSummary>, SelectionError> {
    let mut stmt = conn.prepare(
        "SELECT s.selection_id, s.name, cmd.name, partner.name,
                (s.commander_oracle_id IS NOT NULL)::INTEGER
                    + (s.partner_oracle_id IS NOT NULL)::INTEGER
                    + coalesce((SELECT sum(sc.quantity) FROM selection_cards AS sc
                                WHERE sc.selection_id = s.selection_id
                                  AND sc.oracle_id IS DISTINCT FROM s.commander_oracle_id
                                  AND sc.oracle_id IS DISTINCT FROM s.partner_oracle_id), 0),
                s.updated_at::VARCHAR
         FROM selections AS s
         LEFT JOIN catalog.cards AS cmd ON cmd.oracle_id = s.commander_oracle_id
         LEFT JOIN catalog.cards AS partner ON partner.oracle_id = s.partner_oracle_id
         ORDER BY s.updated_at DESC, s.name",
    )?;
    let summaries = stmt
        .query_map([], |row| {
            Ok(SelectionSummary {
                selection_id: row.get(0)?,
                name: row.get(1)?,
                commander_name: row.get(2)?,
                partner_name: row.get(3)?,
                count: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(summaries)
}

pub fn create(conn: &Connection, name: &str) -> Result<Selection, SelectionError> {
    ensure_name(name)?;
    let selection_id: String = conn.query_row(
        "INSERT INTO selections (name) VALUES (?) RETURNING selection_id",
        [name],
        |row| row.get(0),
    )?;
    get(conn, &selection_id)
}

pub fn delete(conn: &Connection, selection_id: &str) -> Result<(), SelectionError> {
    ensure_exists(conn, selection_id)?;
    in_transaction(conn, || {
        conn.execute(
            "DELETE FROM selection_cards WHERE selection_id = ?",
            [selection_id],
        )?;
        conn.execute(
            "DELETE FROM selections WHERE selection_id = ?",
            [selection_id],
        )?;
        Ok(())
    })
}

pub fn update(
    conn: &Connection,
    selection_id: &str,
    meta: &SelectionMeta,
) -> Result<Selection, SelectionError> {
    ensure_exists(conn, selection_id)?;
    ensure_name(&meta.name)?;
    if meta.partner_oracle_id.is_some() && meta.partner_oracle_id == meta.commander_oracle_id {
        return Err(SelectionError::PartnerIsCommander);
    }
    for (oracle_id, print_id) in [
        (&meta.commander_oracle_id, &meta.commander_print_id),
        (&meta.partner_oracle_id, &meta.partner_print_id),
    ] {
        if let Some(oracle_id) = oracle_id {
            ensure_card(conn, oracle_id)?;
        }
        if let Some(print_id) = print_id {
            let oracle_id = oracle_id
                .as_deref()
                .ok_or_else(|| SelectionError::PrintWithoutCard(print_id.clone()))?;
            ensure_print_of(conn, print_id, oracle_id)?;
        }
    }
    conn.execute(
        "UPDATE selections
         SET name = ?, notes = ?, commander_oracle_id = ?, partner_oracle_id = ?,
             commander_print_id = ?, partner_print_id = ?, updated_at = current_timestamp
         WHERE selection_id = ?",
        params![
            meta.name,
            meta.notes,
            meta.commander_oracle_id,
            meta.partner_oracle_id,
            meta.commander_print_id,
            meta.partner_print_id,
            selection_id
        ],
    )?;
    get(conn, selection_id)
}

/// A card already in the selection gets one more copy.
pub fn add_card(
    conn: &Connection,
    selection_id: &str,
    oracle_id: &str,
) -> Result<Selection, SelectionError> {
    ensure_exists(conn, selection_id)?;
    ensure_card(conn, oracle_id)?;
    in_transaction(conn, || {
        conn.execute(
            "INSERT INTO selection_cards (selection_id, oracle_id) VALUES (?, ?)
             ON CONFLICT DO UPDATE SET quantity = quantity + 1",
            [selection_id, oracle_id],
        )?;
        touch(conn, selection_id)
    })?;
    get(conn, selection_id)
}

pub fn set_card(
    conn: &Connection,
    selection_id: &str,
    oracle_id: &str,
    change: &CardChange,
) -> Result<Selection, SelectionError> {
    ensure_exists(conn, selection_id)?;
    ensure_card(conn, oracle_id)?;
    if change.quantity < 1 {
        return Err(SelectionError::EmptyQuantity);
    }
    if let Some(print_id) = &change.print_id {
        ensure_print_of(conn, print_id, oracle_id)?;
    }
    conn.execute(
        "INSERT INTO selection_cards (selection_id, oracle_id, quantity, print_id, category)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT DO UPDATE SET
             quantity = excluded.quantity,
             print_id = excluded.print_id,
             category = excluded.category",
        params![
            selection_id,
            oracle_id,
            change.quantity,
            change.print_id,
            change.category
        ],
    )?;
    touch(conn, selection_id)?;
    get(conn, selection_id)
}

pub fn remove_card(
    conn: &Connection,
    selection_id: &str,
    oracle_id: &str,
) -> Result<Selection, SelectionError> {
    ensure_exists(conn, selection_id)?;
    in_transaction(conn, || {
        conn.execute(
            "DELETE FROM selection_cards WHERE selection_id = ? AND oracle_id = ?",
            [selection_id, oracle_id],
        )?;
        touch(conn, selection_id)
    })?;
    get(conn, selection_id)
}

/// Adds every card the filter matches, once. Returns how many were not there yet.
pub fn add_filtered(
    conn: &Connection,
    selection_id: &str,
    filter: &Group,
) -> Result<usize, SelectionError> {
    ensure_exists(conn, selection_id)?;
    check_regexes(conn, filter)?;
    let compiled = compile(filter);
    let mut params = vec![Value::Text(selection_id.to_owned())];
    params.extend(compiled.params);
    in_transaction(conn, || {
        let added = conn.execute(
            &format!(
                "INSERT INTO selection_cards (selection_id, oracle_id)
                 SELECT ?, c.oracle_id FROM {CARDS_FROM} WHERE {}
                 ON CONFLICT DO NOTHING",
                compiled.sql
            ),
            params_from_iter(params),
        )?;
        touch(conn, selection_id)?;
        Ok(added)
    })
}

fn ensure_print_of(
    conn: &Connection,
    print_id: &str,
    oracle_id: &str,
) -> Result<(), SelectionError> {
    let owner: Option<String> = conn
        .query_row(
            "SELECT oracle_id FROM catalog.prints WHERE print_id = ?",
            [print_id],
            |row| row.get(0),
        )
        .optional()?;
    if owner.as_deref() == Some(oracle_id) {
        Ok(())
    } else {
        Err(SelectionError::ForeignPrint {
            print_id: print_id.to_owned(),
            oracle_id: oracle_id.to_owned(),
        })
    }
}

struct SelectionRow {
    name: String,
    notes: String,
    commander_id: Option<String>,
    partner_id: Option<String>,
    commander_print: Option<String>,
    partner_print: Option<String>,
    updated_at: String,
}

pub fn get(conn: &Connection, selection_id: &str) -> Result<Selection, SelectionError> {
    let row = conn
        .query_row(
            "SELECT name, notes, commander_oracle_id, partner_oracle_id,
                    commander_print_id, partner_print_id, updated_at::VARCHAR
             FROM selections WHERE selection_id = ?",
            [selection_id],
            |row| {
                Ok(SelectionRow {
                    name: row.get(0)?,
                    notes: row.get(1)?,
                    commander_id: row.get(2)?,
                    partner_id: row.get(3)?,
                    commander_print: row.get(4)?,
                    partner_print: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or(SelectionError::NotFound)?;
    let SelectionRow {
        name,
        notes,
        commander_id,
        partner_id,
        commander_print,
        partner_print,
        updated_at,
    } = row;

    let cards = cards(conn, selection_id)?;
    let commanders: Vec<&String> = [&commander_id, &partner_id].into_iter().flatten().collect();
    let commander = commander_id
        .as_deref()
        .map(|id| card_row(conn, id, commander_print.as_deref()))
        .transpose()?;
    let partner = partner_id
        .as_deref()
        .map(|id| card_row(conn, id, partner_print.as_deref()))
        .transpose()?;
    let in_list: i64 = cards
        .iter()
        .filter(|card| !commanders.contains(&&card.oracle_id))
        .map(|card| i64::from(card.quantity))
        .sum();
    let commander_count = i64::try_from(commanders.len()).unwrap_or(0);

    Ok(Selection {
        selection_id: selection_id.to_owned(),
        name,
        notes,
        commander,
        partner,
        updated_at,
        count: commander_count + in_list,
        alerts: alerts(conn, selection_id)?,
        cards,
    })
}

const CARD_COLUMNS: &str = "c.name, c.mana_cost, c.type_line, c.cmc,
    coalesce(p.image_small, dp.image_small), coalesce(c.layout IN ('transform', 'modal_dfc'), false)";

fn cards(conn: &Connection, selection_id: &str) -> Result<Vec<SelectionCard>, SelectionError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT sc.oracle_id, {CARD_COLUMNS}, sc.quantity, p.print_id, sc.category
         FROM selection_cards AS sc
         LEFT JOIN catalog.cards AS c ON c.oracle_id = sc.oracle_id
         LEFT JOIN catalog.prints AS p ON p.print_id = sc.print_id
         LEFT JOIN catalog.prints AS dp ON dp.print_id = c.default_print_id
         WHERE sc.selection_id = ?
         ORDER BY c.cmc NULLS LAST, c.name, sc.oracle_id"
    ))?;
    let cards = stmt
        .query_map([selection_id], |row| {
            Ok(SelectionCard {
                oracle_id: row.get(0)?,
                name: row.get(1)?,
                mana_cost: row.get(2)?,
                type_line: row.get(3)?,
                cmc: row.get(4)?,
                image_small: row.get(5)?,
                double_faced: row.get(6)?,
                quantity: row.get(7)?,
                print_id: row.get(8)?,
                category: row.get(9)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(cards)
}

/// A commander a sync removed from the catalog comes back without a name.
/// A print a sync removed comes back as `None`, so a later edit never sends it again.
fn card_row(
    conn: &Connection,
    oracle_id: &str,
    print_id: Option<&str>,
) -> Result<SelectionCard, SelectionError> {
    let found = conn
        .query_row(
            &format!(
                "SELECT {CARD_COLUMNS}, p.print_id
             FROM catalog.cards AS c
             LEFT JOIN catalog.prints AS p ON p.print_id = ?
             LEFT JOIN catalog.prints AS dp ON dp.print_id = c.default_print_id
             WHERE c.oracle_id = ?"
            ),
            params![print_id, oracle_id],
            |row| {
                Ok(SelectionCard {
                    oracle_id: oracle_id.to_owned(),
                    name: row.get(0)?,
                    mana_cost: row.get(1)?,
                    type_line: row.get(2)?,
                    cmc: row.get(3)?,
                    image_small: row.get(4)?,
                    double_faced: row.get(5)?,
                    quantity: 1,
                    print_id: row.get(6)?,
                    category: None,
                })
            },
        )
        .optional()?;
    Ok(found.unwrap_or_else(|| SelectionCard {
        oracle_id: oracle_id.to_owned(),
        name: None,
        mana_cost: None,
        type_line: None,
        cmc: None,
        quantity: 1,
        print_id: None,
        image_small: None,
        double_faced: false,
        category: None,
    }))
}

/// Copies a deck may hold, NULL for no limit. Basic lands and cards such as
/// Relentless Rats have none, Seven Dwarves and the Nazgul cap themselves.
const COPY_LIMIT_SQL: &str = "CASE
    WHEN contains(type_line, 'Basic')
        OR regexp_matches(coalesce(oracle_text, ''), 'deck can have any number of cards named') THEN NULL
    ELSE coalesce(
        CASE regexp_extract(coalesce(oracle_text, ''), 'deck can have up to (\\w+) cards named', 1)
            WHEN 'two' THEN 2 WHEN 'three' THEN 3 WHEN 'four' THEN 4 WHEN 'five' THEN 5
            WHEN 'six' THEN 6 WHEN 'seven' THEN 7 WHEN 'eight' THEN 8 WHEN 'nine' THEN 9
            WHEN 'ten' THEN 10
        END,
        1)
END";

const ALERTS_SQL: &str = "
WITH s AS (SELECT * FROM selections WHERE selection_id = $1),
commanders AS (
    SELECT c.* FROM catalog.cards AS c, s
    WHERE c.oracle_id IN (s.commander_oracle_id, s.partner_oracle_id)
),
pool AS (SELECT flatten(list(color_identity)) AS colors, count(*) AS n FROM commanders),
listed AS (
    SELECT sc.oracle_id, sc.quantity, c.name, c.type_line, c.oracle_text, c.color_identity,
           coalesce(c.legalities['commander'], 'not_legal') AS status, c.oracle_id IS NULL AS missing,
           {COPY_LIMIT} AS copy_limit
    FROM selection_cards AS sc
    LEFT JOIN catalog.cards AS c ON c.oracle_id = sc.oracle_id
    WHERE sc.selection_id = $1
)
SELECT 'missing_card', oracle_id, NULL, NULL, NULL, NULL FROM listed WHERE missing
UNION ALL
SELECT 'missing_card', id, NULL, NULL, NULL, NULL
FROM (SELECT unnest([commander_oracle_id, partner_oracle_id]) AS id FROM s)
WHERE id IS NOT NULL AND id NOT IN (SELECT oracle_id FROM catalog.cards)
UNION ALL
SELECT 'singleton', oracle_id, name, quantity, NULL, copy_limit FROM listed
WHERE NOT missing AND quantity > copy_limit
UNION ALL
SELECT 'outside_identity', l.oracle_id, l.name, NULL, NULL, NULL FROM listed AS l, pool
WHERE NOT l.missing AND pool.n > 0 AND NOT list_has_all(pool.colors, l.color_identity)
UNION ALL
SELECT 'not_legal', oracle_id, name, NULL, status, NULL FROM listed
WHERE NOT missing AND status <> 'legal'
UNION ALL
SELECT 'not_legal', oracle_id, name, NULL, coalesce(legalities['commander'], 'not_legal'), NULL FROM commanders
WHERE coalesce(legalities['commander'], 'not_legal') <> 'legal'
UNION ALL
SELECT 'not_commander', oracle_id, name, NULL, NULL, NULL FROM commanders WHERE NOT is_commander_eligible
UNION ALL
SELECT 'commander_in_list', l.oracle_id, l.name, NULL, NULL, NULL FROM listed AS l, s
WHERE l.oracle_id IN (s.commander_oracle_id, s.partner_oracle_id)
";

fn alerts(conn: &Connection, selection_id: &str) -> Result<Vec<Alert>, SelectionError> {
    let mut stmt = conn.prepare(&ALERTS_SQL.replace("{COPY_LIMIT}", COPY_LIMIT_SQL))?;
    let rows = stmt.query_map([selection_id], |row| {
        let kind: String = row.get(0)?;
        let oracle_id: String = row.get(1)?;
        let name: Option<String> = row.get(2)?;
        let name = name.unwrap_or_default();
        Ok(match kind.as_str() {
            "missing_card" => Alert::MissingCard { oracle_id },
            "singleton" => Alert::Singleton {
                oracle_id,
                name,
                quantity: row.get(3)?,
                limit: row.get(5)?,
            },
            "outside_identity" => Alert::OutsideIdentity { oracle_id, name },
            "not_legal" => Alert::NotLegal {
                oracle_id,
                name,
                status: row.get(4)?,
            },
            "not_commander" => Alert::NotCommander { oracle_id, name },
            _ => Alert::CommanderInList { oracle_id, name },
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn ensure_exists(conn: &Connection, selection_id: &str) -> Result<(), SelectionError> {
    let found: bool = conn.query_row(
        "SELECT count(*) > 0 FROM selections WHERE selection_id = ?",
        [selection_id],
        |row| row.get(0),
    )?;
    found.then_some(()).ok_or(SelectionError::NotFound)
}

fn ensure_card(conn: &Connection, oracle_id: &str) -> Result<(), SelectionError> {
    let found: bool = conn.query_row(
        "SELECT count(*) > 0 FROM catalog.cards WHERE oracle_id = ?",
        [oracle_id],
        |row| row.get(0),
    )?;
    found
        .then_some(())
        .ok_or_else(|| SelectionError::UnknownCard(oracle_id.to_owned()))
}

fn ensure_name(name: &str) -> Result<(), SelectionError> {
    if name.trim().is_empty() {
        Err(SelectionError::EmptyName)
    } else {
        Ok(())
    }
}

/// Several connections share one instance, so a change spanning two statements
/// must not show half done to another request.
fn in_transaction<T>(
    conn: &Connection,
    change: impl FnOnce() -> Result<T, SelectionError>,
) -> Result<T, SelectionError> {
    conn.execute_batch("BEGIN TRANSACTION")?;
    match change() {
        Ok(value) => {
            conn.execute_batch("COMMIT")?;
            Ok(value)
        }
        Err(error) => {
            conn.execute_batch("ROLLBACK")?;
            Err(error)
        }
    }
}

fn touch(conn: &Connection, selection_id: &str) -> Result<(), SelectionError> {
    conn.execute(
        "UPDATE selections SET updated_at = current_timestamp WHERE selection_id = ?",
        [selection_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(type_line: &str, oracle_text: &str) -> Option<i32> {
        Connection::open_in_memory()
            .unwrap()
            .query_row(
                &format!("SELECT {COPY_LIMIT_SQL} FROM (SELECT ? AS type_line, ? AS oracle_text)"),
                [type_line, oracle_text],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn basic_lands_and_any_number_cards_have_no_copy_limit() {
        assert_eq!(limit("Basic Land — Forest", "({T}: Add {G}.)"), None);
        assert_eq!(limit("Basic Snow Land — Island", ""), None);
        assert_eq!(
            limit(
                "Creature — Rat",
                "A deck can have any number of cards named Relentless Rats."
            ),
            None
        );
    }

    #[test]
    fn capped_cards_take_their_own_limit() {
        assert_eq!(
            limit(
                "Creature — Dwarf",
                "A deck can have up to seven cards named Seven Dwarves."
            ),
            Some(7)
        );
        assert_eq!(
            limit(
                "Legendary Creature — Wraith Knight",
                "A deck can have up to nine cards named Nazgûl."
            ),
            Some(9)
        );
    }

    #[test]
    fn other_cards_are_singletons() {
        assert_eq!(limit("Creature — Dragon", "Flying"), Some(1));
        assert_eq!(limit("Land", "{T}: Add {C}."), Some(1));
    }
}
