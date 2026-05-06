use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use scraper::{Html, Selector};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Run `pagekit show <name>`. Looks up `<fragments_dir>/<name>.html`
/// and prints the fragment HTML, the deduped sorted list of classes
/// used, and the deduped sorted list of asset references (href, src,
/// srcset values). The agent's "give me everything about this
/// component" call.
///
/// Returns process exit code: 0 on success, 1 if fragment not found.
pub fn run_show(root: &Path, config: &Config, name: &str) -> Result<i32> {
    let fragments_dir = root.join(&config.core.fragments_dir);
    let candidate = fragments_dir.join(format!("{name}.html"));
    if !candidate.exists() {
        return Err(anyhow!(
            "fragment '{name}' not found at {} (expected file: {}.html in {})",
            candidate.display(),
            name,
            config.core.fragments_dir
        ));
    }
    let html = fs::read_to_string(&candidate)
        .with_context(|| format!("reading {}", candidate.display()))?;

    let (classes, urls) = extract_classes_and_urls(&html);

    println!("# fragment: {name}");
    println!();
    println!("## HTML\n");
    println!("{}", html.trim_end());
    println!();
    println!("## classes ({})\n", classes.len());
    if classes.is_empty() {
        println!("(none)");
    } else {
        for c in &classes {
            println!("{c}");
        }
    }
    println!();
    println!("## referenced URLs ({})\n", urls.len());
    if urls.is_empty() {
        println!("(none)");
    } else {
        for u in &urls {
            println!("{u}");
        }
    }

    Ok(0)
}

/// Extract the (deduped, alphabetical) class tokens and the
/// (deduped, alphabetical) URL references from a fragment HTML body.
/// URL references = href, src, and srcset values (srcset's URL part
/// per entry only).
fn extract_classes_and_urls(html: &str) -> (Vec<String>, Vec<String>) {
    let doc = Html::parse_document(html);

    let mut classes: BTreeSet<String> = BTreeSet::new();
    for el in doc.select(&Selector::parse("[class]").unwrap()) {
        if let Some(c) = el.value().attr("class") {
            for tok in c.split_whitespace() {
                classes.insert(tok.to_string());
            }
        }
    }

    let mut urls: BTreeSet<String> = BTreeSet::new();
    for el in doc.select(&Selector::parse("[href]").unwrap()) {
        if let Some(v) = el.value().attr("href") {
            urls.insert(v.to_string());
        }
    }
    for el in doc.select(&Selector::parse("[src]").unwrap()) {
        if let Some(v) = el.value().attr("src") {
            urls.insert(v.to_string());
        }
    }
    for el in doc.select(&Selector::parse("[srcset]").unwrap()) {
        if let Some(v) = el.value().attr("srcset") {
            for entry in v.split(',') {
                if let Some(u) = entry.split_whitespace().next() {
                    if !u.is_empty() {
                        urls.insert(u.to_string());
                    }
                }
            }
        }
    }

    (classes.into_iter().collect(), urls.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_classes_dedup_sorted() {
        let html = r#"<nav class="foo bar"><a class="bar baz" href="/x">X</a></nav>"#;
        let (classes, _) = extract_classes_and_urls(html);
        assert_eq!(classes, vec!["bar", "baz", "foo"]);
    }

    #[test]
    fn extracts_urls_from_href_src_srcset() {
        let html = r#"<a href="/about">About</a>
<img src="/_assets/logo.svg" srcset="/_assets/logo-500.svg 500w, /_assets/logo-800.svg 800w">"#;
        let (_, urls) = extract_classes_and_urls(html);
        assert_eq!(
            urls,
            vec![
                "/_assets/logo-500.svg",
                "/_assets/logo-800.svg",
                "/_assets/logo.svg",
                "/about"
            ]
        );
    }
}
