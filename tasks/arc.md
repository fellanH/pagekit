# pagekit

Stages 1, 2, and 3 done. Binary ships, integration suite green (13/13), `extract` runs on `lol_html` for source rewrite.

## Active arc

**Sprint 4 — variant-aware sync.** Folder: [`sprints/2026-05-variant-aware-sync/`](../sprints/2026-05-variant-aware-sync/README.md). Three deliverables sequenced D1 → D2 → D3: `pagekit check --strict` (visibility), path-relative sync transforms (depth-collapse), `pagekit extract --split-variants` (granular fragment scaffolding). Driven by ettsmart.se's blocked sync (Webflow `w-variant-*` classes inside marker regions, per-depth relative paths) and the n=2 read that every future Webflow-export consumer hits the same wall.

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.
- Stage 3 hybrid: scraper for cross-page detection (multi-pass query), `lol_html` for per-page source rewrite (single-pass streaming). The bridge is sibling-index matching — scraper picks "wrap the Nth `<footer>`", lol_html walks elements by selector and counts to that index. Eliminates `find_first_tag_span` / `find_matching_tag_span` and the source-vs-DOM reconciliation bug class.

## Backlog

- **`pagekit check --strict`** — pre-flight diff of marker regions across all pages by name. Warns when content under the same `<!-- fragment:NAME -->` name varies between pages. Catches a real bug class before sync silently overwrites variant content. Surfaced from ettsmart.se integration where 6 hero pages carry Webflow `w-variant-aa333e92-...` classes on inner `.navbar1_link` elements that the other 10 pages don't — naive sync would have wiped those variants and broken white-on-image text. Vault insight `01KQYRQ3075FWR2B08G5BFCXC6` has the writeup. **Trigger:** dispatch when next consumer surfaces a similar variant-class mismatch, OR when ettsmart.se reaches sync-ready and we want this as the safety gate. **Owner:** chad-pagekit.
- **Stage 3+** — framework-export profiles (Webflow, Bootstrap-class), HTML-validity in `doctor`, link-integrity checks.
- **Cleanup (low priority)** — `ExtractCandidate.tag` field is no longer consumed by `extract.rs`; kept in the schema for one cycle for backward compat. Either drop it (breaking config) or move to `Option<String>` with deprecation note.

## Blocked

Nothing.
