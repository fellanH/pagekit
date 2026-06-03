# pagekit — handoff baton

_Updated 2026-06-02 (felix). Boot order: this file → `AGENTS.md` → `tasks/arc.md`._

## State

Feature-complete, **no active sprint, no open blockers**. Build green, binary shipped to
`~/.local/bin/pagekit` (v0.1.0, includes this sprint's surface).

> ⚠️ **PATH note:** the `pagekit` first on PATH is `~/Library/Application Support/cargo/bin/pagekit`
> (cargo bin precedes `~/.local/bin`). Ship to **both** after a build, or `which pagekit` runs stale.
> This session shipped to both.

## Latest session — extract fidelity fix (2026-06-03, tas-0ffcaf88, `e69edc4`)

Class-A fidelity defect (finder: migrate seat, FIDELITY.md asedo #1): `pagekit extract` wrote
each fragment's content verbatim from its dominant source page, so a block whose dominant variant
came from a deep page (asedo posts at depth 2) baked `../../_assets/…` into `_fragments/navbar.html`
(depth 1) — refs one dir too high when the fragment is opened/composed standalone. 0 deliverable
impact (served pages always correct; marker insertion never rewrites in-page content).
- **Fix:** track a representative source page per shared block; on write, rebase each **relative**
  src/href/srcset (resolve vs source dir → lexical `..` normalize → re-express vs fragment dir, via
  `pathdiff`). Robust to any source depth. **Root-absolute `/…` left untouched** — correct from any
  depth + the sync `DepthRelativizer`'s job (this scoping also keeps the served-page flow intact).
- New helpers in `src/extract.rs`: `relativize_asset_refs`/`rebase_ref`/`rebase_srcset`/
  `lexically_normalize`/`is_skippable_ref`; `SharedBlock` gained a `source: PathBuf`.
- Verified on REAL asedo.se (`../../_assets/…logo.png` → `../_assets/…`, zero `../../_assets/` left).
  + regression test. **49 unit + 71 integ** green, clippy + fmt clean, binary reshipped to both paths.
- **Migrate seat:** re-run `pagekit extract` / `migrate asedo.se` to regenerate fragments — on-disk
  `_fragments/*.html` keep the old `../../` until re-extracted.

## Earlier session — suite exit-code/JSON standard adopted (2026-06-03, tas-0b56c632)

Coordinator-approved alignment to the published `fragments-sync` suite standard. **Breaking
contract change** (pagekit is v0.1.0 unpublished, so it's the cheap side to move):
- `--json` envelope `{check, status:"pass"|"fail", findings}` → **`{check, ok:bool, findings}`**
  (`ok = clean`, mirrors exit code). Applies to `links`/`seo`/`a11y`.
- Exit codes: findings now **`1`** (was `2`); `0` still clean. **`2` is now reserved for
  tool-internal errors** (bad args/unreadable root) via a `main`→`run` wrapper — so `exit == 1`
  means "check found problems" suite-wide, distinct from "the tool failed".
- Internal consumer aligned: `site-health-audit` connector emits 1/0/2 (was severity-keyed for
  the envelope, so unaffected there). Docs updated (AGENTS.md exit-code + `--json` sections).
- 49 unit + 70 integ green (all `Some(2)`→`Some(1)`, `status`→`ok` assertions updated), clippy +
  fmt clean, binary reshipped to `~/.local/bin` **and** the cargo-bin PATH shadow.
- Any future consumer that regexed `"status": "pass"` or gated on `exit == 2`-for-findings must update.

## Earlier session — GREEN slice landed: Site Health Audit report (2026-06-02)

The coordinator-approved QUEUED slice (`verify → --json → branded HTML report`) is **done,
runnable, dogfooded**. Built as a **presentation connector OUTSIDE the binary** —
`connectors/site-health-audit/audit.py` (Python stdlib only). No Rust touched; pagekit
stays pure mechanism (charter "no GUI" + `minimal-core-connectors` + `design-system-compliance`).

- Runs `pagekit <site> links|seo|a11y --json`, aggregates the 3 envelopes, renders ONE
  self-contained branded HTML report (CSS inlined → client-deliverable as-is).
- Styling reads the omni `packages/ui` tokens (`tokens.css`+`theme.css`) at render time →
  stays in lockstep with the design system, no hand-rolled CSS.
- Exit code mirrors the suite: `0` all-pass, `2` a check failed (≥1 error). Warnings advisory.
- Dogfooded: `we-know-aeo` (1 error + 19 warns → red verdict, 20-row SEO table) and
  `stormfors/knowledge-base` (warns-only → exit 0). Headless-screenshot verified the render.
- Scope held honest per migration relay: structural+SEO+a11y only; visual-diff/parity stay
  in the migration connector. README documents the boundary.
- **Offer build still PARKED** behind Felix's distribution hold — this is the internal tooling
  slice only, not the billable "Site Health Audit" GTM offer. Do not pursue the offer.

## Earlier session — CAND-A + CAND-B landed (2026-06-02, `4186730`)

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

Latest sprint — agent-consumable substrate (`2bab3de` + `1b67de8`). **NOTE: the envelope/exit-code
specifics below were superseded 2026-06-03 — see top section. Now `{check,ok,findings}`, findings exit `1`,
tool-error exit `2`.**
- **#1 `--json`** on `links`/`seo`/`a11y` — envelope (then `{check,status,findings}`, now `{check,ok,findings}`)
  via `src/report.rs`. Agents deserialize instead of regexing. (inventory/assets
  already emit TSV; `doctor`/`check` route through the fragments lib → deferred, would need upstream work.)
- **#2 uniform exit codes** — `check`/`doctor` exit nonzero on findings, matching the rest (then `2`, now `1`).
- **#3 `normalize-paths` safe-by-default** — dry-run unless `--write`, nonzero exit on pending changes (now `1`).
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
- **GREEN internal slice = ✅ DONE** (this session) — `connectors/site-health-audit/audit.py`. See
  "Latest session" above. The packaging slice is closed; only the GTM offer remains parked.

## Then

**No active sprint; the GREEN slice is closed.** Everything left is trigger-gated:
`tasks/arc.md` backlog (image-dims, expected_origin, framework profiles, meta-image BUG-3), the two
core-vs-opinion refactors (A: `sitemodel` dedup, B: `[policy]` config), CAND-C. **Don't pull gated
items speculatively** — wait for a consumer trigger. Connector next-steps if a consumer asks:
preflight summary row in the report, multi-site roll-up, or `--json` aggregate output.

## Recent commits

- `a07af1d` docs: core-vs-opinion review+proposal (first-principles, no refactor)
- `a0b3f17` docs: drain relay-inbox + first-principles minimal-core/connector decision
- `4186730` fix: CAND-A preflight stale-list + CAND-B skip non-web source files in orphan checks
- `2bab3de` feat: --json output, uniform exit codes, normalize-paths safe-by-default
