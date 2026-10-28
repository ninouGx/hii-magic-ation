//! Filter trees in the react-querybuilder JSON shape, compiled to a
//! parameterized SQL condition. User values only ever travel as parameters.

use duckdb::Connection;
use duckdb::types::Value;
use serde::{Deserialize, Deserializer, Serialize};

/// Source of every column a compiled condition references.
pub const CARDS_FROM: &str =
    "catalog.cards AS c LEFT JOIN catalog.prints AS dp ON dp.print_id = c.default_print_id";

#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("invalid regex `{pattern}`: {message}")]
    InvalidRegex { pattern: String, message: String },
    #[error(transparent)]
    Db(#[from] duckdb::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub combinator: Combinator,
    #[serde(default)]
    pub not: bool,
    pub rules: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Combinator {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Node {
    Group(Group),
    Rule(Rule),
}

/// A node with `rules` is a group. Deciding up front keeps the rule's own
/// error message, which an untagged enum would replace with a generic one.
impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("rules").is_some() {
            Group::deserialize(value).map(Node::Group)
        } else {
            Rule::deserialize(value).map(Node::Rule)
        }
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum Rule {
    Legality(LegalityCondition),
    Name(TextCondition),
    OracleText(TextCondition),
    TypeLine(TextCondition),
    FlavorText(TextCondition),
    Rulings(TextCondition),
    Colors(ColorCondition),
    ColorIdentity(ColorCondition),
    Cmc(NumberCondition),
    Power(NumberCondition),
    Toughness(NumberCondition),
    Loyalty(NumberCondition),
    Keywords(KeywordCondition),
    Tag(TagCondition),
    CommanderEligible(BoolCondition),
    EdhrecRank(NumberCondition),
    PriceEur(NumberCondition),
    Artist(PickCondition<String>),
    Set(PickCondition<String>),
    Year(PickCondition<i32>),
    CommanderRankWeek(NumberCondition),
    CommanderRankMonth(NumberCondition),
    CommanderRankYear(NumberCondition),
    NumDecksWeek(NumberCondition),
    NumDecksMonth(NumberCondition),
    NumDecksYear(NumberCondition),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value")]
pub enum LegalityCondition {
    #[serde(rename = "=")]
    Is(Legality),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Legality {
    pub format: String,
    pub status: LegalityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalityStatus {
    Legal,
    NotLegal,
    Banned,
    Restricted,
}

impl LegalityStatus {
    fn code(self) -> &'static str {
        match self {
            Self::Legal => "legal",
            Self::NotLegal => "not_legal",
            Self::Banned => "banned",
            Self::Restricted => "restricted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value", rename_all = "snake_case")]
pub enum TextCondition {
    Contains(String),
    NotContains(String),
    /// RE2 syntax, case-insensitive, `^` and `$` match at each line and face.
    Regex(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value", rename_all = "snake_case")]
pub enum ColorCondition {
    Exactly(Vec<Color>),
    Includes(Vec<Color>),
    AtMost(Vec<Color>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    W,
    U,
    B,
    R,
    G,
}

impl Color {
    fn code(self) -> &'static str {
        match self {
            Self::W => "W",
            Self::U => "U",
            Self::B => "B",
            Self::R => "R",
            Self::G => "G",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value")]
pub enum NumberCondition {
    #[serde(rename = "=")]
    Eq(f64),
    #[serde(rename = "<")]
    Lt(f64),
    #[serde(rename = "<=")]
    Le(f64),
    #[serde(rename = ">")]
    Gt(f64),
    #[serde(rename = ">=")]
    Ge(f64),
    #[serde(rename = "between")]
    Between(f64, f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value", rename_all = "snake_case")]
pub enum KeywordCondition {
    ContainsAny(Vec<String>),
    ContainsAll(Vec<String>),
}

/// Matches when at least one English print has one of the values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value")]
pub enum PickCondition<T> {
    #[serde(rename = "=")]
    Eq(T),
    #[serde(rename = "in")]
    In(Vec<T>),
}

impl<T: Clone> PickCondition<T> {
    fn values(&self) -> Vec<T> {
        match self {
            Self::Eq(value) => vec![value.clone()],
            Self::In(values) => values.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value", rename_all = "snake_case")]
pub enum TagCondition {
    Has(TagRef),
    HasNot(TagRef),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagRef {
    pub tag_id: String,
    #[serde(default)]
    pub include_children: bool,
}

/// EDHREC popularity window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Week,
    Month,
    Year,
}

impl Period {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", content = "value")]
pub enum BoolCondition {
    #[serde(rename = "=")]
    Is(bool),
}

/// A SQL condition over [`CARDS_FROM`] and the values for its placeholders, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    pub sql: String,
    pub params: Vec<Value>,
}

#[must_use]
pub fn compile(filter: &Group) -> Compiled {
    let mut params = Vec::new();
    let sql = group_sql(filter, &mut params);
    Compiled { sql, params }
}

/// Fails on the first pattern that RE2 rejects, with the RE2 message.
pub fn check_regexes(conn: &Connection, filter: &Group) -> Result<(), FilterError> {
    let mut stmt = conn.prepare("SELECT regexp_matches('', ?)")?;
    for pattern in regexes(filter) {
        if let Err(error) = stmt.query_row([pattern], |row| row.get::<_, bool>(0)) {
            return Err(match error {
                duckdb::Error::DuckDBFailure(_, Some(message)) => FilterError::InvalidRegex {
                    pattern: pattern.to_owned(),
                    message: message
                        .trim_start_matches("Invalid Input Error: ")
                        .to_owned(),
                },
                other => other.into(),
            });
        }
    }
    Ok(())
}

fn regexes(group: &Group) -> Vec<&str> {
    group
        .rules
        .iter()
        .flat_map(|node| match node {
            Node::Group(group) => regexes(group),
            Node::Rule(rule) => rule.regex().into_iter().collect(),
        })
        .collect()
}

impl Rule {
    fn regex(&self) -> Option<&str> {
        match self {
            Self::Name(TextCondition::Regex(pattern))
            | Self::OracleText(TextCondition::Regex(pattern))
            | Self::TypeLine(TextCondition::Regex(pattern))
            | Self::FlavorText(TextCondition::Regex(pattern))
            | Self::Rulings(TextCondition::Regex(pattern)) => Some(pattern),
            _ => None,
        }
    }
}

fn group_sql(group: &Group, params: &mut Vec<Value>) -> String {
    let parts: Vec<String> = group
        .rules
        .iter()
        .map(|node| match node {
            Node::Group(group) => group_sql(group, params),
            Node::Rule(rule) => format!("coalesce({}, false)", rule_sql(rule, params)),
        })
        .collect();
    let separator = match group.combinator {
        Combinator::And => " AND ",
        Combinator::Or => " OR ",
    };
    let body = if parts.is_empty() {
        "true".to_owned()
    } else {
        format!("({})", parts.join(separator))
    };
    if group.not {
        format!("NOT {body}")
    } else {
        body
    }
}

/// Multi-face names and type lines join faces with ` // `. Regexes see one
/// line per face, so `^` anchors on every face.
const NAME_LINES: &str = "replace(c.name, ' // ', chr(10))";
const TYPE_LINES: &str = "replace(c.type_line, ' // ', chr(10))";

fn rule_sql(rule: &Rule, params: &mut Vec<Value>) -> String {
    match rule {
        Rule::Legality(LegalityCondition::Is(legality)) => {
            params.push(Value::Text(legality.format.clone()));
            params.push(Value::Text(legality.status.code().to_owned()));
            "c.legalities[?] = ?".to_owned()
        }
        Rule::Name(condition) => text_sql("c.name", NAME_LINES, condition, params),
        Rule::OracleText(condition) => {
            text_sql("c.oracle_text", "c.oracle_text", condition, params)
        }
        Rule::TypeLine(condition) => text_sql("c.type_line", TYPE_LINES, condition, params),
        Rule::FlavorText(condition) => {
            any_row_text_sql("catalog.prints", "flavor_text", condition, params)
        }
        Rule::Rulings(condition) => {
            any_row_text_sql("catalog.rulings", "comment", condition, params)
        }
        Rule::Colors(condition) => color_sql("c.colors", condition, params),
        Rule::ColorIdentity(condition) => color_sql("c.color_identity", condition, params),
        Rule::Cmc(condition) => number_sql("c.cmc", condition, params),
        Rule::Power(condition) => number_sql("c.power_num", condition, params),
        Rule::Toughness(condition) => number_sql("c.toughness_num", condition, params),
        Rule::Loyalty(condition) => number_sql("c.loyalty_num", condition, params),
        Rule::Keywords(condition) => keyword_sql(condition, params),
        Rule::CommanderEligible(BoolCondition::Is(value)) => {
            params.push(Value::Boolean(*value));
            "c.is_commander_eligible = ?".to_owned()
        }
        Rule::Tag(condition) => tag_sql(condition, params),
        Rule::EdhrecRank(condition) => number_sql("c.edhrec_rank", condition, params),
        Rule::PriceEur(condition) => number_sql("dp.price_eur", condition, params),
        Rule::Artist(condition) => {
            let list = text_list(
                condition
                    .values()
                    .iter()
                    .map(|artist| artist.to_lowercase()),
                params,
            );
            any_print_sql(&format!("list_contains({list}, lower(p.artist))"))
        }
        Rule::Set(condition) => {
            let list = text_list(
                condition.values().iter().map(|set| set.to_lowercase()),
                params,
            );
            any_print_sql(&format!("list_contains({list}, lower(p.set_code))"))
        }
        Rule::Year(condition) => {
            let placeholders: Vec<&str> = condition
                .values()
                .into_iter()
                .map(|year| {
                    params.push(Value::Int(year));
                    "?"
                })
                .collect();
            any_print_sql(&format!(
                "list_contains([{}]::INTEGER[], year(p.released_at))",
                placeholders.join(", ")
            ))
        }
        Rule::CommanderRankWeek(condition) => {
            edhrec_sql(EdhrecStat::Rank, Period::Week, condition, params)
        }
        Rule::CommanderRankMonth(condition) => {
            edhrec_sql(EdhrecStat::Rank, Period::Month, condition, params)
        }
        Rule::CommanderRankYear(condition) => {
            edhrec_sql(EdhrecStat::Rank, Period::Year, condition, params)
        }
        Rule::NumDecksWeek(condition) => {
            edhrec_sql(EdhrecStat::NumDecks, Period::Week, condition, params)
        }
        Rule::NumDecksMonth(condition) => {
            edhrec_sql(EdhrecStat::NumDecks, Period::Month, condition, params)
        }
        Rule::NumDecksYear(condition) => {
            edhrec_sql(EdhrecStat::NumDecks, Period::Year, condition, params)
        }
    }
}

fn text_sql(
    column: &str,
    lines: &str,
    condition: &TextCondition,
    params: &mut Vec<Value>,
) -> String {
    let (negation, matches) = text_match(column, lines, condition, params);
    format!("{negation}{matches}")
}

/// "Does not contain" holds when no row contains the text.
fn any_row_text_sql(
    table: &str,
    column: &str,
    condition: &TextCondition,
    params: &mut Vec<Value>,
) -> String {
    let column = format!("x.{column}");
    let (negation, matches) = text_match(&column, &column, condition, params);
    format!(
        "{negation}EXISTS (SELECT 1 FROM {table} AS x WHERE x.oracle_id = c.oracle_id AND {matches})"
    )
}

fn any_print_sql(matches: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM catalog.prints AS p WHERE p.oracle_id = c.oracle_id AND {matches})"
    )
}

/// The positive match, and the negation to put in front of it.
fn text_match(
    column: &str,
    lines: &str,
    condition: &TextCondition,
    params: &mut Vec<Value>,
) -> (&'static str, String) {
    match condition {
        TextCondition::Contains(text) | TextCondition::NotContains(text) => {
            params.push(Value::Text(text.clone()));
            let negation = if matches!(condition, TextCondition::NotContains(_)) {
                "NOT "
            } else {
                ""
            };
            (negation, format!("contains(lower({column}), lower(?))"))
        }
        TextCondition::Regex(pattern) => {
            params.push(Value::Text(pattern.clone()));
            ("", format!("regexp_matches({lines}, '(?m)' || ?, 'i')"))
        }
    }
}

fn color_sql(column: &str, condition: &ColorCondition, params: &mut Vec<Value>) -> String {
    let (template, colors) = match condition {
        ColorCondition::Exactly(colors) => (
            "list_sort(list_distinct({column})) = list_sort(list_distinct({list}))",
            colors,
        ),
        ColorCondition::Includes(colors) => ("list_has_all({column}, {list})", colors),
        ColorCondition::AtMost(colors) => ("list_has_all({list}, {column})", colors),
    };
    let list = text_list(colors.iter().map(|color| color.code().to_owned()), params);
    template
        .replace("{column}", column)
        .replace("{list}", &list)
}

fn keyword_sql(condition: &KeywordCondition, params: &mut Vec<Value>) -> String {
    let (function, keywords) = match condition {
        KeywordCondition::ContainsAny(keywords) => ("list_has_any", keywords),
        KeywordCondition::ContainsAll(keywords) => ("list_has_all", keywords),
    };
    let list = text_list(
        keywords.iter().map(|keyword| keyword.to_lowercase()),
        params,
    );
    format!("{function}(list_transform(c.keywords, lambda k: lower(k)), {list})")
}

/// One placeholder per element: the count is structure, the elements are values.
fn text_list(values: impl Iterator<Item = String>, params: &mut Vec<Value>) -> String {
    let placeholders: Vec<&str> = values
        .map(|value| {
            params.push(Value::Text(value));
            "?"
        })
        .collect();
    format!("[{}]::VARCHAR[]", placeholders.join(", "))
}

fn number_sql(column: &str, condition: &NumberCondition, params: &mut Vec<Value>) -> String {
    let (operator, value) = match condition {
        NumberCondition::Eq(value) => ("=", value),
        NumberCondition::Lt(value) => ("<", value),
        NumberCondition::Le(value) => ("<=", value),
        NumberCondition::Gt(value) => (">", value),
        NumberCondition::Ge(value) => (">=", value),
        NumberCondition::Between(first, second) => {
            params.push(Value::Double(first.min(*second)));
            params.push(Value::Double(first.max(*second)));
            return format!("{column} BETWEEN ? AND ?");
        }
    };
    params.push(Value::Double(*value));
    format!("{column} {operator} ?")
}

fn tag_sql(condition: &TagCondition, params: &mut Vec<Value>) -> String {
    let (negation, tag) = match condition {
        TagCondition::Has(tag) => ("", tag),
        TagCondition::HasNot(tag) => ("NOT ", tag),
    };
    params.push(Value::Text(tag.tag_id.clone()));
    let tag_match = if tag.include_children {
        "t.tag_id IN (SELECT a.tag_id FROM catalog.tag_ancestors AS a WHERE a.ancestor_id = ?)"
    } else {
        "t.tag_id = ?"
    };
    format!(
        "{negation}EXISTS (SELECT 1 FROM catalog.card_tag_flat AS t WHERE t.oracle_id = c.oracle_id AND {tag_match})"
    )
}

/// A commander ranks alone and in partner pairs. Some rank only in pairs, so
/// the card takes its best rank and the sum of decks over all its entries.
#[derive(Debug, Clone, Copy)]
pub enum EdhrecStat {
    Rank,
    NumDecks,
    TrendZscore,
}

/// SQL for the stat, with one placeholder for the period.
#[must_use]
pub fn edhrec_stat_sql(stat: EdhrecStat) -> String {
    let aggregate = match stat {
        EdhrecStat::Rank => "min(r.rank)",
        EdhrecStat::NumDecks => "sum(r.num_decks)",
        EdhrecStat::TrendZscore => "arg_min(r.trend_zscore, r.rank)",
    };
    format!(
        "(SELECT {aggregate} FROM catalog.edhrec_commander_ranks AS r WHERE r.oracle_id = c.oracle_id AND r.period = ?)"
    )
}

fn edhrec_sql(
    stat: EdhrecStat,
    period: Period,
    condition: &NumberCondition,
    params: &mut Vec<Value>,
) -> String {
    params.push(Value::Text(period.code().to_owned()));
    number_sql(&edhrec_stat_sql(stat), condition, params)
}
