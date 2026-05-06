# Sprint 6 — correctness checks (Phase 2)

**Thesis:** every site-correctness concern an agent currently solves by reading-then-grepping (broken links, SEO compliance, basic a11y, layout drift) becomes a single pagekit invocation that returns a list of actual issues on this specific site. The check IS the agent-callable artifact; static guides are anti-pattern. Phase 2 of the agent-tooling trajectory.

**Why now:** Sprint 5 inventory primitive validated the query-layer thesis (35 pages, 40ms, 5x reduction, real SEO bug surfaced day-one). The same week's verification surfaced a real ettsmart.se canonical mismatch — exactly the class of finding `pagekit seo` would auto-flag. ettsmart.se and felixhellstrom.com both stand to gain real value the moment these ship; no consumer waiting needed. Per the strategic exchange before Sprint 5 land: Phase 2 follows obviously if Phase 1 thesis holds. It held.

## Done-when (sprint-level)

- D1, D2, D3, D4 below shipped, tested, pushed
- `pagekit links`, `pagekit seo`, `pagekit a11y` against ettsmart.se each complete in <2s; output is grep-friendly; the canonical-mismatch SEO bug surfaces in `pagekit seo` output; no false positives on the slug's intentional patterns
- `pagekit check --strict --selector "..."` works on arbitrary CSS selectors, generalizing Sprint 4 D1 from marker-region scope
- `cargo test` ≥ 38 (30 existing + ~8 new minimum)
- `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean

---

## D1. `pagekit links`

**Artifact:** new `pagekit links` subcommand. Walks every HTML file, extracts every `href` and `src`, classifies, validates, reports. Three classes of finding:

- **Broken internal links** — `href="/foo"` where no `foo.html` or `foo/index.html` exists at the target path
- **Broken anchors** — `href="page#id"` where `id` is not declared on `page`
- **Orphan assets** — files in `_assets/` (or anywhere outside the fragments dir and excludes) referenced by zero pages

**Spec:**

- Classify each `href`/`src` value:
  - External (`http://`, `https://`, `//`, `mailto:`, `tel:`, `data:`, `javascript:`) — skip; do NOT attempt to fetch
  - Anchor-only (`#foo`) — verify `id="foo"` exists on the same page
  - Internal absolute (`/foo`) — resolve to filesystem path under `target_dir`, check existence
  - Internal relative (`../foo`, `./foo`, `foo`) — resolve relative to current page's directory, check existence
  - Cross-page anchor (`/page#foo` or `page#foo`) — both file existence AND `id="foo"` on target
- Asset orphan pass: walk every file under root (excluding fragments dir + configured excludes), compare against the union of all `src` and `href` values from the page walk. Files not referenced are orphans.
- Skip the orphan check for HTML files themselves (those are the pages, not assets)
- Output shape: text, sectioned by finding kind. Exit 0 = clean, exit 2 = issues found.

**Output sample:**

```
broken internal links (2):
  /index.html → /sollentua/contact (404)
  /kista/index.html → /missing-page (404)

broken anchors (1):
  /faq/index.html → #not-an-id (target not declared on page)

orphan assets (3):
  _assets/site/old-hero.avif (no references)
  _assets/site/unused-icon.svg (no references)
  _assets/wf-runtime/legacy.js (no references)
```

**Done-when:**

- Subcommand wired in `src/main.rs` (`Cmd::Links`)
- `src/links.rs` (new) implements the walker
- Tests in `tests/integration.rs`:
  - `links_detects_broken_internal` — page with `/missing` href → exit 2, output mentions the path
  - `links_passes_clean_site` — every href resolves → exit 0
  - `links_skips_external_urls` — external URLs and mailto/tel never appear as broken
  - `links_detects_orphan_asset` — `_assets/orphan.svg` not referenced anywhere → reported

**Verification:** `pagekit links ~/omni/websites/ettsmart.se` completes in <2s. Existing-link findings should be either real bugs (file the slug owner) or known-OK (document false-positive class).

**Worker authority:** decide-and-document on classification rules for edge cases (URL-encoded hrefs, query strings, trailing slashes), exact set of file extensions counted as "asset" vs "page", and exit-code scheme. Halt if ettsmart.se shows >50 false positives — that's a sign the classification logic needs more nuance before shipping.

---

## D2. `pagekit seo`

**Artifact:** new `pagekit seo` subcommand. Per-page SEO health check with multiple rule families:

- **Title rules** — every page has a non-empty `<title>`; titles unique within the site (warn on duplicates); title length 10–70 chars (warn outside range)
- **Description rules** — every page has `<meta name="description">`; description length 50–160 chars; descriptions unique within the site
- **Canonical rules** — every page has a `<link rel="canonical">`; canonical URL is consistent (warn if some pages declare `https://www.example.com` while others declare `https://example.com`); canonical path matches file path (warn if `/foo/index.html` declares canonical `/bar`)
- **OG/Twitter** — every page has `og:title`, `og:description`, `og:type`; warn if missing
- **Hreflang integrity** (when multi-lang detected) — every page that declares `hreflang="en"` should have a sibling page declaring the matching `hreflang="sv"` (or whatever langs are present)
- **JSON-LD** — every `<script type="application/ld+json">` block is valid JSON; report parse errors
- **Heading hierarchy** — each page has exactly one H1; no H2 before the first H1; no H3 before the first H2

**Spec:**

- One pass over pages; collect per-page records
- Run rule families on the records; emit findings
- Output sectioned by rule family; severity levels (error/warn) named per-rule
- Exit 0 if zero errors (warns are OK); exit 2 if any error

**Output sample:**

```
title (2 issues):
  warn: /index.html title is 8 chars (recommend 10-70)
  error: /thank-you/index.html missing <title>

canonical (1 issue):
  error: scheme/host mismatch — 35 pages declare https://www.ettsmart.se/, 0 declare https://ettsmart.se/. Pick one.

heading hierarchy (1 issue):
  warn: /faq/index.html has 2 H1 elements
```

**Done-when:**

- Subcommand wired in `src/main.rs` (`Cmd::Seo`)
- `src/seo.rs` (new) implements the rules
- Tests in `tests/integration.rs`:
  - `seo_flags_missing_title` — page without `<title>` → exit 2, finding mentions it
  - `seo_flags_canonical_host_mismatch` — fixture with mixed `www.` and apex canonicals → finding fires
  - `seo_passes_clean_fixture` — clean site → exit 0
- Verification: `pagekit seo ~/omni/websites/ettsmart.se` reports the canonical www→apex mismatch surfaced 2026-05-06 (already in `tasks/arc.md` "Real bugs surfaced")

**Worker authority:** decide-and-document on rule severity (which are errors vs warns), exact length thresholds for title/description, whether to gate canonical checks behind a config opt-in (some sites genuinely want www. vs apex split). Halt if ettsmart.se's intentional patterns trigger false-positives the rule shape can't accommodate cleanly.

---

## D3. `pagekit a11y`

**Artifact:** new `pagekit a11y` subcommand. Grep-able subset of WCAG checks; explicitly NOT a conformance claim.

- **Missing alt** — `<img>` without `alt` attribute (decorative imgs may use `alt=""` — empty alt is valid, missing alt is not)
- **Unlabeled form fields** — `<input>`, `<textarea>`, `<select>` (excluding `type="submit"`, `type="button"`, `type="hidden"`) without `aria-label` or matching `<label for=...>`
- **Heading order** — H2 before any H1; H3 before any H2
- **Missing `lang` attr** — `<html>` without `lang` attribute
- **Empty links/buttons** — `<a>` and `<button>` with no text content and no `aria-label`
- **Generic link text** — `<a>` with text content = "click here", "here", "read more", "more" (case-insensitive)

**Spec:**

- One pass over pages; collect findings per page
- Output sectioned by rule; per-finding shows page + element location hint (selector or first 50 chars of outer HTML)
- Exit 0 if clean; exit 2 if any finding
- **Honest scope statement** in `--help`: "Subset of WCAG checks doable without rendering. Color contrast, focus-visible styles, and dynamic ARIA semantics are NOT covered. Pass means 'cheap checks pass', not 'WCAG compliant'."

**Done-when:**

- Subcommand wired in `src/main.rs` (`Cmd::A11y`)
- `src/a11y.rs` (new) implements the rules
- Tests in `tests/integration.rs`:
  - `a11y_flags_missing_alt` — `<img src="x">` (no alt) → finding fires; `<img src="x" alt="">` (decorative) → no finding
  - `a11y_flags_unlabeled_input` — `<input type="text">` without label → finding fires
  - `a11y_passes_clean_fixture` — properly-labeled inputs + alt-tagged imgs + lang attr → exit 0

**Worker authority:** decide-and-document on what counts as "labeled" (aria-label, aria-labelledby, label[for], wrapping `<label>`), exact list of generic link-text strings, severity, exit-code scheme. Halt if ettsmart.se's clean-pages-but-known-issues create noisy output that dilutes signal.

---

## D4. Generalized `check --strict`

**Artifact:** extend Sprint 4 D1's `check --strict` from marker-region scope to arbitrary CSS selectors. New `--selector <CSS>` flag.

**Spec:**

- Today: `pagekit check --strict [--name FRAGMENT]` hashes content inside `<!-- fragment:NAME -->...<!-- /fragment:NAME -->` regions, reports variance
- Generalized: `pagekit check --strict --selector "header.nav"` hashes the outer HTML of the matched element on each page, reports variance
- `--name` and `--selector` are mutually exclusive; if both given, error
- If neither given, current behavior (all marker regions)

**Done-when:**

- `Cmd::Check` gains `--selector` flag
- `src/check_strict.rs` extended with selector-mode walker
- Tests in `tests/integration.rs`:
  - `check_strict_selector_uniform_passes` — site with same `<header>` everywhere, `--selector "header"` → exit 0
  - `check_strict_selector_detects_variance` — site with two distinct `<header>` variants → exit 2
  - `check_strict_selector_and_name_conflict` — passing both → error exit 1

**Worker authority:** decide-and-document on selector parse-error handling (helpful message vs anyhow propagate), output format match with the existing marker-region table, and what counts as "no matches" (silent zero or warning).

---

## Sequencing

D1 → D2 → D3 → D4. D1 (links) is the cleanest scope, exercises the per-page walker pattern that D2 and D3 reuse. D4 is a small extension last because it's structurally separate.

Per `epistemic-honesty.md` ÷10-100x: 30 min – 2h per deliverable, sprint total 2-6h agent time.

## Out of scope (named to prevent drift)

- **Color contrast / rendered a11y.** Requires rendering. Phase 4 if a consumer asks; not now.
- **External link reachability.** Don't issue HTTP requests; too slow and flaky. `pagekit links` skips external URLs by design.
- **`pagekit recommend` static text.** Argued against in the strategic exchange; checks > guides.
- **HTML validity.** lol_html and scraper both tolerate non-strict HTML; a strict-validity check is its own scope (HTML5-strict parser). Defer.
- **Image dimension extraction.** Phase 3 `pagekit assets` territory.
- **Refactor to a shared SiteIndex.** Each check walks independently for now; preflight in Phase 3 will motivate the refactor.

## Compounding payoff

Phase 2 turns pagekit from "fragment sync + inventory" into a complete go-live verification toolkit. Phase 3's `preflight` becomes a one-liner that composes all four checks plus existing `check`/`doctor`. Every consumer gains four new agent-callable validations the moment they upgrade pagekit; ettsmart.se gains immediate value (canonical bug auto-flags).

## Origin

Drafted 2026-05-06 by chad-pagekit immediately after Sprint 5 close, on Felix's "let's commit phase 2" trigger. Phase 2 was pre-named in Sprint 5's README and `tasks/arc.md` backlog; this scope ships the named work.
