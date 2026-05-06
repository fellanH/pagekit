# pagekit

Stage 1 scaffolding (this commit). Stage 2 (code split from fragments) is next.

## Active arc

Land Stage 2: extract `lib.rs` from fragments, move `init.rs` + `extract.rs` into pagekit, wire CLI.

## Decisions

- Name: pagekit (single word, descriptive, no name conflicts).
- Composes fragments crate; doesn't duplicate it.
- Opinionated about Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages.
- No template syntax, no variables, no conditionals — same rationale as fragments.
- Single binary CLI re-exposes fragments commands + adds pagekit-specific (`init`, `extract`). Agent UX: one binary, one CLI.

## Backlog

- **Stage 2** — code split: `lib.rs` in fragments exposes core APIs; `init.rs`/`extract.rs` move to pagekit; pagekit binary builds and tests.
- **Stage 3** — rewrite `extract` on `lol_html`. Cleaner source-rewrite, no scraper attribute-normalization hacks, streaming.
- **Stage 3+** — framework-export profiles (Webflow, Bootstrap-class), HTML-validity in doctor, link-integrity checks.

## Blocked

Nothing.
