# Decisions

Each entry departs from or refines the spec. Evidence was measured on the live sources on 2026-10-28 unless stated otherwise.

## Data sources

1. **Scryfall bulk files are `.jsonl.gz`.** The bulk-data endpoint only exposes `jsonl_download_uri`. Download each file to the data directory, then load it with DuckDB `read_json(..., format = 'newline_delimited')`, which reads gzip natively. No custom streaming parser. Sizes compressed: oracle_cards 25 MB, default_cards 80 MB, rulings 5 MB, oracle_tags 6 MB, art_tags 13 MB.
2. **EDHREC `id` is a Scryfall print id.** 100 of 100 entries on the week page 1 match a Scryfall card id, 0 match an oracle_id. Map `id -> prints.print_id -> oracle_id` through default_cards. Name matching is the fallback for partner pairs only (they carry `is_partner`, `names[]` and one `id`).
3. **EDHREC pages have two shapes.** Page 1 nests the list under `container.json_dict.cardlists[0]`. Pages 2+ put `cardviews` at the top level. The parser accepts both. Volume: week 45 pages, year 69 pages, so a full sync at 1 req/s takes about 3 min.
4. **Partner pairs need their own key.** A commander can rank solo and in several pairs. Key `edhrec_commander_ranks` on `(oracle_id, partner_oracle_id, window)`, with an empty partner for solo entries.

## Data model

5. **Tag weight is not a filter or sort criterion.** 99.7 % of oracle taggings and 97 % of art taggings are `median`. Keep the column, drop it from F3 and F4.
6. **Multi-face text is materialized at ingest.** 3327 of 39099 oracle cards have a null top-level `oracle_text`. Store `search_name`, `search_text` and `search_flavor`, concatenated over all faces, so filters never parse JSON at query time.
7. **Power, toughness and loyalty keep two columns.** 547 values are not numeric (`*`, `1+*`, `?`). Store the display string and a nullable number from `TRY_CAST`.
8. **Tag hierarchy is flattened at sync.** A `tag_ancestors` closure table makes "include child tags" a join. A `card_tag_flat (oracle_id, tag_id)` table resolves art tags from illustration to card. Both are rebuilt with the catalog.
9. **Scryfall and user tags live in separate files.** `catalog.tags` and `catalog.card_tags` hold Scryfall data. `user.tags` and `user.card_tags` hold custom and AI tags. A view unions them for filters. A sync never writes to `user.duckdb`.
10. **`prints` stores `image_small`**, used by the grid (F5).

## Sync and concurrency

11. **A sync builds a new catalog file and swaps it.** DuckDB allows one writing process per file. The sync writes `catalog.duckdb.new`, validates row counts, then the server detaches the old catalog, renames the file and attaches it again. This replaces the staging-tables design. The CLI `sync` command never opens `user.duckdb`, so it runs while the server is up, and asks the server to reload when it is reachable.

## Performance

12. **No precomputation before a measurement.** The catalog holds about 39k cards and 740k taggings. F6 evaluates the filter once into a temporary table of oracle_ids, then runs the aggregations on it. Precomputed reference distributions for the lift come only if the 500 ms target fails.
13. **No `vss` extension.** Brute-force `array_cosine_similarity` over 39k vectors is the plan for F11.

## AI (M5+)

14. **Citation check normalizes both sides** (case, whitespace, curly quotes, reminder text) before the substring test. A strict byte match would reject valid citations.

## Build

15. **`duckdb` crate with `bundled` and `json` features.** DuckDB builds from C++ source, several minutes per cold profile. The alternative is `download-lib`, which links a prebuilt shared library at runtime and complicates the Docker image.
16. **The bundled DuckDB is `v2.0.0-dev88614`.** `duckdb` 1.20000.0, the latest crate, ships a development build. Watch for engine bugs, and pin the crate version in `Cargo.lock`.
