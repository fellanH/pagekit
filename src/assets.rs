use crate::config::Config;
use crate::css_refs::extract_url_refs;
use crate::extract::collect_html_files;
use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Run `pagekit assets`. Builds a complete asset reference graph
/// (HTML hrefs/srcs/srcsets + CSS url(...) references) and emits a
/// TSV manifest covering hash, byte count, MIME-like type, and the
/// per-asset list of referencing pages and stylesheets. Files with
/// zero references in either graph are emitted with an `orphan`
/// line. Always exits 0; orphan information is data, not failure.
pub fn run_assets(root: &Path, config: &Config, save: Option<PathBuf>) -> Result<()> {
    let target_dir = root.join(&config.core.target_dir);
    let scan_root = if target_dir.is_dir() {
        target_dir
    } else {
        root.to_path_buf()
    };
    let fragments_dir = root.join(&config.core.fragments_dir);

    // Pass 1: HTML reference graph.
    let html_files = collect_html_files(
        &scan_root,
        &fragments_dir,
        &config.core.exclude_dirs,
        config.core.max_depth,
    );
    let mut refs_from_pages: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let doc = Html::parse_document(&content);
        let mut values: BTreeSet<String> = BTreeSet::new();
        for el in doc.select(&Selector::parse("[href]").unwrap()) {
            if let Some(v) = el.value().attr("href") {
                values.insert(v.to_string());
            }
        }
        for el in doc.select(&Selector::parse("[src]").unwrap()) {
            if let Some(v) = el.value().attr("src") {
                values.insert(v.to_string());
            }
        }
        for el in doc.select(&Selector::parse("[srcset]").unwrap()) {
            if let Some(v) = el.value().attr("srcset") {
                for u in parse_srcset(v) {
                    values.insert(u);
                }
            }
        }
        for v in &values {
            if let Some(target) = resolve_internal(v, path, &scan_root) {
                refs_from_pages
                    .entry(target)
                    .or_default()
                    .insert(path.clone());
            }
        }
    }

    // Pass 2: CSS reference graph.
    let css_files =
        collect_files_by_ext(&scan_root, &fragments_dir, &config.core.exclude_dirs, "css");
    let mut refs_from_css: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    for css_path in &css_files {
        let body = fs::read_to_string(css_path)
            .with_context(|| format!("reading {}", css_path.display()))?;
        for url in extract_url_refs(&body) {
            if let Some(target) = resolve_internal(&url, css_path, &scan_root) {
                refs_from_css
                    .entry(target)
                    .or_default()
                    .insert(css_path.clone());
            }
        }
    }

    // Pass 3: enumerate every file under scan_root that is an "asset"
    // candidate (excluding HTML, fragments_dir, exclude_dirs,
    // dotfile-prefixed paths, markdown, dotfile-basename).
    let mut all_assets: BTreeSet<PathBuf> = BTreeSet::new();
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
            !path_has_dotfile_component(p, &scan_root)
        })
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if p.extension().map(|x| x == "html").unwrap_or(false) {
            continue;
        }
        if p.extension().map(|x| x == "md").unwrap_or(false) {
            continue;
        }
        if p.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }
        all_assets.insert(p.to_path_buf());
    }

    // Output assembly.
    let mut output = String::new();
    let mut total_bytes: u64 = 0;
    let mut orphan_count: usize = 0;
    let mut canonical_orphans: Vec<&str> = Vec::new();
    for asset_path in &all_assets {
        let bytes = fs::metadata(asset_path).map(|m| m.len()).unwrap_or(0);
        total_bytes += bytes;
        let content = fs::read(asset_path).unwrap_or_default();
        let hash = hash8(&content);
        let mime = mime_for(asset_path);
        let display = display_url(asset_path, &scan_root);

        output.push_str(&format!("{display}\thash\t{hash}\n"));
        output.push_str(&format!("{display}\tbytes\t{bytes}\n"));
        output.push_str(&format!("{display}\ttype\t{mime}\n"));

        let from_pages = refs_from_pages.get(asset_path);
        let from_css = refs_from_css.get(asset_path);
        let mut had_any_ref = false;
        if let Some(pages) = from_pages {
            for p in pages {
                let url = display_url(p, &scan_root);
                output.push_str(&format!("{display}\treferenced-by\t{url}\n"));
                had_any_ref = true;
            }
        }
        if let Some(stylesheets) = from_css {
            for s in stylesheets {
                let url = display_url(s, &scan_root);
                output.push_str(&format!("{display}\treferenced-from-css\t{url}\n"));
                had_any_ref = true;
            }
        }

        // Skip orphan flag for known platform files (Cloudflare Pages,
        // PWA manifests, robots/sitemap, pagekit/clone tooling).
        let basename = asset_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let rel = asset_path.strip_prefix(&scan_root).unwrap_or(asset_path);
        let in_functions = rel.starts_with("functions/") || rel.starts_with("functions\\");
        let is_platform_file = PLATFORM_FILES.contains(&basename) || in_functions;

        if !had_any_ref && !is_platform_file {
            output.push_str(&format!("{display}\torphan\tyes\n"));
            orphan_count += 1;
            canonical_orphans.push(basename);
        }
    }

    match save {
        Some(p) => {
            fs::write(&p, &output).with_context(|| format!("writing {}", p.display()))?;
            println!(
                "pagekit: assets manifest written to {} ({} asset(s), {orphan_count} orphan(s), {total_bytes} bytes)",
                p.display(),
                all_assets.len(),
            );
        }
        None => {
            print!("{output}");
            eprintln!(
                "pagekit: {} asset(s), {orphan_count} orphan(s), {total_bytes} bytes",
                all_assets.len()
            );
        }
    }

    Ok(())
}

const PLATFORM_FILES: &[&str] = &[
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
];

/// FNV-1a 64-bit, truncated to top 32 bits. Same primitive as
/// `check_strict::hash8`. Sufficient for change-detection at site
/// scale (collision risk on N ≈ 200 ≈ 1/2^32).
fn hash8(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (h >> 32) as u32)
}

/// Best-effort MIME-like type from extension. Covers the vast majority
/// of web assets; falls back to `application/octet-stream`.
fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") | Some("mjs") => "application/javascript",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        _ => "application/octet-stream",
    }
}

/// Walk `scan_root` for files matching the given extension (no leading
/// dot). Same exclusion rules as `pagekit links`.
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
            if exclude_dirs
                .iter()
                .any(|d| p.starts_with(scan_root.join(d)))
            {
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

fn parse_srcset(srcset: &str) -> Vec<String> {
    srcset
        .split(',')
        .filter_map(|entry| entry.split_whitespace().next().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect()
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

/// Resolve a reference value (from HTML href/src/srcset OR CSS url())
/// to an actual filesystem path under `scan_root`. Returns `None` if
/// the reference is external or doesn't resolve. Tries direct match,
/// then `<path>/index.html`.
fn resolve_internal(value: &str, source_path: &Path, scan_root: &Path) -> Option<PathBuf> {
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
        source_path.parent().map(|p| p.join(&decoded))?
    };

    let candidate = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            let with_index = candidate.join("index.html");
            with_index.canonicalize().ok()?
        }
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_for_common_types() {
        assert_eq!(mime_for(Path::new("a.css")), "text/css");
        assert_eq!(mime_for(Path::new("a.png")), "image/png");
        assert_eq!(mime_for(Path::new("a.avif")), "image/avif");
        assert_eq!(mime_for(Path::new("a.woff2")), "font/woff2");
        assert_eq!(mime_for(Path::new("a.unknown")), "application/octet-stream");
    }

    #[test]
    fn hash8_stable() {
        assert_eq!(hash8(b"foo"), hash8(b"foo"));
        assert_ne!(hash8(b"foo"), hash8(b"bar"));
        assert_eq!(hash8(b"foo").len(), 8);
    }
}
