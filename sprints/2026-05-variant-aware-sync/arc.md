# Active arc — 2026-05-variant-aware-sync

Sprint focus pointer; full scope in [`README.md`](README.md).

## Now

D3 — `pagekit extract --split-variants`. Next dispatch. Emit N variant fragment files + rewrite source markers so per-content variation stops requiring manual triage.

## Done

- D1 — `pagekit check --strict` shipped. Subcommand is `check --strict [--name <fragment>]`; FNV-1a 64→32 hex hash per marker region, exit 0/2 on uniform/varies. Smoke-tested against ettsmart.se (reports nav 4 variants — dominant 10/4 split is the transparent vs default class — plus footer and sub-nav-sollentuna variance D2 will partly collapse). 13 prior + 3 new integration + 4 new unit tests passing.
- D2 — path-relative sync transforms shipped. `[transforms]` section with `path_root` + `attrs` (default `["href", "src"]`); `DepthRelativizer: SyncHook` walks fragment content with lol_html and rewrites attrs whose value starts with `path_root` to per-depth relative paths. Both `Cmd::Sync` and `Cmd::Check` (non-strict) wired through `sync_all_with` / `check_all_with` with the same hook stack (consistency contract). 4 new integration + 7 new unit tests passing; smoke tested on synthetic 3-page site at depths 0/1/2 (correct `../` counts, externals preserved, idempotent, check clean).

## Up next

- D2 follow-up: `Cmd::Watch` reactive resyncs still call the hookless `fragments::watch::run`, so transforms only apply on the initial sync inside watch. Needs a `watch::run_with(hooks)` upstream in fragments core; out of scope for D2 spec but flagged here so D3 or a follow-up worker picks it up. Workaround for now: re-run `pagekit sync` after fragment edits when watching a transformed site.
- D3 — `pagekit extract --split-variants` (emit N variant fragment files + rewrite source markers)

## Decisions captured during scoping

- Numerical variant naming (`nav-1.html`, `nav-2.html`) for D3 MVP; semantic naming is a follow-up if it earns its keep.
- D2 impl cut **closed** by upstream: fragments v0.6.0 (commit 7783628) ships `SyncHook` trait + `sync_all_with` / `check_all_with`. D2 implements `DepthRelativizer: SyncHook` and wires both `Cmd::Sync` and `Cmd::Check` to pass the same hook stack (consistency contract). No more pagekit-side sync reimplementation needed.
- D1 classifier (variant-class / path-depth / active-state) deferred to a follow-up — MVP just reports variance with content hashes and per-page groups.
- Sprint sequenced D1 → D2 → D3, each one independent worker dispatch.

## Halts to surface (none open)

When a worker hits a halt mid-deliverable, it surfaces in its own pane per `harness/rules/omni/halt-surfacing.md`. Mirror the reason here at sprint level when it implies cross-deliverable scope changes.
