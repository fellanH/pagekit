use crate::config::Config;
use crate::extract::collect_html_files;
use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Generic link-text strings that read as "click here". Case-insensitive
/// match. Used by the `link-text` rule to flag uninformative anchors.
const GENERIC_LINK_TEXTS: &[&str] = &[
    "click here",
    "click",
    "here",
    "read more",
    "more",
    "learn more",
    "this",
    "this link",
];

/// Run `pagekit a11y`. Subset of WCAG checks doable without rendering.
/// Color contrast, focus-visible styles, and dynamic ARIA semantics
/// are explicitly NOT covered. Pass means "cheap checks pass", not
/// "WCAG compliant".
pub fn run_a11y(root: &Path, config: &Config) -> Result<i32> {
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

    let mut findings: Vec<Finding> = Vec::new();
    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let url = display_url(path, &scan_root);
        let doc = Html::parse_document(&content);
        check_html_lang(&url, &doc, &mut findings);
        check_image_alts(&url, &doc, &mut findings);
        check_form_labels(&url, &doc, &mut findings);
        check_empty_interactives(&url, &doc, &mut findings);
        check_link_text(&url, &doc, &mut findings);
    }

    if findings.is_empty() {
        println!(
            "pagekit: a11y checks pass on {} page(s) (subset only — see --help)",
            html_files.len()
        );
        return Ok(0);
    }

    let mut grouped: BTreeMap<&'static str, Vec<&Finding>> = BTreeMap::new();
    for f in &findings {
        grouped.entry(f.rule).or_default().push(f);
    }

    for (rule, group) in &grouped {
        println!(
            "{rule} ({} issue{}):",
            group.len(),
            if group.len() == 1 { "" } else { "s" }
        );
        for f in group {
            println!("  {} — {}", f.page, f.message);
        }
        println!();
    }
    println!(
        "(scope note: pagekit a11y covers the grep-able WCAG subset only. \
Color contrast, focus-visible styles, dynamic ARIA semantics, and \
SVG-icon accessible-name resolution need rendering and are NOT checked.)"
    );

    Ok(2)
}

#[derive(Debug)]
struct Finding {
    rule: &'static str,
    page: String,
    message: String,
}

fn check_html_lang(url: &str, doc: &Html, findings: &mut Vec<Finding>) {
    let html_el = doc.select(&Selector::parse("html").unwrap()).next();
    let has_lang = html_el
        .and_then(|el| el.value().attr("lang"))
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_lang {
        findings.push(Finding {
            rule: "html-lang",
            page: url.to_string(),
            message: "missing lang attribute on <html>".to_string(),
        });
    }
}

fn check_image_alts(url: &str, doc: &Html, findings: &mut Vec<Finding>) {
    for el in doc.select(&Selector::parse("img").unwrap()) {
        // alt="" is valid (decorative); MISSING alt is the violation.
        if el.value().attr("alt").is_none() {
            let src = el.value().attr("src").unwrap_or("(no src)");
            findings.push(Finding {
                rule: "img-alt",
                page: url.to_string(),
                message: format!("<img src=\"{}\"> has no alt attribute", truncate(src, 80)),
            });
        }
    }
}

fn check_form_labels(url: &str, doc: &Html, findings: &mut Vec<Finding>) {
    // Collect every label[for=...] target id on the page.
    let label_for_targets: std::collections::BTreeSet<String> = doc
        .select(&Selector::parse("label[for]").unwrap())
        .filter_map(|el| el.value().attr("for"))
        .map(|s| s.to_string())
        .collect();

    let mut check_field = |el: ElementRef<'_>, tag: &str| {
        let attrs = el.value();

        // Skip non-interactive input types.
        if tag == "input" {
            let typ = attrs.attr("type").unwrap_or("text");
            if matches!(
                typ,
                "submit" | "button" | "reset" | "hidden" | "image" | "file"
            ) {
                return;
            }
        }

        // Labeled if: aria-label, aria-labelledby, has matching label[for],
        // OR is wrapped inside a <label> element.
        if attrs
            .attr("aria-label")
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        {
            return;
        }
        if attrs.attr("aria-labelledby").is_some() {
            return;
        }
        if let Some(id) = attrs.attr("id") {
            if label_for_targets.contains(id) {
                return;
            }
        }
        // Wrapped: any ancestor is a <label>.
        let mut p = el.parent();
        while let Some(node) = p {
            if let Some(parent_el) = ElementRef::wrap(node) {
                if parent_el.value().name() == "label" {
                    return;
                }
                p = parent_el.parent();
            } else {
                break;
            }
        }

        let kind = el.value().attr("name").unwrap_or(tag);
        findings.push(Finding {
            rule: "form-label",
            page: url.to_string(),
            message: format!("<{tag} name=\"{kind}\"> has no associated label"),
        });
    };

    for el in doc.select(&Selector::parse("input").unwrap()) {
        check_field(el, "input");
    }
    for el in doc.select(&Selector::parse("textarea").unwrap()) {
        check_field(el, "textarea");
    }
    for el in doc.select(&Selector::parse("select").unwrap()) {
        check_field(el, "select");
    }
}

fn check_empty_interactives(url: &str, doc: &Html, findings: &mut Vec<Finding>) {
    for tag in ["a", "button"] {
        for el in doc.select(&Selector::parse(tag).unwrap()) {
            // aria-label takes precedence; if present we accept.
            if el
                .value()
                .attr("aria-label")
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            {
                continue;
            }
            // Image-only links: an <img alt="..."> child is acceptable
            // (the alt provides the accessible name).
            let has_img_with_alt = el.select(&Selector::parse("img[alt]").unwrap()).any(|img| {
                img.value()
                    .attr("alt")
                    .map(|a| !a.trim().is_empty())
                    .unwrap_or(false)
            });
            if has_img_with_alt {
                continue;
            }
            // SVG icons are the dominant pattern in framework exports
            // (Webflow, Bootstrap). Flagging every SVG-only link
            // produces noise without signal at this scope. Skip and
            // document the limitation in the output footer.
            // Proper accessible-name resolution for SVG (aria-label on
            // link, <title> inside svg, aria-labelledby) is rendering-
            // class work, not grep-able subset.
            let has_svg_child = el.select(&Selector::parse("svg").unwrap()).next().is_some();
            if has_svg_child {
                continue;
            }
            let text = el.text().collect::<String>().trim().to_string();
            if text.is_empty() {
                findings.push(Finding {
                    rule: "empty-interactive",
                    page: url.to_string(),
                    message: format!("<{tag}> has no text content and no aria-label"),
                });
            }
        }
    }
}

fn check_link_text(url: &str, doc: &Html, findings: &mut Vec<Finding>) {
    for el in doc.select(&Selector::parse("a").unwrap()) {
        let text = el.text().collect::<String>().trim().to_lowercase();
        if text.is_empty() {
            continue; // covered by empty-interactive
        }
        if GENERIC_LINK_TEXTS.iter().any(|g| text == *g) {
            findings.push(Finding {
                rule: "link-text",
                page: url.to_string(),
                message: format!("<a> has generic text \"{text}\" (use descriptive text)"),
            });
        }
    }
}

fn display_url(path: &Path, scan_root: &Path) -> String {
    let rel = path.strip_prefix(scan_root).unwrap_or(path);
    format!("/{}", rel.display())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(n).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> Html {
        Html::parse_document(html)
    }

    #[test]
    fn html_lang_missing_flagged() {
        let doc = parse("<!DOCTYPE html><html><body></body></html>");
        let mut f = Vec::new();
        check_html_lang("/x.html", &doc, &mut f);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "html-lang");
    }

    #[test]
    fn html_lang_present_passes() {
        let doc = parse("<!DOCTYPE html><html lang=\"en\"><body></body></html>");
        let mut f = Vec::new();
        check_html_lang("/x.html", &doc, &mut f);
        assert!(f.is_empty());
    }

    #[test]
    fn img_missing_alt_flagged() {
        let doc = parse(r#"<html><body><img src="/a.png"></body></html>"#);
        let mut f = Vec::new();
        check_image_alts("/x.html", &doc, &mut f);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "img-alt");
    }

    #[test]
    fn img_with_empty_alt_passes_decorative() {
        let doc = parse(r#"<html><body><img src="/a.png" alt=""></body></html>"#);
        let mut f = Vec::new();
        check_image_alts("/x.html", &doc, &mut f);
        assert!(f.is_empty(), "alt=\"\" is the decorative-image idiom");
    }

    #[test]
    fn input_with_aria_label_passes() {
        let doc = parse(
            r#"<html><body><form><input type="text" aria-label="Email"></form></body></html>"#,
        );
        let mut f = Vec::new();
        check_form_labels("/x.html", &doc, &mut f);
        assert!(f.is_empty());
    }

    #[test]
    fn input_unlabeled_flagged() {
        let doc =
            parse(r#"<html><body><form><input type="text" name="email"></form></body></html>"#);
        let mut f = Vec::new();
        check_form_labels("/x.html", &doc, &mut f);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "form-label");
    }

    #[test]
    fn input_with_label_for_passes() {
        let doc = parse(
            r#"<html><body><form>
<label for="email-field">Email</label>
<input type="text" id="email-field" name="email">
</form></body></html>"#,
        );
        let mut f = Vec::new();
        check_form_labels("/x.html", &doc, &mut f);
        assert!(f.is_empty());
    }

    #[test]
    fn submit_input_skipped() {
        let doc =
            parse(r#"<html><body><form><input type="submit" value="Send"></form></body></html>"#);
        let mut f = Vec::new();
        check_form_labels("/x.html", &doc, &mut f);
        assert!(f.is_empty(), "type=submit doesn't need a label");
    }

    #[test]
    fn empty_anchor_flagged() {
        let doc = parse(r#"<html><body><a href="/x"></a></body></html>"#);
        let mut f = Vec::new();
        check_empty_interactives("/x.html", &doc, &mut f);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "empty-interactive");
    }

    #[test]
    fn anchor_with_aria_label_passes() {
        let doc = parse(r#"<html><body><a href="/x" aria-label="Home"></a></body></html>"#);
        let mut f = Vec::new();
        check_empty_interactives("/x.html", &doc, &mut f);
        assert!(f.is_empty());
    }

    #[test]
    fn anchor_with_alt_image_passes() {
        let doc =
            parse(r#"<html><body><a href="/x"><img src="/y.png" alt="Home"></a></body></html>"#);
        let mut f = Vec::new();
        check_empty_interactives("/x.html", &doc, &mut f);
        assert!(
            f.is_empty(),
            "<img alt=\"...\"> child is the accessible name"
        );
    }

    #[test]
    fn generic_link_text_flagged() {
        let doc = parse(r#"<html><body><a href="/x">click here</a></body></html>"#);
        let mut f = Vec::new();
        check_link_text("/x.html", &doc, &mut f);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "link-text");
    }
}
