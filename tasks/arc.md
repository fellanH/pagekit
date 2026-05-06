# pagekit

Binary ships. Sprint 4 D1+D2+D3 landed (commits `597478e`, `a75a4bb`, `634dc36`). Test suite green: 23 integration + 11 unit, clippy + fmt clean.

## Active arc

**Sprint 4 — variant-aware sync. Code complete; verification gate next.** Sprint folder: [`sprints/2026-05-variant-aware-sync/`](../sprints/2026-05-variant-aware-sync/README.md). All three deliverables shipped: `pagekit check --strict` (D1, variance visibility), path-relative sync via `DepthRelativizer` hook (D2, depth-collapse), `pagekit extract --split-variants` (D3, granular fragment scaffolding). Sprint done-when remaining: ettsmart.se end-to-end run using the new toolchain — `extract --split-variants` against the slug, sync into pages, visual parity check against the live deployable. Driven by ettsmart.se's blocked sync (Webflow `w-variant-*` classes inside marker regions, per-depth relative paths).

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.
- Stage 3 hybrid: scraper for cross-page detection (multi-pass query), `lol_html` for per-page source rewrite (single-pass streaming). The bridge is sibling-index matching — scraper picks "wrap the Nth `<footer>`", lol_html walks elements by selector and counts to that index. Eliminates `find_first_tag_span` / `find_matching_tag_span` and the source-vs-DOM reconciliation bug class.

## Backlog

- **ettsmart.se sprint-4 end-to-end** — run the new toolchain against the slug: `pagekit check --strict` for baseline variance, `pagekit extract --split-variants` to scaffold variant-aware fragments, sync into pages with `[transforms].path_root = "/"`, visual parity check against the live deployable. **Trigger:** sprint done-when gate. **Owner:** chad-pagekit (or chad-ettsmart_se with relay).
- **Framework-export profiles** — Webflow + Bootstrap-class profiles; HTML-validity in `doctor`; link-integrity checks. Real consumer needed before building.
- **Cleanup (low priority)** — `ExtractCandidate.tag` field is no longer consumed by `extract.rs`; kept in the schema for one cycle for backward compat. Either drop it (breaking config) or move to `Option<String>` with deprecation note.
- **Migration ergonomics for `--split-variants`** — current scope is fresh-run only. A user who previously ran plain `extract` and then runs `--split-variants` is silently no-op'd on already-marked pages (idempotency check matches sibling markers). If real consumers hit this, add an explicit migration sub-pass that rewrites `<!-- fragment:nav -->` → `<!-- fragment:nav-N -->` based on which variant content sits between the markers. **Trigger:** first time a consumer asks for it.

## Blocked

Nothing.
