//! Everything the card detail window shows about one card.

use duckdb::{Connection, OptionalExt, Row};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CardDetail {
    pub oracle_id: String,
    pub name: String,
    pub mana_cost: Option<String>,
    pub cmc: Option<f64>,
    pub type_line: Option<String>,
    pub oracle_text: Option<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub layout: String,
    pub double_faced: bool,
    pub colors: Vec<String>,
    pub color_identity: Vec<String>,
    pub keywords: Vec<String>,
    pub is_commander_eligible: bool,
    pub game_changer: bool,
    pub edhrec_rank: Option<i32>,
    pub default_print_id: Option<String>,
    pub faces: Vec<Face>,
    pub legalities: Vec<Legality>,
    pub commander_ranks: Vec<CommanderRank>,
    pub tags: Vec<CardTag>,
    pub rulings: Vec<Ruling>,
    pub prints: Vec<Print>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Face {
    pub name: Option<String>,
    pub mana_cost: Option<String>,
    pub type_line: Option<String>,
    pub oracle_text: Option<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Legality {
    pub format: String,
    pub status: String,
}

/// Merged over solo and partner entries, as in the search.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommanderRank {
    pub period: String,
    pub rank: i32,
    pub num_decks: Option<i64>,
    pub trend_zscore: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardTag {
    pub tag_id: String,
    pub slug: String,
    pub label: String,
    pub kind: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ruling {
    pub published_at: String,
    pub source: String,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Print {
    pub print_id: String,
    pub set_code: String,
    pub collector_number: String,
    pub released_at: Option<String>,
    pub rarity: Option<String>,
    pub artist: Option<String>,
    pub image_small: Option<String>,
    pub price_eur: Option<f64>,
    pub price_usd: Option<f64>,
}

/// `None` when no card has this `oracle_id`.
pub fn card_detail(
    conn: &Connection,
    oracle_id: &str,
) -> Result<Option<CardDetail>, duckdb::Error> {
    let Some(mut card) = conn
        .query_row(
            "SELECT oracle_id, name, mana_cost, cmc, type_line, oracle_text, power, toughness, loyalty,
                    layout, layout IN ('transform', 'modal_dfc'),
                    to_json(coalesce(colors, []))::VARCHAR,
                    to_json(coalesce(color_identity, []))::VARCHAR,
                    to_json(coalesce(keywords, []))::VARCHAR,
                    is_commander_eligible, game_changer, edhrec_rank, default_print_id
             FROM catalog.cards
             WHERE oracle_id = ?",
            [oracle_id],
            card_row,
        )
        .optional()?
    else {
        return Ok(None);
    };
    card.faces = faces(conn, oracle_id)?;
    card.legalities = legalities(conn, oracle_id)?;
    card.commander_ranks = commander_ranks(conn, oracle_id)?;
    card.tags = tags(conn, oracle_id)?;
    card.rulings = rulings(conn, oracle_id)?;
    card.prints = prints(conn, oracle_id)?;
    Ok(Some(card))
}

fn card_row(row: &Row) -> Result<CardDetail, duckdb::Error> {
    Ok(CardDetail {
        oracle_id: row.get(0)?,
        name: row.get(1)?,
        mana_cost: row.get(2)?,
        cmc: row.get(3)?,
        type_line: row.get(4)?,
        oracle_text: row.get(5)?,
        power: row.get(6)?,
        toughness: row.get(7)?,
        loyalty: row.get(8)?,
        layout: row.get(9)?,
        double_faced: row.get(10)?,
        colors: json_list(row, 11)?,
        color_identity: json_list(row, 12)?,
        keywords: json_list(row, 13)?,
        is_commander_eligible: row.get(14)?,
        game_changer: row.get(15)?,
        edhrec_rank: row.get(16)?,
        default_print_id: row.get(17)?,
        faces: Vec::new(),
        legalities: Vec::new(),
        commander_ranks: Vec::new(),
        tags: Vec::new(),
        rulings: Vec::new(),
        prints: Vec::new(),
    })
}

fn json_list(row: &Row, index: usize) -> Result<Vec<String>, duckdb::Error> {
    let json: String = row.get(index)?;
    serde_json::from_str(&json).map_err(|error| {
        duckdb::Error::FromSqlConversionFailure(index, duckdb::types::Type::Text, Box::new(error))
    })
}

fn rows<T>(
    conn: &Connection,
    sql: &str,
    oracle_id: &str,
    map: impl FnMut(&Row) -> Result<T, duckdb::Error>,
) -> Result<Vec<T>, duckdb::Error> {
    conn.prepare(sql)?.query_map([oracle_id], map)?.collect()
}

fn faces(conn: &Connection, oracle_id: &str) -> Result<Vec<Face>, duckdb::Error> {
    rows(
        conn,
        "SELECT f.name, f.mana_cost, f.type_line, f.oracle_text, f.power, f.toughness, f.loyalty
         FROM (SELECT unnest(card_faces) AS f FROM catalog.cards WHERE oracle_id = ?)",
        oracle_id,
        |row| {
            Ok(Face {
                name: row.get(0)?,
                mana_cost: row.get(1)?,
                type_line: row.get(2)?,
                oracle_text: row.get(3)?,
                power: row.get(4)?,
                toughness: row.get(5)?,
                loyalty: row.get(6)?,
            })
        },
    )
}

fn legalities(conn: &Connection, oracle_id: &str) -> Result<Vec<Legality>, duckdb::Error> {
    rows(
        conn,
        "SELECT unnest(map_keys(legalities)) AS format, unnest(map_values(legalities))
         FROM catalog.cards WHERE oracle_id = ? ORDER BY format",
        oracle_id,
        |row| {
            Ok(Legality {
                format: row.get(0)?,
                status: row.get(1)?,
            })
        },
    )
}

fn commander_ranks(
    conn: &Connection,
    oracle_id: &str,
) -> Result<Vec<CommanderRank>, duckdb::Error> {
    rows(
        conn,
        "SELECT period, min(rank), sum(num_decks), arg_min(trend_zscore, rank)
         FROM catalog.edhrec_commander_ranks
         WHERE oracle_id = ?
         GROUP BY period
         ORDER BY CASE period WHEN 'week' THEN 1 WHEN 'month' THEN 2 ELSE 3 END",
        oracle_id,
        |row| {
            Ok(CommanderRank {
                period: row.get(0)?,
                rank: row.get(1)?,
                num_decks: row.get(2)?,
                trend_zscore: row.get(3)?,
            })
        },
    )
}

fn tags(conn: &Connection, oracle_id: &str) -> Result<Vec<CardTag>, duckdb::Error> {
    rows(
        conn,
        "SELECT t.tag_id, t.slug, t.label, t.kind, 'scryfall'
         FROM catalog.card_tag_flat AS f
         JOIN catalog.tags AS t USING (tag_id)
         WHERE f.oracle_id = ?
         ORDER BY t.kind DESC, t.slug",
        oracle_id,
        |row| {
            Ok(CardTag {
                tag_id: row.get(0)?,
                slug: row.get(1)?,
                label: row.get(2)?,
                kind: row.get(3)?,
                source: row.get(4)?,
            })
        },
    )
}

fn rulings(conn: &Connection, oracle_id: &str) -> Result<Vec<Ruling>, duckdb::Error> {
    rows(
        conn,
        "SELECT published_at::VARCHAR, source, comment
         FROM catalog.rulings WHERE oracle_id = ? ORDER BY published_at, comment",
        oracle_id,
        |row| {
            Ok(Ruling {
                published_at: row.get(0)?,
                source: row.get(1)?,
                comment: row.get(2)?,
            })
        },
    )
}

fn prints(conn: &Connection, oracle_id: &str) -> Result<Vec<Print>, duckdb::Error> {
    rows(
        conn,
        "SELECT print_id, set_code, collector_number, released_at::VARCHAR, rarity, artist,
                image_small, price_eur::DOUBLE, price_usd::DOUBLE
         FROM catalog.prints
         WHERE oracle_id = ?
         ORDER BY released_at DESC, set_code, TRY_CAST(collector_number AS INTEGER), collector_number",
        oracle_id,
        |row| {
            Ok(Print {
                print_id: row.get(0)?,
                set_code: row.get(1)?,
                collector_number: row.get(2)?,
                released_at: row.get(3)?,
                rarity: row.get(4)?,
                artist: row.get(5)?,
                image_small: row.get(6)?,
                price_eur: row.get(7)?,
                price_usd: row.get(8)?,
            })
        },
    )
}
