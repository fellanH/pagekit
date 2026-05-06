use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_FILE: &str = "fragments.toml";

/// pagekit config: composes fragments core (text-sync primitive) with
/// HTML-specific options (extract candidates). Both layers parse from the
/// same `fragments.toml` file via flatten — users see one flat schema,
/// pagekit internally has two layers.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Fragments core fields (marker_prefix, fragments_dir, target_dir,
    /// exclude_dirs, max_depth) — flattened into the top-level TOML.
    #[serde(flatten)]
    pub core: fragments::Config,
    /// HTML-specific extract configuration.
    pub extract: ExtractConfig,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExtractCandidate {
    /// Fragment file basename produced by extract (`<name>.html`).
    pub name: String,
    /// CSS selector used to locate the element in the parsed DOM.
    pub selector: String,
    /// HTML tag name of the element. Used to walk the raw source when
    /// inserting marker pairs.
    pub tag: String,
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
