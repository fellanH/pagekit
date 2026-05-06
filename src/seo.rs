use crate::config::Config;
use crate::extract::collect_html_files;
use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Title length range below which we warn (too-short titles tend to
/// underperform in SERPs).
const TITLE_MIN: usize = 10;
/// Title length range above which we warn (truncation in SERPs).
const TITLE_MAX: usize = 70;
/// Description length below which we warn.
const DESCRIPTION_MIN: usize = 50;
/// Description length above which we warn.
const DESCRIPTION_MAX: usize = 160;

/// Run `pagekit seo`. Returns the process exit code: 0 = no errors
/// (warns are OK), 2 = at least one error.
pub fn run_seo(root: &Path, config: &Config) -> Result<i32> {
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

    let mut metas: Vec<PageMeta> = Vec::new();
    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let url = display_url(path, &scan_root);
        metas.push(extract_meta(&url, &content));
    }

    let mut findings: Vec<Finding> = Vec::new();
    rule_titles(&metas, &mut findings);
    rule_descriptions(&metas, &mut findings);
    rule_canonicals(&metas, &mut findings);
    rule_og(&metas, &mut findings);
    rule_hreflang(&metas, &mut findings);
    rule_json_ld(&metas, &mut findings);
    rule_heading_hierarchy(&metas, &mut findings);

    if findings.is_empty() {
        println!("pagekit: SEO checks pass on {} page(s)", html_files.len());
        return Ok(0);
    }

    // Group findings by rule family.
    let mut grouped: BTreeMap<&'static str, Vec<&Finding>> = BTreeMap::new();
    for f in &findings {
        grouped.entry(f.rule).or_default().push(f);
    }

    let mut had_errors = false;
    for (rule, group) in &grouped {
        let errs = group
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        let warns = group
            .iter()
            .filter(|f| f.severity == Severity::Warn)
            .count();
        if errs > 0 {
            had_errors = true;
        }
        println!(
            "{rule} ({} issue{}, {errs} error{}, {warns} warn{}):",
            group.len(),
            if group.len() == 1 { "" } else { "s" },
            if errs == 1 { "" } else { "s" },
            if warns == 1 { "" } else { "s" },
        );
        for f in group {
            let tag = match f.severity {
                Severity::Error => "error",
                Severity::Warn => "warn",
            };
            match &f.page {
                Some(p) => println!("  {tag}: {p} — {}", f.message),
                None => println!("  {tag}: {}", f.message),
            }
        }
        println!();
    }

    if had_errors {
        Ok(2)
    } else {
        Ok(0)
    }
}

#[derive(Debug)]
struct PageMeta {
    url: String,
    title: Option<String>,
    description: Option<String>,
    canonical: Option<String>,
    og_title: Option<String>,
    og_description: Option<String>,
    og_type: Option<String>,
    hreflang: Vec<String>,
    json_ld_blocks: Vec<String>,
    headings_in_order: Vec<&'static str>,
    h1_count: usize,
}

fn extract_meta(url: &str, html: &str) -> PageMeta {
    let doc = Html::parse_document(html);

    let title = doc
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty());

    let mut description: Option<String> = None;
    let mut og_title: Option<String> = None;
    let mut og_description: Option<String> = None;
    let mut og_type: Option<String> = None;
    for el in doc.select(&Selector::parse("meta").unwrap()) {
        let name = el.value().attr("name");
        let property = el.value().attr("property");
        let content = el
            .value()
            .attr("content")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if let Some(c) = content {
            match (name, property) {
                (Some("description"), _) => description = Some(c),
                (_, Some("og:title")) => og_title = Some(c),
                (_, Some("og:description")) => og_description = Some(c),
                (_, Some("og:type")) => og_type = Some(c),
                _ => {}
            }
        }
    }

    let canonical = doc
        .select(&Selector::parse(r#"link[rel="canonical"]"#).unwrap())
        .next()
        .and_then(|el| el.value().attr("href"))
        .map(|s| s.to_string());

    let mut hreflang: Vec<String> = Vec::new();
    for el in doc.select(&Selector::parse(r#"link[rel="alternate"][hreflang]"#).unwrap()) {
        if let Some(lang) = el.value().attr("hreflang") {
            hreflang.push(lang.to_string());
        }
    }

    let mut json_ld_blocks: Vec<String> = Vec::new();
    for el in doc.select(&Selector::parse(r#"script[type="application/ld+json"]"#).unwrap()) {
        json_ld_blocks.push(el.text().collect::<String>());
    }

    let mut headings_in_order: Vec<&'static str> = Vec::new();
    let mut h1_count = 0;
    for el in doc.select(&Selector::parse("h1, h2, h3").unwrap()) {
        let tag = match el.value().name() {
            "h1" => {
                h1_count += 1;
                "h1"
            }
            "h2" => "h2",
            "h3" => "h3",
            _ => continue,
        };
        headings_in_order.push(tag);
    }

    PageMeta {
        url: url.to_string(),
        title,
        description,
        canonical,
        og_title,
        og_description,
        og_type,
        hreflang,
        json_ld_blocks,
        headings_in_order,
        h1_count,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Severity {
    Error,
    Warn,
}

#[derive(Debug)]
struct Finding {
    rule: &'static str,
    severity: Severity,
    page: Option<String>,
    message: String,
}

fn rule_titles(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in metas {
        match &m.title {
            None => findings.push(Finding {
                rule: "title",
                severity: Severity::Error,
                page: Some(m.url.clone()),
                message: "missing <title>".to_string(),
            }),
            Some(t) => {
                let len = t.chars().count();
                if !(TITLE_MIN..=TITLE_MAX).contains(&len) {
                    findings.push(Finding {
                        rule: "title",
                        severity: Severity::Warn,
                        page: Some(m.url.clone()),
                        message: format!(
                            "title is {len} chars (recommend {TITLE_MIN}-{TITLE_MAX})"
                        ),
                    });
                }
                seen.entry(t.clone()).or_default().push(m.url.clone());
            }
        }
    }
    for (t, pages) in seen {
        if pages.len() > 1 {
            findings.push(Finding {
                rule: "title",
                severity: Severity::Warn,
                page: None,
                message: format!(
                    "duplicate title \"{t}\" on {} pages: {}",
                    pages.len(),
                    pages.join(", ")
                ),
            });
        }
    }
}

fn rule_descriptions(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in metas {
        match &m.description {
            None => findings.push(Finding {
                rule: "description",
                severity: Severity::Error,
                page: Some(m.url.clone()),
                message: r#"missing <meta name="description">"#.to_string(),
            }),
            Some(d) => {
                let len = d.chars().count();
                if !(DESCRIPTION_MIN..=DESCRIPTION_MAX).contains(&len) {
                    findings.push(Finding {
                        rule: "description",
                        severity: Severity::Warn,
                        page: Some(m.url.clone()),
                        message: format!(
                            "description is {len} chars (recommend {DESCRIPTION_MIN}-{DESCRIPTION_MAX})"
                        ),
                    });
                }
                seen.entry(d.clone()).or_default().push(m.url.clone());
            }
        }
    }
    for (d, pages) in seen {
        if pages.len() > 1 {
            findings.push(Finding {
                rule: "description",
                severity: Severity::Warn,
                page: None,
                message: format!(
                    "duplicate description on {} pages: {}",
                    pages.len(),
                    truncate(&d, 60)
                ),
            });
        }
    }
}

fn rule_canonicals(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    // Per-page presence.
    for m in metas {
        if m.canonical.is_none() {
            findings.push(Finding {
                rule: "canonical",
                severity: Severity::Error,
                page: Some(m.url.clone()),
                message: r#"missing <link rel="canonical">"#.to_string(),
            });
        }
    }
    // Site-wide scheme/host consistency. Bucket every canonical by its
    // origin (`scheme://host`); if more than one bucket exists, flag.
    let mut origins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in metas {
        if let Some(c) = &m.canonical {
            if let Some(origin) = origin_of(c) {
                origins.entry(origin).or_default().push(m.url.clone());
            }
        }
    }
    if origins.len() > 1 {
        let parts: Vec<String> = origins
            .iter()
            .map(|(o, p)| format!("{o} ({} pages)", p.len()))
            .collect();
        findings.push(Finding {
            rule: "canonical",
            severity: Severity::Error,
            page: None,
            message: format!(
                "scheme/host mismatch — pages declare {} different canonical origins: {}. Pick one.",
                origins.len(),
                parts.join(", ")
            ),
        });
    }
}

fn rule_og(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    for m in metas {
        if m.og_title.is_none() {
            findings.push(Finding {
                rule: "og",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: "missing og:title".to_string(),
            });
        }
        if m.og_description.is_none() {
            findings.push(Finding {
                rule: "og",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: "missing og:description".to_string(),
            });
        }
        if m.og_type.is_none() {
            findings.push(Finding {
                rule: "og",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: "missing og:type".to_string(),
            });
        }
    }
}

fn rule_hreflang(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    // Only check hreflang integrity if multiple langs are observed
    // across the site. Single-lang sites legitimately have no hreflang.
    let mut all_langs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for m in metas {
        for l in &m.hreflang {
            all_langs.insert(l.clone());
        }
    }
    if all_langs.len() < 2 {
        return;
    }

    for m in metas {
        if m.hreflang.is_empty() {
            findings.push(Finding {
                rule: "hreflang",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: format!(
                    "missing hreflang annotations (site declares {} langs: {})",
                    all_langs.len(),
                    all_langs.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
            });
            continue;
        }
        let page_langs: std::collections::BTreeSet<&String> = m.hreflang.iter().collect();
        let missing: Vec<String> = all_langs
            .iter()
            .filter(|l| !page_langs.contains(l))
            .cloned()
            .collect();
        if !missing.is_empty() {
            findings.push(Finding {
                rule: "hreflang",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: format!("missing hreflang for: {}", missing.join(", ")),
            });
        }
    }
}

fn rule_json_ld(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    for m in metas {
        for (i, body) in m.json_ld_blocks.iter().enumerate() {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                findings.push(Finding {
                    rule: "json-ld",
                    severity: Severity::Error,
                    page: Some(m.url.clone()),
                    message: format!("script[{i}] is empty"),
                });
                continue;
            }
            if let Err(e) = serde_json::from_str::<serde_json::Value>(trimmed) {
                findings.push(Finding {
                    rule: "json-ld",
                    severity: Severity::Error,
                    page: Some(m.url.clone()),
                    message: format!("script[{i}] invalid JSON: {e}"),
                });
            }
        }
    }
}

fn rule_heading_hierarchy(metas: &[PageMeta], findings: &mut Vec<Finding>) {
    for m in metas {
        if m.h1_count == 0 && !m.headings_in_order.is_empty() {
            findings.push(Finding {
                rule: "headings",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: "no <h1> on page (has subheadings)".to_string(),
            });
        }
        if m.h1_count > 1 {
            findings.push(Finding {
                rule: "headings",
                severity: Severity::Warn,
                page: Some(m.url.clone()),
                message: format!("{} <h1> elements (recommend 1)", m.h1_count),
            });
        }
        // First h2 before first h1 ⇒ disorder.
        let first_h1 = m.headings_in_order.iter().position(|t| *t == "h1");
        let first_h2 = m.headings_in_order.iter().position(|t| *t == "h2");
        let first_h3 = m.headings_in_order.iter().position(|t| *t == "h3");
        if let (Some(h1_pos), Some(h2_pos)) = (first_h1, first_h2) {
            if h2_pos < h1_pos {
                findings.push(Finding {
                    rule: "headings",
                    severity: Severity::Warn,
                    page: Some(m.url.clone()),
                    message: "<h2> appears before first <h1>".to_string(),
                });
            }
        }
        if let (Some(h2_pos), Some(h3_pos)) = (first_h2, first_h3) {
            if h3_pos < h2_pos {
                findings.push(Finding {
                    rule: "headings",
                    severity: Severity::Warn,
                    page: Some(m.url.clone()),
                    message: "<h3> appears before first <h2>".to_string(),
                });
            }
        }
    }
}

fn display_url(path: &Path, scan_root: &Path) -> String {
    let rel = path.strip_prefix(scan_root).unwrap_or(path);
    format!("/{}", rel.display())
}

/// Extract `scheme://host` from a URL string. Returns `None` for
/// non-URL inputs (relative paths etc.).
fn origin_of(url: &str) -> Option<String> {
    let s = url.trim();
    let (scheme_end, _) = s.split_once("://")?;
    let after = &s[scheme_end.len() + 3..];
    let host_end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let host = &after[..host_end];
    Some(format!("{scheme_end}://{host}"))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(n).collect();
        format!("\"{truncated}...\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_basic_meta() {
        let html = r#"<!DOCTYPE html><html><head>
<title>Hello World</title>
<meta name="description" content="A demo page">
<meta property="og:title" content="OG Title">
<meta property="og:description" content="OG Desc">
<meta property="og:type" content="website">
<link rel="canonical" href="https://example.com/">
<link rel="alternate" hreflang="en" href="https://example.com/">
<link rel="alternate" hreflang="sv" href="https://example.com/sv/">
</head><body><h1>Hi</h1><h2>Sub</h2></body></html>"#;
        let m = extract_meta("/index.html", html);
        assert_eq!(m.title.as_deref(), Some("Hello World"));
        assert_eq!(m.description.as_deref(), Some("A demo page"));
        assert_eq!(m.og_title.as_deref(), Some("OG Title"));
        assert_eq!(m.og_type.as_deref(), Some("website"));
        assert_eq!(m.canonical.as_deref(), Some("https://example.com/"));
        assert_eq!(m.hreflang.len(), 2);
        assert_eq!(m.h1_count, 1);
    }

    #[test]
    fn origin_of_extracts_scheme_host() {
        assert_eq!(
            origin_of("https://www.example.com/foo"),
            Some("https://www.example.com".to_string())
        );
        assert_eq!(
            origin_of("https://example.com/"),
            Some("https://example.com".to_string())
        );
        assert_eq!(origin_of("/foo"), None);
        assert_eq!(origin_of(""), None);
    }

    #[test]
    fn rule_canonicals_flags_origin_mismatch() {
        let metas = vec![
            PageMeta {
                url: "/a.html".into(),
                title: None,
                description: None,
                canonical: Some("https://www.example.com/a".into()),
                og_title: None,
                og_description: None,
                og_type: None,
                hreflang: vec![],
                json_ld_blocks: vec![],
                headings_in_order: vec![],
                h1_count: 0,
            },
            PageMeta {
                url: "/b.html".into(),
                title: None,
                description: None,
                canonical: Some("https://example.com/b".into()),
                og_title: None,
                og_description: None,
                og_type: None,
                hreflang: vec![],
                json_ld_blocks: vec![],
                headings_in_order: vec![],
                h1_count: 0,
            },
        ];
        let mut findings = Vec::new();
        rule_canonicals(&metas, &mut findings);
        assert!(
            findings
                .iter()
                .any(|f| f.severity == Severity::Error && f.message.contains("scheme/host")),
            "canonical-origin-mismatch finding must fire: {findings:?}"
        );
    }

    #[test]
    fn rule_json_ld_flags_invalid() {
        let metas = vec![PageMeta {
            url: "/a.html".into(),
            title: None,
            description: None,
            canonical: None,
            og_title: None,
            og_description: None,
            og_type: None,
            hreflang: vec![],
            json_ld_blocks: vec!["{\"@type\": Hotel}".into()], // missing quotes
            headings_in_order: vec![],
            h1_count: 0,
        }];
        let mut findings = Vec::new();
        rule_json_ld(&metas, &mut findings);
        assert!(findings.iter().any(|f| f.rule == "json-ld"));
    }
}
