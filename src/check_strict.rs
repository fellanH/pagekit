use crate::config::Config;
use crate::extract::collect_html_files;
use anyhow::{anyhow, Context, Result};
use scraper::{Html, Selector};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// FNV-1a 64-bit, truncated to top 32 bits as 8 hex chars. Stable across
/// builds and platforms (no compiler-version dependency, no new crate).
/// Collision probability on N≈30 marker regions is ~1/2^32 — negligible
/// for the diagnostic use case; sha-256 would buy nothing here.
fn hash8(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (h >> 32) as u32)
}

/// Walk `html` and return one `(name, hash)` tuple per matched
/// `<!-- {prefix}:NAME -->...<!-- /{prefix}:NAME -->` pair.
///
/// Inner bytes are captured verbatim (between the opening comment's
/// trailing ` -->` and the closing comment's leading `<`). Pairs nest;
/// unpaired closes are ignored silently — fragments core's regular
/// `check` already surfaces those as `UnpairedClose`.
fn capture_marker_hashes(html: &str, prefix: &str) -> Vec<(String, String)> {
    let open_prefix = format!("<!-- {prefix}:");
    let close_prefix = format!("<!-- /{prefix}:");
    let suffix = " -->";

    enum Tok {
        Open(String, usize),  // name, content_start (byte after ` -->`)
        Close(String, usize), // name, marker_start (byte at `<`)
    }
    let mut tokens: Vec<Tok> = Vec::new();
    let mut idx = 0;
    while idx < html.len() {
        let next_open = html[idx..].find(&open_prefix);
        let next_close = html[idx..].find(&close_prefix);
        let (start, is_open, plen) = match (next_open, next_close) {
            (None, None) => break,
            (Some(o), None) => (idx + o, true, open_prefix.len()),
            (None, Some(c)) => (idx + c, false, close_prefix.len()),
            (Some(o), Some(c)) => {
                let oa = idx + o;
                let ca = idx + c;
                if oa < ca {
                    (oa, true, open_prefix.len())
                } else {
                    (ca, false, close_prefix.len())
                }
            }
        };
        let name_start = start + plen;
        let Some(suffix_off) = html[name_start..].find(suffix) else {
            break;
        };
        let raw_name = html[name_start..name_start + suffix_off].trim();
        let comment_end = name_start + suffix_off + suffix.len();
        if !raw_name.is_empty()
            && raw_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            if is_open {
                tokens.push(Tok::Open(raw_name.to_string(), comment_end));
            } else {
                tokens.push(Tok::Close(raw_name.to_string(), start));
            }
        }
        idx = comment_end;
    }

    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<(String, usize)> = Vec::new();
    for tok in tokens {
        match tok {
            Tok::Open(name, content_start) => stack.push((name, content_start)),
            Tok::Close(name, close_pos) => {
                if let Some(pos) = stack.iter().rposition(|(n, _)| n == &name) {
                    let (n, content_start) = stack.remove(pos);
                    let inner = &html[content_start..close_pos];
                    out.push((n, hash8(inner.as_bytes())));
                }
            }
        }
    }
    out
}

/// Run `pagekit check --strict`, marker-region mode (the default).
/// Hashes content inside `<!-- prefix:NAME -->...<!-- /prefix:NAME -->`
/// pairs and reports per-name variance across pages.
pub fn run_check_strict(root: &Path, config: &Config, name_filter: Option<&str>) -> Result<i32> {
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

    // name -> hash -> pages (paths relative to scan_root, with leading `/`).
    let mut by_name: BTreeMap<String, BTreeMap<String, Vec<PathBuf>>> = BTreeMap::new();
    let prefix = &config.core.marker_prefix;

    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        for (name, hash) in capture_marker_hashes(&content, prefix) {
            if let Some(f) = name_filter {
                if name != f {
                    continue;
                }
            }
            let rel = path.strip_prefix(&scan_root).unwrap_or(path).to_path_buf();
            by_name
                .entry(name)
                .or_default()
                .entry(hash)
                .or_default()
                .push(rel);
        }
    }

    if by_name.is_empty() {
        match name_filter {
            Some(f) => println!("pagekit: no marker regions found for '{f}'"),
            None => println!("pagekit: no marker regions found"),
        }
        return Ok(0);
    }

    // Header + per-fragment row.
    let name_w = by_name.keys().map(|s| s.len()).max().unwrap_or(8).max(8);
    println!(
        "{:<name_w$}  {:>5}  {:>8}  status",
        "fragment",
        "pages",
        "variants",
        name_w = name_w,
    );
    let mut had_variance = false;
    for (name, hashes) in &by_name {
        let pages: usize = hashes.values().map(|v| v.len()).sum();
        let variants = hashes.len();
        let status = if variants > 1 {
            "⚠ varies"
        } else {
            "✓ uniform"
        };
        if variants > 1 {
            had_variance = true;
        }
        println!(
            "{:<name_w$}  {:>5}  {:>8}  {}",
            name,
            pages,
            variants,
            status,
            name_w = name_w
        );
    }

    if !had_variance {
        return Ok(0);
    }

    // Variance detail: per-fragment, per-hash, sample of pages.
    println!();
    for (name, hashes) in &by_name {
        if hashes.len() <= 1 {
            continue;
        }
        println!("⚠ {} has {} variants:", name, hashes.len());
        // Sort variants by descending page count, then hash for stability.
        let mut groups: Vec<(&String, &Vec<PathBuf>)> = hashes.iter().collect();
        groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
        for (h, pages) in groups {
            let sample: Vec<String> = pages
                .iter()
                .take(5)
                .map(|p| format!("/{}", p.display()))
                .collect();
            let suffix = if pages.len() > 5 {
                format!(", … (+{} more)", pages.len() - 5)
            } else {
                String::new()
            };
            println!(
                "  hash {} ({} pages): {}{}",
                h,
                pages.len(),
                sample.join(", "),
                suffix
            );
        }
    }

    Ok(2)
}

/// Run `pagekit check --strict --selector "..."`. Generalizes the
/// marker-region check to arbitrary CSS selectors: walk every page,
/// hash the concatenated outer-HTML of all matching elements, group
/// pages by hash. Different hashes ⇒ variance.
///
/// Multiple matches on a single page are concatenated in document
/// order before hashing, so a page with 2 matching elements and a
/// page with 1 produce different hashes (the count varies).
pub fn run_check_strict_selector(root: &Path, config: &Config, selector_str: &str) -> Result<i32> {
    let parsed = Selector::parse(selector_str)
        .map_err(|e| anyhow!("invalid CSS selector '{selector_str}': {e:?}"))?;

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

    // hash → (match-count, pages with that hash)
    let mut by_hash: BTreeMap<String, (usize, Vec<PathBuf>)> = BTreeMap::new();
    let mut pages_with_no_match: Vec<PathBuf> = Vec::new();

    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let doc = Html::parse_document(&content);
        let matches: Vec<String> = doc.select(&parsed).map(|el| el.html()).collect();
        let rel = path.strip_prefix(&scan_root).unwrap_or(path).to_path_buf();
        if matches.is_empty() {
            pages_with_no_match.push(rel);
            continue;
        }
        let concat = matches.join("\n");
        let hash = hash8(concat.as_bytes());
        let entry = by_hash.entry(hash).or_insert((matches.len(), Vec::new()));
        entry.1.push(rel);
    }

    if by_hash.is_empty() && pages_with_no_match.is_empty() {
        println!("pagekit: no pages scanned");
        return Ok(0);
    }

    println!(
        "selector \"{}\" — {} variant{} across {} matching page{}:",
        selector_str,
        by_hash.len(),
        if by_hash.len() == 1 { "" } else { "s" },
        by_hash.values().map(|(_, p)| p.len()).sum::<usize>(),
        if by_hash.values().map(|(_, p)| p.len()).sum::<usize>() == 1 {
            ""
        } else {
            "s"
        },
    );

    if !pages_with_no_match.is_empty() {
        println!(
            "  ({} page{} did not match the selector and were skipped)",
            pages_with_no_match.len(),
            if pages_with_no_match.len() == 1 {
                ""
            } else {
                "s"
            }
        );
    }

    if by_hash.len() <= 1 {
        return Ok(0);
    }

    // Sort variants: most pages first, then hash for stability.
    let mut groups: Vec<(&String, &(usize, Vec<PathBuf>))> = by_hash.iter().collect();
    groups.sort_by(|a, b| b.1 .1.len().cmp(&a.1 .1.len()).then_with(|| a.0.cmp(b.0)));

    println!();
    for (hash, (count, pages)) in &groups {
        let sample: Vec<String> = pages
            .iter()
            .take(5)
            .map(|p| format!("/{}", p.display()))
            .collect();
        let suffix = if pages.len() > 5 {
            format!(", … (+{} more)", pages.len() - 5)
        } else {
            String::new()
        };
        println!(
            "  hash {hash} ({} pages, {count} match{} each): {}{}",
            pages.len(),
            if *count == 1 { "" } else { "es" },
            sample.join(", "),
            suffix
        );
    }

    Ok(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash8_stable_and_distinct() {
        assert_eq!(hash8(b"foo"), hash8(b"foo"));
        assert_ne!(hash8(b"foo"), hash8(b"bar"));
        assert_eq!(hash8(b"foo").len(), 8);
    }

    #[test]
    fn capture_paired_regions() {
        let html = "before<!-- fragment:nav -->INNER<!-- /fragment:nav -->after";
        let regions = capture_marker_hashes(html, "fragment");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].0, "nav");
    }

    #[test]
    fn ignore_unpaired_close() {
        let html = "<!-- /fragment:nav -->only-close";
        let regions = capture_marker_hashes(html, "fragment");
        assert!(regions.is_empty());
    }

    #[test]
    fn identical_inner_yields_same_hash() {
        let a = capture_marker_hashes("<!-- fragment:nav -->X<!-- /fragment:nav -->", "fragment");
        let b = capture_marker_hashes("<!-- fragment:nav -->X<!-- /fragment:nav -->", "fragment");
        assert_eq!(a, b);
    }
}
