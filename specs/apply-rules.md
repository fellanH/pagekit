# pagekit apply — parameterized bulk edits

Goal: give agents a **site-agnostic**, safe-by-default way to update many pages at once without templates or build pipelines.

`pagekit apply` reads a TOML rules file with a small schema:

- `params`: key/value map; can be overridden by CLI `--set key=value`
- `edits[]`: target selection + scope + ordered steps

## CLI

```bash
pagekit apply rules.toml --set key=value [--set key2=value2] [--write]
```

- Default is **dry-run** (prints how many files would change, exits 2 if any would change)
- `--write` applies changes to disk and exits 0

## v1 schema (implemented)

```toml
version = 1

[params]
# optional

[[edits]]
[edits.target]
kind = "all_pages" | "pages_with_marker" | "pages_matching_selector"

# pages_with_marker
name = "${marker_name}"

# pages_matching_selector
selector = "${css_selector}"

[edits.scope]
kind = "whole_document" | "marker" | "selector"

# marker
name = "${marker_name}"

# selector
selector = "${component_root_selector}"

[[edits.steps]]
op = "rename_tag"
from = "h2"
to = "h3"

[[edits.steps]]
op = "set_attr"
selector = "button[aria-haspopup='dialog']"
attr = "aria-label"
value = "Open image"
```

### Notes / constraints

- `rename_tag` supports scope `whole_document` and `marker` (not `selector`) because it is byte-preserving source rewriting, not DOM reserialization.
- `set_attr` uses `lol_html` rewriter and supports all scopes.
- Marker scoping currently uses the `fragment` prefix: `<!-- fragment:NAME --> ... <!-- /fragment:NAME -->`.

## Example: “h2→h3 inside a component marker” (site-agnostic)

```toml
version = 1

[params]
marker_name = "cta1_component"

[[edits]]
[edits.target]
kind = "pages_with_marker"
name = "${marker_name}"

[edits.scope]
kind = "marker"
name = "${marker_name}"

[[edits.steps]]
op = "rename_tag"
from = "h2"
to = "h3"
```

## Example: “fix aria-label under a component root selector”

```toml
version = 1

[params]
root = ".lightbox"
button = "button[aria-haspopup='dialog']"

[[edits]]
[edits.target]
kind = "pages_matching_selector"
selector = "${root}"

[edits.scope]
kind = "selector"
selector = "${root}"

[[edits.steps]]
op = "set_attr"
selector = "${button}"
attr = "aria-label"
value = "Open image"
```

