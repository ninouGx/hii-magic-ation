# Progress

Milestones from the spec. A box is checked when its acceptance criteria pass.

## M0, skeleton

- [x] Cargo workspace, TOML config, embedded DuckDB, axum serving the front, `scripts/check.sh`

## M1, data

- [x] Catalog built from Scryfall bulk files (cards, prints, rulings, tags, legalities)
- [x] Scryfall bulk download, skipped when upstream is unchanged (second sync 2.4 s, no download)
- [x] EDHREC commander ranks, three windows, all pages (169), mapping rate 100 % on 2026-10-28
- [x] Catalog swap, CLI `sync`, sync API, counters in the UI
- [x] Full sync under 5 min on the laptop (4 min 12 s, EDHREC at 1 req/s is 3 min of it)
- [x] `verify commanders`: 3497 on both sides, 0 difference

## M2, search

- [ ] Filter JSON to SQL compiler with tests
- [ ] F3 filter builder, F4 multi-criteria sort, simple result grid
