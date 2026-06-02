# pagekit

Single Rust binary for managing vanilla HTML + CSS websites. Opinionated about the agent-first stack: no JS frameworks, no template engines, no `node_modules`. Composes the `fragments` text-sync primitive with HTML-specific helpers (page scaffolding, DOM-aware extraction, link integrity).

Persona (long-lived, cd-into).

Worker-tier per `harness/rules/omni/tier-architecture.md`.

## Boot

```bash
cd ~/omni/products/pagekit
cargo build --release
cp target/release/pagekit ~/.local/bin/pagekit
```

Binary ships. Composes the `fragments` crate (path = `../fragments`) for sync/watch/check/list/doctor/config; adds `init` and `extract` directly.

## When in doubt, prompt with

- "Wire pagekit into <site>"
- "Audit the pagekit surface; what's still leaking from fragments core?"
- "What's the next consumer-driven feature worth building?"

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

Full shipped surface (ground truth: `pagekit --help`):

- **Build/edit:** `init`, `extract` (+ `--split-variants`), `sync`, `watch`, `list`, `config`. `extract` source-rewrite runs on `lol_html` for byte-preserving wrap; cross-page detection uses `scraper`.
- **Bulk edit** (safe-by-default — dry-run unless `--write`): `apply` (parameterized rule file), `mv-asset`, `rename-assets`, `normalize-paths`.
- **Read** (token-efficient): `inventory`, `show`, `assets`.
- **Verify:** `check` (+ `--strict`, `--strict --selector`), `doctor`, `links` (+ `--json`), `seo` (+ `--json`), `a11y` (+ `--json`), `preflight` (single go-live gate).

**Exit-code convention** (suite standard, anchored by published `fragments-sync`): `0` = clean/pass, `1` = findings (broken links, SEO/a11y violations, stale/malformed markers, pending dry-run changes), `2` = tool-internal error (bad args, unreadable root). Every verify command and every safe-by-default mutator follows this — `1` uniformly means "the check found problems", and the distinct `2` means "the tool itself failed" so agents gating on `exit == 1` never confuse the two.

**`--json`** (`links`, `seo`, `a11y`): emits `{check, ok, findings:[{rule, severity, page?, message}]}` instead of prose, where `ok` is a boolean mirroring the exit code (`ok: true` ⟺ exit `0`). Field name and semantics match the `fragments-sync` suite standard. Exit code is unchanged by `--json`. Deserialize instead of regexing stdout.

Still gated (no trigger fired): framework-export profiles for Webflow/Bootstrap-class layouts; Phase 4 candidates in `tasks/arc.md` backlog.

**Connectors** (`connectors/`) — presentation/composition layers OVER the binary's mechanism, kept out of the binary itself (charter "no GUI" + `minimal-core-connectors`). Current: `site-health-audit/` — aggregates `links|seo|a11y --json` into one branded client-facing HTML report via `packages/ui` tokens. See its README.

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
