# pagekit

Binary ships. Sprint 4 closed; cleanup commit (`ce649b3`) drops deprecated `tag` field. Test suite green: 24 integration + 11 unit, clippy + fmt clean.

## Active arc

**Sprint 5 — query layer (Phase 1 of multi-sprint agent-tooling trajectory).** Folder: [`sprints/2026-05-query-layer/`](../sprints/2026-05-query-layer/README.md). Two deliverables: `pagekit inventory` (one-pass index over selectors/classes/hrefs/srcs/metas, grep-friendly output) and `pagekit normalize-paths` (page-wide depth-relativization, generalizing Sprint 4 D2 from fragment-region scope to whole pages). Thesis: Rust query layer over the file tree means agent context cost scales with what changed, not what exists. Phase 2 (correctness checks: links/seo/a11y) and Phase 3 (retrieval: show/assets/preflight) are pre-named but trigger-gated on Sprint 5 thesis validation. Per `subtract-before-building.md`: build foundation first, validate, commit Phase 2 only if measurable token reduction on a real edit task.

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.
- Stage 3 hybrid: scraper for cross-page detection (multi-pass query), `lol_html` for per-page source rewrite (single-pass streaming). The bridge is sibling-index matching — scraper picks "wrap the Nth `<footer>`", lol_html walks elements by selector and counts to that index. Eliminates `find_first_tag_span` / `find_matching_tag_span` and the source-vs-DOM reconciliation bug class.

## Backlog

- **Framework-export profiles** — Webflow + Bootstrap-class profiles; HTML-validity in `doctor`; link-integrity checks. Real consumer needed before building.
- **D2 transforms — second-consumer test** — D2 was sprint scope but not exercised on ettsmart.se (slug uses absolute paths on CF Pages, no depth-collapse needed). Validate against a consumer that needs path-relative output: file:// preview, sub-path deploy, or static export to a non-root mount. **Trigger:** next consumer that needs depth-relative paths.
- **Semantic variant naming for `extract --split-variants`** — current scope emits numerical names (`nav-1`, `nav-2`). ettsmart.se demonstrates the manual end-state (`nav-default`, `nav-transparent`) is more readable. Auto-detect from class diffs (e.g. `class="navbar1_menu transparent"` → `nav-transparent`). **Trigger:** when numerical naming costs a manual rename pass on a real consumer.
- **Migration ergonomics for `--split-variants`** — current scope is fresh-run only. A user who previously ran plain `extract` and then runs `--split-variants` is silently no-op'd on already-marked pages (idempotency check matches sibling markers). If real consumers hit this, add an explicit migration sub-pass that rewrites `<!-- fragment:nav -->` → `<!-- fragment:nav-N -->` based on which variant content sits between the markers. **Trigger:** first time a consumer asks for it.

## Blocked

Nothing.
