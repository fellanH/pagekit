# pagekit rename-assets — fast, safe asset renaming

Goal: collapse the “rename bad asset filenames + update all refs + verify” agent workflow into a single command.

## CLI

```bash
pagekit rename-assets [--write]
```

- Default is **dry-run** (prints planned renames and how many files would change; exits 2 if any would change)
- `--write` rewrites references and renames the files on disk

## v1 behavior (implemented)

Policy: **spaces → hyphens** for any asset whose basename contains spaces.

References rewritten:

- **HTML**: `src`, `href`, `srcset` (URL token of each entry; descriptors preserved)
- **CSS**: `url(...)`
- **JSON**: `*.json` files under site root (conservative quoted-string rewrite)

Matching is filesystem-based with percent-decoding, so refs like `Gallery%201.avif` are understood.

## Safety rails

- Refuses to run if any rename would collide with an existing target path.
- Rewrites happen before file moves; in `--write` mode, moves happen last.

## Next increments (not implemented yet)

- srcset “export bug” repair (`.avif-500w` style) as a separate `pagekit fix-srcset` command.
- Optional “only under _assets/” scoping flag.

