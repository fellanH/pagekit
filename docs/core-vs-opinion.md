# pagekit: core vs. opinion

_Read-only review + proposal, 2026-06-02 (Felix first-principles directive). **No refactor performed** — the refactor is RED, this is the map for it. Operationalizes `harness/rules/behavior/minimal-core-connectors.md`: minimal mechanism-only core; opinion lives in swappable connectors, never the core._

## The line

A vanilla-HTML site has no build step — **the repo is the artifact**, served verbatim. The agent's job over that corpus is **navigate + mutate + verify safely**. Everything pagekit does decomposes into two kinds of thing:

- **MECHANISM** — format-shaped but choice-free. _"Parse the corpus into facts." "Replace this marked region." "Rewrite this path without disturbing bytes." "Dry-run unless `--write`."_ Two correct implementations produce identical output; there is nothing to disagree about.
- **OPINION** — the judgement calls that make pagekit _pagekit_. _"A `.sh` file is not a deployable asset." "A title over 70 chars is too long." "These six selectors are worth extracting." "Go-live means check+doctor+links+seo+a11y all pass."_ A different tool would choose differently and still be correct.

The principle: **mechanism belongs in a shared, unopinionated core; opinion stays in the connector.** pagekit, viewed from the suite, _is_ the HTML connector over the `fragments` core. Viewed internally, it still mixes the two and can be split further.

## Three tiers (what is where today)

### Tier 1 — Shared unopinionated core: `fragments` (exists, correct ✓)
The **compose** mechanism: marker-region text-sync for _any_ text format. Detect `<!-- fragment:NAME -->…<!-- /NAME -->` regions, propagate fragment content, report staleness. Format-agnostic (comment syntax is a config concern, not baked in). Lives in its own crate; pagekit consumes the public library surface. **This is the part already done right** — pagekit does not reimplement sync.

### Tier 2 — pagekit's MECHANISM, currently duplicated inline (extract candidate)
Two mechanisms are real, choice-free, and **copy-pasted across command modules today**:

- **site-model** — walk the corpus, parse each page into DOM facts (ids, classes, hrefs/srcs/srcsets, meta, headings, JSON-LD) and build the asset reference graph (HTML + CSS `url()`). `inventory`, `links`, `seo`, `a11y`, `assets` each re-walk and re-parse independently.
- **emit-to-vanilla** — produce/мutate valid vanilla HTML: `lol_html` byte-preserving wrap (extract), path-relativize (normalize/transforms), reference rewrite (mv-asset/rename-assets). The output-correctness mechanism.

Evidence of the duplication (same private fn re-declared in N modules — measured this session):

| helper | modules |
|---|---|
| `display_url` | links, seo, a11y, assets, mv_asset, rename_assets (×6) |
| `resolve_internal` | links, assets, mv_asset, rename_assets (×4) |
| `path_has_dotfile_component` | links, assets, mv_asset, rename_assets (×4) |
| `percent_decode` / `hex_digit` | links, assets, rename_assets (×3) |
| `parse_srcset` | links, assets (×2) |

This is pure mechanism with no per-command opinion, forked six ways. It is the single clearest "extract to a core" target in the codebase.

### Tier 3 — pagekit's OPINION (stays in the connector ✓)
The judgement layer — this is pagekit's identity and **must not migrate into any shared core**:

- **extract candidates** — the six default selectors (`nav`, `footer`, `header`, `.navbar`, `.site-header`, `.site-footer`) and the "what's a shared block" heuristic.
- **verify rule-sets** — `links` orphan whitelists (`PLATFORM_FILES`, non-web extensions, `llms.txt`, `og:image` handling); `seo` thresholds (title ≤70, canonical/OG/hreflang requirements, JSON-LD validity); `a11y`'s chosen WCAG subset.
- **`preflight` composition** — _which_ checks gate go-live and what "pass" means.
- **stack assumptions** — HTML-comment markers, `_fragments/` layout, Cloudflare-Pages platform files, the vanilla-HTML/CSS/Rust/SQLite/CF target. pagekit is _deliberately_ opinionated here (AGENTS.md charter).

## Proposal

**A. Deduplicate Tier-2 mechanism into an internal `sitemodel` module (the real win).**
One corpus walk → one parsed `SiteModel` (pages + DOM facts + reference graph) that `inventory`/`links`/`seo`/`a11y`/`assets` all read. Collapse the ×6/×4/×3 helper forks (`display_url`, `resolve_internal`, `percent_decode`, …) into one home. Pure refactor: identical output, less surface, one place for path-resolution bugs to be fixed once. **Internal module, not a new crate** — see C.

**B. Make Tier-3 opinion swappable as data (`[policy]` config), not constants.**
The whitelists/thresholds are currently hardcoded Rust → every false-positive fix is edit-recompile-reship (CAND-B, `PLATFORM_FILES`, `llms.txt`, `og:image` — ~4 recent). Externalize them to a `[policy]` block in `fragments.toml` with the current values as defaults. This is the minimal-core principle applied _within_ the connector: the binary becomes mechanism + a default policy; the opinion becomes data a consumer can override. (Already logged in `tasks/arc.md` Decisions, trigger-gated: do it on the _next_ whitelist edit, not speculatively.)

**C. Do NOT promote `sitemodel` to a shared crate yet.**
A "site model" is HTML-specific — it is _not_ shared with `fragments`' format-agnostic generality, and pagekit is its only consumer today. Per `reactive-over-prescriptive`, a sibling crate is speculative until a **second** HTML consumer exists. Keep it an internal module; promote only on that trigger.

**D. Keep migration-verification opinions out of pagekit (cross-connector boundary).**
Asset-parity / visual-diff-vs-source (migration-friction relay) are a _different domain's_ opinion — owned by the clone/migration connector, which composes pagekit's `assets` mechanism the way pagekit composes `fragments`. Not a pagekit verify command. (Logged in arc.)

## The fragments → pagekit seam (how the connector attaches to the core)

Clean and worth preserving as the template for "connector over core":

- **Config flatten** — `pagekit::Config` embeds `fragments::Config` as `core` via `#[serde(flatten)]`. Users see one flat `fragments.toml`; pagekit layers `[extract]` / `[transforms]` opinion on top of core mechanism fields.
- **Library calls** — 13 call sites, all through the public surface: `sync_all_with`, `check_all_with`, `watch::run_with`, `doctor::run_doctor`, `list::list_fragments`. pagekit never reaches into fragments internals.
- **`SyncHook` extension point** — the load-bearing seam. fragments exposes a mechanism hook; pagekit injects HTML path-rewrite transforms into sync **without fragments knowing HTML exists**. The core stays unopinionated; the connector supplies the opinion (which attrs, what `path_root`). This is exactly the core/connector contract done right.
- **Known core gap (flagged upstream, not pagekit's to fix):** `--json` on `check`/`doctor` needs `fragments` to expose _structured_ returns from `run_doctor`/check (it now returns counts/prose). A mechanism gap in the core the connector cannot fill alone — coordinate with the fragments owner.

## Summary

| | What | Where it lives | Action |
|---|---|---|---|
| **Core (shared, unopinionated)** | text-region compose | `fragments` crate | ✓ correct, keep composing |
| **Mechanism (pagekit-internal)** | site-model parse + asset graph; emit-to-vanilla rewrite | duplicated across 5–6 modules | **extract to internal `sitemodel`** (A) |
| **Opinion (connector)** | extract candidates, verify rule-sets, preflight gate, stack assumptions | hardcoded in pagekit | keep; make swappable via `[policy]` data (B) |

The macro-architecture is already right: `fragments` = mechanism core, pagekit = HTML-opinion connector, joined by a clean `SyncHook`/flatten seam. The available improvements are both _inside_ the connector — dedup the forked mechanism (A), and turn hardcoded opinion into swappable data (B) — both trigger-gated, neither done here.
