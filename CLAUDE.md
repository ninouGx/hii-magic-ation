# hii-magic-ation

Local web app to explore every Commander-legal Magic card, build themed card selections, and export them to Moxfield.

- Spec: a private French document, the source of truth for features. `CLAUDE.local.md` links it when present.
- Deviations from the spec, with the evidence behind each: `docs/decisions.md`. Read it before a milestone, update it when a choice departs from the spec.

## Layout

- `crates/core` (`hma-core`): domain, DuckDB access, filter AST and SQL compiler.
- `crates/server` (`hii-magic-ation` binary): axum API, CLI, embedded front.
- `web/`: React, TypeScript, Vite. `npm run build` writes `web/dist`, which the binary embeds.
- `crates/ingest` (`hma-ingest`): Scryfall and EDHREC downloads, catalog build. Crate `ai` is created in M5.
- User schema: `crates/core/sql/user/NNN_*.sql`, applied once each by `Db::open`. Never edit a released migration, add the next one.

## Commands

- `scripts/check.sh`: local CI (front lint, Vitest and build, then fmt, clippy, tests). Run it before each commit.
- `cargo run -p hii-magic-ation`: serve on 127.0.0.1:7878. Front dev server: `npm run dev` in `web/`, it proxies `/api`.
- Config: `$XDG_CONFIG_HOME/hii-magic-ation/config.toml`, overridden by `HMA_*` variables. Data: `$XDG_DATA_HOME/hii-magic-ation`.
- Commit chain, so a failed check never lets a commit through: `./scripts/check.sh > target/check.log 2>&1 && git commit ...`.
- Manual test in a browser: a server already running on 7878 holds the lock on its `user.duckdb`. Copy the catalog to `target/verify/data/` and run `HMA_DATA_DIR=$PWD/target/verify/data HMA_BIND=127.0.0.1:7879 cargo run -p hii-magic-ation -- serve`. Stop it by its port owner, `fuser -k 7879/tcp`. Never stop it by a command-line pattern, the 7878 server has the same command line.

## Rules

- Work milestone by milestone (M0 to M7 in the spec). Do not start a milestone before the previous one passes its acceptance criteria. Track milestone progress in `docs/progress.md`.
- Commit one feature step at a time, never one milestone at a time. Conventional commits with a scope (`feat(catalog): ...`, `feat(edhrec): ...`). Never name a milestone in a commit. Each commit passes `scripts/check.sh`.
- Write each acceptance criterion as a test first. Test names state the behavior. Check that a new test fails without its fix.
- Core tests share one fixture catalog (`tests/common::db()`). A test that writes user data uses `common::fresh_db()`.
- After each feature, get an outside review of the diff. Verify every finding in the code before acting, and report which ones were dismissed and why.
- Test every UI path in a real browser before calling it done: clicks, keys, closing, URL state.
- Agreed spec changes go into the spec and into `docs/decisions.md`, with the date.
- Code is self-explanatory. A comment only states a non-obvious why.
- Add dependencies with `cargo add` or `npm install`, never versions from memory.
- The filter compiler emits parameterized SQL only. Never concatenate a user value into SQL.
- External HTTP clients sit behind traits (`CardSource`, `PopularitySource`, `ComboSource`, `Classifier`, `Embedder`) with recorded JSON fixtures. No test calls Scryfall, EDHREC, Spellbook or Vertex.
- Respect Scryfall rules (bulk files only, unmodified images) and 1 req/s on EDHREC and Spellbook.
- `thiserror` in libraries, `anyhow` only in the binary, `tracing` for logs.
- DuckDB regex is RE2. No lookahead, no backreference.
- Prompts live in `prompts/<feature>/<version>.md`. The version is part of the AI cache key. The eval set runs by hand, never in CI.

## DuckDB

- `user.duckdb` is the default database and holds data that cannot be rebuilt. `catalog.duckdb` is attached read-only as `catalog` and only a sync replaces it.
- `Db::connection()` clones a connection on the shared instance. Run each query inside `spawn_blocking`.
- The `duckdb` crate links the prebuilt libduckdb v2.0.0, downloaded into `target/` by its build script. `cargo test` and `cargo run` find it there. A binary run by hand needs `LD_LIBRARY_PATH` set to that directory. Never enable `bundled`, see `docs/decisions.md`.
