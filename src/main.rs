mod a11y;
mod apply_rules;
mod assets;
mod mv_asset;
mod rename_assets;
mod check_strict;
mod config;
mod css_refs;
mod extract;
mod init;
mod inventory;
mod links;
mod normalize;
mod preflight;
mod seo;
mod show;
mod transforms;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pagekit",
    version,
    about = "Vanilla HTML + CSS site management for agents",
    long_about = "pagekit composes the `fragments` text-sync primitive with HTML-specific \
helpers: page scaffolding, DOM-aware shared-block extraction, health checks. \
\n\nMarkers are HTML comments: `<!-- fragment:NAME -->...<!-- /fragment:NAME -->`. \
Edit `fragments/NAME.html`, run `pagekit sync`, every page with the marker pair updates. \
\n\nConfig lives in `fragments.toml` (optional). See specs/pagekit.md for the schema."
)]
struct Cli {
    /// Project root (contains fragments/ and target files)
    #[arg(default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Sync all files with current fragment content (default)
    Sync,
    /// Watch fragments/ for changes, sync on save
    Watch,
    /// Dry-run: exit 1 if any file is stale or has malformed markers
    Check {
        /// Variance check: hash each marker region per page, report names
        /// whose content differs across pages. Exit 0 = all uniform,
        /// exit 2 = variance detected.
        #[arg(long)]
        strict: bool,
        /// Limit the strict check to a single fragment name.
        /// Mutually exclusive with --selector.
        #[arg(long, value_name = "NAME", conflicts_with = "selector")]
        name: Option<String>,
        /// Generalized strict check: hash all elements matching the
        /// given CSS selector per page (concatenated in document order),
        /// report variance. Implies --strict. Mutually exclusive with
        /// --name.
        #[arg(long, value_name = "CSS")]
        selector: Option<String>,
    },
    /// Create a new HTML page with marker pairs for all fragments
    Init {
        /// Filename to create (e.g. about.html)
        file: String,
    },
    /// Scan pages, detect shared blocks, extract to fragments/ and insert markers
    Extract {
        /// Emit one fragment file per detected content variant; rewrite
        /// markers in source pages to match each page's variant.
        #[arg(long)]
        split_variants: bool,
    },
    /// List every fragment and how many pages reference it
    List,
    /// Print the effective config (defaults merged with fragments.toml)
    Config,
    /// Health check: report orphan fragments, orphan markers, malformed markers
    Doctor,
    /// One-pass site inventory: tab-separated stream of every page's
    /// classes, ids, hrefs, srcs, title, meta tags, headings, and
    /// JSON-LD types. Grep-friendly; the foundation under all the
    /// query-layer checks (links, seo, a11y) that build on top.
    Inventory {
        /// Save inventory to file instead of stdout. Prints a summary line.
        #[arg(long, value_name = "PATH")]
        save: Option<PathBuf>,
    },
    /// Rewrite root-absolute paths (href/src) in every page to be
    /// relative to each page's depth. Idempotent. Defaults
    /// `path_root="/"` when no `[transforms]` config is present —
    /// running the command is the opt-in.
    NormalizePaths,
    /// Find broken internal links, broken anchors, and orphan assets.
    /// External URLs (http://, mailto:, tel:) are NOT fetched. Exit 0
    /// = clean, exit 2 = issues found.
    Links,
    /// SEO health check: titles, descriptions, canonicals, OG/Twitter,
    /// hreflang, JSON-LD validity, heading hierarchy. Exit 0 = no
    /// errors (warns are OK), exit 2 = at least one error.
    Seo,
    /// Accessibility check: grep-able WCAG subset (img alts, form
    /// labels, html lang attr, empty/generic links). Color contrast,
    /// focus styles, and dynamic ARIA need rendering and are NOT
    /// covered. Pass means "cheap checks pass", not "WCAG compliant".
    A11y,
    /// Asset reference graph (HTML hrefs/srcs/srcsets + CSS url()
    /// references). TSV manifest covering hash, byte count, MIME type,
    /// referencing pages and stylesheets, and orphan flag for
    /// unreferenced files. Closes the CSS-loaded-asset gap that
    /// `pagekit links` documents.
    Assets {
        /// Save manifest to file instead of stdout. Prints summary line.
        #[arg(long, value_name = "PATH")]
        save: Option<PathBuf>,
    },
    /// Bundle assembly: print a fragment's HTML + the deduped sorted
    /// list of classes used + the deduped sorted list of URL
    /// references, in one structured report. Replaces three file reads
    /// (fragment, classes-via-grep, urls-via-grep) with one command.
    Show {
        /// Fragment name (no extension; resolves to <name>.html in fragments_dir).
        name: String,
    },
    /// Composes check + doctor + links + seo + a11y into a single
    /// go-live gate. Each check's findings are forwarded inline; the
    /// final summary table reports per-check pass/fail. Exit 0 if all
    /// pass, exit 2 if any fail.
    Preflight,
    /// Apply a parameterized rule file to update many pages at once.
    /// Safe-by-default: runs as a dry-run unless --write is passed.
    Apply {
        /// Path to a TOML rule file.
        rules: PathBuf,
        /// Set parameter values (repeatable): --set key=value
        #[arg(long, value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Actually write changes to disk (default is dry-run).
        #[arg(long)]
        write: bool,
    },
    /// Rename/move an asset and update all references (HTML src/href/srcset, CSS url()).
    /// Safe-by-default: runs as a dry-run unless --write is passed.
    MvAsset {
        /// Existing asset path (relative to target_dir if it exists, else repo root).
        from: PathBuf,
        /// New asset path (relative to target_dir if it exists, else repo root).
        to: PathBuf,
        /// Actually write changes and move the file (default is dry-run).
        #[arg(long)]
        write: bool,
    },
    /// Batch-rename assets to safe names and rewrite all references.
    /// Currently supports a single policy: spaces-to-hyphens.
    /// Safe-by-default: runs as a dry-run unless --write is passed.
    RenameAssets {
        /// Actually write changes and rename files (default is dry-run).
        #[arg(long)]
        write: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = std::fs::canonicalize(&cli.root)
        .with_context(|| format!("cannot resolve root: {}", cli.root.display()))?;

    let config = Config::load(&root)?;

    let hooks = transforms::build_hooks(&config.transforms, &config.core.target_dir);

    match cli.cmd.unwrap_or(Cmd::Sync) {
        Cmd::Sync => {
            let n = fragments::sync_all_with(&root, &config.core, &hooks)?;
            println!("pagekit: updated {n} file(s)");
        }
        Cmd::Watch => {
            let n = fragments::sync_all_with(&root, &config.core, &hooks)?;
            println!(
                "pagekit: synced {n} file(s), watching {}/ …",
                config.core.fragments_dir
            );
            fragments::watch::run_with(&root, &config.core, &hooks)?;
        }
        Cmd::Check {
            strict,
            name,
            selector,
        } => {
            if let Some(sel) = selector.as_deref() {
                let code = check_strict::run_check_strict_selector(&root, &config, sel)?;
                if code != 0 {
                    std::process::exit(code);
                }
                return Ok(());
            }
            if strict {
                let code = check_strict::run_check_strict(&root, &config, name.as_deref())?;
                if code != 0 {
                    std::process::exit(code);
                }
                return Ok(());
            }
            if name.is_some() {
                eprintln!("pagekit: --name requires --strict; ignoring");
            }
            let issues = fragments::check_all_with(&root, &config.core, &hooks)?;
            if issues.is_empty() {
                println!("pagekit: all files up to date");
            } else {
                for issue in &issues {
                    match issue {
                        fragments::CheckIssue::Stale(p) => eprintln!("stale: {}", p.display()),
                        fragments::CheckIssue::UnpairedOpen { path, name } => {
                            eprintln!("unpaired open marker '{}' in {}", name, path.display())
                        }
                        fragments::CheckIssue::UnpairedClose { path, name } => {
                            eprintln!("unpaired close marker '{}' in {}", name, path.display())
                        }
                        fragments::CheckIssue::DuplicatePair { path, name } => eprintln!(
                            "duplicate marker pair '{}' in {} (only first pair gets synced)",
                            name,
                            path.display()
                        ),
                    }
                }
                std::process::exit(1);
            }
        }
        Cmd::Init { file } => {
            init::init_page(&root, &file, &config)?;
        }
        Cmd::Extract { split_variants } => {
            let n = extract::extract_fragments(&root, &config, split_variants)?;
            if n > 0 {
                println!("pagekit: extraction complete, {n} page(s) updated");
            }
        }
        Cmd::List => {
            fragments::list::list_fragments(&root, &config.core)?;
        }
        Cmd::Config => {
            let toml = toml::to_string_pretty(&config).context("serializing config")?;
            print!("{toml}");
        }
        Cmd::Doctor => {
            let issues = fragments::doctor::run_doctor(&root, &config.core)?;
            if issues > 0 {
                std::process::exit(1);
            }
        }
        Cmd::Inventory { save } => {
            inventory::run_inventory(&root, &config, save)?;
        }
        Cmd::NormalizePaths => {
            normalize::normalize_paths(&root, &config)?;
        }
        Cmd::Links => {
            let code = links::run_links(&root, &config)?;
            if code != 0 {
                std::process::exit(code);
            }
        }
        Cmd::Seo => {
            let code = seo::run_seo(&root, &config)?;
            if code != 0 {
                std::process::exit(code);
            }
        }
        Cmd::A11y => {
            let code = a11y::run_a11y(&root, &config)?;
            if code != 0 {
                std::process::exit(code);
            }
        }
        Cmd::Assets { save } => {
            assets::run_assets(&root, &config, save)?;
        }
        Cmd::Show { name } => {
            let code = show::run_show(&root, &config, &name)?;
            if code != 0 {
                std::process::exit(code);
            }
        }
        Cmd::Preflight => {
            let code = preflight::run_preflight(&root, &config)?;
            if code != 0 {
                std::process::exit(code);
            }
        }
        Cmd::Apply { rules, set, write } => {
            let modified = apply_rules::run_apply(&root, &config, &rules, &set, write)?;
            if !write && modified > 0 {
                // Dry-run with pending changes: exit 2 so callers can gate.
                std::process::exit(2);
            }
        }
        Cmd::MvAsset { from, to, write } => {
            let modified = mv_asset::run_mv_asset(&root, &config, &from, &to, write)?;
            if !write && modified > 0 {
                std::process::exit(2);
            }
        }
        Cmd::RenameAssets { write } => {
            let modified = rename_assets::run_rename_assets(&root, &config, write)?;
            if !write && modified > 0 {
                std::process::exit(2);
            }
        }
    }
    Ok(())
}
