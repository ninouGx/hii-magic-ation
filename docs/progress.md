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

- [x] Filter JSON to SQL compiler with tests (reference query 238 cards, 1 explained difference with Scryfall, count in 45 ms)
- [x] F3 filter builder, F4 multi-criteria sort, simple result grid
  - Search API answers in 75 to 100 ms on the full catalog, reference query included
  - Week commander rank then mana value on a filtered result, sort kept in the URL
  - Invalid regex: HTTP 400 with the RE2 message before the search runs, shown at once
  - Infinite scroll loads all 238 cards of the reference query, 100 per page
  - Left for later milestones: deck mode (M3, needs selections), custom and AI tags (M4, M5), similarity, synergy and graph rules (M5 to M7)

## M3, themed deck

- [x] F5 grid: progressive image quality (`thumb`, then `grid` or `display` by `srcset`), double-faced badge that flips the card
- [x] F5 detail window: centered over the grid, arrows move through the result, Escape or a click outside closes it, card in the URL
- [x] F5 print change in the detail window updates the image and the art tags of that print (decision 35)
- [ ] F5 print choice saved in the selection, commanders included
- [ ] F5 category view
- [x] F9 selections, back end: user migrations, `hma_core::selection`, `/api/selections` (17 core tests, Gemini review applied)
- [x] F9 selections, front: Selections screen (list, create, rename, delete, commander picker through the search API with `commander_eligible`, cards by type, count out of 100, alerts, quantity and removal)
  - Selection and open card in the URL (`/selections?id=&card=`), a reload restores both
  - Changes to one selection run one at a time, so an older answer never overwrites a newer one
- [x] F9 selections, Explorer: active selection in the URL (`selection=`), add and remove from a tile and from the detail window, "in selection" badge on tiles, add all results of the filter
  - Add on a card already present adds a copy, remove drops every copy. A commander shows a badge and no add button
  - Add all asks for a second click and reports how many cards were new
  - A deleted selection left in the URL is dropped without an error
  - Print choice when adding stays with the F5 print item below
- [x] Selection alerts grouped by kind, serious ones first and open, a short label on each card. Off-color and a commander in the list stay neutral, since a theme deck can choose them
- [x] F3 deck mode: with a selection commander, an identity rule is added and can be turned off (decision 34)
- [ ] F10 Moxfield export, first deck imported

## M4, enrichment

- [ ] F5 table view: thumbnail on the left, one column per field, a header click sets the main sort
  - Build it as one component that the Selections screen reuses for its card list, asked by Nils on 2026-10-29
- [ ] F6, F7, Explorer screen finished
