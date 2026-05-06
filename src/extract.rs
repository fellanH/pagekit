use crate::config::Config;
use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A DOM block found shared across pages, ready to extract into a fragment.
struct SharedBlock {
    name: String,
    /// CSS selector used to find this element in parsed pages (preserved
    /// so per-page presence checks use the original selector, not a
    /// hardcoded mapping from tag back to class).
    selector: String,
    /// HTML tag name (used for raw-source span matching).
    tag: String,
    /// Scraper's `.html()` output — the canonical content written to the
    /// fragment file and compared against on per-page matching.
    content: String,
}

/// Find the source span of the FIRST top-level `<tag ...>...</tag>` element
/// in `src`. Returns (start, end) byte offsets. Uses a depth counter to
/// handle nested same-name tags.
fn find_first_tag_span(src: &str, tag: &str) -> Option<(usize, usize)> {
    let open_prefix = format!("<{}", tag);
    let close_tag = format!("</{}>", tag);

    let start = src.find(&open_prefix)?;
    let after = src.as_bytes().get(start + open_prefix.len())?;
    if !matches!(after, b' ' | b'>' | b'/' | b'\n' | b'\r' | b'\t') {
        return None;
    }

    let mut depth = 0i32;
    let haystack = &src[start..];

    let mut idx = 0;
    while idx < haystack.len() {
        if haystack[idx..].starts_with(&open_prefix) {
            let after_idx = idx + open_prefix.len();
            if after_idx < haystack.len() {
                let ch = haystack.as_bytes()[after_idx];
                if matches!(ch, b' ' | b'>' | b'/' | b'\n' | b'\r' | b'\t') {
                    depth += 1;
                }
            }
            idx += open_prefix.len();
        } else if haystack[idx..].starts_with(&close_tag) {
            depth -= 1;
            if depth == 0 {
                let end = start + idx + close_tag.len();
                return Some((start, end));
            }
            idx += close_tag.len();
        } else {
            idx += haystack[idx..].chars().next().map_or(1, |c| c.len_utf8());
        }
    }
    None
}

/// Find the source span of the top-level `<tag>...</tag>` whose parsed
/// outer-HTML equals `expected`. Walks every top-level same-tag occurrence.
fn find_matching_tag_span(src: &str, tag: &str, expected: &str) -> Option<(usize, usize)> {
    let Ok(sel) = Selector::parse(tag) else {
        return None;
    };
    let mut from = 0;
    while from < src.len() {
        let (rel_start, rel_end) = find_first_tag_span(&src[from..], tag)?;
        let abs_start = from + rel_start;
        let abs_end = from + rel_end;
        let candidate = &src[abs_start..abs_end];
        let frag = Html::parse_fragment(candidate);
        if frag.select(&sel).any(|el| el.html() == expected) {
            return Some((abs_start, abs_end));
        }
        from = abs_end;
    }
    None
}

fn collect_html_files(
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
    let mut candidates: Vec<(String, String, String)> = vec![
        ("nav".into(), "nav".into(), "nav".into()),
        ("footer".into(), "footer".into(), "footer".into()),
        ("header".into(), "header".into(), "header".into()),
        ("navbar".into(), ".navbar".into(), "div".into()),
        ("site-header".into(), ".site-header".into(), "div".into()),
        ("site-footer".into(), ".site-footer".into(), "div".into()),
    ];
    for c in &config.extract.candidates {
        candidates.push((c.name.clone(), c.selector.clone(), c.tag.clone()));
    }

    let mut shared_blocks: Vec<SharedBlock> = Vec::new();

    for (name, sel_str, tag_name) in &candidates {
        let sel = match Selector::parse(sel_str) {
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
                    tag: tag_name.clone(),
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
        let doc = Html::parse_document(content);
        let mut modified = content.clone();
        let mut changed = false;

        for block in &shared_blocks {
            let open_marker = format!("<!-- {}:{} -->", prefix, block.name);
            let close_marker = format!("<!-- /{}:{} -->", prefix, block.name);

            if modified.contains(&open_marker) {
                continue;
            }

            let sel = match Selector::parse(&block.selector) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let has_block = doc.select(&sel).any(|el| el.html() == block.content);
            if !has_block {
                continue;
            }

            if let Some((start, end)) =
                find_matching_tag_span(&modified, &block.tag, &block.content)
            {
                let raw_block = &modified[start..end];
                let replacement = format!("{open_marker}\n{raw_block}\n{close_marker}");
                modified = format!("{}{}{}", &modified[..start], replacement, &modified[end..]);
                changed = true;
            }
        }

        if changed {
            fs::write(path, &modified).with_context(|| format!("writing {}", path.display()))?;
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
