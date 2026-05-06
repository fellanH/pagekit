# pagekit — vanilla HTML site management

## What pagekit is

Opinionated tool for managing vanilla HTML + CSS websites. No JS frameworks, no template engines, no `node_modules`. Built for agents managing static sites.

Composes the `fragments` text-sync primitive with HTML-specific helpers.

## Why it's separate from fragments

`fragments` is the primitive: marker-region sync for any text format with comment-pair syntax. Format-agnostic, narrow scope.

`pagekit` is the opinionated layer: HTML-specific commands (`init`, `extract`), DOM-aware checks, recommended config defaults for static sites, framework-export profiles.

Splitting them gives each a clean story:

- `fragments` stays small and reusable (any text consumer, future or current)
- `pagekit` can specialize without conceptual debt

Empirically: n=2 fragments consumers (felixhellstrom.com integrated, ettsmart.se in progress) are both vanilla-HTML websites. The "general primitive" framing was true but the demand is HTML-shaped. Specialization beats false generality.

## Scope

### In scope

- Page scaffolding (`init`) — DOCTYPE, head, body, semantic marker placement
- Shared-block extraction across pages (`extract`) — DOM-aware via CSS selectors
- HTML-aware health checks (link integrity, framework-export anomalies, validity)
- Recommended `exclude_dirs` defaults for static-site conventions (`backups`, `mockups`, `_audit`, `dist`, `build`, `public`, `_assets`, `css`, `fonts`)
- Framework-export profiles (Webflow, Bootstrap-class layouts, etc.) — Stage 3
- `lol_html`-based extract rewrite — Stage 3 (replaces `scraper`)

### Out of scope

- General text sync across non-HTML formats — use `fragments`
- Variables, partials, conditionals, repeats — same rationale as fragments (every file must be valid HTML at all times)
- Build pipelines, template rendering, schema-driven generation
- GUI, CMS, hosting

## Architecture

`pagekit` depends on the `fragments` crate via path dependency:

```toml
[dependencies]
fragments = { path = "../fragments" }
```

Stage 2 will:

1. Add `lib.rs` to fragments exposing public API: `Config`, `sync_all`, `check_all`, `Fragments::load`, `referenced_fragment_names`, `watch::run`, etc.
2. Move `init.rs` and `extract.rs` from fragments into pagekit
3. Move HTML-specific config (`[[extract.candidates]]`) from fragments::Config to pagekit's Config
4. pagekit wraps fragments' Config in its own TOML schema (or composes via separate sections)
5. pagekit binary's CLI re-exposes the fragments commands (`sync`, `watch`, `check`, `list`, `doctor`) plus its own (`init`, `extract`) — agent-facing UX is one binary, one CLI

## Status

| Stage | What | Status |
|---|---|---|
| 1 | Scaffold workspace, update framing | **In progress** (this commit) |
| 2 | Code split: lib.rs in fragments, move init.rs + extract.rs into pagekit | Pending |
| 3 | Rewrite `extract` on `lol_html` (eliminates source-vs-DOM reconciliation) | Pending |
| 3+ | Framework-export profiles, HTML-validity in doctor, link integrity | Future |

## Origin

Forked from `fragments` 2026-05-06 after second consumer (ettsmart.se) confirmed the HTML-website specialization. The "general primitive that also does HTML" framing was true but the user demand was HTML-shaped. Splitting gives each tool a sharper identity.

Felix's framing: *"the fragments naming is kind of more suitable for a general unopinionated file fragment tool, and we might benefit from a separate workspace for the website specific tool."*
