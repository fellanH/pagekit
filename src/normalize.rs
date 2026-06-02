use crate::config::Config;
use crate::extract::collect_html_files;
use crate::transforms::DepthRelativizer;
use anyhow::{Context, Result};
use fragments::SyncHook;
use std::fs;
use std::path::{Path, PathBuf};

/// Default attrs rewritten when `[transforms].attrs` is empty.
const DEFAULT_ATTRS: &[&str] = &["href", "src"];

/// Rewrite root-absolute paths in every HTML page to be depth-relative
/// to each page's location. Reuses `DepthRelativizer` (the SyncHook
/// shipped in Sprint 4 D2 for fragment regions); applies it page-wide
/// here. Idempotent — relative paths are skipped by the rewriter, so a
/// second run produces no diff.
///
/// **Defaulting differs from `sync`.** `sync` is no-op without an
/// explicit `[transforms].path_root`; `normalize-paths` defaults
/// `path_root="/"` when invoked, because the user opted in by running
/// the command. Configured `path_root` still overrides the default.
///
/// Safe-by-default like the other bulk mutators (`apply`, `mv-asset`,
/// `rename-assets`): with `write=false` it reports what would change
/// without touching disk. Returns the number of pages modified (or that
/// would be modified in dry-run).
pub fn normalize_paths(root: &Path, config: &Config, write: bool) -> Result<usize> {
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

    let path_root = config
        .transforms
        .path_root
        .clone()
        .unwrap_or_else(|| "/".to_string());
    let attrs: Vec<String> = if config.transforms.attrs.is_empty() {
        DEFAULT_ATTRS.iter().map(|s| s.to_string()).collect()
    } else {
        config.transforms.attrs.clone()
    };

    let relativizer = DepthRelativizer {
        path_root,
        attrs,
        target_dir: PathBuf::from(&config.core.target_dir),
    };

    let mut modified = 0;
    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        // Pass `path` as the target so `DepthRelativizer.depth()` resolves
        // against `target_dir`/`root` correctly. `name` is unused for our
        // call site (the SyncHook contract treats it as the fragment name;
        // page-wide rewriting has no fragment context).
        let new_content = relativizer
            .transform("", &content, path, root)
            .with_context(|| format!("rewriting {}", path.display()))?;
        if new_content != content {
            if write {
                fs::write(path, &new_content)
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            modified += 1;
        }
    }

    let (mode, verb) = if write {
        ("write", "changed")
    } else {
        ("dry-run", "would change")
    };
    println!(
        "pagekit: normalize-paths ({mode}): {modified} of {} page(s) {verb}",
        html_files.len()
    );
    Ok(modified)
}
