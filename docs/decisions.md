# Decisions

Each entry departs from or refines the spec. Evidence was measured on the live sources on 2026-10-28 unless stated otherwise.

## Data sources

1. **Scryfall bulk files are `.jsonl.gz`.** The bulk-data endpoint only exposes `jsonl_download_uri`. Download each file to the data directory, then load it with DuckDB `read_json(..., format = 'newline_delimited')`, which reads gzip natively. No custom streaming parser. Sizes compressed: oracle_cards 25 MB, default_cards 80 MB, rulings 5 MB, oracle_tags 6 MB, art_tags 13 MB.
2. **EDHREC `id` is a Scryfall print id.** 100 of 100 entries on the week page 1 match a Scryfall card id, 0 match an oracle_id. Map `id -> prints.print_id -> oracle_id` through default_cards. Name matching is the fallback for partner pairs only (they carry `is_partner`, `names[]` and one `id`).
3. **EDHREC pages have two shapes.** Page 1 nests the list under `container.json_dict.cardlists[0]`. Pages 2+ put `cardviews` at the top level. The parser accepts both. Volume: week 45 pages, year 69 pages, so a full sync at 1 req/s takes about 3 min.
4. **Partner pairs need their own key.** A commander can rank solo and in several pairs. Key `edhrec_commander_ranks` on `(oracle_id, partner_oracle_id, window)`, with an empty partner for solo entries.

## Data model

5. **Tag weight is not a filter or sort criterion.** 99.7 % of oracle taggings and 97 % of art taggings are `median`. Keep the column, drop it from F3 and F4.
6. **Multi-face text is materialized at ingest.** 3327 of 39099 oracle cards have a null top-level `oracle_text`. `cards.oracle_text` joins the text of every face, `cards.name` already holds `Front // Back`, and `front_name` keeps the first face for the Moxfield export. Filters never parse JSON at query time.
7. **Power, toughness and loyalty keep two columns.** 547 values are not numeric (`*`, `1+*`, `?`). Store the display string and a nullable number from `TRY_CAST`.
8. **Tag hierarchy is flattened at sync.** A `tag_ancestors` closure table makes "include child tags" a join. A `card_tag_flat (oracle_id, tag_id)` table resolves art tags from illustration to card. Both are rebuilt with the catalog.
9. **Scryfall and user tags live in separate files.** `catalog.tags` and `catalog.card_tags` hold Scryfall data. `user.tags` and `user.card_tags` hold custom and AI tags. A view unions them for filters. A sync never writes to `user.duckdb`.
10. **`prints` stores `image_small`**, used by the grid (F5).

17. **Non-game objects stay out of `cards`.** `oracle_cards` holds about 4200 art series, tokens, emblems, planes, schemes and vanguards. Some share a name with a real card (Tarmogoyf token), which would break name mapping. The build drops these layouts.
18. **Commander eligibility matches Scryfall exactly.** Rule: commander-legal, not a meld result, and one of legendary creature, Background, legendary Vehicle or Spacecraft with power, "can be your commander", or a Grist-style "isn't on the battlefield" creature. Checked on 2026-10-28 against `is:commander legal:commander`, 3497 oracle_ids on both sides, 0 difference. Re-run the check when a set adds a new commander type.
19. **Prints are English only, as the spec says.** Cost measured: 17841 art taggings (2593 illustrations, 2.4 %) point to non-English prints only and are lost. 8 cards have no English print and no default print.
20. **Default print needs a numeric collector number.** Without it, The List reprints (`plst`, `CLB-187`) won the "recent classic" rule.
21. **The catalog build takes 8 s on the full data** (35k cards, 110k prints, 740k taggings), measured with DuckDB 2.0.0.

## Search

24. **Regexes anchor on each face and each line.** Names and type lines split on ` // `, and `(?m)` makes `^` and `$` match per line. The spec asks for search on every face. Scryfall differs: the reference query `(name:/^dragon/ or t:dragon) id<=brg -otag:removal legal:commander` returns 237 cards, the compiler 238. The extra card is The Dragon-Kami Reborn, whose back face is Dragon-Kami's Egg. Scryfall runs the regex on the full `Front // Back` name. Checked on 2026-10-28.
25. **"Include child tags" is on by default.** Scryfall `otag:` includes child tags. With children, the reference query matches Scryfall on every other card. Without them, it returns 83 cards more.
26. **A commander's EDHREC stats merge its solo and pair entries.** On the week window, 131 commanders rank only in partner pairs (Kraum, Ludevic's Opus at 107). The card takes its best rank, the sum of its decks, and the trend of its best entry. A solo-only rule would send these commanders to the end of a sort.
27. **The server checks regexes, not the front.** JavaScript `RegExp` accepts lookahead, RE2 does not. Before a search runs, the server tests each pattern with DuckDB and returns HTTP 400 with the RE2 message. The search query itself does not run.
28. **Counts stay under 100 ms on the full catalog.** The reference query counts in 45 ms after warm-up. A five-way OR over a text regex, flavor text, rulings, deck count and artist counts in 85 ms. No index or precomputation.

29. **The card detail is a centered window, not a right panel.** The spec placed a shared detail panel on the right. Chosen on 2026-10-29, after the EDHREC model: a window over the current screen, arrows to move through the result, Escape or a click outside to close. The window stays the same on every screen.
30. **The grid uses the Scryfall WebP sizes.** `small` (146×204) looks pixelated once a column grows and on a high-density screen. Measured on 2026-10-29: `thumb` 6.5 KB, `grid` 30 KB (488×680), `display` 40 KB (672×936), against 62 KB for the `normal` JPG. All sizes share one URL except the size folder, and a back face swaps `/front/` for `/back/`.
31. **A double-faced card always shows its front in the grid.** A badge flips it. Showing the face that matched the filter would need a per-face evaluation on the server, and a flip by hand is enough.

32. **A selection card has a quantity.** The spec lists no quantity column. Without it, basic lands cannot reach their count and the singleton alert has nothing to detect. Adding a card again adds a copy, and the alert flags it. Basic lands and cards that allow any number of copies (Relentless Rats, Seven Dwarves) escape the alert.
33. **User data has its own migrations.** `user.duckdb` cannot be rebuilt, so each schema change is a numbered SQL file applied once by `Db::open`, tracked in `user_schema_version`.
34. **Deck mode is a checkbox beside the selection, not a rule in the builder.** The spec adds an identity rule that can be turned off. A rule inside the builder would go into the filter URL and stay after the selection changes. The Explorer sends `AND(filter, color identity at most the commanders)` instead, with the union of the commander and partner identities, and keeps `deck=off` in the URL when it is off. A colorless commander keeps only colorless cards. The grid waits for the commander cards, so no search runs without the rule.
35. **Art tags are listed per print, without a precomputed table.** The spec asks that a print change updates the art tags shown, and art tags sit on illustrations. The card detail now joins `print_illustrations` to `card_tags` at request time. Measured on 2026-10-29 on the full catalog: Forest (853 prints) 64 ms before, 112 ms after, Sol Ring (138 prints) 80 ms. The extra time is the scan of `card_tags` (740k rows). A table keyed by oracle_id at sync comes only if the detail window feels slow.
36. **Commanders keep their print in `selections`.** Commanders live in `selections`, not in `selection_cards`, so migration 002 adds `commander_print_id` and `partner_print_id`. A null print falls back to the default print, in the app and in the export. A print of another card, or a print without its commander, is refused with HTTP 400. A commander change clears its print.
37. **The Moxfield export leaves the commanders out of the list.** Chosen on 2026-10-29: in Moxfield the user picks the commander in the deck form first, then pastes the list, so a commander line in the list would add it twice. The app shows the commander lines apart, with their print to set in Moxfield. The spec asks for a dedicated commander section or a simple export. Evidence on 2026-10-29: the Moxfield help page "Creating Decks" (updated 2026-06-16) gives `AMOUNT CARDNAME (SETCODE) NUMBER *F*` as the only format and has the commander picked in the Add Deck form. Moxfield's own "Copy for Moxfield" writes the commander on the first line with no marker. The prism project tested a `Commander` header in Moxfield Bulk Edit, and the commander did not land in its zone (github.com/codwats/prism/issues/381). The list is sorted by name. Double-faced, adventure and flip cards use the front name, split cards (aftermath included, their Scryfall layout is `split`) keep `A // B`. A card without a print keeps its name alone. A card a sync removed is left out and reported.

## Sync and concurrency

11. **A sync builds a new catalog file and swaps it.** DuckDB allows one writing process per file. The sync writes `catalog.duckdb.new`, validates row counts, then the server detaches the old catalog, renames the file and attaches it again. This replaces the staging-tables design. The CLI `sync` command never opens `user.duckdb`, so it runs while the server is up, and asks the server to reload when it is reachable.

22. **No `sync_state` table.** Each raw source keeps a `manifest.json` (Scryfall bulk `updated_at`, EDHREC fetch time per window) beside its files, and `/api/catalog` exposes the bulk versions. The catalog stays a pure function of the raw directory, so it needs no state of its own.
23. **Validation covers keys and null rates, not every column.** A build fails on missing keys or legalities, duplicate oracle_ids, empty tables, more than 1 % of cards without `type_line` or `cmc`, or more than 5 % of prints without `image_small`. Other renamed fields would load as nulls and show in the counters.

## Performance

12. **No precomputation before a measurement.** The catalog holds about 39k cards and 740k taggings. F6 evaluates the filter once into a temporary table of oracle_ids, then runs the aggregations on it. Precomputed reference distributions for the lift come only if the 500 ms target fails.
13. **No `vss` extension.** Brute-force `array_cosine_similarity` over 39k vectors is the plan for F11.

## AI (M5+)

14. **Citation check normalizes both sides** (case, whitespace, curly quotes, reminder text) before the substring test. A strict byte match would reject valid citations.

## Build

15. **DuckDB links the prebuilt official library, not a bundled C++ build.** The `duckdb` crate default feature `download-lib` fetches `duckdb-shared-libs-linux-amd64.tar.gz` from the v2.0.0 release into `target/`, and `cargo test` and `cargo run` find it there. The `bundled` build was tried first and dropped. The build script's host dependencies (`serde`, `serde_json`, `cc`, `tar`, `ureq`) get different features for each cargo invocation (`test`, `run`, `-p`, rust-analyzer check), so each one rebuilt DuckDB from C++. Measured: 9 debug copies, 25.5 GB in `target/`, 23 min for one debug build, and one build killed for lack of RAM. No `bundled` cargo feature either, since `scripts/check.sh` runs clippy with `--all-features`.
16. **The Docker image ships libduckdb (M7).** Either copy `libduckdb.so` beside the binary, or link the official static libs with `DUCKDB_LIB_DIR` and `DUCKDB_STATIC=1` in the image build only.
