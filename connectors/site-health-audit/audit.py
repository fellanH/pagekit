#!/usr/bin/env python3
"""Site Health Audit — branded HTML report over pagekit's verify suite.

A *presentation connector*: it composes pagekit's existing `--json` mechanism
(`links` / `seo` / `a11y`) and renders the aggregated findings as a single,
self-contained, client-facing HTML report. The pagekit binary stays pure
mechanism — all branding/presentation opinion lives here, rendered through the
omni `packages/ui` design tokens (per design-system-compliance).

Scope is honest: structural + SEO + accessibility defects only. Visual-diff and
asset-parity-vs-source are the *migration connector's* job, not this report's
(see tasks/arc.md Decisions, cross-connector boundary).

Usage:
    audit.py <site-dir> [--out report.html] [--title "..."]
             [--pagekit PATH] [--ui-dir DIR] [--open]

Exit code mirrors the suite: 0 = all checks pass, 2 = at least one finding.
"""

from __future__ import annotations

import argparse
import datetime
import html
import json
import shutil
import subprocess
import sys
from pathlib import Path

CHECKS = [
    ("links", "Links", "Broken internal links, dead anchors, orphan assets"),
    ("seo", "SEO", "Titles, descriptions, canonicals, OG/Twitter, JSON-LD, headings"),
    ("a11y", "Accessibility", "Alt text, form labels, lang attr, link clarity (cheap WCAG subset)"),
]

DEFAULT_UI_DIR = Path.home() / "omni/omni-os/omni-system/packages/ui"


# ---------------------------------------------------------------- data model


def run_check(pagekit: str, site: Path, check: str) -> dict:
    """Run `pagekit <site> <check> --json`; return the parsed envelope.

    Exit code 2 (findings) is expected, not an error. Anything else with no
    parseable JSON is a real failure and bubbles up.
    """
    proc = subprocess.run(
        [pagekit, str(site), check, "--json"],
        capture_output=True,
        text=True,
    )
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError:
        # Tool-internal error (distinct from findings) → exit 2, matching the suite.
        print(
            f"pagekit {check} produced no JSON (exit {proc.returncode}):\n"
            f"{proc.stderr.strip() or proc.stdout.strip()}",
            file=sys.stderr,
        )
        sys.exit(2)


def collect(pagekit: str, site: Path) -> list[dict]:
    reports = []
    for key, _label, _desc in CHECKS:
        rep = run_check(pagekit, site, key)
        rep.setdefault("findings", [])
        reports.append(rep)
    return reports


def counts(findings: list[dict]) -> tuple[int, int]:
    errors = sum(1 for f in findings if f.get("severity") == "error")
    warns = sum(1 for f in findings if f.get("severity") == "warn")
    return errors, warns


# ------------------------------------------------------------------- render


def esc(s: object) -> str:
    return html.escape(str(s), quote=True)


def load_css(ui_dir: Path) -> str:
    """Inline the design-system tokens + theme so the report is portable.

    Reading them at render time keeps the report in lockstep with packages/ui
    rather than hand-rolling CSS (design-system-compliance)."""
    parts = []
    for name in ("tokens.css", "theme.css"):
        p = ui_dir / name
        if p.exists():
            parts.append(p.read_text())
        else:
            print(f"warning: {p} missing — report may be unstyled", file=sys.stderr)
    return "\n".join(parts)


REPORT_CSS = """
/* Site Health Audit — layout over packages/ui tokens (no new colors) */
body { background: var(--bg-secondary); }
.report { max-width: var(--container-max); margin: 0 auto; padding: var(--space-12) var(--space-6) var(--space-20); }
.report-header { margin-bottom: var(--space-10); }
.eyebrow { font-size: var(--type-eyebrow); font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.08em; color: var(--color-accent); margin-bottom: var(--space-3); }
.report-header h1 { margin-bottom: var(--space-3); }
.report-meta { color: var(--text-secondary); font-size: var(--type-small); margin-bottom: var(--space-5); }
.report-meta code { font-size: 0.85em; }
.verdict { font-size: var(--type-body); padding: var(--space-3) var(--space-5); }
.dashboard-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: var(--space-5); margin-bottom: var(--space-12); }
.card { background: var(--bg-primary); border: var(--border-width) solid var(--border-color-subtle);
  border-radius: var(--radius-md); padding: var(--space-6); }
.card-label { font-size: var(--type-eyebrow); font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.05em; color: var(--text-secondary); margin-bottom: var(--space-3); }
.card-value { font-size: 2rem; font-weight: 700; margin-bottom: var(--space-2); }
.card-detail { font-size: var(--type-small); color: var(--text-tertiary); }
.card.ok .card-value { color: var(--color-success); }
.card.bad .card-value { color: var(--color-error); }
.card.warnonly .card-value { color: var(--color-warning); }
.check-section { margin-bottom: var(--space-12); }
.check-section h2 { margin-bottom: var(--space-2); }
.check-section .sub { color: var(--text-secondary); font-size: var(--type-small); margin-bottom: var(--space-5); }
.table-wrapper { overflow-x: auto; border: var(--border-width) solid var(--border-color-subtle);
  border-radius: var(--radius-md); background: var(--bg-primary); }
table { width: 100%; border-collapse: collapse; font-size: var(--type-small); }
thead { background: var(--bg-secondary); border-bottom: var(--border-width) solid var(--border-color); }
thead th { padding: var(--space-3) var(--space-4); text-align: left; font-weight: 600;
  font-size: var(--type-eyebrow); text-transform: uppercase; letter-spacing: 0.05em; }
tbody tr { border-bottom: var(--border-width) solid var(--border-color-subtle); }
tbody tr:last-child { border-bottom: none; }
tbody td { padding: var(--space-3) var(--space-4); vertical-align: top; }
td.page code, td.rule code { font-size: 0.85em; word-break: break-all; }
.badge { display: inline-block; padding: 2px var(--space-3); border-radius: var(--radius-pill);
  font-size: var(--type-eyebrow); font-weight: 600; white-space: nowrap; }
.badge-success { background: rgba(5,150,105,.15); color: var(--color-success); }
.badge-warning { background: rgba(217,119,6,.15); color: var(--color-warning); }
.badge-error { background: rgba(220,38,38,.15); color: var(--color-error); }
.clean-note { background: var(--bg-primary); border: var(--border-width) solid var(--border-color-subtle);
  border-radius: var(--radius-md); padding: var(--space-6); color: var(--text-secondary); }
.report-footer { margin-top: var(--space-16); padding-top: var(--space-6);
  border-top: var(--border-width) solid var(--border-color-subtle);
  font-size: var(--type-small); color: var(--text-tertiary); }
.report-footer strong { color: var(--text-secondary); }
"""


def badge(severity: str) -> str:
    cls = "badge-error" if severity == "error" else "badge-warning"
    return f'<span class="badge {cls}">{esc(severity)}</span>'


def stat_card(label: str, report: dict) -> str:
    errors, warns = counts(report["findings"])
    if errors:
        cls, value = "bad", str(errors)
        detail = f"{errors} error{'s' * (errors != 1)}" + (f", {warns} warning{'s' * (warns != 1)}" if warns else "")
    elif warns:
        cls, value = "warnonly", str(warns)
        detail = f"{warns} warning{'s' * (warns != 1)}, 0 errors"
    else:
        cls, value = "ok", "✓"
        detail = "clean"
    return (
        f'<div class="card {cls}"><div class="card-label">{esc(label)}</div>'
        f'<div class="card-value">{value}</div>'
        f'<div class="card-detail">{esc(detail)}</div></div>'
    )


def findings_table(report: dict) -> str:
    rows = []
    # errors first, then warns; stable within group
    ordered = sorted(report["findings"], key=lambda f: 0 if f.get("severity") == "error" else 1)
    for f in ordered:
        page = f.get("page")
        page_cell = f'<code>{esc(page)}</code>' if page else '<span class="card-detail">site-wide</span>'
        rows.append(
            "<tr>"
            f'<td>{badge(f.get("severity", "warn"))}</td>'
            f'<td class="rule"><code>{esc(f.get("rule", ""))}</code></td>'
            f'<td class="page">{page_cell}</td>'
            f'<td>{esc(f.get("message", ""))}</td>'
            "</tr>"
        )
    return (
        '<div class="table-wrapper"><table><thead><tr>'
        "<th>Severity</th><th>Rule</th><th>Page</th><th>Detail</th>"
        "</tr></thead><tbody>" + "".join(rows) + "</tbody></table></div>"
    )


def render(site: Path, title: str, reports: list[dict], css: str) -> str:
    total_errors = sum(counts(r["findings"])[0] for r in reports)
    total_warns = sum(counts(r["findings"])[1] for r in reports)
    date = datetime.date.today().isoformat()

    if total_errors:
        verdict = (
            f'<span class="verdict badge badge-error">{total_errors} issue'
            f'{"s" * (total_errors != 1)} to act on</span>'
        )
    elif total_warns:
        verdict = f'<span class="verdict badge badge-warning">Pass · {total_warns} advisory warning{"s" * (total_warns != 1)}</span>'
    else:
        verdict = '<span class="verdict badge badge-success">All checks pass</span>'

    cards = "".join(stat_card(label, rep) for (_k, label, _d), rep in zip(CHECKS, reports))

    sections = []
    for (_key, label, desc), rep in zip(CHECKS, reports):
        body = (
            findings_table(rep)
            if rep["findings"]
            else '<div class="clean-note">No findings — this check passes cleanly.</div>'
        )
        sections.append(
            f'<section class="check-section"><h2>{esc(label)}</h2>'
            f'<p class="sub">{esc(desc)}</p>{body}</section>'
        )

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{esc(title)} — Site Health Audit</title>
<style>
{css}
{REPORT_CSS}
</style>
</head>
<body>
<main class="report">
  <header class="report-header">
    <div class="eyebrow">Site Health Audit</div>
    <h1>{esc(title)}</h1>
    <p class="report-meta">Source: <code>{esc(site)}</code> · Generated {esc(date)} · pagekit verify suite</p>
    {verdict}
  </header>

  <section class="dashboard-grid">
    {cards}
  </section>

  {"".join(sections)}

  <footer class="report-footer">
    <p><strong>Methodology.</strong> Static analysis of the source tree via the
    <code>pagekit</code> verify suite (<code>links</code>, <code>seo</code>,
    <code>a11y</code>). Errors fail a check; warnings are advisory. Coverage is
    structural, SEO, and a cheap WCAG subset — color contrast, focus order, and
    dynamic ARIA require a rendering engine and are out of scope. Visual fidelity
    and migration parity are not assessed here.</p>
  </footer>
</main>
</body>
</html>
"""


# --------------------------------------------------------------------- main


def main() -> int:
    ap = argparse.ArgumentParser(description="Branded Site Health Audit report over pagekit's verify suite.")
    ap.add_argument("site", type=Path, help="Site directory (contains the HTML pages)")
    ap.add_argument("--out", type=Path, default=Path("site-health-audit.html"), help="Output HTML path")
    ap.add_argument("--title", help="Report title (default: site directory name)")
    ap.add_argument("--pagekit", default=shutil.which("pagekit") or "pagekit", help="pagekit binary")
    ap.add_argument("--ui-dir", type=Path, default=DEFAULT_UI_DIR, help="packages/ui dir for design tokens")
    ap.add_argument("--open", action="store_true", help="Open the report after writing (macOS `open`)")
    args = ap.parse_args()

    site = args.site.expanduser().resolve()
    if not site.is_dir():
        print(f"not a directory: {site}", file=sys.stderr)
        sys.exit(2)
    title = args.title or site.name

    reports = collect(args.pagekit, site)
    css = load_css(args.ui_dir.expanduser())
    out_path = args.out.expanduser()
    out_path.write_text(render(site, title, reports, css))

    total_errors = sum(counts(r["findings"])[0] for r in reports)
    total_warns = sum(counts(r["findings"])[1] for r in reports)
    print(f"wrote {out_path}  ({total_errors} errors, {total_warns} warnings across {len(reports)} checks)")

    if args.open:
        subprocess.run(["open", str(out_path)], check=False)
    # Mirror the suite standard: errors fail (exit 1), warnings are advisory (pass).
    # Tool-internal errors use the distinct code 2 (see run_check / not-a-dir guard).
    return 1 if total_errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
