# pagekit

Binary ships. Sprints 4-6 closed. Test suite green: 50 integration + 35 unit, clippy + fmt clean.

## Active arc

**Sprint 7 — retrieval + composition (Phase 3, closes agent-tooling trajectory).** Folder: [`sprints/2026-05-retrieval-composition/`](../sprints/2026-05-retrieval-composition/README.md). Three deliverables: `pagekit assets` (HTML+CSS reference graph, hash/byte/type manifest, closes the CSS-loaded-orphan gap from Sprint 6 D1), `pagekit show <name>` (fragment + classes + asset bundle in one report), `pagekit preflight` (composes check + doctor + links + seo + a11y into a single go-live gate). After this lands, pagekit is feature-complete for the strategic exchange's Phase 1-3 plan.

**Sprint 6 closed (Phase 2).** All four deliverables shipped: `pagekit links`, `pagekit seo`, `pagekit a11y`, generalized `check --strict --selector`. Sprint folder: [`sprints/2026-05-correctness-checks/`](../sprints/2026-05-correctness-checks/README.md).

**Sprint 5 closed (Phase 1).** Both deliverables shipped: `pagekit inventory` (commit `cdfd2e7`) and `pagekit normalize-paths` (commit `efc39a7`). Sprint folder: [`sprints/2026-05-query-layer/`](../sprints/2026-05-query-layer/README.md).

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.
- Stage 3 hybrid: scraper for cross-page detection (multi-pass query), `lol_html` for per-page source rewrite (single-pass streaming). The bridge is sibling-index matching — scraper picks "wrap the Nth `<footer>`", lol_html walks elements by selector and counts to that index. Eliminates `find_first_tag_span` / `find_matching_tag_span` and the source-vs-DOM reconciliation bug class.

## Backlog

- **Phase 3 — retrieval + composition (Sprint 7 candidate)** — `pagekit show <component>` (bundle: fragment + CSS rules + assets in one report), `pagekit assets` (manifest with hashes, dims, orphan detection, semantic aliases for hash-named files; naturally closes the CSS-loaded-orphan gap that `pagekit links` documents), `pagekit preflight` (composes links + seo + a11y + check + doctor into a single go-live gate). **Trigger:** consumer demand for component-level token efficiency, OR a real go-live wanting the one-call gate. **Owner:** chad-pagekit.
- **`[seo].expected_origin` config option** — `pagekit seo` currently flags scheme/host MISMATCH within in-HTML canonicals (catches mixed www/apex declarations). It does NOT catch the case where every page declares the same canonical but the live deploy serves a different host (ettsmart.se's www→apex pattern). Adding `[seo].expected_origin = "https://ettsmart.se"` to fragments.toml lets the check fire on this case. **Trigger:** next consumer hits the same deploy-vs-HTML mismatch.
- **Framework-export profiles** — Webflow + Bootstrap-class profiles. Speculative; needs a third consumer pattern.
- **D2 transforms — second-consumer test** — Sprint 4 D2 + Sprint 5 D2 share rewriting logic; neither exercised against a real consumer that needs depth-relative output (ettsmart.se uses absolute paths intentionally). Validate against file:// preview, sub-path deploy, or non-root static export when one surfaces.
- **Semantic variant naming for `extract --split-variants`** — current scope emits numerical names (`nav-1`, `nav-2`). ettsmart.se demonstrates the manual end-state (`nav-default`, `nav-transparent`) is more readable. Auto-detect from class diffs. **Trigger:** when numerical naming costs a manual rename pass on a real consumer.
- **Migration ergonomics for `--split-variants`** — fresh-run only today; a re-run after plain `extract` is silently no-op'd. **Trigger:** first time a consumer asks for it.

## Real bugs surfaced (not pagekit's responsibility)

Findings from running pagekit's checks against ettsmart.se on 2026-05-06. Surfaced naturally; owner: chad-ettsmart_se.

- **`pagekit links`** — 6 broken internal links (404.html and contact form referencing stale Webflow paths: `_assets/site/css/main.css`, `_assets/site.css`, `_assets/site.js`, `_assets/hubspot/forms/embed/v2.js`)
- **`pagekit seo`** — 3 missing canonicals (`/thank-you/`, `/sv/thank-you/`, `/test/`); 11 missing meta descriptions on SV subpages; 8 multiple-H1 warnings on Webflow templates; 25 missing OG-tag warnings; 1 duplicate-description; canonical www→apex mismatch (declared canonicals all use `www.ettsmart.se` but live deploy serves apex — needs `expected_origin` config to auto-flag, see backlog)
- **`pagekit a11y`** — 4 unlabeled honeypot anti-spam fields (`<input name="website">` without proper hiding) on contact forms

## Blocked

Nothing.
