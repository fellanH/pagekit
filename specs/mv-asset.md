# pagekit mv-asset — keep paths in sync

Goal: make “rename/move an asset and fix every reference” a one-command, safe-by-default operation.

## CLI

```bash
pagekit mv-asset <from> <to> [--write]
```

- Default is **dry-run** (reports how many files would change; exits 2 if any would change)
- `--write` updates referencing files and moves the asset on disk

## What gets updated

- **HTML**: `src`, `href`, `srcset` (URL token of each entry; descriptors preserved)
- **CSS**: `url(...)` (quoted/unquoted forms)

## How rewriting works (site-agnostic)

For each reference:

1. Resolve it to an absolute path on disk relative to the referencing file.
2. If it points to `<from>`, rewrite it to point to `<to>`.
3. Preserve “style”:
   - if reference was root-absolute (`/images/a.png`), rewrite to root-absolute
   - if reference was relative (`../images/a.png`), rewrite to a relative path from the same file

This avoids brittle string matching and makes it work on any site layout.

