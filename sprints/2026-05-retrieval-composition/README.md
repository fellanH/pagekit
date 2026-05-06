# Sprint 7 — retrieval + composition (Phase 3)

**Thesis:** close the agent-tooling trajectory — Phase 1 indexed, Phase 2 validated, Phase 3 delivers the bundles agents read and the orchestrator that gates deploys. Ships three deliverables that compose what came before.

**Why now:** Phase 2 closed with `pagekit links` documenting a CSS-loaded-orphan gap (font files referenced from `@font-face`, image variants from `background-image: url()`). The right closer is a primitive that walks BOTH HTML and CSS reference graphs. Once we have that primitive, the bundle assembly (`show`) and orchestrator (`preflight`) become small composing wrappers.

## Done-when (sprint-level)

- D1, D2, D3 below shipped, tested, pushed
- `pagekit assets` against ettsmart.se completes in <1s; closes the CSS-orphan gap (the 3 font/icon orphans currently flagged by `pagekit links` no longer appear, OR appear with their CSS reference path correctly attributed)
- `pagekit show nav` against ettsmart.se returns the fragment HTML + classes used + assets referenced as a single ~5KB report
- `pagekit preflight` against ettsmart.se runs all five checks (sync, doctor, links, seo, a11y) and reports per-check pass/fail with aggregated exit code
- `cargo test` ≥ 56 (50 existing + ~6 new minimum)
- `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean

---

## D1. `pagekit assets`

**Artifact:** new `pagekit assets` subcommand. Walks every HTML file for hrefs/srcs/srcsets, walks every CSS file for `url(...)` references, walks the filesystem for actual asset files. Emits a TSV manifest: per-asset hash, byte count, every referencing page or stylesheet, orphan flag.

**Spec:**

- HTML reference extraction reuses logic from `src/links.rs` (`href`, `src`, `srcset`)
- CSS reference extraction: walk `**/*.css` (excluding fragments_dir, exclude_dirs, dotfile components); for each file, scan `url(...)` patterns. Handle quoted (`url("x")`, `url('x')`) and unquoted (`url(x)`) forms. Skip `data:` URLs.
- For each asset file under scan_root (excluding HTML, markdown, dotfile components, platform skiplist from `pagekit links`):
  - Compute content hash (FNV-32 truncated — same `hash8` used by `check_strict`; sufficient for change-detection and dedup)
  - Capture file size in bytes
  - Capture MIME-like type from extension (best-effort: `.avif`/`.png`/`.jpg`/`.svg`/`.css`/`.js`/`.woff2`/etc.)
  - List every referencing page (HTML refs)
  - List every referencing stylesheet (CSS refs)
- Output TSV: `<asset>\t<kind>\t<value>` where kinds are `hash`, `bytes`, `type`, `referenced-by` (per page), `referenced-from-css` (per stylesheet), `orphan` (emitted only when zero refs)
- Default to stdout; `--save <path>` writes to file with summary line
- Final summary line: `pagekit: N asset(s), M orphan(s), <total-bytes> bytes`

**Output sample:**

```
/_assets/site.css	hash	a1b2c3d4
/_assets/site.css	bytes	34521
/_assets/site.css	type	text/css
/_assets/site.css	referenced-by	/index.html
/_assets/site.css	referenced-by	/kista/index.html
/_assets/site/futuraptbold.otf	hash	f0e1d2c3
/_assets/site/futuraptbold.otf	bytes	142567
/_assets/site/futuraptbold.otf	type	font/otf
/_assets/site/futuraptbold.otf	referenced-from-css	/_assets/site.css
/_assets/site/legacy-icon.svg	hash	deadbeef
/_assets/site/legacy-icon.svg	bytes	1024
/_assets/site/legacy-icon.svg	type	image/svg+xml
/_assets/site/legacy-icon.svg	orphan	yes
```

**Done-when:**
- Subcommand wired in `src/main.rs` (`Cmd::Assets { save: Option<PathBuf> }`)
- `src/assets.rs` (new) implements walker
- `src/css_refs.rs` (new) implements `extract_url_refs(css: &str) -> Vec<String>` for `url(...)` extraction
- Tests in `tests/integration.rs`:
  - `assets_lists_referenced_files` — page + asset → asset appears with `referenced-by` line
  - `assets_extracts_css_url_refs` — `_assets/site.css` with `@font-face src: url(/foo.woff2)` → `/foo.woff2` shows `referenced-from-css`
  - `assets_flags_true_orphan` — file referenced nowhere → `orphan` line
  - `assets_skips_data_urls` — `url(data:image/png;base64,xx)` not captured

**Verification:** `pagekit assets ~/omni/websites/ettsmart.se` completes in <1s. The 3 CSS-loaded orphans currently flagged by `pagekit links` (`futuraptbold.otf`, `futuraptbook.otf`, `custom-checkbox-checkmark.svg`) appear in the manifest with `referenced-from-css` lines pointing at the stylesheet, NOT as orphans.

**Worker authority:** decide-and-document on hash function (reuse hash8 vs upgrade to sha-256), output format (TSV vs JSON), platform-skiplist semantics for asset orphans, and exit code (always 0, or non-zero on orphans). Halt if CSS parsing reveals patterns the regex-style extractor can't handle cleanly (then surface options).

---

## D2. `pagekit show <name>`

**Artifact:** new `pagekit show <name>` subcommand. Bundle assembly: looks up `_fragments/<name>.html`, prints the fragment content + classes used + assets referenced as a single structured report. The agent's "give me everything about this component" call.

**Spec:**

- Argument: fragment name (no extension; `nav` resolves to `<fragments_dir>/nav.html`)
- Read fragment file
- Walk fragment HTML for: `[class]` attribute tokens (deduped), `[href]`, `[src]`, `[srcset]` values
- Output: structured text report (NOT TSV — this is for human/agent reading)
  - Section: `# fragment: <name>`
  - Section: HTML content (as-is)
  - Section: `## classes (N unique)`: alphabetical list
  - Section: `## referenced assets (N unique)`: list with hash + bytes IF the asset exists at the resolved path (relative to project root or fragment dir)
- Exit 0 on success; exit 1 if fragment not found

**Output sample:**

```
# fragment: nav

## HTML

<nav role="navigation" class="navbar1_menu w-nav-menu">
  <a href="/" class="navbar1_link w-nav-link">Home</a>
  <a href="/locations" class="navbar1_link w-nav-link">Locations</a>
  <a href="/contact" class="button">Booking Request</a>
</nav>

## classes (4)

button
navbar1_link
navbar1_menu
w-nav-link
w-nav-menu

## referenced assets (0)

(none)

## referenced URLs (3)

/
/contact
/locations
```

**Done-when:**
- Subcommand wired in `src/main.rs` (`Cmd::Show { name: String }`)
- `src/show.rs` (new) implements lookup + report
- Tests in `tests/integration.rs`:
  - `show_returns_fragment_content` — fragment with classes + hrefs → report contains all three sections
  - `show_missing_fragment_errors` — name with no matching file → exit 1, helpful error
  - `show_dedupes_classes_alphabetical` — fragment with class repeated → one entry, sorted

**Verification:** `pagekit show nav` against ettsmart.se returns the fragment + class list + URL list in a single command, ~5KB output (vs the agent loading `_fragments/nav.html` directly + grepping for classes).

**Worker authority:** decide-and-document on output format (markdown-ish vs plain text), whether to include CSS-rule lookups (DEFER to follow-up; out of scope for MVP per `subtract-before-building.md`), behavior on missing fragment (error vs empty success). Halt if name resolution becomes ambiguous (multiple fragment dirs, etc.).

---

## D3. `pagekit preflight`

**Artifact:** new `pagekit preflight` subcommand. Composes all five checks (sync `check`, `doctor`, `links`, `seo`, `a11y`) in sequence, reports per-check pass/fail, exits non-zero if any check fails.

**Spec:**

- Run each check internally (call the existing `run_*` functions, not subprocess them)
- For each: capture exit code (0 / 2 / other)
- Print summary table:
  ```
  pagekit preflight:
    check     PASS
    doctor    PASS
    links     FAIL  (6 broken internal links, 3 orphan assets)
    seo       FAIL  (3 errors, 17 warns)
    a11y      FAIL  (4 issues)

  preflight: 3 of 5 checks failing
  ```
- Final exit: 0 if all pass, 2 if any fail
- Per-check output goes to stderr or is suppressed by default; `--verbose` flag prints each check's full output inline

**Done-when:**
- Subcommand wired in `src/main.rs` (`Cmd::Preflight { verbose: bool }`)
- `src/preflight.rs` (new) implements orchestration
- Tests in `tests/integration.rs`:
  - `preflight_passes_clean_site` — clean fixture with all checks green → exit 0
  - `preflight_aggregates_failures` — fixture with one broken link → exit 2, summary names `links` as failing

**Verification:** `pagekit preflight ~/omni/websites/ettsmart.se` runs all five checks in <2s, surfaces the same findings each individual check does, exits 2 (because links/seo/a11y all flag known-real bugs on ettsmart.se).

**Worker authority:** decide-and-document on which checks are mandatory vs optional (e.g., `check --strict` is currently OFF unless the user opts in; should preflight include it?), output format, suppression behavior, and the verbose flag's exact semantics. Halt if check composition needs internal refactoring beyond the sprint scope.

---

## Sequencing

D1 → D2 → D3. D1 establishes the asset reference graph that D2 lookups can use; D3 composes everything. D2 and D3 are both small after D1 lands.

Per `epistemic-honesty.md` ÷10-100x: 30 min – 2h per deliverable, sprint total 2-5h agent time.

## Out of scope (named to prevent drift)

- **Image dimension extraction.** Adds dep complexity (`image` or `imagesize` crate) for one feature. Defer to a focused Phase 4 follow-up if a consumer needs it for LCP/responsive-image work.
- **CSS-rule extraction in `show`.** Bundle assembly's "show me the rules for these classes" requires real CSS parsing (or lenient regex with edge cases). Defer; the MVP `show` outputs classes-as-names which the agent can grep against the stylesheet.
- **Semantic asset aliases for hash-named files** (`abc123_favicon.svg` → `favicon.svg`). Felix-stack-specific Webflow problem; defer until consumer demand.
- **Asset deduplication report.** Same hash on multiple files = byte-identical duplicates. Worth flagging but adds output complexity; defer.
- **Hash upgrade from FNV-32 to sha-256.** Collision probability on site-scale (N ≈ 200 assets) is 1/2^32 which is negligible; FNV-32 is sufficient for the change-detection use case.

## Compounding payoff

Phase 3 closes the agent-tooling trajectory. The full pagekit surface for vanilla-HTML site management:
- **Build:** `init`, `extract`, `extract --split-variants`, `sync`, `watch`, `normalize-paths`
- **Read (token-efficient):** `inventory`, `show`, `assets`, `list`
- **Verify:** `check`, `check --strict`, `check --strict --selector`, `doctor`, `links`, `seo`, `a11y`, `preflight`
- **Config:** `config`

Every consumer adopting pagekit gets the full kit. Phase 4 (selective: framework-export profiles, semantic variant naming, expected_origin config, image dims) becomes purely demand-driven.

## Origin

Drafted 2026-05-06 by chad-pagekit on Felix's "commit phase 3" trigger. Phase 3 was named in Sprint 5 + Sprint 6 backlogs; this scope ships the named work.
