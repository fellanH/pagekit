use crate::config::Config;
use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

/// Well-known fragment names with specific placement in the HTML structure.
fn placement(name: &str) -> Placement {
    match name {
        "head" => Placement::Head,
        "body-open" => Placement::BodyOpen,
        "body-close" => Placement::BodyClose,
        _ => Placement::Body,
    }
}

enum Placement {
    Head,
    BodyOpen,
    BodyClose,
    Body,
}

pub fn init_page(root: &Path, file: &str, config: &Config) -> Result<()> {
    let target_root = root.join(&config.core.target_dir);
    let dest = target_root.join(file);
    if dest.exists() {
        bail!("{file} already exists");
    }

    let fragments_dir = root.join(&config.core.fragments_dir);
    let names = discover_fragment_names(&fragments_dir);

    let prefix = &config.core.marker_prefix;

    let mut head_markers = Vec::new();
    let mut body_open_markers = Vec::new();
    let mut body_markers = Vec::new();
    let mut body_close_markers = Vec::new();

    for name in &names {
        let block = format!("<!-- {prefix}:{name} -->\n<!-- /{prefix}:{name} -->");
        match placement(name) {
            Placement::Head => head_markers.push(block),
            Placement::BodyOpen => body_open_markers.push(block),
            Placement::BodyClose => body_close_markers.push(block),
            Placement::Body => body_markers.push(block),
        }
    }

    let title = file.trim_end_matches(".html");
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html lang=\"en\">\n");
    html.push_str("<head>\n");
    html.push_str(&format!("  <title>{title}</title>\n"));
    for m in &head_markers {
        html.push_str(&indent(m, 2));
        html.push('\n');
    }
    html.push_str("</head>\n");
    html.push_str("<body>\n");
    for m in &body_open_markers {
        html.push_str(&indent(m, 2));
        html.push('\n');
    }
    for m in &body_markers {
        html.push_str(&indent(m, 2));
        html.push('\n');
    }
    html.push('\n');
    for m in &body_close_markers {
        html.push_str(&indent(m, 2));
        html.push('\n');
    }
    html.push_str("</body>\n");
    html.push_str("</html>\n");

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dest, &html)?;
    println!("  created {file}");

    ensure_agents_md(&fragments_dir, config)?;

    Ok(())
}

fn ensure_agents_md(fragments_dir: &Path, config: &Config) -> Result<()> {
    let path = fragments_dir.join("AGENTS.md");
    if path.exists() {
        return Ok(());
    }

    let prefix = &config.core.marker_prefix;
    let dir = &config.core.fragments_dir;

    let content = format!(
        r#"# Shared Fragments

Files in this folder are synced into target files by `pagekit` (built on `fragments`).

## How it works

- Each `<name>.html` here is a shared fragment
- Pages opt in with marker pairs: `<!-- {prefix}:<name> -->...<!-- /{prefix}:<name> -->`
- `pagekit sync` propagates changes from here into all pages
- `pagekit check` verifies all pages are up to date (CI/pre-commit)

## Editing

1. Edit a fragment file in `{dir}/`
2. Run `pagekit sync`
3. Every page with matching markers is updated

Never edit content between markers in page files — it gets overwritten on next sync.
"#
    );

    fs::write(&path, content)?;
    println!("  created {dir}/AGENTS.md");
    Ok(())
}

fn discover_fragment_names(fragments_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(fragments_dir) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "html")
                .unwrap_or(false)
        })
        .map(|e| e.path().file_stem().unwrap().to_string_lossy().to_string())
        .collect();

    names.sort();
    names
}

fn indent(s: &str, spaces: usize) -> String {
    let pad: String = " ".repeat(spaces);
    s.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
