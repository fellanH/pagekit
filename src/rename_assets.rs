use crate::config::Config;
use crate::css_refs::extract_url_refs;
use crate::extract::collect_html_files;
use anyhow::{anyhow, Context, Result};
use lol_html::html_content::Element;
use lol_html::{element, HtmlRewriter, Settings};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Batch rename assets with spaces to hyphenated names, and rewrite references.
///
/// - Renames files whose *basename* contains spaces.
/// - Rewrites references in HTML: `src`, `href`, `srcset` (URL token).
/// - Rewrites references in CSS: `url(...)`.
/// - Rewrites references in JSON files (`*.json`) by conservative string substitution
///   when a token resolves to a renamed asset.
///
/// Safe-by-default: dry-run unless `write=true`.
///
/// Returns number of files modified (or that WOULD be modified in dry-run).
pub fn run_rename_assets(root: &Path, config: &Config, write: bool) -> Result<usize> {
    let target_dir = root.join(&config.core.target_dir);
    let scan_root = if target_dir.is_dir() {
        target_dir
    } else {
        root.to_path_buf()
    };
    let fragments_dir = root.join(&config.core.fragments_dir);

    let plan = build_spaces_to_hyphens_plan(&scan_root, &fragments_dir, &config.core.exclude_dirs)?;
    if plan.is_empty() {
        println!("pagekit: rename-assets: no assets with spaces found");
        return Ok(0);
    }

    // Collision safety: do not proceed if any target already exists.
    let mut collisions: Vec<String> = Vec::new();
    for (from, to) in &plan {
        if to.exists() && to != from {
            collisions.push(format!(
                "{} -> {} (target exists)",
                display_url(from, &scan_root),
                display_url(to, &scan_root)
            ));
        }
    }
    if !collisions.is_empty() {
        return Err(anyhow!(
            "rename-assets: refusing due to {} collision(s):\n{}",
            collisions.len(),
            collisions.join("\n")
        ));
    }

    // Rewrite pass across HTML/CSS/JSON.
    let html_files = collect_html_files(
        &scan_root,
        &fragments_dir,
        &config.core.exclude_dirs,
        config.core.max_depth,
    );
    let css_files = collect_files_by_ext(&scan_root, &fragments_dir, &config.core.exclude_dirs, "css");
    let json_files =
        collect_files_by_ext(&scan_root, &fragments_dir, &config.core.exclude_dirs, "json");

    let map = make_map(&plan)?;

    let mut modified: BTreeSet<PathBuf> = BTreeSet::new();

    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let new_content = rewrite_html_with_map(&content, path, &scan_root, &map)?;
        if new_content != content {
            if write {
                fs::write(path, &new_content).with_context(|| format!("writing {}", path.display()))?;
            }
            modified.insert(path.clone());
        }
    }

    for css_path in &css_files {
        let body =
            fs::read_to_string(css_path).with_context(|| format!("reading {}", css_path.display()))?;
        let new_body = rewrite_css_with_map(&body, css_path, &scan_root, &map)?;
        if new_body != body {
            if write {
                fs::write(css_path, &new_body)
                    .with_context(|| format!("writing {}", css_path.display()))?;
            }
            modified.insert(css_path.clone());
        }
    }

    for json_path in &json_files {
        let body =
            fs::read_to_string(json_path).with_context(|| format!("reading {}", json_path.display()))?;
        let new_body = rewrite_json_with_map(&body, json_path, &scan_root, &map)?;
        if new_body != body {
            if write {
                fs::write(json_path, &new_body)
                    .with_context(|| format!("writing {}", json_path.display()))?;
            }
            modified.insert(json_path.clone());
        }
    }

    // Move assets last (only in write mode).
    if write {
        for (from, to) in &plan {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
            }
            fs::rename(from, to).with_context(|| {
                format!(
                    "renaming asset {} -> {}",
                    from.display(),
                    to.display()
                )
            })?;
        }
    }

    let mode = if write { "write" } else { "dry-run" };
    println!(
        "pagekit: rename-assets ({mode}): {} rename(s), {} file(s) would change",
        plan.len(),
        modified.len()
    );
    println!("planned renames:");
    for (from, to) in &plan {
        println!(
            "  {} -> {}",
            display_url(from, &scan_root),
            display_url(to, &scan_root)
        );
    }

    Ok(modified.len())
}

fn build_spaces_to_hyphens_plan(
    scan_root: &Path,
    fragments_dir: &Path,
    exclude_dirs: &[String],
) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut out: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in WalkDir::new(scan_root)
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
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        // Skip HTML and CSS and JSON; this command is "assets".
        if p.extension().map(|x| x == "html").unwrap_or(false) {
            continue;
        }
        if p.extension().map(|x| x == "css").unwrap_or(false) {
            continue;
        }
        if p.extension().map(|x| x == "json").unwrap_or(false) {
            continue;
        }
        if p.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.contains(' ') {
            continue;
        }

        let new_name = hyphenate_name(name);
        if new_name == name {
            continue;
        }
        let to = p.with_file_name(new_name);
        out.push((p.to_path_buf(), to));
    }

    // Stable ordering for deterministic output.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn hyphenate_name(name: &str) -> String {
    // Replace whitespace runs with a single hyphen, and trim hyphens at ends.
    let mut out = String::with_capacity(name.len());
    let mut last_hyphen = false;
    for ch in name.chars() {
        if ch.is_whitespace() {
            if !last_hyphen {
                out.push('-');
                last_hyphen = true;
            }
        } else {
            out.push(ch);
            last_hyphen = false;
        }
    }
    out.trim_matches('-').to_string()
}

type AbsMap = BTreeMap<PathBuf, PathBuf>;

fn make_map(plan: &[(PathBuf, PathBuf)]) -> Result<AbsMap> {
    let mut map: AbsMap = BTreeMap::new();
    for (from, to) in plan {
        let from_abs = from.canonicalize().with_context(|| format!("canonicalizing {}", from.display()))?;
        let to_abs = if to.is_absolute() {
            to.to_path_buf()
        } else {
            // Plan paths are under scan_root already, so `to` should be absolute.
            to.to_path_buf()
        };
        map.insert(from_abs, to_abs);
    }
    Ok(map)
}

fn rewrite_html_with_map(
    html: &str,
    page_path: &Path,
    scan_root: &Path,
    map: &AbsMap,
) -> Result<String> {
    let mut output: Vec<u8> = Vec::new();
    {
        let page_dir = page_path
            .parent()
            .ok_or_else(|| anyhow!("page has no parent dir: {}", page_path.display()))?
            .to_path_buf();
        let scan_root = scan_root.to_path_buf();
        let map = map.clone();

        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("*", move |el: &mut Element| {
                    for attr in ["src", "href"] {
                        if let Some(v) = el.get_attribute(attr) {
                            if let Some(new_v) = rewrite_value_with_map(&v, &page_dir, &scan_root, &map) {
                                el.set_attribute(attr, &new_v).map_err(|e| {
                                    anyhow::anyhow!("set_attribute({attr}): {e}")
                                })?;
                            }
                        }
                    }
                    if let Some(v) = el.get_attribute("srcset") {
                        if let Some(new_v) = rewrite_srcset_with_map(&v, &page_dir, &scan_root, &map) {
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
            .context("lol_html write failed in rename-assets html pass")?;
        rewriter
            .end()
            .context("lol_html end failed in rename-assets html pass")?;
    }
    String::from_utf8(output).context("rename-assets html output was not valid utf-8")
}

fn rewrite_srcset_with_map(
    srcset: &str,
    page_dir: &Path,
    scan_root: &Path,
    map: &AbsMap,
) -> Option<String> {
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
        let rest: Vec<&str> = it.collect();

        // Preserve query/fragment as-is (we match by stripping for resolution).
        let (url_path, suffix) = split_suffix(url);
        let new_url = rewrite_value_with_map(url_path, page_dir, scan_root, map);
        if let Some(mut nu) = new_url {
            changed = true;
            nu.push_str(suffix);
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

fn rewrite_css_with_map(css: &str, css_path: &Path, scan_root: &Path, map: &AbsMap) -> Result<String> {
    let urls = extract_url_refs(css);
    if urls.is_empty() {
        return Ok(css.to_string());
    }
    let css_dir = css_path
        .parent()
        .ok_or_else(|| anyhow!("css has no parent dir: {}", css_path.display()))?;

    let mut out = css.to_string();
    for url in urls {
        let (path_part, suffix) = split_suffix(&url);
        if let Some(mut new_url) = rewrite_value_with_map(path_part, css_dir, scan_root, map) {
            new_url.push_str(suffix);
            out = out.replace(&url, &new_url);
        }
    }
    Ok(out)
}

fn rewrite_json_with_map(
    json: &str,
    json_path: &Path,
    scan_root: &Path,
    map: &AbsMap,
) -> Result<String> {
    // Conservative: look for quoted strings and attempt to rewrite them
    // if they resolve to a renamed asset. This avoids full JSON parsing
    // and preserves formatting.
    let source_dir = json_path
        .parent()
        .ok_or_else(|| anyhow!("json has no parent dir: {}", json_path.display()))?;

    let mut out = String::with_capacity(json.len());
    let mut chars = json.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '"' {
            out.push(ch);
            continue;
        }
        // Start of string literal.
        out.push('"');
        let mut buf = String::new();
        let mut escaped = false;
        while let Some(c) = chars.next() {
            if escaped {
                buf.push(c);
                escaped = false;
                continue;
            }
            if c == '\\' {
                buf.push(c);
                escaped = true;
                continue;
            }
            if c == '"' {
                // End.
                let candidate = unescape_json_string_minimal(&buf);
                if let Some(new_val) = rewrite_value_with_map(&candidate, source_dir, scan_root, map) {
                    // Re-escape minimal JSON characters.
                    out.push_str(&escape_json_string_minimal(&new_val));
                } else {
                    out.push_str(&buf);
                }
                out.push('"');
                break;
            }
            buf.push(c);
        }
    }
    Ok(out)
}

fn rewrite_value_with_map(
    value: &str,
    source_dir: &Path,
    scan_root: &Path,
    map: &AbsMap,
) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if is_external_or_anchor(v) {
        return None;
    }
    let (path_part, suffix) = split_suffix(v);
    let abs = resolve_internal_abs_with_decode(path_part, source_dir, scan_root)?;
    let to_abs = map.get(&abs)?;

    // Preserve absolute-vs-relative style from the original token.
    let mut rewritten = if path_part.starts_with('/') {
        let rel = to_abs.strip_prefix(scan_root).ok()?;
        format!("/{}", rel.to_string_lossy())
    } else {
        let rel = pathdiff::diff_paths(to_abs, source_dir)?;
        rel.to_string_lossy().to_string()
    };
    rewritten.push_str(suffix);
    Some(rewritten)
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

fn split_suffix(s: &str) -> (&str, &str) {
    // Split off first '?' or '#', preserving the suffix.
    let q = s.find('?');
    let h = s.find('#');
    match (q, h) {
        (None, None) => (s, ""),
        (Some(i), None) => (&s[..i], &s[i..]),
        (None, Some(i)) => (&s[..i], &s[i..]),
        (Some(i), Some(j)) => {
            let k = i.min(j);
            (&s[..k], &s[k..])
        }
    }
}

fn resolve_internal_abs_with_decode(
    value: &str,
    source_dir: &Path,
    scan_root: &Path,
) -> Option<PathBuf> {
    let v = value.trim();
    if v.is_empty() || is_external_or_anchor(v) {
        return None;
    }
    let decoded = percent_decode(v);
    let candidate = if let Some(stripped) = decoded.strip_prefix('/') {
        scan_root.join(stripped)
    } else {
        source_dir.join(decoded)
    };
    if candidate.is_dir() {
        return candidate.join("index.html").canonicalize().ok();
    }
    candidate.canonicalize().ok()
}

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

fn escape_json_string_minimal(s: &str) -> String {
    // Escape only backslash and quotes; leave unicode intact.
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

fn unescape_json_string_minimal(s: &str) -> String {
    // Best-effort: handle \" and \\.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(ch);
        }
    }
    out
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

fn display_url(path: &Path, scan_root: &Path) -> String {
    let rel = path.strip_prefix(scan_root).unwrap_or(path);
    format!("/{}", rel.display())
}

