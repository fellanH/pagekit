# Dogfood: pagekit audit against stormfors/knowledge-base (2026-06-02)

Ran the full read-only audit against `~/omni/companies/stormfors/knowledge-base` (Stormfors KB,
hand-authored vanilla HTML, 31 pages, `target_dir="."`, fragments: head/header/footer/sidebar).
A *different* generator shape from the we-know-aeo Next export — exactly the point. No crashes.
`preflight`: check FAIL, links FAIL, seo PASS(warns), doctor PASS, a11y PASS.

## TOOL-BUG fix-candidates (pagekit — my scope; NOT yet implemented)

### CAND-A — `preflight` swallows `check`'s stale-file list (medium)
`pagekit check` standalone lists all 27 stale pages. But in `preflight`, the `== check ==` section
prints **blank** on failure — only the summary count ("27 stale or malformed") survives. Root cause:
`run_sync_check` (`src/preflight.rs:130-136`) gets the full `issues` Vec from
`fragments::check_all_with` and discards it, keeping only `issues.len()`. An agent gating go-live on
`preflight` can't see WHICH pages are stale — defeats the single-gate purpose.
**Fix:** iterate `issues` and print each stale/malformed path under the section (data already in hand;
~5 lines). Mirror the pass-branch which already prints "all files up to date". Add a preflight integ
test asserting a stale filename appears in stdout. **Cheap.**

### CAND-B — orphan-asset detection sweeps non-web source/build files (medium)
`build.sh`, `scripts/build-serve-app.sh`, `scripts/inject-meta.py` flagged orphan by `links`/`assets`.
These are build tooling, never deployable web assets (MIME resolves to `application/octet-stream`).
False positives — you'd never "remove the orphan" build.sh. Surfaces on any repo where `target_dir="."`
sweeps source alongside pages.
**Fix:** skip known non-web-deployable extensions (`.sh`, `.py`, `.rb`, `.pl`, `.toml`, `Makefile`…) in
orphan detection — symmetric with the existing `PLATFORM_FILES` whitelist but by extension class. Both
`links.rs` + `assets.rs` (shared list, like the meta-image helper). Add a test. **Cheap-moderate.**

### CAND-C — `_`-prefixed scaffolding templates audited as real pages (low confidence)
`seo` warns on `_template.html` + `_report-template.html` (short descriptions). These are pagekit
scaffolding templates, not shipped pages. Debatable: some sites use `_name.html` for partials, so a
blanket `_`-prefix skip could over-reach. **Defer** unless a consumer complains. Note only.

## SITE BUGS to relay (owner: stormfors/knowledge-base seat) — via hub, do NOT absorb

- **HEADLINE — 27 pages stale vs `_fragments/`.** `pagekit check` flags 27 of 33 pages out of sync
  with the head/header/footer/sidebar fragments. Someone edited a fragment (or pages) without running
  `pagekit sync`. Fix = `pagekit sync` + verify the diff before commit. (This is a real content-drift
  bug the tool caught correctly — NOT a pagekit defect.)
- SEO warns (low): one report page title 71 chars (recommend ≤70). Template short-descriptions are
  CAND-C, not a site bug.
