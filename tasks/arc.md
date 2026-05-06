# pagekit

Binary ships. Sprint 4 closed: D1+D2+D3 landed (commits `597478e`, `a75a4bb`, `634dc36`); watch hookable via fragments v0.6.1 (`6b524a2`). Test suite green: 23 integration + 11 unit, clippy + fmt clean.

## Active arc

**Sprint 4 closed.** All three deliverables shipped + verified end-to-end against ettsmart.se on 2026-05-06: `pagekit list` shows 8 fragments × 35 pages, `check` clean, `check --strict` returns 8/8 uniform across all marker regions, `doctor` no issues. Live site (`https://ettsmart.se/`, `/kista/`, `/sv/`) returns HTTP 200 at all depths. The slug uses semantic variant names (`nav-default`, `nav-transparent`, `nav-sv-default`, `nav-sv-transparent`, `footer`, `footer-sv`, `sub-nav-kista`, `sub-nav-sollentuna`) — equivalent to what `--split-variants` produces with numerical naming. **D2 transforms not exercised** because ettsmart.se's fragments use absolute paths (`/locations`, `/contact`) that resolve from domain root on CF Pages — the depth-collapse use case D2 solves doesn't apply to this slug. Sprint folder: [`sprints/2026-05-variant-aware-sync/`](../sprints/2026-05-variant-aware-sync/README.md).

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
