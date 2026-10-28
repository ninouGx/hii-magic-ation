//! One page of filtered, sorted cards, with the total count of the filter.

use duckdb::types::Value;
use duckdb::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::filter::{
    CARDS_FROM, EdhrecStat, FilterError, Group, Period, check_regexes, compile, edhrec_stat_sql,
};

pub const MAX_LIMIT: u32 = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub filter: Group,
    #[serde(default)]
    pub sort: Vec<SortKey>,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    100
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    pub field: SortField,
    #[serde(default)]
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    EdhrecRank,
    CommanderRankWeek,
    CommanderRankMonth,
    CommanderRankYear,
    NumDecksWeek,
    NumDecksMonth,
    NumDecksYear,
    TrendWeek,
    TrendMonth,
    TrendYear,
    Cmc,
    PriceEur,
    Name,
    ReleasedAt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchPage {
    pub total: i64,
    pub cards: Vec<CardSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CardSummary {
    pub oracle_id: String,
    pub name: String,
    pub mana_cost: Option<String>,
    pub type_line: Option<String>,
    pub image_small: Option<String>,
    pub price_eur: Option<f64>,
}

/// Rejects an invalid regex before any search query runs.
pub fn search(conn: &Connection, request: &SearchRequest) -> Result<SearchPage, FilterError> {
    check_regexes(conn, &request.filter)?;
    let filter = compile(&request.filter);

    let total = conn.query_row(
        &format!("SELECT count(*) FROM {CARDS_FROM} WHERE {}", filter.sql),
        params_from_iter(&filter.params),
        |row| row.get(0),
    )?;

    let mut params = filter.params;
    let order = order_sql(&request.sort, &mut params);
    params.push(Value::UInt(request.limit.min(MAX_LIMIT)));
    params.push(Value::UInt(request.offset));
    let mut stmt = conn.prepare(&format!(
        "SELECT c.oracle_id, c.name, c.mana_cost, c.type_line, dp.image_small, dp.price_eur::DOUBLE
         FROM {CARDS_FROM}
         WHERE {}
         ORDER BY {order}
         LIMIT ? OFFSET ?",
        filter.sql
    ))?;
    let cards = stmt
        .query_map(params_from_iter(&params), |row| {
            Ok(CardSummary {
                oracle_id: row.get(0)?,
                name: row.get(1)?,
                mana_cost: row.get(2)?,
                type_line: row.get(3)?,
                image_small: row.get(4)?,
                price_eur: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(SearchPage { total, cards })
}

/// Cards without a value for a key go last in both directions. Name and
/// `oracle_id` close the order, so pages never overlap.
fn order_sql(keys: &[SortKey], params: &mut Vec<Value>) -> String {
    keys.iter()
        .map(|key| {
            let direction = match key.direction {
                Direction::Asc => "ASC",
                Direction::Desc => "DESC",
            };
            format!("{} {direction} NULLS LAST", sort_sql(key.field, params))
        })
        .chain(["c.name ASC".to_owned(), "c.oracle_id ASC".to_owned()])
        .collect::<Vec<_>>()
        .join(", ")
}

fn sort_sql(field: SortField, params: &mut Vec<Value>) -> String {
    let edhrec = |stat, period: Period, params: &mut Vec<Value>| {
        params.push(Value::Text(period.code().to_owned()));
        edhrec_stat_sql(stat)
    };
    match field {
        SortField::EdhrecRank => "c.edhrec_rank".to_owned(),
        SortField::CommanderRankWeek => edhrec(EdhrecStat::Rank, Period::Week, params),
        SortField::CommanderRankMonth => edhrec(EdhrecStat::Rank, Period::Month, params),
        SortField::CommanderRankYear => edhrec(EdhrecStat::Rank, Period::Year, params),
        SortField::NumDecksWeek => edhrec(EdhrecStat::NumDecks, Period::Week, params),
        SortField::NumDecksMonth => edhrec(EdhrecStat::NumDecks, Period::Month, params),
        SortField::NumDecksYear => edhrec(EdhrecStat::NumDecks, Period::Year, params),
        SortField::TrendWeek => edhrec(EdhrecStat::TrendZscore, Period::Week, params),
        SortField::TrendMonth => edhrec(EdhrecStat::TrendZscore, Period::Month, params),
        SortField::TrendYear => edhrec(EdhrecStat::TrendZscore, Period::Year, params),
        SortField::Cmc => "c.cmc".to_owned(),
        SortField::PriceEur => "dp.price_eur".to_owned(),
        SortField::Name => "c.name".to_owned(),
        SortField::ReleasedAt => {
            "(SELECT min(p.released_at) FROM catalog.prints AS p WHERE p.oracle_id = c.oracle_id)"
                .to_owned()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TagSuggestion {
    pub tag_id: String,
    pub slug: String,
    pub label: String,
    pub kind: String,
    pub cards: i64,
}

/// Tags whose slug, label or alias contains the text. Prefix matches come
/// first, then the most used tags.
pub fn suggest_tags(
    conn: &Connection,
    text: &str,
    limit: u32,
) -> Result<Vec<TagSuggestion>, duckdb::Error> {
    let mut stmt = conn.prepare(
        "WITH matched AS (
             SELECT tag_id, slug, label, kind,
                    starts_with(lower(slug), lower($1)) OR starts_with(lower(label), lower($1)) AS prefix
             FROM catalog.tags
             WHERE contains(lower(slug), lower($1))
                OR contains(lower(label), lower($1))
                OR contains(lower(array_to_string(aliases, chr(10))), lower($1))
         )
         SELECT m.tag_id, m.slug, m.label, m.kind, count(f.oracle_id) AS cards
         FROM matched AS m
         LEFT JOIN catalog.card_tag_flat AS f ON f.tag_id = m.tag_id
         GROUP BY m.tag_id, m.slug, m.label, m.kind, m.prefix
         ORDER BY m.prefix DESC, cards DESC, m.slug
         LIMIT $2",
    )?;
    stmt.query_map(duckdb::params![text, limit.min(MAX_LIMIT)], |row| {
        Ok(TagSuggestion {
            tag_id: row.get(0)?,
            slug: row.get(1)?,
            label: row.get(2)?,
            kind: row.get(3)?,
            cards: row.get(4)?,
        })
    })?
    .collect()
}

/// Keywords that contain the text, the most frequent first.
pub fn suggest_keywords(
    conn: &Connection,
    text: &str,
    limit: u32,
) -> Result<Vec<String>, duckdb::Error> {
    let mut stmt = conn.prepare(
        "SELECT keyword
         FROM (SELECT unnest(keywords) AS keyword FROM catalog.cards)
         WHERE contains(lower(keyword), lower($1))
         GROUP BY keyword
         ORDER BY starts_with(lower(keyword), lower($1)) DESC, count(*) DESC, keyword
         LIMIT $2",
    )?;
    stmt.query_map(duckdb::params![text, limit.min(MAX_LIMIT)], |row| {
        row.get(0)
    })?
    .collect()
}

/// Every format in the Scryfall legalities, in alphabetical order.
pub fn formats(conn: &Connection) -> Result<Vec<String>, duckdb::Error> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT unnest(map_keys(legalities)) AS format FROM catalog.cards ORDER BY format",
    )?;
    stmt.query_map([], |row| row.get(0))?.collect()
}
