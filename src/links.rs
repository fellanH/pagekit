use crate::config::Config;
use crate::extract::collect_html_files;
use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Run `pagekit links`. Returns the process exit code: 0 = clean,
/// 2 = issues found. IO/parse errors bubble via `Result` (caller
/// surfaces exit 1).
pub fn run_links(root: &Path, config: &Config) -> Result<i32> {
    let target_dir = root.join(&config.core.target_dir);
    let scan_root = if target_dir.is_dir() {
        target_dir
    } else {
        root.to_path_buf()
    };
    let fragments_dir = root.join(&config.core.fragments_dir);

    let html_files = collect_html_files(
        &scan_root,
        &fragments_dir,
        &config.core.exclude_dirs,
        config.core.max_depth,
    );

    // Pre-walk: per-page id sets (for anchor checks) and the union of
    // referenced files (for orphan detection).
    let mut page_ids: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    let mut referenced_files: BTreeSet<PathBuf> = BTreeSet::new();
    let mut page_refs: BTreeMap<PathBuf, Vec<Ref>> = BTreeMap::new();

    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let doc = Html::parse_document(&content);

        let mut ids: BTreeSet<String> = BTreeSet::new();
        for el in doc.select(&Selector::parse("[id]").unwrap()) {
            if let Some(i) = el.value().attr("id") {
                ids.insert(i.to_string());
            }
        }
        page_ids.insert(path.clone(), ids);

        let mut refs: Vec<Ref> = Vec::new();
        for el in doc.select(&Selector::parse("[href]").unwrap()) {
            if let Some(v) = el.value().attr("href") {
                refs.push(Ref::new("href", v));
            }
        }
        for el in doc.select(&Selector::parse("[src]").unwrap()) {
            if let Some(v) = el.value().attr("src") {
                refs.push(Ref::new("src", v));
            }
        }

        // Track referenced files for orphan detection. href/src enter
        // both the per-page broken-link check AND the orphan set;
        // srcset is orphan-only (browsers fall back gracefully on
        // missing srcset entries, and malformed unencoded srcset is
        // common in framework exports — treating it as broken-link
        // input causes false positives).
        for r in &refs {
            if let Some(p) = resolve_internal(&r.value, path, &scan_root) {
                referenced_files.insert(p);
            }
        }
        for el in doc.select(&Selector::parse("[srcset]").unwrap()) {
            if let Some(v) = el.value().attr("srcset") {
                for url in parse_srcset(v) {
                    if let Some(p) = resolve_internal(&url, path, &scan_root) {
                        referenced_files.insert(p);
                    }
                }
            }
        }

        page_refs.insert(path.clone(), refs);
    }

    // Findings buckets.
    let mut broken_internal: Vec<(String, String)> = Vec::new();
    let mut broken_anchors: Vec<(String, String)> = Vec::new();

    for (page_path, refs) in &page_refs {
        let page_url = display_url(page_path, &scan_root);
        for r in refs {
            match classify(&r.value) {
                LinkKind::External | LinkKind::Mailto | LinkKind::Tel | LinkKind::Data => {}
                LinkKind::Anchor(id) => {
                    let ids = page_ids.get(page_path).unwrap();
                    if !ids.contains(&id) {
                        broken_anchors
                            .push((page_url.clone(), format!("#{id} (not declared on page)")));
                    }
                }
                LinkKind::Internal { path: _, anchor } => {
                    let target = resolve_internal(&r.value, page_path, &scan_root);
                    match target {
                        None => {
                            broken_internal.push((page_url.clone(), r.value.clone()));
                        }
                        Some(target_path) => {
                            if let Some(id) = anchor {
                                let ids = page_ids.get(&target_path);
                                if ids.map(|s| !s.contains(&id)).unwrap_or(true) {
                                    broken_anchors.push((
                                        page_url.clone(),
                                        format!(
                                            "{} → #{id} (target page exists but anchor missing)",
                                            r.value
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Orphan asset pass: every file in scope (excluding HTML pages and
    // fragments dir) that is NOT in `referenced_files`.
    let mut all_files: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in WalkDir::new(&scan_root)
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            if p.starts_with(&fragments_dir) {
                return false;
            }
            if config
                .core
                .exclude_dirs
                .iter()
                .any(|d| p.starts_with(scan_root.join(d)))
            {
                return false;
            }
            // Skip any path that contains a dot-prefixed directory
            // component (.git, .wrangler, .freedom, .audit, .DS_Store
            // etc.). These are tooling artifacts, not deployable assets.
            !path_has_dotfile_component(p, &scan_root)
        })
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        // Skip HTML pages — those are content, not assets.
        if p.extension().map(|x| x == "html").unwrap_or(false) {
            continue;
        }
        // Skip markdown — docs (AGENTS.md, README.md), not deployable.
        if p.extension().map(|x| x == "md").unwrap_or(false) {
            continue;
        }
        // Skip dotfiles by basename (.gitignore, .DS_Store, etc.).
        if p.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }
        all_files.insert(p.to_path_buf());
    }

    // Platform/build-system files that are NEVER referenced from HTML
    // by design (Cloudflare Pages, PWA, crawler discovery, pagekit/clone
    // tooling, Pages Functions). Skip them so the orphan signal stays
    // useful.
    let platform_files: BTreeSet<&str> = [
        "_headers",
        "_redirects",
        "_routes.json",
        "_worker.js",
        "robots.txt",
        "sitemap.xml",
        "sitemap.txt",
        "manifest.json",
        "clone.yaml",
        "fragments.toml",
        "wrangler.toml",
        "site.webmanifest",
        "favicon.ico",
        "humans.txt",
        "security.txt",
    ]
    .into_iter()
    .collect();

    let orphans: Vec<PathBuf> = all_files
        .difference(&referenced_files)
        .filter(|p| {
            // Skip platform files (matched by basename).
            let basename = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if platform_files.contains(basename) {
                return false;
            }
            // Skip Cloudflare Pages Functions (server-side; never
            // referenced from HTML).
            let rel = p.strip_prefix(&scan_root).unwrap_or(p);
            if rel.starts_with("functions/") || rel.starts_with("functions\\") {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    // Render report.
    let mut had_issues = false;

    if !broken_internal.is_empty() {
        had_issues = true;
        println!("broken internal links ({}):", broken_internal.len());
        for (page, target) in &broken_internal {
            println!("  {page} → {target} (404)");
        }
        println!();
    }

    if !broken_anchors.is_empty() {
        had_issues = true;
        println!("broken anchors ({}):", broken_anchors.len());
        for (page, detail) in &broken_anchors {
            println!("  {page} → {detail}");
        }
        println!();
    }

    if !orphans.is_empty() {
        had_issues = true;
        println!("orphan assets ({}):", orphans.len());
        for p in &orphans {
            let rel = p.strip_prefix(&scan_root).unwrap_or(p);
            println!("  {} (no references)", rel.display());
        }
        println!("  (note: files referenced ONLY from CSS — fonts via @font-face, background-image url() — may appear here. Phase 3 `pagekit assets` will close this gap.)");
        println!();
    }

    if !had_issues {
        println!("pagekit: all links resolve, no orphan assets");
        return Ok(0);
    }

    Ok(2)
}

/// Single href or src reference captured from a page.
struct Ref {
    #[allow(dead_code)]
    attr: &'static str,
    value: String,
}

impl Ref {
    fn new(attr: &'static str, value: &str) -> Self {
        Self {
            attr,
            value: value.to_string(),
        }
    }
}

#[derive(Debug)]
enum LinkKind {
    External,
    Mailto,
    Tel,
    Data,
    Anchor(String),
    Internal {
        #[allow(dead_code)]
        path: String,
        anchor: Option<String>,
    },
}

fn classify(value: &str) -> LinkKind {
    let v = value.trim();
    if v.is_empty() {
        return LinkKind::External; // empty value: ignore
    }
    if v.starts_with("http://")
        || v.starts_with("https://")
        || v.starts_with("//")
        || v.starts_with("javascript:")
    {
        return LinkKind::External;
    }
    if v.starts_with("mailto:") {
        return LinkKind::Mailto;
    }
    if v.starts_with("tel:") {
        return LinkKind::Tel;
    }
    if v.starts_with("data:") {
        return LinkKind::Data;
    }
    if let Some(id) = v.strip_prefix('#') {
        if id.is_empty() {
            // Bare `#` is a Webflow/Bootstrap placeholder for buttons
            // with JS handlers attached. Not a real anchor; skip.
            return LinkKind::External;
        }
        return LinkKind::Anchor(id.to_string());
    }
    // Strip query string (links resolve by path; query is ignored).
    let v = v.split('?').next().unwrap_or(v);
    let (path, anchor) = match v.split_once('#') {
        Some((p, a)) => (p.to_string(), Some(a.to_string())),
        None => (v.to_string(), None),
    };
    LinkKind::Internal { path, anchor }
}

/// Resolve an internal href/src to an actual filesystem path under
/// `scan_root`. Returns `None` if the path doesn't resolve to an
/// existing file. Tries direct file match, then `<path>/index.html`.
/// Percent-decodes the value before filesystem lookup so URL-encoded
/// filenames (`Gallery%201.avif`) resolve to actual files.
fn resolve_internal(value: &str, page_path: &Path, scan_root: &Path) -> Option<PathBuf> {
    let v = value.trim().split('?').next().unwrap_or(value);
    let v = v.split('#').next().unwrap_or(v);
    if v.is_empty() {
        return None;
    }
    if v.starts_with("http://")
        || v.starts_with("https://")
        || v.starts_with("//")
        || v.starts_with("mailto:")
        || v.starts_with("tel:")
        || v.starts_with("data:")
        || v.starts_with("javascript:")
        || v.starts_with('#')
    {
        return None;
    }

    let decoded = percent_decode(v);
    let candidate = if let Some(stripped) = decoded.strip_prefix('/') {
        scan_root.join(stripped)
    } else {
        page_path.parent().map(|p| p.join(&decoded))?
    };

    let candidate = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Try <path>/index.html for directory-style URLs.
            let with_index = candidate.join("index.html");
            with_index.canonicalize().ok()?
        }
    };

    // If we landed on a directory, try its index.html.
    if candidate.is_dir() {
        let with_index = candidate.join("index.html");
        return with_index.canonicalize().ok();
    }

    Some(candidate)
}

fn display_url(path: &Path, scan_root: &Path) -> String {
    let rel = path.strip_prefix(scan_root).unwrap_or(path);
    format!("/{}", rel.display())
}

/// Minimal percent-decoder for filename-style paths. Handles `%XX`
/// hex sequences; leaves other bytes alone. Avoids a dependency on
/// the `percent-encoding` crate for this single use.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_digit(bytes[i + 1]);
            let lo = hex_digit(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// True if any path component below `root` starts with `.`. Catches
/// tooling dirs like `.git/`, `.wrangler/`, `.freedom/` without
/// requiring each to be listed in `exclude_dirs`.
fn path_has_dotfile_component(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    })
}

/// Parse a `srcset` attribute into its URL list. Each comma-separated
/// entry's first whitespace-separated token is the URL; the rest
/// (`1x`, `500w`, etc.) is the descriptor and ignored here.
fn parse_srcset(srcset: &str) -> Vec<String> {
    srcset
        .split(',')
        .filter_map(|entry| entry.split_whitespace().next().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_external_urls() {
        assert!(matches!(
            classify("https://example.com"),
            LinkKind::External
        ));
        assert!(matches!(classify("//cdn.x.com/y"), LinkKind::External));
        assert!(matches!(classify("mailto:a@b.c"), LinkKind::Mailto));
        assert!(matches!(classify("tel:+1234"), LinkKind::Tel));
        assert!(matches!(
            classify("data:image/png;base64,xx"),
            LinkKind::Data
        ));
    }

    #[test]
    fn classify_anchor_only() {
        match classify("#hero") {
            LinkKind::Anchor(id) => assert_eq!(id, "hero"),
            _ => panic!("expected Anchor"),
        }
    }

    #[test]
    fn classify_internal_with_anchor() {
        match classify("/page#section") {
            LinkKind::Internal { path, anchor } => {
                assert_eq!(path, "/page");
                assert_eq!(anchor.as_deref(), Some("section"));
            }
            _ => panic!("expected Internal"),
        }
    }

    #[test]
    fn classify_internal_strips_query() {
        match classify("/page?ref=x") {
            LinkKind::Internal { path, anchor } => {
                assert_eq!(path, "/page");
                assert!(anchor.is_none());
            }
            _ => panic!("expected Internal"),
        }
    }
}
