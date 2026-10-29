//! Writes a selection in the text format that the Moxfield import reads.

use duckdb::Connection;
use serde::Serialize;

use crate::selection::{SelectionError, ensure_exists};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Export {
    pub text: String,
    /// Oracle ids a sync removed from the catalog, left out of `text`.
    pub skipped: Vec<String>,
}

/// One line per card, `1 Name (SET) number`, as Moxfield writes it.
#[must_use]
pub fn line(quantity: i32, name: &str, print: Option<(&str, &str)>) -> String {
    match print {
        Some((set_code, number)) => {
            format!("{quantity} {name} ({}) {number}", set_code.to_uppercase())
        }
        None => format!("{quantity} {name}"),
    }
}

/// Commanders come first with no header, as in Moxfield's own export, since its
/// import picks the commander in the deck form and reads no section marker.
/// Double-faced and adventure cards go by their front name, split cards by both halves.
const EXPORT_SQL: &str = "
WITH s AS (SELECT * FROM selections WHERE selection_id = $1),
entries AS (
    SELECT 0 AS rank, commander_oracle_id AS oracle_id, 1 AS quantity, commander_print_id AS print_id
    FROM s WHERE commander_oracle_id IS NOT NULL
    UNION ALL
    SELECT 1, partner_oracle_id, 1, partner_print_id
    FROM s WHERE partner_oracle_id IS NOT NULL
    UNION ALL
    SELECT 2, sc.oracle_id, sc.quantity, sc.print_id
    FROM selection_cards AS sc, s
    WHERE sc.selection_id = $1
      AND sc.oracle_id IS DISTINCT FROM s.commander_oracle_id
      AND sc.oracle_id IS DISTINCT FROM s.partner_oracle_id
)
SELECT e.oracle_id, e.quantity,
       CASE WHEN c.layout = 'split' THEN c.name ELSE c.front_name END,
       CASE WHEN p.print_id IS NULL THEN dp.set_code ELSE p.set_code END,
       CASE WHEN p.print_id IS NULL THEN dp.collector_number ELSE p.collector_number END
FROM entries AS e
LEFT JOIN catalog.cards AS c ON c.oracle_id = e.oracle_id
LEFT JOIN catalog.prints AS p ON p.print_id = e.print_id
LEFT JOIN catalog.prints AS dp ON dp.print_id = c.default_print_id
ORDER BY e.rank, c.name NULLS LAST, e.oracle_id";

pub fn moxfield(conn: &Connection, selection_id: &str) -> Result<Export, SelectionError> {
    ensure_exists(conn, selection_id)?;
    let mut stmt = conn.prepare(EXPORT_SQL)?;
    let rows = stmt.query_map([selection_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut lines = Vec::new();
    let mut skipped = Vec::new();
    for row in rows {
        let (oracle_id, quantity, name, set_code, number) = row?;
        match name {
            Some(name) => {
                let print = set_code.as_deref().zip(number.as_deref());
                lines.push(line(quantity, &name, print));
            }
            None => skipped.push(oracle_id),
        }
    }
    Ok(Export {
        text: lines.join("\n"),
        skipped,
    })
}
