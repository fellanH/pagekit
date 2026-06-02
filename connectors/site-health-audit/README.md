# Site Health Audit (presentation connector)

Turns pagekit's verify suite into a single, branded, client-facing HTML report.

```bash
python3 connectors/site-health-audit/audit.py <site-dir> --out report.html [--open]
```

- Runs `pagekit <site> links|seo|a11y --json` and aggregates the envelopes.
- Renders one self-contained HTML file (CSS inlined → email/upload-able as-is).
- Styling comes from the omni `packages/ui` design tokens (`tokens.css` + `theme.css`),
  read at render time so the report stays in lockstep with the design system.
- Exit code mirrors the suite standard: `0` = all checks pass, `1` = a check failed
  (≥1 error), `2` = tool-internal error (bad dir, pagekit emitted no JSON). Warnings are
  advisory and do not flip the exit code (same as `pagekit seo`/`a11y`).

## Why it lives outside the binary

pagekit is mechanism: it emits `--json` findings and predictable exit codes, nothing
more. The *branded HTML report* is presentation opinion, so it lives in a connector that
composes the mechanism — keeping the binary free of a GUI/templating layer (charter) and
the branding out of the core (`minimal-core-connectors`, `design-system-compliance`).

## Scope (honest)

Structural + SEO + a cheap WCAG subset only. Color contrast, focus order, and dynamic
ARIA need a rendering engine and are out of scope. **Visual-diff and migration parity
(byte/rule-count vs a source site) are the migration connector's job, not this report's**
— see `tasks/arc.md` Decisions (cross-connector boundary).

## Options

| flag | default | meaning |
|------|---------|---------|
| `--out PATH` | `site-health-audit.html` | output file |
| `--title STR` | site dir name | report heading |
| `--pagekit PATH` | `which pagekit` | binary to invoke |
| `--ui-dir DIR` | `~/omni/.../packages/ui` | design-token source |
| `--open` | off | `open` the report after writing (macOS) |

## Dependencies

Python 3 stdlib only. Requires `pagekit` on PATH and the `packages/ui` tokens for styling
(falls back to unstyled with a warning if the tokens are missing).
