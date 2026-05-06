# pagekit

Vanilla HTML + CSS site management for agents. Single Rust binary.

Composes [`fragments`](../fragments) (text-sync primitive) with HTML-specific helpers: page scaffolding, DOM-aware shared-block extraction, link integrity, framework-export profiles.

## Install

```bash
cargo build --release
cp target/release/pagekit ~/.local/bin/pagekit
```

## Commands

- `pagekit init <file>` — scaffold a new HTML page with marker pairs for every fragment
- `pagekit extract` — detect DOM blocks shared across pages and pull them into `_fragments/<name>.html` with markers inserted (lol_html-backed; preserves source bytes verbatim)
- `pagekit sync` / `watch` / `check` / `list` / `doctor` / `config` — delegated to fragments core

See [`specs/pagekit.md`](specs/pagekit.md) and [`tasks/arc.md`](tasks/arc.md) for design rationale and the active arc.
