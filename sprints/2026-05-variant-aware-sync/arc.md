# Active arc — 2026-05-variant-aware-sync

Sprint focus pointer; full scope in [`README.md`](README.md).

## Now

D2 — path-relative sync transforms. Next dispatch. `[transforms]` config + per-depth path rewriting at sync time, wired via fragments v0.6.0's `SyncHook` trait.

## Done

- D1 — `pagekit check --strict` shipped. Subcommand is `check --strict [--name <fragment>]`; FNV-1a 64→32 hex hash per marker region, exit 0/2 on uniform/varies. Smoke-tested against ettsmart.se (reports nav 4 variants — dominant 10/4 split is the transparent vs default class — plus footer and sub-nav-sollentuna variance D2 will partly collapse). 13 prior + 3 new integration + 4 new unit tests passing.

## Up next

- D3 — `pagekit extract --split-variants` (emit N variant fragment files + rewrite source markers)

## Decisions captured during scoping

- Numerical variant naming (`nav-1.html`, `nav-2.html`) for D3 MVP; semantic naming is a follow-up if it earns its keep.
- D2 impl cut **closed** by upstream: fragments v0.6.0 (commit 7783628) ships `SyncHook` trait + `sync_all_with` / `check_all_with`. D2 implements `DepthRelativizer: SyncHook` and wires both `Cmd::Sync` and `Cmd::Check` to pass the same hook stack (consistency contract). No more pagekit-side sync reimplementation needed.
- D1 classifier (variant-class / path-depth / active-state) deferred to a follow-up — MVP just reports variance with content hashes and per-page groups.
- Sprint sequenced D1 → D2 → D3, each one independent worker dispatch.

## Halts to surface (none open)

When a worker hits a halt mid-deliverable, it surfaces in its own pane per `harness/rules/omni/halt-surfacing.md`. Mirror the reason here at sprint level when it implies cross-deliverable scope changes.
