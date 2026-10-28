# hii-magic-ation

Local web app to explore every Commander-legal Magic card, build themed card selections, and export them to Moxfield.

- Spec (French, source of truth for features): a private document
- Deviations from the spec, with the evidence behind each: `docs/decisions.md`. Read it before a milestone, update it when a choice departs from the spec.

## Layout

- `crates/core` (`hma-core`): domain, DuckDB access, filter AST and SQL compiler.
- `crates/server` (`hii-magic-ation` binary): axum API, CLI, embedded front.
- `web/`: React, TypeScript, Vite. `npm run build` writes `web/dist`, which the binary embeds.
- Crates `ingest` (M1) and `ai` (M5) are created when their milestone starts.

## Commands

- `scripts/check.sh`: local CI (front lint and build, fmt, clippy, tests). Run it before each commit.
- `cargo run -p hii-magic-ation`: serve on 127.0.0.1:7878. Front dev server: `npm run dev` in `web/`, it proxies `/api`.
- Config: `$XDG_CONFIG_HOME/hii-magic-ation/config.toml`, overridden by `HMA_*` variables. Data: `$XDG_DATA_HOME/hii-magic-ation`.

## Rules

- Work milestone by milestone (M0 to M7 in the spec). Do not start a milestone before the previous one passes its acceptance criteria. Track milestone progress in `docs/progress.md`.
- Commit one feature step at a time, never one milestone at a time. Conventional commits with a scope (`feat(catalog): ...`, `feat(edhrec): ...`). Never name a milestone in a commit. Each commit passes `scripts/check.sh`.
- Write each acceptance criterion as a test first. Test names state the behavior.
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
- The `duckdb` crate builds DuckDB from C++ source (`bundled`), about 10 min per cold build on this laptop.
- Each distinct cargo feature set triggers a new C++ build. Use only `scripts/check.sh`, `cargo test --workspace`, `cargo run -p hii-magic-ation` and `cv`, which are already built. Avoid `-p hma-core` and new feature flags.
- `.cargo/config.toml` caps jobs at 4. One DuckDB unity file needs up to 1.5 GB of RAM.
