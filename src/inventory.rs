use crate::config::Config;
use crate::extract::collect_html_files;
use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// One walk over every HTML file in scope; emit a deterministic
/// tab-separated stream of `<page>\t<kind>\t<value>` lines covering
/// classes, ids, hrefs, srcs, titles, meta tags, headings (h1-h3), and
/// JSON-LD `@type` declarations. Output is sorted (pages alphabetical;
/// per-page kind+value alphabetical) so diffs are stable.
pub fn run_inventory(root: &Path, config: &Config, save: Option<PathBuf>) -> Result<()> {
    let fragments_dir = root.join(&config.core.fragments_dir);
    let scan_root = root.join(&config.core.target_dir);
    let scan_root = if scan_root.is_dir() {
        scan_root
    } else {
        root.to_path_buf()
    };

    let html_files = collect_html_files(
        &scan_root,
        &fragments_dir,
        &config.core.exclude_dirs,
        config.core.max_depth,
    );

    let mut output = String::new();
    for path in &html_files {
        let rel = path.strip_prefix(&scan_root).unwrap_or(path);
        let page_url = format!("/{}", rel.display());
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let entries = collect_entries(&content);
        for (kind, value) in entries {
            let value = sanitize(&value);
            output.push_str(&format!("{page_url}\t{kind}\t{value}\n"));
        }
    }

    match save {
        Some(p) => {
            fs::write(&p, &output).with_context(|| format!("writing {}", p.display()))?;
            let lines = output.lines().count();
            let bytes = output.len();
            println!(
                "pagekit: inventory written to {} ({lines} entries, {bytes} bytes, {} pages)",
                p.display(),
                html_files.len()
            );
        }
        None => print!("{output}"),
    }

    Ok(())
}

/// Per-page inventory record builder. Returns `(kind, value)` pairs
/// sorted by kind then value so output is stable across runs.
fn collect_entries(html: &str) -> Vec<(&'static str, String)> {
    let doc = Html::parse_document(html);
    let mut entries: Vec<(&'static str, String)> = Vec::new();

    if let Some(el) = doc.select(&Selector::parse("title").unwrap()).next() {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            entries.push(("title", text));
        }
    }

    let canonical_sel = Selector::parse(r#"link[rel="canonical"]"#).unwrap();
    for el in doc.select(&canonical_sel) {
        if let Some(href) = el.value().attr("href") {
            entries.push(("meta", format!("canonical={href}")));
        }
    }

    let meta_sel = Selector::parse("meta").unwrap();
    for el in doc.select(&meta_sel) {
        let key = el
            .value()
            .attr("name")
            .or_else(|| el.value().attr("property"));
        let content = el.value().attr("content");
        if let (Some(k), Some(c)) = (key, content) {
            entries.push(("meta", format!("{k}={c}")));
        }
    }

    for tag in ["h1", "h2", "h3"] {
        let sel = Selector::parse(tag).unwrap();
        let kind: &'static str = match tag {
            "h1" => "h1",
            "h2" => "h2",
            "h3" => "h3",
            _ => unreachable!(),
        };
        for el in doc.select(&sel) {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                entries.push((kind, text));
            }
        }
    }

    let mut classes: BTreeSet<String> = BTreeSet::new();
    for el in doc.select(&Selector::parse("[class]").unwrap()) {
        if let Some(c) = el.value().attr("class") {
            for tok in c.split_whitespace() {
                classes.insert(tok.to_string());
            }
        }
    }
    for c in classes {
        entries.push(("class", c));
    }

    let mut ids: BTreeSet<String> = BTreeSet::new();
    for el in doc.select(&Selector::parse("[id]").unwrap()) {
        if let Some(i) = el.value().attr("id") {
            ids.insert(i.to_string());
        }
    }
    for i in ids {
        entries.push(("id", i));
    }

    let mut hrefs: BTreeSet<String> = BTreeSet::new();
    for el in doc.select(&Selector::parse("[href]").unwrap()) {
        if let Some(h) = el.value().attr("href") {
            hrefs.insert(h.to_string());
        }
    }
    for h in hrefs {
        entries.push(("href", h));
    }

    let mut srcs: BTreeSet<String> = BTreeSet::new();
    for el in doc.select(&Selector::parse("[src]").unwrap()) {
        if let Some(s) = el.value().attr("src") {
            srcs.insert(s.to_string());
        }
    }
    for s in srcs {
        entries.push(("src", s));
    }

    let schema_sel = Selector::parse(r#"script[type="application/ld+json"]"#).unwrap();
    for el in doc.select(&schema_sel) {
        let body = el.text().collect::<String>();
        entries.push(("schema-type", extract_schema_type(&body)));
    }

    entries.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
    entries
}

/// Best-effort `@type` extraction from a JSON-LD body. Avoids adding a
/// JSON parser dep for one field; covers `@type` at top level and the
/// common `@graph[0].@type` shape. Anything else returns a marker
/// string so the inventory line still emits.
fn extract_schema_type(json_text: &str) -> String {
    if let Some(t) = first_type_after(json_text, 0) {
        return t;
    }
    "(no @type)".to_string()
}

fn first_type_after(s: &str, start: usize) -> Option<String> {
    let needle = "\"@type\"";
    let pos = s[start..].find(needle)? + start;
    let after = &s[pos + needle.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Tab and newline characters in values would break the line format;
/// replace with single spaces. Preserves the value as a human-readable
/// string while keeping the stream awk/grep safe.
fn sanitize(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_title_and_meta() {
        let html = r#"<!DOCTYPE html><html><head>
<title>Hello</title>
<meta name="description" content="A page">
<meta property="og:title" content="HelloOG">
<link rel="canonical" href="https://example.com/">
</head><body></body></html>"#;
        let entries = collect_entries(html);
        let kinds: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
        let values: Vec<String> = entries.iter().map(|(_, v)| v.clone()).collect();
        assert!(kinds.contains(&"title"));
        assert!(values.contains(&"Hello".to_string()));
        assert!(values.iter().any(|v| v == "description=A page"));
        assert!(values.iter().any(|v| v == "og:title=HelloOG"));
        assert!(values.iter().any(|v| v == "canonical=https://example.com/"));
    }

    #[test]
    fn dedupes_classes_per_page() {
        let html = r#"<html><body>
<div class="foo bar"></div>
<div class="foo baz"></div>
</body></html>"#;
        let entries = collect_entries(html);
        let class_values: Vec<&String> = entries
            .iter()
            .filter(|(k, _)| *k == "class")
            .map(|(_, v)| v)
            .collect();
        assert_eq!(class_values.len(), 3);
        assert!(class_values.iter().any(|v| *v == "foo"));
        assert!(class_values.iter().any(|v| *v == "bar"));
        assert!(class_values.iter().any(|v| *v == "baz"));
    }

    #[test]
    fn extracts_schema_type_simple() {
        let html = r#"<html><head>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"Hotel","name":"X"}</script>
</head></html>"#;
        let entries = collect_entries(html);
        let schema = entries
            .iter()
            .find(|(k, _)| *k == "schema-type")
            .map(|(_, v)| v.as_str());
        assert_eq!(schema, Some("Hotel"));
    }

    #[test]
    fn sanitizes_tabs_in_values() {
        assert_eq!(sanitize("a\tb"), "a b");
        assert_eq!(sanitize("a\nb"), "a b");
    }
}
