use crate::config::Config;
use crate::css_refs::extract_url_refs;
use crate::extract::collect_html_files;
use anyhow::{anyhow, Context, Result};
use lol_html::html_content::Element;
use lol_html::{element, HtmlRewriter, Settings};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// TODO (next): add filesystem-backed tests using tempfile once `cargo` is available in the runner.

/// Rename/move an asset and update references across HTML and CSS.
///
/// - Updates HTML: `src`, `href`, `srcset` (URL part of each entry)
/// - Updates CSS: `url(...)`
///
/// Reference rewriting is filesystem-based: each found URL is resolved
/// relative to the referencing file; if it points to `from`, we rewrite
/// it to point to `to` using the same "absolute vs relative" style as
/// the original reference.
///
/// Returns the number of files modified (or that WOULD be modified in dry-run).
pub fn run_mv_asset(
    root: &Path,
    config: &Config,
    from: &Path,
    to: &Path,
    write: bool,
) -> Result<usize> {
    let target_dir = root.join(&config.core.target_dir);
    let scan_root = if target_dir.is_dir() {
        target_dir
    } else {
        root.to_path_buf()
    };
    let fragments_dir = root.join(&config.core.fragments_dir);

    let from_abs = absolutize_under(&scan_root, from)
        .with_context(|| format!("resolving from path {}", from.display()))?;
    let to_abs = scan_root.join(to);
    if !from_abs.exists() {
        return Err(anyhow!("from asset does not exist: {}", from_abs.display()));
    }

    let html_files = collect_html_files(
        &scan_root,
        &fragments_dir,
        &config.core.exclude_dirs,
        config.core.max_depth,
    );
    let css_files = collect_files_by_ext(&scan_root, &fragments_dir, &config.core.exclude_dirs, "css");

    let mut modified: BTreeSet<PathBuf> = BTreeSet::new();

    // HTML rewrite pass.
    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let new_content = rewrite_html_refs(&content, path, &scan_root, &from_abs, &to_abs)?;
        if new_content != content {
            if write {
                fs::write(path, &new_content).with_context(|| format!("writing {}", path.display()))?;
            }
            modified.insert(path.clone());
        }
    }

    // CSS rewrite pass.
    for css_path in &css_files {
        let body =
            fs::read_to_string(css_path).with_context(|| format!("reading {}", css_path.display()))?;
        let new_body = rewrite_css_urls(&body, css_path, &scan_root, &from_abs, &to_abs)?;
        if new_body != body {
            if write {
                fs::write(css_path, &new_body)
                    .with_context(|| format!("writing {}", css_path.display()))?;
            }
            modified.insert(css_path.clone());
        }
    }

    // Finally, move the file (only in write mode).
    if write {
        if let Some(parent) = to_abs.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::rename(&from_abs, &to_abs).with_context(|| {
            format!(
                "moving asset {} -> {}",
                from_abs.display(),
                to_abs.display()
            )
        })?;
    }

    let mode = if write { "write" } else { "dry-run" };
    println!(
        "pagekit: mv-asset ({mode}): {} file(s) would change; move: {} -> {}",
        modified.len(),
        display_url(&from_abs, &scan_root),
        display_url(&to_abs, &scan_root)
    );
    Ok(modified.len())
}

fn rewrite_html_refs(
    html: &str,
    page_path: &Path,
    scan_root: &Path,
    from_abs: &Path,
    to_abs: &Path,
) -> Result<String> {
    let mut output: Vec<u8> = Vec::new();
    {
        let page_dir = page_path
            .parent()
            .ok_or_else(|| anyhow!("page has no parent dir: {}", page_path.display()))?
            .to_path_buf();
        let scan_root = scan_root.to_path_buf();
        let from_abs = from_abs.to_path_buf();
        let to_abs = to_abs.to_path_buf();

        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("*", move |el: &mut Element| {
                    for attr in ["src", "href"] {
                        if let Some(v) = el.get_attribute(attr) {
                            if let Some(new_v) =
                                rewrite_one_ref(&v, &page_dir, &scan_root, &from_abs, &to_abs)
                            {
                                el.set_attribute(attr, &new_v).map_err(|e| {
                                    anyhow::anyhow!("set_attribute({attr}): {e}")
                                })?;
                            }
                        }
                    }

                    if let Some(v) = el.get_attribute("srcset") {
                        if let Some(new_v) =
                            rewrite_srcset(&v, &page_dir, &scan_root, &from_abs, &to_abs)
                        {
                            el.set_attribute("srcset", &new_v)
                                .map_err(|e| anyhow::anyhow!("set_attribute(srcset): {e}"))?;
                        }
                    }
                    Ok(())
                })],
                ..Settings::new()
            },
            |c: &[u8]| output.extend_from_slice(c),
        );
        rewriter
            .write(html.as_bytes())
            .context("lol_html write failed in mv-asset html pass")?;
        rewriter
            .end()
            .context("lol_html end failed in mv-asset html pass")?;
    }
    String::from_utf8(output).context("mv-asset html output was not valid utf-8")
}

fn rewrite_srcset(
    srcset: &str,
    page_dir: &Path,
    scan_root: &Path,
    from_abs: &Path,
    to_abs: &Path,
) -> Option<String> {
    // Preserve descriptors (e.g. "500w", "2x") and spacing reasonably.
    // For each comma-separated entry, rewrite only the URL token (first
    // whitespace-separated token).
    let mut changed = false;
    let mut out_parts: Vec<String> = Vec::new();
    for entry in srcset.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut it = trimmed.split_whitespace();
        let Some(url) = it.next() else {
            continue;
        };
        // srcset URLs can legally carry query strings; skip rewriting those
        // for now since our resolver canonicalizes filesystem paths.
        if url.contains('?') || url.contains('#') {
            out_parts.push(trimmed.to_string());
            continue;
        }
        let rest: Vec<&str> = it.collect();

        let new_url = rewrite_one_ref(url, page_dir, scan_root, from_abs, to_abs);
        if let Some(nu) = new_url {
            changed = true;
            if rest.is_empty() {
                out_parts.push(nu);
            } else {
                out_parts.push(format!("{nu} {}", rest.join(" ")));
            }
        } else {
            out_parts.push(trimmed.to_string());
        }
    }
    if changed {
        Some(out_parts.join(", "))
    } else {
        None
    }
}

fn rewrite_css_urls(
    css: &str,
    css_path: &Path,
    scan_root: &Path,
    from_abs: &Path,
    to_abs: &Path,
) -> Result<String> {
    // Use the existing url() extractor for detection; rewrite only the
    // extracted url bodies. This preserves the surrounding `url(...)`
    // wrapper but may touch multiple occurrences if the same token is
    // used elsewhere in the file.
    let urls = extract_url_refs(css);
    if urls.is_empty() {
        return Ok(css.to_string());
    }

    let css_dir = css_path
        .parent()
        .ok_or_else(|| anyhow!("css has no parent dir: {}", css_path.display()))?;

    let mut out = css.to_string();
    for url in urls {
        // Skip url tokens with CSS fragments/query: they are legal in CSS
        // and canonicalize() will not resolve them.
        if url.contains('#') || url.contains('?') {
            continue;
        }
        if let Some(new_url) = rewrite_one_ref(&url, css_dir, scan_root, from_abs, to_abs) {
            // Replace occurrences of the exact extracted token. This is
            // conservative: we only touch places we already recognized as
            // url(...) bodies.
            out = out.replace(&url, &new_url);
        }
    }
    Ok(out)
}

fn rewrite_one_ref(
    value: &str,
    source_dir: &Path,
    scan_root: &Path,
    from_abs: &Path,
    to_abs: &Path,
) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if is_external_or_anchor(v) {
        return None;
    }

    let abs = resolve_internal_abs(v, source_dir, scan_root)?;
    if abs != *from_abs {
        return None;
    }

    // Preserve absolute-vs-relative style based on the original token.
    if v.starts_with('/') {
        // Root-absolute within the site.
        let rel = to_abs.strip_prefix(scan_root).ok()?;
        Some(format!("/{}", rel.to_string_lossy()))
    } else {
        // Relative to source dir.
        let rel = pathdiff::diff_paths(to_abs, source_dir)?;
        Some(rel.to_string_lossy().to_string())
    }
}

fn is_external_or_anchor(v: &str) -> bool {
    v.starts_with("http://")
        || v.starts_with("https://")
        || v.starts_with("//")
        || v.starts_with("mailto:")
        || v.starts_with("tel:")
        || v.starts_with("data:")
        || v.starts_with("javascript:")
        || v.starts_with('#')
}

fn resolve_internal_abs(value: &str, source_dir: &Path, scan_root: &Path) -> Option<PathBuf> {
    let v = value.trim().split('?').next().unwrap_or(value);
    let v = v.split('#').next().unwrap_or(v);
    if v.is_empty() || is_external_or_anchor(v) {
        return None;
    }
    let candidate = if let Some(stripped) = v.strip_prefix('/') {
        scan_root.join(stripped)
    } else {
        source_dir.join(v)
    };
    // Directory-style references in HTML/CSS should resolve to their index.html.
    // This matters for cases like `/assets/` (rare for images, but can
    // show up for docs/pdf links).
    if candidate.is_dir() {
        return candidate.join("index.html").canonicalize().ok();
    }
    candidate.canonicalize().ok()
}

fn absolutize_under(scan_root: &Path, p: &Path) -> Result<PathBuf> {
    // Treat `p` as a path under scan_root unless it's absolute.
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        scan_root.join(p)
    };
    Ok(joined.canonicalize()?)
}

fn display_url(path: &Path, scan_root: &Path) -> String {
    let rel = path.strip_prefix(scan_root).unwrap_or(path);
    format!("/{}", rel.display())
}

fn collect_files_by_ext(
    scan_root: &Path,
    fragments_dir: &Path,
    exclude_dirs: &[String],
    ext: &str,
) -> Vec<PathBuf> {
    WalkDir::new(scan_root)
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            if p.starts_with(fragments_dir) {
                return false;
            }
            if exclude_dirs.iter().any(|d| p.starts_with(scan_root.join(d))) {
                return false;
            }
            !path_has_dotfile_component(p, scan_root)
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some(ext))
        .map(|e| e.into_path())
        .collect()
}

fn path_has_dotfile_component(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    // (No unit tests yet: the core rewrite path canonicalizes via the filesystem.)
}

