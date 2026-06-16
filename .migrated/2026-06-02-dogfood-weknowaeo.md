# Dogfood: pagekit audit against we-know-aeo (2026-06-02)

Ran the full read-only audit (`preflight` + `links`/`seo`/`a11y`/`doctor`/`check`/`assets`,
incl. `--json`) against `~/omni/products/we-know-aeo` (the AEO offer's own site — a Next.js
static export to `pages/` + `_fragments/`, wired via `fragments.toml`). No crashes; exit codes
correct (`links`/`seo` → 2, `a11y`/`doctor`/`check` → 0, verified without the pipe artifact).

## pagekit bugs surfaced

### BUG-1 — orphan detection misses `<meta>` social-card images (FIXED)
`og-image.png` flagged orphan, but it's referenced via `<meta name="twitter:image" content="og-image.png">`.
`links`/`assets` scanned only `href`/`src`/`srcset`/CSS-`url()` — never OG/Twitter image meta.
→ false orphan AND a real broken-social-card blind spot (directly relevant to AEO preview cards).
**Fix:** feed `og:image`/`og:image:url`/`og:image:secure_url`/`twitter:image`/`twitter:image:src`
content into the reference set in `links.rs` + `assets.rs`. Orphan-set only (mirrors the existing
conservative `srcset` choice — see BUG-3).

### BUG-2 — `llms.txt` not whitelisted as a well-known root file (FIXED)
`llms.txt` flagged orphan. Every "reference" in the HTML is plain body text ("89% lack an llms.txt
file"); the file itself is intentionally unreferenced, fetched by convention like `robots.txt` /
`sitemap.xml` (both already whitelisted, both sit beside it at `pages/` root). Embarrassing on the
*We Know AEO* site specifically — `llms.txt` is THE AEO artifact.
**Fix:** add `llms.txt` (+ `ads.txt`, `app-ads.txt`) to `PLATFORM_FILES` in `links.rs` + `assets.rs`.
(`.well-known/` already covered by the dotfile-component skip.)

### BUG-3 — meta image refs not checked for broken-link (DEFERRED, trigger-gated)
The BUG-1 fix is orphan-set only: a `<meta og:image>` pointing at a *missing* file is not yet
flagged as a broken link. Spec convention is absolute OG URLs (classified External → skipped
anyway), so relative/root-absolute is the unusual case. **Trigger:** a consumer ships a broken
social card via a relative og:image and wants it caught. Cheap when it fires (route meta image
refs through the same broken-link path as `href`/`src`).

## Real site findings — NOT pagekit's responsibility (owner: chad-weknowaeo)

Relay to the we-know-aeo seat:
- **`seo` errors:** `/aeo-audit/result/` + `/aeo-audit/start/` missing `<link rel="canonical">`;
  `/aeo-audit/result/` missing `<meta name="description">`.
- **Broken Next export (high):** `/privacy/` and `/terms/` both have `<title>404: This page could
  not be found.</title>` — the privacy & terms pages exported as the 404 fallback. Real bug.
- **`seo` warns:** long titles/descriptions on home + blog posts; `og:type` missing site-wide;
  2× `<h1>` on `/aeo-audit/start/` and one blog post; duplicate description on 2 pages.
- **`a11y`:** clean (subset only).
