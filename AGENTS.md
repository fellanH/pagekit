# pagekit

Single Rust binary for managing vanilla HTML + CSS websites. Opinionated about the agent-first stack: no JS frameworks, no template engines, no `node_modules`. Composes the `fragments` text-sync primitive with HTML-specific helpers (page scaffolding, DOM-aware extraction, link integrity).

Persona (long-lived, cd-into).

Worker-tier per `harness/rules/omni/tier-architecture.md`.

## Boot

```bash
cd ~/omni/workspaces/pagekit
# Stage 1: scaffolding only — no code yet
# Stage 2 (next): split init.rs/extract.rs from fragments and build here
# cargo build --release
# cp target/release/pagekit ~/.local/bin/pagekit
```

Stage 1 is documentation + workspace scaffolding only. The binary materializes in Stage 2 when code is split from fragments.

## When in doubt, prompt with

- "What's the next step in the pagekit ↔ fragments split?"
- "Wire pagekit into <site>" (post Stage 2)
- "Audit the pagekit surface; what's still leaking from fragments core?"

## When to use

- Managing a vanilla HTML site (init pages, extract shared blocks, audit health)
- Designing patterns for vanilla-HTML + agent workflows (shared-subset, fragment variants)
- Felix's stack: vanilla HTML + CSS + Rust + SQLite + CF Pages

## When NOT to use

- General text sync across non-HTML files → use `fragments` (the primitive)
- Sites built on JS frameworks (React, Next, Astro) — wrong tool entirely
- Template-engine workflows (Jinja, Handlebars) — pagekit explicitly rejects template syntax

## Charter

This workspace produces a binary that:

- Scaffolds new HTML pages with semantic marker placement (`init`)
- Detects shared DOM blocks across pages and extracts them to `_fragments/` (`extract`)
- Surfaces HTML-specific health checks (link integrity, framework-export anomalies)
- Composes the `fragments` primitive for the underlying sync mechanism

This workspace does NOT:

- Reinvent text-sync primitives — that's `fragments`
- Run a build pipeline, render templates, or generate from schema
- Provide a GUI, CMS, or hosting layer
- Apply variables, conditionals, or any template syntax

## Skills in scope

- `init`, `extract`, HTML-aware `doctor` extensions (Stage 2)
- Framework-export profiles for common Webflow/Bootstrap-class layouts (Stage 3+)
- `lol_html`-based extract rewrite (Stage 3) — replaces `scraper`, eliminates source-vs-DOM reconciliation hacks

## Tools in scope

- `cargo build --release` for builds
- `cargo test` for the integration suite
- `~/.local/bin/pagekit` as the canonical install location
- Depends on `fragments` crate (path = `../fragments`)

## Canon rules especially load-bearing here

- `harness/rules/workflow/build-not-dev.md` — release builds, not dev watchers
- `harness/rules/omni/dispatch-verification.md` — verify by running the binary
- `harness/rules/workflow/valuable-deliverable.md` — deliverable is a working binary
- `harness/rules/workflow/subtract-before-building.md` — opinionated tool ≠ feature kitchen sink

## Rails

- Files always valid HTML at every step. No template syntax, no placeholder leakage.
- Format scope is HTML/CSS — for other formats, defer to `fragments`.
- Composes fragments core; doesn't duplicate it.

## .mcp.json

Empty MCP scope. Rust tool workspace — no MCP servers consumed.

## Origin

Forked from `fragments` workspace 2026-05-06 after recognizing the natural split between general text-sync (`fragments`) and opinionated HTML site management (`pagekit`).

n=2 fragment consumers (felixhellstrom.com integrated, ettsmart.se in progress) both turned out to be vanilla HTML sites; the "general primitive" framing was speculative. Specialization beats false generality. See `specs/pagekit.md` and `../fragments/tasks/arc.md` for full rationale.
