# Active arc — 2026-05-variant-aware-sync

Sprint focus pointer; full scope in [`README.md`](README.md).

## Now

D1 — `pagekit check --strict`. First dispatch. Visibility deliverable; unblocks the diagnostic that D2 and D3 need.

## Up next

- D2 — path-relative sync transforms (`[transforms]` config + per-depth path rewriting at sync time)
- D3 — `pagekit extract --split-variants` (emit N variant fragment files + rewrite source markers)

## Decisions captured during scoping

- Numerical variant naming (`nav-1.html`, `nav-2.html`) for D3 MVP; semantic naming is a follow-up if it earns its keep.
- Worker decides on D2's impl cut (A: pagekit reimplements sync; B: post-process after `fragments::sync_all`); both are reversible.
- D1 classifier (variant-class / path-depth / active-state) deferred to a follow-up — MVP just reports variance with content hashes and per-page groups.
- Sprint sequenced D1 → D2 → D3, each one independent worker dispatch.

## Halts to surface (none open)

When a worker hits a halt mid-deliverable, it surfaces in its own pane per `harness/rules/omni/halt-surfacing.md`. Mirror the reason here at sprint level when it implies cross-deliverable scope changes.
