use crate::a11y;
use crate::config::Config;
use crate::links;
use crate::seo;
use anyhow::Result;
use std::path::Path;

/// Run `pagekit preflight`. Composes the five go-live verification
/// checks (sync `check`, `doctor`, `links`, `seo`, `a11y`) in
/// sequence; each check's output is forwarded inline under a section
/// header, followed by a final summary table. Aggregated exit code:
/// 0 if all pass, 2 if any fail or error.
///
/// Per-check stdout is NOT silenced — the findings ARE the value;
/// agents read both the per-check detail and the summary. Per
/// `subtract-before-building.md`, refactoring every check module to
/// take a `&mut dyn Write` for output capture is over-scoping for
/// MVP.
pub fn run_preflight(root: &Path, config: &Config) -> Result<i32> {
    let hooks = crate::transforms::build_hooks(&config.transforms, &config.core.target_dir);

    let mut results: Vec<CheckResult> = Vec::new();

    // sync check
    println!("== check ==");
    let check_outcome = run_sync_check(root, config, &hooks);
    if let CheckOutcome::Pass = &check_outcome {
        println!("pagekit: all files up to date");
    }
    results.push(CheckResult::new("check", check_outcome));
    println!();

    // doctor
    println!("== doctor ==");
    let doctor_outcome = match fragments::doctor::run_doctor(root, &config.core) {
        Ok(0) => CheckOutcome::Pass,
        Ok(n) => CheckOutcome::Fail(format!("{n} issue(s)")),
        Err(e) => CheckOutcome::Error(e.to_string()),
    };
    results.push(CheckResult::new("doctor", doctor_outcome));
    println!();

    // links
    println!("== links ==");
    let links_outcome = match links::run_links(root, config) {
        Ok(0) => CheckOutcome::Pass,
        Ok(_) => CheckOutcome::Fail("broken links or orphans".into()),
        Err(e) => CheckOutcome::Error(e.to_string()),
    };
    results.push(CheckResult::new("links", links_outcome));
    println!();

    // seo
    println!("== seo ==");
    let seo_outcome = match seo::run_seo(root, config) {
        Ok(0) => CheckOutcome::Pass,
        Ok(_) => CheckOutcome::Fail("SEO errors".into()),
        Err(e) => CheckOutcome::Error(e.to_string()),
    };
    results.push(CheckResult::new("seo", seo_outcome));
    println!();

    // a11y
    println!("== a11y ==");
    let a11y_outcome = match a11y::run_a11y(root, config) {
        Ok(0) => CheckOutcome::Pass,
        Ok(_) => CheckOutcome::Fail("a11y issues".into()),
        Err(e) => CheckOutcome::Error(e.to_string()),
    };
    results.push(CheckResult::new("a11y", a11y_outcome));
    println!();

    // Summary table.
    let name_w = results.iter().map(|r| r.name.len()).max().unwrap_or(8);
    println!("pagekit preflight:");
    let mut had_failures = 0;
    let mut had_errors = 0;
    for r in &results {
        let (status, detail) = match &r.outcome {
            CheckOutcome::Pass => ("PASS", String::new()),
            CheckOutcome::Fail(msg) => {
                had_failures += 1;
                ("FAIL", format!("  ({msg})"))
            }
            CheckOutcome::Error(msg) => {
                had_errors += 1;
                ("ERROR", format!("  ({msg})"))
            }
        };
        println!("  {:<name_w$}  {status}{detail}", r.name, name_w = name_w);
    }
    println!();

    let total = results.len();
    let passed = total - had_failures - had_errors;
    if had_failures == 0 && had_errors == 0 {
        println!("preflight: {passed} of {total} checks passing — go-live clear");
        Ok(0)
    } else {
        println!(
            "preflight: {} of {total} check(s) failing",
            had_failures + had_errors
        );
        Ok(2)
    }
}

struct CheckResult {
    name: &'static str,
    outcome: CheckOutcome,
}

impl CheckResult {
    fn new(name: &'static str, outcome: CheckOutcome) -> Self {
        Self { name, outcome }
    }
}

enum CheckOutcome {
    Pass,
    Fail(String),
    Error(String),
}

fn run_sync_check(
    root: &Path,
    config: &Config,
    hooks: &[Box<dyn fragments::SyncHook>],
) -> CheckOutcome {
    match fragments::check_all_with(root, &config.core, hooks) {
        Ok(issues) => {
            if issues.is_empty() {
                CheckOutcome::Pass
            } else {
                CheckOutcome::Fail(format!("{} stale or malformed", issues.len()))
            }
        }
        Err(e) => CheckOutcome::Error(e.to_string()),
    }
}
