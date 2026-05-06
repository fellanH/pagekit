use crate::config::TransformsConfig;
use anyhow::{Context, Result};
use fragments::SyncHook;
use lol_html::html_content::Element;
use lol_html::{element, HtmlRewriter, Settings};
use std::path::{Path, PathBuf};

/// Default attrs rewritten when `path_root` is set and `attrs` is empty.
/// `href` and `src` cover the dominant cases (anchor + link rels + media);
/// users with non-standard attrs (`data-href`, `srcset`) can override.
const DEFAULT_ATTRS: &[&str] = &["href", "src"];

/// Build a `Vec<Box<dyn SyncHook>>` from config. Empty when no transforms
/// are configured ⇒ falls through to plain `sync_all` / `check_all`
/// behavior.
pub fn build_hooks(transforms: &TransformsConfig, target_dir: &str) -> Vec<Box<dyn SyncHook>> {
    let mut hooks: Vec<Box<dyn SyncHook>> = Vec::new();

    if let Some(path_root) = &transforms.path_root {
        let attrs: Vec<String> = if transforms.attrs.is_empty() {
            DEFAULT_ATTRS.iter().map(|s| s.to_string()).collect()
        } else {
            transforms.attrs.clone()
        };
        hooks.push(Box::new(DepthRelativizer {
            path_root: path_root.clone(),
            attrs,
            target_dir: PathBuf::from(target_dir),
        }));
    }

    hooks
}

/// Rewrites absolute paths inside fragment content to be relative to the
/// destination page's depth from `target_dir`. Fragment files on disk
/// stay absolute; only the per-target copy that lands in the marker
/// region is transformed.
pub struct DepthRelativizer {
    pub path_root: String,
    pub attrs: Vec<String>,
    pub target_dir: PathBuf,
}

impl DepthRelativizer {
    /// Compute target depth from `target_dir`. Depth is the count of
    /// directory components between the page's parent dir and `target_dir`.
    fn depth(&self, target: &Path, root: &Path) -> usize {
        let target_dir_abs = root.join(&self.target_dir);
        let rel = target
            .strip_prefix(&target_dir_abs)
            .or_else(|_| target.strip_prefix(root))
            .unwrap_or(target);
        rel.components().count().saturating_sub(1)
    }

    /// Rewrite a single attribute value. Returns `None` when the value
    /// should be left untouched (external URL, scheme, fragment-only, or
    /// not under `path_root`).
    fn rewrite_value(&self, value: &str, depth: usize) -> Option<String> {
        if is_external_or_relative(value) {
            return None;
        }
        let stripped = value.strip_prefix(&self.path_root)?;
        // After stripping path_root, the result should be a path inside
        // the site. If path_root was "/" the stripped value has no
        // leading slash; if path_root was "/app/" it's also slashless.
        // depth==0 ⇒ keep as-is (page lives at target_dir/, link target
        // resolves directly). depth>0 ⇒ prepend `../` per level.
        if depth == 0 {
            Some(stripped.to_string())
        } else {
            let mut out = String::with_capacity(stripped.len() + depth * 3);
            for _ in 0..depth {
                out.push_str("../");
            }
            out.push_str(stripped);
            Some(out)
        }
    }
}

fn is_external_or_relative(value: &str) -> bool {
    let v = value.trim_start();
    if v.is_empty() {
        return true;
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
        return true;
    }
    // Already-relative: anything not starting with `/` is left alone.
    !v.starts_with('/')
}

impl SyncHook for DepthRelativizer {
    fn transform(&self, _name: &str, content: &str, target: &Path, root: &Path) -> Result<String> {
        let depth = self.depth(target, root);

        // Fast path: depth 0 with path_root == "/" still triggers a
        // rewrite (strips the leading slash) so we always run the walk
        // when path_root is configured. Skipping when depth is 0 would
        // leave absolute paths in depth-0 pages, which breaks parity with
        // the rest of the site.

        let mut output: Vec<u8> = Vec::new();
        {
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![element!("*", |el: &mut Element| {
                        for attr in &self.attrs {
                            if let Some(value) = el.get_attribute(attr) {
                                if let Some(new_value) = self.rewrite_value(&value, depth) {
                                    el.set_attribute(attr, &new_value).map_err(|e| {
                                        anyhow::anyhow!("set_attribute({attr}): {e}")
                                    })?;
                                }
                            }
                        }
                        Ok(())
                    })],
                    ..Settings::new()
                },
                |c: &[u8]| output.extend_from_slice(c),
            );
            rewriter
                .write(content.as_bytes())
                .context("lol_html write failed in DepthRelativizer")?;
            rewriter
                .end()
                .context("lol_html end failed in DepthRelativizer")?;
        }

        String::from_utf8(output).context("DepthRelativizer output was not valid utf-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn relativizer() -> DepthRelativizer {
        DepthRelativizer {
            path_root: "/".to_string(),
            attrs: vec!["href".to_string(), "src".to_string()],
            target_dir: PathBuf::from("www"),
        }
    }

    #[test]
    fn rewrite_value_depth_zero_strips_leading_slash() {
        let r = relativizer();
        assert_eq!(
            r.rewrite_value("/sollentuna/index.html", 0).as_deref(),
            Some("sollentuna/index.html")
        );
    }

    #[test]
    fn rewrite_value_depth_one_prepends_one_dotdot() {
        let r = relativizer();
        assert_eq!(
            r.rewrite_value("/sollentuna/index.html", 1).as_deref(),
            Some("../sollentuna/index.html")
        );
    }

    #[test]
    fn rewrite_value_depth_two_prepends_two_dotdots() {
        let r = relativizer();
        assert_eq!(
            r.rewrite_value("/sollentuna/index.html", 2).as_deref(),
            Some("../../sollentuna/index.html")
        );
    }

    #[test]
    fn rewrite_value_skips_external_urls() {
        let r = relativizer();
        assert_eq!(r.rewrite_value("https://example.com", 1), None);
        assert_eq!(r.rewrite_value("http://example.com/x", 1), None);
        assert_eq!(r.rewrite_value("mailto:hi@example.com", 1), None);
        assert_eq!(r.rewrite_value("tel:+46123", 1), None);
        assert_eq!(r.rewrite_value("#section", 1), None);
        assert_eq!(r.rewrite_value("data:image/png;base64,xx", 1), None);
        assert_eq!(r.rewrite_value("//cdn.example.com/x", 1), None);
    }

    #[test]
    fn rewrite_value_skips_already_relative() {
        let r = relativizer();
        assert_eq!(r.rewrite_value("relative/path.html", 1), None);
        assert_eq!(r.rewrite_value("./local.html", 1), None);
        assert_eq!(r.rewrite_value("../sibling.html", 1), None);
    }

    #[test]
    fn rewrite_value_with_non_root_prefix() {
        let r = DepthRelativizer {
            path_root: "/app/".to_string(),
            attrs: vec!["href".to_string()],
            target_dir: PathBuf::from("www"),
        };
        assert_eq!(
            r.rewrite_value("/app/page.html", 2).as_deref(),
            Some("../../page.html")
        );
        // `/other` doesn't start with `/app/` ⇒ untouched.
        assert_eq!(r.rewrite_value("/other.html", 2), None);
    }

    #[test]
    fn depth_computed_relative_to_target_dir() {
        let r = relativizer();
        let root = PathBuf::from("/site");
        assert_eq!(r.depth(&root.join("www/index.html"), &root), 0);
        assert_eq!(r.depth(&root.join("www/sollentuna/index.html"), &root), 1);
        assert_eq!(r.depth(&root.join("www/a/b/c.html"), &root), 2);
    }
}
