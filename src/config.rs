use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_FILE: &str = "fragments.toml";

/// pagekit config: composes fragments core (text-sync primitive) with
/// HTML-specific options (extract candidates, sync-time transforms). All
/// layers parse from the same `fragments.toml` file via flatten — users
/// see one flat schema, pagekit internally has multiple layers.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Fragments core fields (marker_prefix, fragments_dir, target_dir,
    /// exclude_dirs, max_depth) — flattened into the top-level TOML.
    #[serde(flatten)]
    pub core: fragments::Config,
    /// HTML-specific extract configuration.
    pub extract: ExtractConfig,
    /// Sync-time content transforms (path rewriting, future variant
    /// selection). Empty section ⇒ no hooks installed ⇒ unchanged behavior.
    pub transforms: TransformsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ExtractConfig {
    /// Custom candidate selectors for `pagekit extract`. User entries are
    /// APPENDED to the six built-in candidates (nav, footer, header,
    /// .navbar, .site-header, .site-footer) — adding one doesn't remove
    /// the others.
    pub candidates: Vec<ExtractCandidate>,
}

/// Sync-time transform config. With `path_root` unset, no transforms run
/// and behavior matches plain `fragments::sync_all`.
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct TransformsConfig {
    /// Absolute prefix that fragment files use for site-internal paths
    /// (typically `"/"`). When set, attrs in `attrs` whose value starts
    /// with this prefix are rewritten to be relative to the destination
    /// page's depth from `target_dir`.
    pub path_root: Option<String>,
    /// HTML attributes to rewrite. Empty ⇒ defaults to `["href", "src"]`
    /// when `path_root` is set; ignored otherwise.
    pub attrs: Vec<String>,
}

/// User-defined extract candidate. `tag = "..."` was required in the
/// pre-lol_html implementation; serde silently ignores it now if present
/// in legacy configs.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExtractCandidate {
    /// Fragment file basename produced by extract (`<name>.html`).
    pub name: String,
    /// CSS selector used to locate the element in the parsed DOM.
    pub selector: String,
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILE);
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            Ok(toml::from_str(&text)?)
        } else {
            Ok(Self::default())
        }
    }
}
