# pagekit

Binary ships. Sprints 4-5 closed. Test suite green: 30 integration + 15 unit, clippy + fmt clean.

## Active arc

**Sprint 5 closed (Phase 1 of agent-tooling trajectory).** Both deliverables shipped: `pagekit inventory` (commit `cdfd2e7`, one-pass index over classes/ids/hrefs/srcs/title/meta/headings/JSON-LD types, grep-friendly output) and `pagekit normalize-paths` (commit `efc39a7`, page-wide depth-relativization reusing DepthRelativizer from Sprint 4 D2). Thesis validated against ettsmart.se: 35 pages indexed in 40ms, 326KB inventory file (~5x reduction vs raw 1.7MB), three benchmark queries (pages-using-class, external-hrefs, canonical-declarations) all return correctly with sub-5KB output per query. Real-world side-effect: the canonical-query exposed a www→apex SEO bug on the live site — surfaced naturally by the inventory primitive working as designed. Sprint folder: [`sprints/2026-05-query-layer/`](../sprints/2026-05-query-layer/README.md).

**Phase 2 candidate (Sprint 6):** correctness checks — `links`, `seo`, `a11y`, generalized `check --strict`. Each queries the inventory primitive instead of re-walking the tree. Trigger-gated on next consumer demand or Felix prioritization; the thesis is validated and the path is clear, but per `subtract-before-building.md` no commitment until pulled.

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.
- Stage 3 hybrid: scraper for cross-page detection (multi-pass query), `lol_html` for per-page source rewrite (single-pass streaming). The bridge is sibling-index matching — scraper picks "wrap the Nth `<footer>`", lol_html walks elements by selector and counts to that index. Eliminates `find_first_tag_span` / `find_matching_tag_span` and the source-vs-DOM reconciliation bug class.

## Backlog

- **Phase 2 — correctness checks (Sprint 6 candidate)** — `pagekit links` (dead/broken hrefs, orphan asset detection), `pagekit seo` (missing/duplicate titles + meta descriptions + canonicals + OG/Twitter cards, hreflang triplet integrity, malformed JSON-LD, heading hierarchy), `pagekit a11y` (grep-able subset: missing alts, unlabeled form fields, heading order, missing lang attr), generalized `check --strict` for arbitrary selectors. Each queries inventory output instead of re-walking the tree. **Trigger:** Felix prioritization or next consumer surfacing real friction. **Owner:** chad-pagekit.
- **Phase 3 — retrieval + composition (Sprint 7 candidate)** — `pagekit show <component>` (bundle: fragment + CSS rules + assets in one report), `pagekit assets` (manifest with hashes, dims, orphan detection, semantic aliases for hash-named files), `pagekit preflight` (composes all checks into a single go-live gate). **Trigger:** Phase 2 ships and a consumer asks for component-level token efficiency. **Owner:** chad-pagekit.
- **Framework-export profiles** — Webflow + Bootstrap-class profiles. Speculative; needs a third consumer pattern.
- **D2 transforms — second-consumer test** — Sprint 4 D2 + Sprint 5 D2 share rewriting logic; neither exercised against a real consumer that needs depth-relative output (ettsmart.se uses absolute paths intentionally). Validate against file:// preview, sub-path deploy, or non-root static export when one surfaces.
- **Semantic variant naming for `extract --split-variants`** — current scope emits numerical names (`nav-1`, `nav-2`). ettsmart.se demonstrates the manual end-state (`nav-default`, `nav-transparent`) is more readable. Auto-detect from class diffs. **Trigger:** when numerical naming costs a manual rename pass on a real consumer.
- **Migration ergonomics for `--split-variants`** — fresh-run only today; a re-run after plain `extract` is silently no-op'd. **Trigger:** first time a consumer asks for it.

## Real bugs surfaced (not pagekit's responsibility)

- **ettsmart.se canonical mismatch** — every page declares `canonical=https://www.ettsmart.se/...` but the live site redirects www→apex. Surfaced 2026-05-06 by `pagekit inventory` running the canonical query on the slug. Owner: chad-ettsmart_se. (Exactly the kind of finding Phase 2's `pagekit seo` would auto-flag.)

## Blocked

Nothing.
