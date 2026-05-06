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
    /// Fragment-file name. Equal to `base_name` for single-variant blocks;
    /// `base_name-N` (1-indexed by descending frequency) under
    /// `--split-variants` when a candidate has multiple distinct contents.
    name: String,
    /// Candidate name (e.g. "nav") shared by all variants of the same
    /// candidate. Used for idempotency checks so re-running across modes
    /// doesn't double-wrap a page that already carries a sibling marker.
    base_name: String,
    /// CSS selector used to find this element in parsed pages (preserved
    /// so per-page presence checks use the original selector, not a
    /// hardcoded mapping from tag back to class).
    selector: String,
    /// scraper's `.html()` output — the canonical content written to the
    /// fragment file and used to identify the matching element among
    /// same-selector siblings on a given page.
    content: String,
}

/// True if `src` already carries a marker for the candidate `base` —
/// either `<!-- prefix:base -->` (plain extract) or
/// `<!-- prefix:base-<digits> -->` (a sibling variant). Used so a re-run
/// in either mode skips pages already wrapped by the other mode rather
/// than nesting markers.
fn page_has_base_marker(src: &str, prefix: &str, base: &str) -> bool {
    let needle = format!("<!-- {prefix}:{base}");
    let mut cursor = 0;
    while let Some(pos) = src[cursor..].find(&needle) {
        let after = cursor + pos + needle.len();
        let rest = &src[after..];
        if let Some(stripped) = rest.strip_prefix(" -->") {
            let _ = stripped;
            return true;
        }
        if let Some(rest) = rest.strip_prefix('-') {
            let digit_end = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if digit_end > 0 && rest[digit_end..].starts_with(" -->") {
                return true;
            }
        }
        cursor = after;
    }
    false
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
        if page_has_base_marker(src, prefix, &block.base_name) {
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
///
/// `split_variants`: when true, candidates with multiple distinct content
/// variants on ≥2 pages each emit one fragment file per variant
/// (`<name>-1.html` … `<name>-N.html`, ranked by descending page count)
/// and per-page markers point at the variant matching that page's
/// content. When false, only the dominant variant ships and a one-line
/// warning surfaces per multi-variant candidate.
pub fn extract_fragments(root: &Path, config: &Config, split_variants: bool) -> Result<usize> {
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
    let mut variant_warnings: Vec<String> = Vec::new();

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

        // Variants are distinct contents present on ≥2 pages each. Sort
        // by descending page count, then ascending content for stable
        // ordering when counts tie.
        let mut variants: Vec<(String, usize)> = html_to_count
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .collect();
        variants.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        match variants.len() {
            0 => {}
            1 => {
                let (content, _) = variants.into_iter().next().unwrap();
                shared_blocks.push(SharedBlock {
                    name: name.clone(),
                    base_name: name.clone(),
                    selector: sel_str.clone(),
                    content,
                });
            }
            n if split_variants => {
                for (i, (content, _)) in variants.into_iter().enumerate() {
                    shared_blocks.push(SharedBlock {
                        name: format!("{}-{}", name, i + 1),
                        base_name: name.clone(),
                        selector: sel_str.clone(),
                        content,
                    });
                }
                let _ = n;
            }
            n => {
                let (content, _) = variants.into_iter().next().unwrap();
                shared_blocks.push(SharedBlock {
                    name: name.clone(),
                    base_name: name.clone(),
                    selector: sel_str.clone(),
                    content,
                });
                variant_warnings.push(format!(
                    "  Note: {name} has {n} variants on ≥2 pages — keeping dominant only. Use --split-variants to emit all.",
                ));
            }
        }
    }

    for w in &variant_warnings {
        println!("{w}");
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
