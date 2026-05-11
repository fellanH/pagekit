use crate::config::Config;
use crate::extract::collect_html_files;
use anyhow::{anyhow, Context, Result};
use lol_html::html_content::Element;
use lol_html::{element, HtmlRewriter, Settings};
use scraper::{Html, Selector};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Run `pagekit apply <rules> [--set k=v]* [--write]`.
///
/// Returns the number of modified files (or files that WOULD be modified
/// in dry-run mode).
pub fn run_apply(
    root: &Path,
    config: &Config,
    rules_path: &Path,
    set_kvs: &[String],
    write: bool,
) -> Result<usize> {
    let rules_text = fs::read_to_string(rules_path)
        .with_context(|| format!("reading rules file {}", rules_path.display()))?;
    let mut rules: RuleFile = toml::from_str(&rules_text).context("parsing rules TOML")?;
    if rules.version != 1 {
        return Err(anyhow!(
            "unsupported rules version {} (expected 1)",
            rules.version
        ));
    }

    let cli_params = parse_set_params(set_kvs)?;
    for (k, v) in cli_params {
        rules.params.insert(k, v);
    }

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

    let mut affected_files: BTreeMap<PathBuf, String> = BTreeMap::new();

    for path in &html_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut new_content = content.clone();

        for edit in &rules.edits {
            let edit = edit.resolve(&rules.params)?;
            if !target_matches_path(&edit.target, path, &content, &config.core.marker_prefix)? {
                continue;
            }
            new_content = apply_edit_to_content(&new_content, &edit)?;
        }

        if new_content != content {
            if write {
                fs::write(path, &new_content).with_context(|| format!("writing {}", path.display()))?;
            }
            affected_files.insert(path.clone(), new_content);
        }
    }

    let mode = if write { "write" } else { "dry-run" };
    println!(
        "pagekit: apply ({mode}): {} edit(s), {} file(s) affected",
        rules.edits.len(),
        affected_files.len()
    );
    Ok(affected_files.len())
}

#[derive(Debug, Deserialize)]
struct RuleFile {
    version: u32,
    #[serde(default)]
    params: BTreeMap<String, String>,
    #[serde(default)]
    edits: Vec<EditSpec>,
}

#[derive(Debug, Deserialize, Clone)]
struct EditSpec {
    target: TargetSpec,
    #[serde(default = "default_scope")]
    scope: ScopeSpec,
    steps: Vec<StepSpec>,
}

fn default_scope() -> ScopeSpec {
    ScopeSpec::WholeDocument
}

impl EditSpec {
    fn resolve(&self, params: &BTreeMap<String, String>) -> Result<ResolvedEdit> {
        Ok(ResolvedEdit {
            target: self.target.resolve(params)?,
            scope: self.scope.resolve(params)?,
            steps: self
                .steps
                .iter()
                .map(|s| s.resolve(params))
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

#[derive(Debug, Clone)]
struct ResolvedEdit {
    target: Target,
    scope: Scope,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TargetSpec {
    AllPages,
    PagesMatchingSelector { selector: String },
    PagesWithMarker { name: String },
}

#[derive(Debug, Clone)]
enum Target {
    AllPages,
    PagesMatchingSelector { selector: String },
    PagesWithMarker { name: String },
}

impl TargetSpec {
    fn resolve(&self, params: &BTreeMap<String, String>) -> Result<Target> {
        Ok(match self {
            TargetSpec::AllPages => Target::AllPages,
            TargetSpec::PagesMatchingSelector { selector } => Target::PagesMatchingSelector {
                selector: substitute(selector, params)?,
            },
            TargetSpec::PagesWithMarker { name } => Target::PagesWithMarker {
                name: substitute(name, params)?,
            },
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScopeSpec {
    WholeDocument,
    Marker { name: String },
    Selector { selector: String },
}

#[derive(Debug, Clone)]
enum Scope {
    WholeDocument,
    Marker { name: String },
    Selector { selector: String },
}

impl ScopeSpec {
    fn resolve(&self, params: &BTreeMap<String, String>) -> Result<Scope> {
        Ok(match self {
            ScopeSpec::WholeDocument => Scope::WholeDocument,
            ScopeSpec::Marker { name } => Scope::Marker {
                name: substitute(name, params)?,
            },
            ScopeSpec::Selector { selector } => Scope::Selector {
                selector: substitute(selector, params)?,
            },
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
enum StepSpec {
    RenameTag { from: String, to: String },
    SetAttr {
        selector: String,
        attr: String,
        value: String,
    },
}

#[derive(Debug, Clone)]
enum Step {
    RenameTag { from: String, to: String },
    SetAttr {
        selector: String,
        attr: String,
        value: String,
    },
}

impl StepSpec {
    fn resolve(&self, params: &BTreeMap<String, String>) -> Result<Step> {
        Ok(match self {
            StepSpec::RenameTag { from, to } => Step::RenameTag {
                from: substitute(from, params)?,
                to: substitute(to, params)?,
            },
            StepSpec::SetAttr {
                selector,
                attr,
                value,
            } => Step::SetAttr {
                selector: substitute(selector, params)?,
                attr: substitute(attr, params)?,
                value: substitute(value, params)?,
            },
        })
    }
}

fn apply_edit_to_content(content: &str, edit: &ResolvedEdit) -> Result<String> {
    let mut out = content.to_string();
    for step in &edit.steps {
        out = apply_step(&out, &edit.scope, step)?;
    }
    Ok(out)
}

fn apply_step(content: &str, scope: &Scope, step: &Step) -> Result<String> {
    match step {
        Step::RenameTag { from, to } => apply_rename_tag(content, scope, from, to),
        Step::SetAttr {
            selector,
            attr,
            value,
        } => apply_set_attr(content, scope, selector, attr, value),
    }
}

fn apply_rename_tag(content: &str, scope: &Scope, from: &str, to: &str) -> Result<String> {
    let from = from.trim().to_ascii_lowercase();
    let to = to.trim().to_ascii_lowercase();
    if from.is_empty() || to.is_empty() {
        return Err(anyhow!("rename_tag requires non-empty from/to"));
    }

    match scope {
        Scope::WholeDocument => Ok(rename_tag_in_slice(content, &from, &to)),
        Scope::Marker { name } => {
            let (pre, mid, post) = split_marker_region_fragment_prefix(content, name)?;
            let mid2 = rename_tag_in_slice(mid, &from, &to);
            Ok(format!("{pre}{mid2}{post}"))
        }
        Scope::Selector { .. } => Err(anyhow!(
            "rename_tag does not support selector scope (use marker or whole_document)"
        )),
    }
}

fn rename_tag_in_slice(s: &str, from: &str, to: &str) -> String {
    let mut out = s.to_string();
    out = replace_tag_starts(&out, from, to);
    out = out.replace(&format!("</{from}"), &format!("</{to}"));

    let from_up = from.to_ascii_uppercase();
    let to_up = to.to_ascii_uppercase();
    out = replace_tag_starts(&out, &from_up, &to_up);
    out = out.replace(&format!("</{from_up}"), &format!("</{to_up}"));
    out
}

fn replace_tag_starts(s: &str, from: &str, to: &str) -> String {
    let bytes = s.as_bytes();
    let pat = format!("<{from}");
    let pat_b = pat.as_bytes();

    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + pat_b.len() <= bytes.len() && &bytes[i..i + pat_b.len()] == pat_b {
            let j = i + pat_b.len();
            let ok = j >= bytes.len()
                || matches!(bytes[j], b'>' | b'/' | b'\t' | b'\n' | b'\r' | b' ');
            if ok {
                out.push('<');
                out.push_str(to);
                i = j;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn apply_set_attr(
    content: &str,
    scope: &Scope,
    selector: &str,
    attr: &str,
    value: &str,
) -> Result<String> {
    if selector.trim().is_empty() {
        return Err(anyhow!("set_attr requires non-empty selector"));
    }
    if attr.trim().is_empty() {
        return Err(anyhow!("set_attr requires non-empty attr"));
    }

    match scope {
        Scope::WholeDocument => rewrite_set_attr_lol_html(content, selector, attr, value),
        Scope::Selector { selector: scope_sel } => {
            let combined = format!("{scope_sel} {selector}");
            rewrite_set_attr_lol_html(content, &combined, attr, value)
        }
        Scope::Marker { name } => {
            let (pre, mid, post) = split_marker_region_fragment_prefix(content, name)?;
            let mid2 = rewrite_set_attr_lol_html(mid, selector, attr, value)?;
            Ok(format!("{pre}{mid2}{post}"))
        }
    }
}

fn rewrite_set_attr_lol_html(content: &str, selector: &str, attr: &str, value: &str) -> Result<String> {
    let mut output: Vec<u8> = Vec::new();
    {
        let selector = selector.to_string();
        let attr = attr.to_string();
        let value = value.to_string();
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!(&selector, move |el: &mut Element| {
                    el.set_attribute(&attr, &value)
                        .map_err(|e| anyhow::anyhow!("set_attribute({attr}): {e}"))?;
                    Ok(())
                })],
                ..Settings::new()
            },
            |c: &[u8]| output.extend_from_slice(c),
        );
        rewriter
            .write(content.as_bytes())
            .context("lol_html write failed in set_attr")?;
        rewriter.end().context("lol_html end failed in set_attr")?;
    }
    String::from_utf8(output).context("set_attr output was not valid utf-8")
}

fn split_marker_region_fragment_prefix<'a>(
    content: &'a str,
    name: &str,
) -> Result<(&'a str, &'a str, &'a str)> {
    let open = format!("<!-- fragment:{name} -->");
    let close = format!("<!-- /fragment:{name} -->");

    let start = content
        .find(&open)
        .ok_or_else(|| anyhow!("marker open not found: {open}"))?;
    let after_open = start + open.len();
    let end_rel = content[after_open..]
        .find(&close)
        .ok_or_else(|| anyhow!("marker close not found: {close}"))?;
    let end = after_open + end_rel;
    Ok((&content[..after_open], &content[after_open..end], &content[end..]))
}

fn target_matches_path(
    target: &Target,
    _path: &Path,
    content: &str,
    marker_prefix: &str,
) -> Result<bool> {
    match target {
        Target::AllPages => Ok(true),
        Target::PagesWithMarker { name } => Ok(content.contains(&format!(
            "<!-- {marker_prefix}:{name} -->"
        ))),
        Target::PagesMatchingSelector { selector } => {
            let sel = Selector::parse(selector)
                .map_err(|e| anyhow!("invalid selector '{selector}': {e:?}"))?;
            let doc = Html::parse_document(content);
            Ok(doc.select(&sel).next().is_some())
        }
    }
}

fn parse_set_params(kvs: &[String]) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for kv in kvs {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| anyhow!("--set expects key=value, got '{kv}'"))?;
        let k = k.trim();
        if k.is_empty() {
            return Err(anyhow!("--set key cannot be empty"));
        }
        out.insert(k.to_string(), v.to_string());
    }
    Ok(out)
}

fn substitute(template: &str, params: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .ok_or_else(|| anyhow!("unterminated ${{...}} in '{template}'"))?;
        let key = after[..end].trim();
        let val = params
            .get(key)
            .ok_or_else(|| anyhow!("missing param '{key}' (from '{template}')"))?;
        out.push_str(val);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_params() {
        let mut params = BTreeMap::new();
        params.insert("x".to_string(), "hello".to_string());
        assert_eq!(substitute("a-${x}-b", &params).unwrap(), "a-hello-b");
    }

    #[test]
    fn rename_tag_in_marker_region() {
        let html = r#"<html><body>
<!-- fragment:cta -->
<section><h2 class="t">Hi</h2><div><H2>Yo</H2></div></section>
<!-- /fragment:cta -->
<h2>Outside</h2>
</body></html>"#;

        let out = apply_rename_tag(html, &Scope::Marker { name: "cta".into() }, "h2", "h3")
            .unwrap();
        assert!(out.contains("<h3 class=\"t\">Hi</h3>"));
        assert!(out.contains("<H3>Yo</H3>"));
        assert!(out.contains("<h2>Outside</h2>"));
    }
}

