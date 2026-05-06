# Sprint 4 — variant-aware sync

**Thesis:** turn pagekit from "single fragment per name, dumb byte sync" into a tool that handles the variant patterns real Webflow/Bootstrap exports actually use, so granular fragments don't multiply per page-depth and silent-overwrite surprises stop happening.

**Why now:** ettsmart.se's pagekit wire-up landed config-only (commit `0eedf95` in that slug) precisely because single-fragment-per-name would have wiped Webflow `w-variant-aa333e92-…` classes on 6 hero pages and broken white-on-image text. Same friction will hit every future Webflow-export consumer; this sprint pre-empts it. Vault: `01KQYRQ3075FWR2B08G5BFCXC6`.

## Done-when (sprint-level)

- D1, D2, D3 below all shipped, tested, pushed
- ettsmart.se's `_fragments/` ships from a `pagekit sync` run with no manual sed pass; visual parity verified against the live deployable
- felixhellstrom.com runs `pagekit check --strict` clean
- `cargo test` ≥ 18 passing (13 existing + ~5 new)
- `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean

---

## D1. `pagekit check --strict`

**Artifact:** `pagekit check --strict [--name <fragment-name>]` subcommand. Prints variance table; exit 0 when all marker regions across pages match by content-hash for each name, exit 2 on variance, exit 1 on parser/IO error.

**Spec:**

- Walk every HTML file in scope (existing `collect_html_files` helper)
- For each page, use lol_html to find every `<!-- {prefix}:NAME -->...<!-- /{prefix}:NAME -->` region; capture the inner bytes verbatim
- Hash inner bytes per `(page, name)` pair (sha-256 truncated to 8 hex chars is fine — collision risk on small N is negligible)
- Group by name; report `(name, distinct_hash_count, page_groups)`
- For names with `distinct_hash_count > 1`, classify the variance (MVP: just "varies"; follow-up can add `variant-class` / `path-depth` / `active-state` classifiers)

**Output shape (text-table):**

```
fragment       pages  variants  status
nav            16     2         ⚠ varies
footer         16     1         ✓ uniform
sub-nav-kista  3      1         ✓ uniform

⚠ nav has 2 variants:
  hash a3f7c1e9 (10 pages): /index.html, /faq.html, /contact.html, /kista/family.html, …
  hash b2d4e8a1 (6 pages):  /sollentuna/index.html, /kista/index.html, /sv/sollentuna/index.html, …
```

**Done-when:**
- Subcommand wired in `src/main.rs` (Cmd::Check gains an `--strict` flag, OR a new Cmd::CheckStrict — pick one in the impl)
- `src/check_strict.rs` (new file) implements the walker
- Tests in `tests/integration.rs`:
  - `check_strict_uniform_passes` — 3 pages, identical fragment regions, exit 0
  - `check_strict_detects_variance` — 4 pages, 2 distinct nav contents, exit 2 with both hashes in stdout
  - `check_strict_with_name_filter` — `--name nav` only checks nav, ignores footer variance

**Verification:** `pagekit check --strict` against the ettsmart.se slug (`/Users/admin/omni/websites/ettsmart.se`) produces the expected variance table for `nav` (transparent vs default split).

**Worker authority:** decide-and-document on table format details, hash function choice, flag spelling (`--strict` vs separate `check-strict` command), and exit-code scheme. Halt if the `<!-- fragment:NAME -->` parsing approach turns out incompatible with lol_html's comment handling (then surface both options).

---

## D2. Path-relative sync transforms

**Artifact:** `[transforms]` section in `fragments.toml`; when `path_root` is set, `pagekit sync` rewrites absolute paths inside fragment content to be relative to the destination page's depth.

**Spec:**

- New schema in `src/config.rs`:
  ```toml
  [transforms]
  path_root = "/"          # absolute prefix that fragments use; default unset (no rewriting)
  attrs = ["href", "src"]  # attributes to rewrite; default if path_root is set
  ```
- `transforms.path_root` is `Option<String>`; absent = today's behavior (fragments core sync, no rewriting)
- When set, pagekit's sync path takes ownership of the marker-region replacement instead of delegating to `fragments::sync_all`. Implementation cut to evaluate during scoping:
  - **A:** pagekit reimplements sync (single pass with lol_html: locate marker pairs, load matching fragment, transform paths, splice in)
  - **B:** delegate to `fragments::sync_all` then post-process each modified page in a second lol_html pass
  - Worker decides; A is cleaner if fragments core's marker-walking is easy to mirror, B is simpler at the cost of a second pass per page
- Path rewrite logic:
  - For each attr listed (default `href`, `src`), if value starts with `path_root` — rewrite to relative-to-destination-page-depth
  - Skip values matching `^(https?:|mailto:|tel:|data:|#)` or already-relative (`./` or `../` prefix or no leading `/`)
  - Fragment-internal anchors (`#section-id`) preserved
- Sync stays idempotent: re-syncing a page produces no diff

**Done-when:**
- `[transforms]` schema parses; existing tests stay green (no path_root in their configs)
- `src/transforms.rs` (new) implements path rewriting against an HTML byte slice via lol_html
- Tests in `tests/integration.rs`:
  - `sync_rewrites_paths_per_depth` — 3-page site at depths 0/1/2 with footer fragment containing absolute paths; assert correct relative paths per page after sync
  - `sync_preserves_external_urls` — http://, mailto:, tel:, # left alone
  - `sync_idempotent_with_transforms` — second sync produces identical bytes

**Verification:** ettsmart.se footer absolutized to use `/sollentuna` etc., synced into pages at depths 0/1/2; manual diff confirms `../sollentuna` and `../../sollentuna` land where expected; CF Pages preview deploys without 404s.

**Worker authority:** decide-and-document on impl cut (A vs B), the exact attr set in the default, and whether to introduce a `pagekit::sync` function in the binary or keep the change CLI-internal. Halt only if fragments core's API forces a cross-crate change to support B; in that case write a proposal in `harness/rules/proposals/` and dispatch via chad-omni.

---

## D3. `pagekit extract --split-variants`

**Artifact:** new `--split-variants` flag on `extract` that emits one fragment file per detected content variant and updates marker names in source pages.

**Spec:**

- During the existing detection pass (Phase 1 in `extract.rs`), for each candidate selector keep ALL distinct content variants seen on ≥2 pages (today: only the dominant one is kept)
- Without `--split-variants`: existing behavior preserved (dominant variant only; minorities silently ignored). Detection log gains a one-line warning per candidate with multiple variants pointing the user at `--split-variants`.
- With `--split-variants`:
  - For each candidate with `n ≥ 2` variants, emit `_fragments/<name>-1.html` … `_fragments/<name>-n.html` (numerical naming; renaming is a separate manual sweep — keeps the impl tight and avoids guessing semantic variant names)
  - For each source page, replace its `<!-- {prefix}:{name} -->` marker pair with the variant marker that matches its content (`<!-- {prefix}:{name}-1 -->` etc.)
  - Use lol_html for the marker-name rewrite (same machinery as the Stage 3 source rewrite)

**Done-when:**
- `--split-variants` flag wired in clap; without it, output is unchanged
- Tests in `tests/integration.rs`:
  - `extract_split_variants_emits_n_files` — 6-page site with 2 nav variants × 3 pages each → produces `nav-1.html` + `nav-2.html` with correct content
  - `extract_split_variants_rewrites_markers` — same site → markers in 3 pages point at `nav-1`, other 3 at `nav-2`
  - `extract_default_warns_on_variants` — without flag, log mentions variance, only dominant variant emitted

**Verification:** synthetic Webflow-shaped fixture (3 transparent-class navs, 3 default navs across 6 pages) → one `pagekit extract --split-variants` produces correct per-page markers; `pagekit check --strict` (D1) returns clean afterward.

**Worker authority:** decide-and-document on numbering convention, output ordering of variants by frequency, and warning verbosity. Halt if the marker-name rewrite collides with idempotency (a re-run shouldn't keep splitting an already-split page).

---

## Sequencing

D1 → D2 → D3, in that order:

1. **D1 first** because it's the cheapest and gives visibility. Without it the agent doesn't know which fragments need splitting; the sprint can't measure its own progress.
2. **D2 second** because it changes sync semantics. Landing it before D3 means D3's variant fragments don't accidentally proliferate per-depth on top of per-content variation.
3. **D3 last** because it depends on D1's diagnostic + D2's depth-collapse to actually solve the variant problem in one pass.

Each deliverable is an independent worker dispatch. Per `epistemic-honesty.md` ÷10–100x: 30 min – 2 h per deliverable. Sprint total: a half day to a day-and-a-half of agent time.

## Out of scope (named to prevent drift)

- **`pagekit sweep` for non-marker bulk text edits.** Sed handles this fine. `subtract-before-building.md`.
- **Active-state handling.** Site.js concern (`location.pathname`-driven helper). Pagekit can ship a one-line note in `init`-scaffolded `_fragments/AGENTS.md` pointing at the pattern, but does not bake it in.
- **HTML-validity / link-integrity in `doctor`.** Backlog item; needs a real consumer.
- **`pagekit doctor --image-dims`.** Real felixhellstrom.com friction but different surface; separate sprint.
- **Semantic variant naming** (e.g. auto-detecting "transparent" from class diff). MVP uses numerical naming; semantic naming is a follow-up if it earns its keep.

## Compounding payoff

Once D1+D2 land, every future Webflow-export wire-up gets variant-safety and depth-correctness for free. ettsmart.se becomes the first beneficiary; `_refs/`-cataloged Webflow templates and any future Stormfors site inherit the same. Felix's stack starts feeling agent-native at the file-mutation layer, not just at deploy.

## Origin

Drafted 2026-05-06 by chad-pagekit after a productive review of friction observed in chad-ettsmart_se's pagekit wire-up (commit `0eedf95` in `~/omni/websites/ettsmart.se`) and chad-felixhellstrom_com's prose/asset normalization sweep (image dims, em-dash removal, suffix stripping). Vault `01KQYRQ3075FWR2B08G5BFCXC6` has the originating ettsmart insight; this sprint operationalizes the response.
