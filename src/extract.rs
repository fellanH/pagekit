use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use lol_html::html_content::{ContentType, Element, EndTag};
use lol_html::{ElementContentHandlers, HtmlRewriter, Selector as LolSelector, Settings};
use scraper::{Html, Selector as ScraperSelector};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use walkdir::WalkDir;

/// A DOM block found shared across pages, ready to extract into a fragment.
struct SharedBlock {
    name: String,
    /// CSS selector used to find this element in parsed pages (preserved
    /// so per-page presence checks use the original selector, not a
    /// hardcoded mapping from tag back to class).
    selector: String,
    /// scraper's `.html()` output — the canonical content written to the
    /// fragment file and used to identify the matching element among
    /// same-selector siblings on a given page.
    content: String,
}

pub(crate) fn collect_html_files(
    root: &Path,
    fragments_dir: &Path,
    exclude_dirs: &[String],
    max_depth: usize,
) -> Vec<PathBuf> {
    let excluded: Vec<PathBuf> = exclude_dirs.iter().map(|d| root.join(d)).collect();

    WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            !p.starts_with(fragments_dir) && !excluded.iter().any(|ex| p.starts_with(ex))
        })
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().map(|x| x == "html").unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect()
}

/// Per-page rewrite: insert `<!-- prefix:name -->...<!-- /prefix:name -->`
/// markers around the canonical occurrence of every shared block.
///
/// scraper picks the *sibling-index* of the matching element (e.g. "wrap
/// the 2nd `<footer>`"); lol_html walks the source via CSS selector,
/// counts hits, and wraps the matching index with `before()` + an
/// `on_end_tag()` `after()`. No source-vs-DOM reconciliation: lol_html
/// operates on source bytes directly, so attribute order and whitespace
/// in the original page are preserved verbatim.
fn rewrite_page(src: &str, blocks: &[SharedBlock], prefix: &str) -> Result<String> {
    let doc = Html::parse_document(src);

    // Group target sibling-indices by selector. Skip blocks already marked
    // (idempotent) and blocks whose canonical content isn't present on
    // this page.
    let mut by_selector: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for block in blocks {
        let open_marker = format!("<!-- {}:{} -->", prefix, block.name);
        if src.contains(&open_marker) {
            continue;
        }
        let scraper_sel = match ScraperSelector::parse(&block.selector) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut idx_match: Option<usize> = None;
        for (i, el) in doc.select(&scraper_sel).enumerate() {
            if el.html() == block.content {
                idx_match = Some(i);
                break;
            }
        }
        if let Some(idx) = idx_match {
            by_selector
                .entry(block.selector.clone())
                .or_default()
                .push((idx, block.name.clone()));
        }
    }

    if by_selector.is_empty() {
        return Ok(src.to_string());
    }

    let mut handlers: Vec<(Cow<LolSelector>, ElementContentHandlers)> = Vec::new();
    for (sel_str, targets) in by_selector {
        let lol_sel: LolSelector = sel_str
            .parse()
            .map_err(|e| anyhow!("invalid selector '{}': {:?}", sel_str, e))?;
        let counter = Rc::new(RefCell::new(0usize));
        let prefix_owned = prefix.to_string();

        let counter_clone = Rc::clone(&counter);
        let element_handler = move |el: &mut Element<'_, '_>| -> lol_html::HandlerResult {
            let i = *counter_clone.borrow();
            *counter_clone.borrow_mut() += 1;
            for (target_idx, name) in &targets {
                if i == *target_idx {
                    let open = format!("<!-- {}:{} -->\n", prefix_owned, name);
                    el.before(&open, ContentType::Html);
                    let close = format!("\n<!-- /{}:{} -->", prefix_owned, name);
                    el.on_end_tag(Box::new(move |end: &mut EndTag<'_>| {
                        end.after(&close, ContentType::Html);
                        Ok(())
                    }))?;
                    break;
                }
            }
            Ok(())
        };

        handlers.push((
            Cow::Owned(lol_sel),
            ElementContentHandlers::default().element(element_handler),
        ));
    }

    let mut output = Vec::new();
    {
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: handlers,
                ..Settings::new()
            },
            |c: &[u8]| output.extend_from_slice(c),
        );
        rewriter
            .write(src.as_bytes())
            .context("lol_html write failed")?;
        rewriter.end().context("lol_html end failed")?;
    }
    String::from_utf8(output).context("lol_html output was not valid utf-8")
}

/// Scan HTML files in a site directory, detect shared DOM blocks,
/// extract them to <fragments_dir>/*.html, and insert marker comments.
pub fn extract_fragments(root: &Path, config: &Config) -> Result<usize> {
    let fragments_dir = root.join(&config.core.fragments_dir);

    let html_files = collect_html_files(
        root,
        &fragments_dir,
        &config.core.exclude_dirs,
        config.core.max_depth,
    );

    if html_files.len() < 2 {
        println!("  Less than 2 HTML pages, skipping extraction.");
        return Ok(0);
    }

    let pages: Vec<_> = html_files
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok().map(|c| (p.clone(), c)))
        .collect();

    // Candidate selectors: built-ins + any user-provided. User entries
    // append to defaults — adding one custom doesn't lose the built-ins.
    // The `tag` field on user candidates is accepted for backward compat
    // but no longer consumed: lol_html resolves the source span from the
    // selector alone.
    let mut candidates: Vec<(String, String)> = vec![
        ("nav".into(), "nav".into()),
        ("footer".into(), "footer".into()),
        ("header".into(), "header".into()),
        ("navbar".into(), ".navbar".into()),
        ("site-header".into(), ".site-header".into()),
        ("site-footer".into(), ".site-footer".into()),
    ];
    for c in &config.extract.candidates {
        candidates.push((c.name.clone(), c.selector.clone()));
    }

    let mut shared_blocks: Vec<SharedBlock> = Vec::new();

    for (name, sel_str) in &candidates {
        let sel = match ScraperSelector::parse(sel_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut html_to_count: HashMap<String, usize> = HashMap::new();

        for (_path, content) in &pages {
            let doc = Html::parse_document(content);
            let mut seen: HashSet<String> = HashSet::new();
            for el in doc.select(&sel) {
                let outer = el.html();
                if seen.insert(outer.clone()) {
                    *html_to_count.entry(outer).or_insert(0) += 1;
                }
            }
        }

        if let Some((content, count)) = html_to_count.into_iter().max_by_key(|(_, v)| *v) {
            if count >= 2 {
                shared_blocks.push(SharedBlock {
                    name: name.clone(),
                    selector: sel_str.clone(),
                    content,
                });
            }
        }
    }

    if shared_blocks.is_empty() {
        println!("  No shared blocks detected.");
        return Ok(0);
    }

    fs::create_dir_all(&fragments_dir)?;

    for block in &shared_blocks {
        let frag_path = fragments_dir.join(format!("{}.html", block.name));
        fs::write(&frag_path, &block.content)
            .with_context(|| format!("writing {}", frag_path.display()))?;
        println!(
            "  Extracted: {}/{}.html",
            config.core.fragments_dir, block.name
        );
    }

    let prefix = &config.core.marker_prefix;
    let mut modified_count = 0;

    for (path, content) in &pages {
        let new_content = rewrite_page(content, &shared_blocks, prefix)?;
        if new_content != *content {
            fs::write(path, &new_content).with_context(|| format!("writing {}", path.display()))?;
            modified_count += 1;
        }
    }

    println!(
        "  {} fragment(s) extracted, {} page(s) marked.",
        shared_blocks.len(),
        modified_count
    );

    Ok(modified_count)
}
