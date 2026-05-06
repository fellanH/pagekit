# Sprint 5 — query layer (Phase 1)

**Thesis:** turn pagekit into a query layer over the file tree so agents editing vanilla HTML sites pay token cost proportional to *what changed*, not *what exists*. Phase 1 of a multi-sprint trajectory; ships the foundation primitives that subsequent check/bundle/orchestrate phases depend on.

**Why now:** the Sprint 4 verification on ettsmart.se confirmed 35 × 50KB pages cost ~1.7MB to grep across when answering "where does X appear?" Felix's vault insight `01KQYRQ3075FWR2B08G5BFCXC6` and the felixhellstrom.com prose-normalization sweep both surfaced the same friction class: agents pay full-page read amplification for inventory-style questions. Rust's lol_html + sub-10ms startup makes a one-pass index cheap; this sprint operationalizes that.

**Trajectory (named here so future sprints inherit context, NOT scoped here):**

- Phase 2 (Sprint 6 candidate): correctness checks — `links`, `seo`, `a11y`, generalized `check --strict`. Each query the index built in Phase 1.
- Phase 3 (Sprint 7 candidate): retrieval + composition — `show <component>` (bundle assembly), `assets` (manifest), `preflight` (orchestrator).
- Phase 4 (selective): `retarget`, schema-as-data, sitemap generation. Trigger-gated on real consumer demand.

## Done-when (sprint-level)

- D1 + D2 below shipped, tested, pushed
- `pagekit inventory` against ettsmart.se completes in <1s; output is grep-friendly and answers at least three concrete agent queries (every page using class X, every page linking to URL Y, every page with title Z) in under 5KB of context per query
- `pagekit normalize-paths` on a synthetic 3-depth fixture produces correct relative paths per file; idempotent re-run is a no-op
- `cargo test` ≥ 27 (24 existing + ~3 new minimum)
- `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean
- ettsmart.se re-verifies clean with the new binary (existing checks still pass)

---

## D1. `pagekit inventory`

**Artifact:** `pagekit inventory` subcommand. One walk of every HTML file in scope produces a tab-separated stream on stdout: `<page>\t<kind>\t<value>` per line. Grep-able, sort-able, machine-parseable. Default to stdout; optional `--save <path>` writes to file.

**Spec:**

- Walk every HTML file via existing `collect_html_files` helper
- For each page, use lol_html element handlers to capture (one pass per file):
  - **classes** — every `class="..."` attribute, split on whitespace, deduped per page
  - **ids** — every `id="..."`
  - **hrefs** — `<a href>`, `<link href>` (extract attribute value)
  - **srcs** — `<img src>`, `<script src>`, `<source src>`, `<iframe src>`
  - **title** — text content of `<title>`
  - **meta** — `<meta name="X" content="Y">` and `<meta property="X" content="Y">` and `<link rel="canonical" href="Y">` (emitted as kind=`meta`, value=`name=value`)
  - **headings** — text content of `<h1>`, `<h2>`, `<h3>` (kind=`h1` etc.)
  - **schema** — `<script type="application/ld+json">` blocks; emit one line per block with kind=`schema-type` and value = the top-level `@type` field if parseable, else `(unparseable)`
- Output kinds: `class`, `id`, `href`, `src`, `title`, `meta`, `h1`, `h2`, `h3`, `schema-type`
- Page paths are emitted relative to `target_dir` (or root if no `target_dir` configured), with leading `/` so they look like URLs
- Deterministic order: pages alphabetical; within each page, kind alphabetical, value alphabetical

**Output shape (sample):**

```
/index.html	class	navbar1_component
/index.html	class	w-variant-aa333e92-...
/index.html	h1	Smart hotels in Stockholm
/index.html	href	/contact
/index.html	href	/locations
/index.html	href	mailto:info@ettsmart.se
/index.html	meta	canonical=https://ettsmart.se/
/index.html	meta	description=Smart hotels in Sweden's capital
/index.html	meta	og:title=Ett Smart
/index.html	schema-type	Hotel
/index.html	src	/_assets/site/abc.avif
/index.html	title	Ett Smart - Hotels in Sollentuna and Kista
```

**Done-when:**

- Subcommand wired in `src/main.rs` (new `Cmd::Inventory { save: Option<PathBuf> }`)
- `src/inventory.rs` (new file) implements the walker
- Tests in `tests/integration.rs`:
  - `inventory_emits_class_lines` — page with `<div class="foo bar">`, output contains both kinds, deduped
  - `inventory_emits_meta_and_canonical` — page with title, meta description, canonical link, og:title; all four surface as kind=meta or kind=title
  - `inventory_grep_pipeline` — multi-page fixture, `pagekit inventory | grep -F class\\tnavbar` returns expected page subset

**Verification:** `pagekit inventory ~/omni/websites/ettsmart.se` completes in <1s, output is non-empty, three real queries return correct page lists:
- `pagekit inventory | awk -F'\t' '$2=="class" && $3=="navbar1_link"' | cut -f1 | sort -u` lists all pages using the nav-link class
- `pagekit inventory | awk -F'\t' '$2=="href" && $3 ~ /^http/' | cut -f3 | sort -u` lists all external links
- `pagekit inventory | awk -F'\t' '$2=="meta" && $3 ~ /^canonical=/'` lists all canonical declarations

**Worker authority:** decide-and-document on output format details (TSV vs structured), ordering (alphabetical chosen above for determinism — keep), schema-type extraction (best-effort, mark unparseable rather than fail), and exact set of element handlers wired. Halt if lol_html cannot capture text content of `<title>`/`<h1>` cleanly (then surface the limit and propose a fallback).

---

## D2. `pagekit normalize-paths`

**Artifact:** new `pagekit normalize-paths` subcommand that walks every HTML file in scope and rewrites absolute paths (`href="/..."`, `src="/..."`) to be relative to the file's depth. Idempotent. Composes the same DepthRelativizer logic shipped in Sprint 4 D2, but applied page-wide instead of fragment-region-only.

**Spec:**

- Reuse the rewrite logic from `src/transforms.rs` (DepthRelativizer); factor out the shared core if needed
- New `src/normalize.rs` (or similar) implements `pub fn normalize_paths(root, config) -> Result<usize>`
- For each HTML file: parse via lol_html, rewrite every `<a href>`, `<link href>`, `<img src>`, `<script src>`, `<source src>`, `<iframe src>` whose value starts with `transforms.path_root` (default `/` when normalize-paths is invoked but config doesn't specify)
- Skip values matching `^(https?:|mailto:|tel:|data:|#)` (external/internal-anchor)
- Skip values already relative (no leading `/`, or starts with `./` or `../`)
- Returns count of modified files; print summary to stdout
- **Default behavior when no `[transforms]` block exists:** normalize-paths assumes `path_root="/"` and rewrites all root-absolute paths. Different from sync, which is no-op without `[transforms]`. The user invoking `normalize-paths` is opting in by running the command.
- **Idempotent:** a second run produces no diff (relative paths skip the rewrite predicate).

**Done-when:**

- Subcommand wired in `src/main.rs` (`Cmd::NormalizePaths`)
- Implementation reuses `DepthRelativizer` core (factor out a `rewrite_value` free function if not already)
- Tests in `tests/integration.rs`:
  - `normalize_paths_rewrites_per_depth` — 3-depth fixture (`/index.html`, `/foo/index.html`, `/foo/bar/index.html`) with absolute paths; assert correct `..`-stack per file
  - `normalize_paths_idempotent` — second run produces zero modifications
  - `normalize_paths_skips_externals` — `http://`, `mailto:`, `tel:`, `#` left alone

**Verification:** synthetic fixture verifies; on ettsmart.se (which uses absolute paths intentionally for CF Pages root deploy), running `normalize-paths` would rewrite many things — that's expected. The verification is on the synthetic fixture; we don't run it against ettsmart.se because the slug's deploy model relies on absolute paths.

**Worker authority:** decide-and-document on whether `[transforms].path_root` defaulting differs between sync (currently no-op without config) and normalize-paths (opt-in by invocation). Halt if the DepthRelativizer logic can't be cleanly applied to whole-page rewriting without breaking the SyncHook abstraction.

---

## Sequencing

D1 first, then D2. Inventory is the foundation under all later phases; normalize-paths is a small composable addition that exercises the lol_html-rewrite-page-wide pattern subsequent checks will need.

Per `epistemic-honesty.md` ÷10-100x: 30 min – 2h per deliverable, full sprint 1-3h agent time.

## Out of scope (named to prevent drift)

- **Filters and queries built into the binary.** `pagekit inventory` outputs a stream; agents grep. If a filter pattern proves load-bearing across consumers, promote later. Speculation now would over-build.
- **JSON output mode.** Text/TSV is grep-friendly and machine-parseable enough. JSON is easy to add when a consumer needs it; not yet.
- **CSS-rule extraction in inventory.** D3 of Phase 3 (`show <component>`) is the right home for that; doing it here couples the index pass to CSS parsing without a payoff yet.
- **Color contrast / rendered a11y checks.** Phase 2 a11y; needs rendering, not in scope here.
- **`pagekit recommend` static-text command.** Argued against in the prior strategic discussion; checks > guides.

## Compounding payoff

The inventory primitive makes Phase 2 checks (links/seo/a11y) cheap to implement: each check is a query against the index plus per-page meta extraction, not another full-tree walk. Phase 3 `show <component>` reuses inventory for "what classes does this fragment use." Every subsequent phase pays the index walk once and queries thereafter.

## Origin

Drafted 2026-05-06 by chad-pagekit after Felix authorized the multi-phase plan in the strategic exchange post-Sprint 4 verification. Sprint 5 ships Phase 1; Sprints 6-7 are pre-named for context but trigger-gated on Sprint 5 thesis validation (measurable token reduction on a real edit task).
