# pagekit

Binary ships. Sprints 4-7 closed; agent-tooling trajectory complete. Test suite green: **71 integration + 49 unit**, clippy + fmt clean. Agent-consumable substrate sprint shipped (`--json` on links/seo/a11y, uniform exit codes, normalize-paths safe-by-default). Dogfood fix-candidates CAND-A + CAND-B landed (`4186730`). Suite-standard `--json`/exit-code alignment (`ok:bool`, exit-1-on-findings, exit-2-tool-error) landed (`c9eaea4`, tas-0b56c632). Extract fidelity fix — fragment asset refs re-relativized to fragment depth — landed (`e69edc4`, tas-0ffcaf88).

## Active arc

**Sprint 7 closed (Phase 3, agent-tooling trajectory complete).** All three deliverables shipped: `pagekit assets` (commit `a70313b`, HTML+CSS reference graph closing the CSS-loaded-orphan gap), `pagekit show <name>` (commit `8fb8a90`, fragment+classes+URLs bundle), `pagekit preflight` (commit `a642ecb`, single go-live gate). Sprint folder: [`sprints/2026-05-retrieval-composition/`](../sprints/2026-05-retrieval-composition/README.md).

**Pagekit is feature-complete for the Phase 1-3 plan from the strategic exchange.** Full surface:

- **Build/edit:** `init`, `extract`, `extract --split-variants`, `sync`, `watch`, `normalize-paths`, `file-paths`, `list`, `config`
- **Bulk edit (safe-by-default, dry-run unless `--write`):** `apply`, `mv-asset`, `rename-assets` (commit `cc12ec8`, agent-edits trajectory)
- **Read (token-efficient):** `inventory`, `show`, `assets`
- **Verify:** `check`, `check --strict`, `check --strict --selector`, `doctor`, `links`, `seo`, `a11y`, `preflight`

Phase 4 candidates (image dims, semantic variant naming, framework profiles, expected_origin config) remain trigger-gated. No active sprint planned.

**Site Health Audit connector shipped (2026-06-02).** The coordinator-approved GREEN slice (`verify → --json → branded HTML report`) landed as `connectors/site-health-audit/audit.py` — a presentation connector OUTSIDE the binary that aggregates `pagekit links|seo|a11y --json` into one self-contained branded HTML report, styled via `packages/ui` tokens. No Rust touched (charter "no GUI" + minimal-core/connectors). Dogfooded against we-know-aeo + knowledge-base, headless-screenshot verified. The billable GTM offer of the same name stays PARKED behind Felix's distribution hold.

**Sprint 6 closed (Phase 2).** `pagekit links`, `pagekit seo`, `pagekit a11y`, generalized `check --strict --selector`. Sprint folder: [`sprints/2026-05-correctness-checks/`](../sprints/2026-05-correctness-checks/README.md).

**Sprint 5 closed (Phase 1).** Both deliverables shipped: `pagekit inventory` (commit `cdfd2e7`) and `pagekit normalize-paths` (commit `efc39a7`). Sprint folder: [`sprints/2026-05-query-layer/`](../sprints/2026-05-query-layer/README.md).

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.
- Stage 3 hybrid: scraper for cross-page detection (multi-pass query), `lol_html` for per-page source rewrite (single-pass streaming). The bridge is sibling-index matching — scraper picks "wrap the Nth `<footer>`", lol_html walks elements by selector and counts to that index. Eliminates `find_first_tag_span` / `find_matching_tag_span` and the source-vs-DOM reconciliation bug class.
- **Minimal-core / connectors lens (first-principles pass, 2026-06-02 Felix directive).** Macro-architecture is already correct: `fragments` = unopinionated mechanism core (format-agnostic text-sync); pagekit = the HTML-opinion *connector* over it. Two boundary refinements surfaced, both trigger-gated:
  - *(a) Policy-as-config (internal).* HTML opinions (whitelists, severities, non-web extensions, expected_origin) are hardcoded Rust constants → each false-positive fix is an edit-recompile-reship (CAND-B, `PLATFORM_FILES`, `llms.txt`, `og:image` — ~4 recent). **Trigger:** on the *next* hardcoded-whitelist/severity edit, externalize a `[policy]` config block instead of adding a constant. The binary stays mechanism (graph-build + diff + parse) + a default policy. Do NOT build the config engine speculatively.
  - *(b) Cross-connector boundary (resolves the migration-friction relay).* Asset-parity (byte/rule-count vs source) and visual-diff-vs-source are **migration-verification** opinions — owned by the clone/migration connector, NOT pagekit's steady-state verify suite. pagekit already exposes the mechanism they compose: `assets` emits per-file hash+bytes+type; the migration tool diffs two manifests. **Decision: do not add a parity/visual-diff command to pagekit.** If a consumer needs a finer mechanism (e.g. CSS rule-count per file), that's a small mechanism add — the *comparison opinion* stays in the connector.

## Backlog

- **Image dimension extraction in `pagekit assets`** — assets manifest currently emits hash + bytes + type. Dims (width × height per image) would unlock LCP analysis, responsive-image gap detection, and HTML img-tag dim auto-fill. Needs an image-header parser; lightweight `imagesize` crate handles PNG/JPEG/GIF/WebP/AVIF/SVG without full decoding. **Trigger:** felixhellstrom.com's image-dim friction returns OR a consumer asks for LCP work.
- **CSS-rule extraction in `pagekit show`** — current `show` outputs class names; agent grep CSS to find the rules. A `--with-css` flag could return the matched rules inline. Needs lenient CSS parsing. **Trigger:** consumer asks for full component bundle.
- **`[seo].expected_origin` config option** — `pagekit seo` flags scheme/host MISMATCH within in-HTML canonicals (mixed www/apex declarations). It does NOT catch the deploy-vs-HTML mismatch (ettsmart.se's www→apex pattern). Adding `[seo].expected_origin = "https://ettsmart.se"` lets the check fire on this case. **Trigger:** next consumer hits the same deploy-vs-HTML mismatch.
- **Framework-export profiles** — Webflow + Bootstrap-class profiles. Speculative; needs a third consumer pattern.
- **D2 transforms — second-consumer test** — Sprint 4 D2 + Sprint 5 D2 share rewriting logic; neither exercised against a real consumer that needs depth-relative output (ettsmart.se uses absolute paths intentionally). Validate against file:// preview, sub-path deploy, or non-root static export when one surfaces.
- **Semantic variant naming for `extract --split-variants`** — current scope emits numerical names (`nav-1`, `nav-2`). ettsmart.se demonstrates the manual end-state (`nav-default`, `nav-transparent`) is more readable. Auto-detect from class diffs. **Trigger:** when numerical naming costs a manual rename pass on a real consumer.
- **Migration ergonomics for `--split-variants`** — fresh-run only today; a re-run after plain `extract` is silently no-op'd. **Trigger:** first time a consumer asks for it.
- **Broken-link check on `<meta>` social-card images** — `links`/`assets` now COUNT `og:image`/`twitter:image` content toward the reference graph (orphan-set only; commit `cd9b898`), but a meta image pointing at a *missing* file is not yet flagged broken. Absolute OG URLs are the spec norm (skipped as External), so only relative/root-absolute would be checked. **Trigger:** a consumer ships a broken social card via a relative og:image and wants it caught. See `todo/2026-06-02-dogfood-weknowaeo.md` (BUG-3).
- **Orphan-exclusion mechanism (decision (a) trigger FIRED — do NOT just add a constant).** clone-a-website's `menubar.swift` (macOS source in the web root) is flagged orphan-asset because `.swift`/compiled-language sources aren't in `is_non_web_deployable()` (CAND-B covers scripting+shell+build only), AND the consumer created a `.assetsignore` expecting suppression — **pagekit doesn't honor it** (no such feature). This is the 5th whitelist false-positive (post CAND-B/PLATFORM_FILES/llms.txt/og:image), so it hits **Decisions (a) Policy-as-config**: the next whitelist edit should externalize the opinion, not add another hardcoded constant. Candidate mechanisms: (1) `[policy]` TOML block (decision (a)'s original prescription), or (2) **honor a gitignore-style `.assetsignore`** for orphan exclusions — cleaner/more mechanism-pure (opinion lives in the consumer's repo, not the binary), and the consumer already authored the file. **Leaning (2).** Relayed the `.assetsignore`-not-honored correction back to clone-a-website 2026-06-03. **Trigger to build:** a coordinator/Felix dispatch, or the next consumer to hit an orphan false-positive — not off this FYI receipt alone. Do NOT build speculatively.

### Dogfood RE-AUDIT false-positive classes (clone-a-website funnel, 2026-06-03)

Coordinator GREEN re-dispatch pointed the verify suite at clone-a-website's customer funnel (`app/`).
**Result: the consumer surface is CLEAN** — prior fixes (`3e29f3d`) held, a11y PASS. Every residual
finding traced to a **pagekit-side blind spot**, not a site defect. Relayed the honest "your funnel is
audit-clean, these are ours" result back to clone-a-website. The 4 false-positive classes are pagekit's:

- **FP-1 — `seo` doesn't skip `noindex` pages (NEW).** Flagged missing canonical/description as *errors*
  on `/cloning.html` + `/__checkout/success/` — both carry `<meta name="robots" content="noindex">`.
  Canonical/description are meaningless on noindex pages; flagging them is noise. **Fix:** `seo` should
  skip (or downgrade) canonical/description rules when a page is `noindex`. **Trigger:** FIRED (consumer
  noindex funnel pages mis-flagged). Highest-signal of the four — cheap, mechanism-pure, no policy needed.
- **FP-2 — `links` doesn't model clean-URL rewriting (NEW).** Extensionless internal links (`href="/terms"`,
  `/privacy`) flagged 404 because only `terms.html`/`privacy.html` exist on local FS; CF Pages serves them
  (`.html` 308→clean, 200 in prod). **Fix candidate:** treat `/foo` as resolved if `foo.html` (or `foo/index.html`)
  exists — opt-in or heuristic, since clean-URL is host-dependent. **Trigger:** FIRED (2nd consumer w/ CF clean URLs;
  ettsmart.se pattern adjacent). Needs care — host-specific opinion, candidate for the `[policy]`/config seam.
- **FP-3 — `.assetsignore` still not honored (orphan FP re-fired).** Same as the standing orphan-exclusion
  item above — clone-a-website's `menubar.swift` re-flagged despite a correct `.assetsignore`. This is now the
  **2nd distinct consumer-fire** of that trigger ("next consumer to hit an orphan false-positive"). Leaning
  option (2): honor gitignore-style `.assetsignore`. Trigger condition for the build is now met on evidence.
- **FP-4 — `preflight` check/doctor ERROR on non-fragments sites (NEW).** `app/` has no `_fragments/`, so
  preflight's `check`+`doctor` stages hard-ERROR ("cannot read _fragments"). A site that simply doesn't use
  fragments isn't *broken*. **Fix:** treat absent `_fragments/` as N/A/skip (not ERROR) in preflight. Cheap.

**Disposition:** captured, NOT built — this was an audit+relay dispatch, not a pagekit-fix dispatch. FP-1 and
FP-4 are cheap mechanism fixes with no policy implications; FP-2 + FP-3 touch the host-opinion/`[policy]` seam.
Awaiting a coordinator/Felix pagekit-fix dispatch before building (do NOT build speculatively).

### Dogfood fix-candidates from knowledge-base audit (2026-06-02)

Surfaced running the audit against `stormfors/knowledge-base`. Full detail + root cause in
[`todo/2026-06-02-dogfood-knowledge-base.md`](../todo/2026-06-02-dogfood-knowledge-base.md).
- **CAND-A — `preflight` swallows `check`'s stale-file list.** ✅ DONE (`4186730`). `run_sync_check` now prints each `CheckIssue` under `== check ==` via `format_check_issue`, mirroring standalone `check`. + integ test.
- **CAND-B — orphan-asset detection flags non-web build scripts** (`.sh`/`.py`/Makefile) when `target_dir="."`. ✅ DONE (`4186730`). Shared `links::is_non_web_deployable()` (extension + basename class), used by both `links` + `assets` orphan passes. + 1 unit + 2 integ tests.
- **CAND-C (low-confidence, defer)** — `_`-prefixed scaffolding templates SEO-audited as real pages. Blanket `_`-skip risks over-reach; wait for a consumer complaint. STILL DEFERRED.

## Real bugs surfaced (not pagekit's responsibility)

Findings from running pagekit's checks against ettsmart.se on 2026-05-06. Surfaced naturally; owner: chad-ettsmart_se.

- **`pagekit links`** — 6 broken internal links (404.html and contact form referencing stale Webflow paths: `_assets/site/css/main.css`, `_assets/site.css`, `_assets/site.js`, `_assets/hubspot/forms/embed/v2.js`)
- **`pagekit seo`** — 3 missing canonicals (`/thank-you/`, `/sv/thank-you/`, `/test/`); 11 missing meta descriptions on SV subpages; 8 multiple-H1 warnings on Webflow templates; 25 missing OG-tag warnings; 1 duplicate-description; canonical www→apex mismatch (declared canonicals all use `www.ettsmart.se` but live deploy serves apex — needs `expected_origin` config to auto-flag, see backlog)
- **`pagekit a11y`** — 4 unlabeled honeypot anti-spam fields (`<input name="website">` without proper hiding) on contact forms

## Resolved decisions

- **Suite-wide `--json`/exit-code standard — DONE (tas-0b56c632, coordinator-approved).** pagekit
  now matches the published `fragments-sync` suite standard: `--json` envelope is `{check, ok, findings}`
  (boolean `ok = clean`, not `status:pass/fail`), and exit codes are `0` clean / `1` findings (was `2`).
  A **distinct `2` is reserved for tool-internal errors** (bad args/unreadable root) via a `main`→`run`
  wrapper, per the backlog's "keep a distinct code if you have one" — so `exit == 1` unambiguously means
  "the check found problems" suite-wide. The `site-health-audit` connector was already severity-keyed
  (unaffected by the envelope change); its own emitted exit codes were aligned to 1/0/2 to match.
  49 unit + 70 integ green, clippy + fmt clean, binary reshipped to both PATH locations.

## Blocked

Nothing. The `fragments` crate published as `fragments-sync` v0.7.0 (committed `3ca4e75`); pagekit's `Cargo.toml` adopted `package = "fragments-sync"` (lib target still `fragments`, so `use fragments::…` is unchanged). Build green, 64 integ + 48 unit pass, clippy + fmt clean, binary shipped to `~/.local/bin/pagekit`.
