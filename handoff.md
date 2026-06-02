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

The `fragments` crate published as **`fragments-sync`** (crates.io prep; rename committed `3ca4e75` at v0.7.0).
pagekit's `Cargo.toml` uses `fragments = { path = "../fragments", package = "fragments-sync" }` — the
lib target is still `fragments`, so `use fragments::…` is unchanged throughout the source.

**Dep baseline now `fragments-sync` v0.8.0 (`../fragments` HEAD `d5a6d2d`)** — fragments QoL pass added library
purity (lib is now **stdout-silent**, so pagekit's `sync` output is clean) + `--json` on fragments' own
check/list/doctor. **Backward-compatible**: `sync_all`/`sync_all_with` keep `usize`, `list_fragments`/`run_doctor`
signatures unchanged. New opt-in `sync_all_paths()`/`sync_all_paths_with()` → `Vec<PathBuf>` (unused by pagekit).
Verified green against pagekit's 112 tests (48 unit + 64 integ), clippy + fmt clean — this session, 2026-06-02.

Side effect for the deferred item: the `--json`-on-`check`/`doctor` caveat (handoff #1, "would need upstream
work" because they route through the fragments lib) is partly unblocked — the lib is now stdout-silent, the
necessary precondition. Still trigger-gated; pagekit would need fragments to expose structured return values
from `run_doctor`/check (not yet present) before it can emit JSON there. No trigger fired — left for a consumer.

## Then

Nothing dispatched. `tasks/arc.md` backlog is trigger-gated (no trigger fired). Don't pull gated items
speculatively. If idle, do baton/doc hygiene or wait for a consumer-driven trigger.

## Recent commits this session

- `1b67de8` chore: adopt fragments-sync package name (rename landed upstream)
- `2bab3de` feat: --json output, uniform exit codes, normalize-paths safe-by-default
- `a5c0b67` docs: record fix commit hash in handoff baton
- `75fe017` fix: clippy + fmt on agent-edit tooling
