# Progress

Milestones from the spec. A box is checked when its acceptance criteria pass.

## M0, skeleton

- [x] Cargo workspace, TOML config, embedded DuckDB, axum serving the front, `scripts/check.sh`

## M1, data

- [ ] Catalog built from Scryfall bulk files (cards, prints, rulings, tags, legalities)
- [ ] Scryfall bulk download, skipped when upstream is unchanged
- [ ] EDHREC commander ranks, three windows, mapping rate measured
- [ ] Catalog swap, CLI `sync`, sync API, counters in the UI
- [ ] Full sync under 5 min on the laptop
