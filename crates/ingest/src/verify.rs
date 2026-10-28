use std::collections::BTreeSet;

use hma_core::duckdb::{self, Connection};
use serde::Serialize;

/// The Scryfall search that defines a Commander-legal commander.
pub const SCRYFALL_COMMANDER_QUERY: &str = "is:commander legal:commander";

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CommanderDiff {
    pub catalog: usize,
    pub scryfall: usize,
    /// Eligible in the catalog, not on Scryfall.
    pub extra: Vec<String>,
    /// On Scryfall, not eligible in the catalog.
    pub missing: Vec<String>,
}

impl CommanderDiff {
    #[must_use]
    pub fn is_exact(&self) -> bool {
        self.extra.is_empty() && self.missing.is_empty()
    }
}

/// Compares `is_commander_eligible` with the oracle ids Scryfall returns for
/// [`SCRYFALL_COMMANDER_QUERY`]. Names come from the catalog when it knows the card.
pub fn diff_commanders(
    conn: &Connection,
    schema: &str,
    scryfall: &BTreeSet<String>,
) -> Result<CommanderDiff, duckdb::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT oracle_id, name, is_commander_eligible FROM {schema}.cards"
    ))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let eligible: BTreeSet<&str> = rows
        .iter()
        .filter(|(_, _, eligible)| *eligible)
        .map(|(id, _, _)| id.as_str())
        .collect();
    let name_of = |id: &str| {
        rows.iter()
            .find(|(oracle_id, _, _)| oracle_id == id)
            .map_or_else(|| format!("unknown card {id}"), |(_, name, _)| name.clone())
    };

    Ok(CommanderDiff {
        catalog: eligible.len(),
        scryfall: scryfall.len(),
        extra: eligible
            .iter()
            .filter(|id| !scryfall.contains(**id))
            .map(|id| name_of(id))
            .collect(),
        missing: scryfall
            .iter()
            .filter(|id| !eligible.contains(id.as_str()))
            .map(|id| name_of(id))
            .collect(),
    })
}
