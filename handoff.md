# pagekit — handoff baton

_Updated 2026-06-02 (felix). Boot order: this file → `AGENTS.md` → `tasks/arc.md`._

## State

Feature-complete, **no active sprint, no open blockers**. Build green, binary shipped to
`~/.local/bin/pagekit` (v0.1.0, includes this sprint's surface).

> ⚠️ **PATH note:** the `pagekit` first on PATH is `~/Library/Application Support/cargo/bin/pagekit`
> (cargo bin precedes `~/.local/bin`). Ship to **both** after a build, or `which pagekit` runs stale.
> This session shipped to both.

## Latest session — CAND-A + CAND-B landed (2026-06-02, `4186730`)

The two dogfood fix-candidates from the knowledge-base audit are **done, shipped, verified**:
- **CAND-A** — `preflight`'s `== check ==` section now lists every stale/malformed page (not just a
  count). `run_sync_check` (`src/preflight.rs`) prints each `CheckIssue` via a `format_check_issue`
  helper mirroring standalone `check`. + integ test `preflight_lists_stale_pages_under_check`.
- **CAND-B** — orphan detection in `links`+`assets` skips non-web-deployable source/build files
  (`build.sh`/`*.py`/`Makefile`/`*.toml`…) via shared `links::is_non_web_deployable()` (extension +
  basename class, symmetric with `PLATFORM_FILES`). + 1 unit + 2 integ tests.
- Tests now **49 unit + 70 integ**, clippy + fmt clean. Verified live against
  `stormfors/knowledge-base`: build scripts gone from orphans; preflight names all stale pages.
- **CAND-C still deferred** (low-confidence `_`-prefix SEO skip) — wait for a consumer complaint.

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

## Dogfood hardening (2026-06-02, `cd9b898`)

Ran the full audit against `~/omni/products/we-know-aeo` (AEO offer's own Next-export site). No crashes;
exit codes correct. Fixed two orphan-detection false positives in `links`/`assets`: (1) `<meta og:image /
twitter:image content>` now counts toward the reference graph (was a false orphan + social-card blind spot);
(2) `llms.txt`/`ads.txt`/`app-ads.txt` whitelisted as well-known root files (class of robots.txt/sitemap.xml).
+3 integ tests (now **67 integ + 48 unit**). BUG-3 (broken-link check on meta images) deferred → arc backlog.
Real site findings (broken privacy/terms Next export → 404 titles, missing canonicals, etc.) are **weknowaeo's**,
not pagekit's — captured in `todo/2026-06-02-dogfood-weknowaeo.md` for relay.

## Dogfood #2 — knowledge-base audit (2026-06-02) — FIXES LANDED

Audit ran against `~/omni/companies/stormfors/knowledge-base` (31-page hand-authored KB). Surfaced two
fix-candidates; **CAND-A + CAND-B are now implemented** (see "Latest session" above, `4186730`).
Full original detail + root cause: `todo/2026-06-02-dogfood-knowledge-base.md`.

**Site bugs are NOT pagekit's** — the KB has 27 stale pages needing `pagekit sync` (real content drift
the tool caught correctly). Owner: the knowledge-base seat, relay via hub, do not absorb.

## Relay-inbox drain (2026-06-02 ~22:40)

Drained 5 undrained items in `~/.omni/relay-inbox/pagekit/` (marked `.done`):
- **Suite framing + seam** (08:40 ×2) — ack: 5 seats = ONE website-mgmt product suite, revenue-biased;
  pagekit composes fragments core, flag needs upstream / coordinate before breaking. Live upstream flag
  stands: JSON-on-`check`/`doctor` needs fragments to expose structured returns from `run_doctor`/check.
- **Migration-friction relay** (10:52) — vault `01KT3Z6X38ZZBKJ9SPP5M5H6M2`: asset-parity + visual-diff
  gaps in clone tooling. **Triaged by first-principles (below):** these are *migration-connector*
  opinions, NOT pagekit verify commands. pagekit already ships the mechanism (`assets` hash+bytes); the
  parity gate composes it. Decision recorded in `tasks/arc.md`. No pagekit feature added.
- **Fleet-pitch experiment** (10:52, bounded → 2026-06-16) — filed `~/.omni/idea-queue/pitch-pagekit-1.md`:
  package the verify suite as a billable client "Site Health Audit" (evidence: real defects found
  dogfooding 2 client sites this session). Relay-only, action gate unchanged.
- **Felix first-principles directive** (20:35) — minimal unopinionated core + connectors. Reasoned the
  domain; conclusion: macro-architecture already correct (fragments=mechanism core, pagekit=HTML-opinion
  connector). Two trigger-gated refinements recorded in `tasks/arc.md` Decisions: (a) externalize
  hardcoded HTML policy to a `[policy]` config block on the *next* whitelist edit (not now); (b) keep
  migration-verification opinions out of pagekit (cross-connector boundary).

## Core-vs-opinion review (2026-06-02, `a07af1d`) — DONE, no refactor

Felix first-principles directive delivered: `docs/core-vs-opinion.md` — read-only review+proposal,
**no code touched (refactor is RED)**. Conclusion: macro-architecture already correct (`fragments` =
mechanism core, pagekit = HTML-opinion connector, clean `SyncHook`/flatten seam). Two trigger-gated
wins, both *inside* the connector: **(A)** dedup the forked site-model/emit-to-vanilla mechanism
(`display_url` ×6, `resolve_internal` ×4, etc.) into an internal `sitemodel` module; **(B)** turn
hardcoded opinion (whitelists/thresholds) into a `[policy]` config block on the next whitelist edit.
Both logged in `tasks/arc.md` Decisions. Do NOT do them speculatively.

## QUEUED for next capacity window (coordinator receipt `20260602210710`)

Pitch *Site Health Audit* scored **8/8** (strongest this round). Disposition:
- **GTM/billable-offer = PARKED** behind Felix's active distribution hold (not killed) — logged ready
  for studio+/strategist when he lifts it. **Do not pursue the offer build.**
- **GREEN internal slice = QUEUED** (tooling-refinement-aligned): `verify (links/seo/a11y/preflight)
  → --json → /present branded HTML report`. The mechanism all ships (`--json` + uniform exit codes);
  this is packaging output into a client-facing report. Pick up in a fresh window — it was explicitly
  "don't pile it on now." Honest scope per migration relay: structural+SEO+a11y defects only; visual
  diff is the migration connector's job.

## Then

**Top of the list: the QUEUED GREEN slice above** (Site Health Audit report — verify → `--json` →
`/present`), coordinator-approved for a fresh window. Everything else stays trigger-gated:
`tasks/arc.md` backlog (image-dims, expected_origin, framework profiles, meta-image BUG-3), the two
core-vs-opinion refactors (A: `sitemodel` dedup, B: `[policy]` config), CAND-C. **Don't pull gated
items speculatively** — wait for a consumer trigger or the GREEN slice's window.

## Recent commits

- `a07af1d` docs: core-vs-opinion review+proposal (first-principles, no refactor)
- `a0b3f17` docs: drain relay-inbox + first-principles minimal-core/connector decision
- `4186730` fix: CAND-A preflight stale-list + CAND-B skip non-web source files in orphan checks
- `2bab3de` feat: --json output, uniform exit codes, normalize-paths safe-by-default
