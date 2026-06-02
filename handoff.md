# pagekit — handoff baton

_Updated 2026-06-02 (felix). Boot order: this file → `AGENTS.md` → `tasks/arc.md`._

## State

Feature-complete, **no active sprint, no open blockers**. Build green, binary shipped to
`~/.local/bin/pagekit` (v0.1.0, includes this sprint's surface).

Latest sprint — agent-consumable substrate (`2bab3de` + `1b67de8`):
- **#1 `--json`** on `links`/`seo`/`a11y` — envelope `{check,status,findings:[{rule,severity,page?,message}]}`
  via `src/report.rs`. Agents deserialize instead of regexing. Exit code unchanged. (inventory/assets
  already emit TSV; `doctor`/`check` route through the fragments lib → deferred, would need upstream work.)
- **#2 uniform exit codes** — `check`/`doctor` now exit `2` on findings, matching the rest. `2` = "something to act on."
- **#3 `normalize-paths` safe-by-default** — dry-run unless `--write`, exit `2` on pending changes.
- Held the polish (`--skip`/`--only`/`--policy`/`-q`/`--diff`) per subtract-before-building.
- Tests: 64 integ + 48 unit, clippy + fmt clean.

## fragments dependency note (resolved)

The `fragments` crate published as **`fragments-sync` v0.7.0** (crates.io prep, committed `3ca4e75`).
pagekit's `Cargo.toml` now uses `fragments = { path = "../fragments", package = "fragments-sync" }` — the
lib target is still `fragments`, so `use fragments::…` is unchanged throughout the source. Build is green
against the committed rename. (Earlier this session the rename was mid-flight and transiently blocked the
build; that's done.)

## Then

Nothing dispatched. `tasks/arc.md` backlog is trigger-gated (no trigger fired). Don't pull gated items
speculatively. If idle, do baton/doc hygiene or wait for a consumer-driven trigger.

## Recent commits this session

- `1b67de8` chore: adopt fragments-sync package name (rename landed upstream)
- `2bab3de` feat: --json output, uniform exit codes, normalize-paths safe-by-default
- `a5c0b67` docs: record fix commit hash in handoff baton
- `75fe017` fix: clippy + fmt on agent-edit tooling
