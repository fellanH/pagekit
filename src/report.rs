//! Machine-readable output for check commands.
//!
//! Every check that emits human-prose findings (`links`, `seo`, `a11y`)
//! also speaks `--json`: the same findings as a structured envelope an
//! agent can deserialize instead of regexing stdout. Exit codes are
//! unchanged by `--json` — `0` clean, `2` findings.

use serde::Serialize;

/// One machine-readable finding.
#[derive(Serialize)]
pub struct JsonFinding {
    /// Rule family that produced the finding (e.g. `"title"`,
    /// `"broken-internal-link"`, `"img-alt"`).
    pub rule: String,
    /// `"error"` or `"warn"`. Only errors flip a check's status to fail;
    /// warns are reported but pass.
    pub severity: String,
    /// Page URL the finding applies to; omitted for site-wide findings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// Human-readable detail.
    pub message: String,
}

/// The `--json` envelope serialized to stdout by a check command.
#[derive(Serialize)]
pub struct Report {
    /// Check name (`"links"`, `"seo"`, `"a11y"`).
    pub check: &'static str,
    /// `"pass"` or `"fail"` — mirrors the process exit code (`0`/`2`).
    pub status: &'static str,
    pub findings: Vec<JsonFinding>,
}

impl Report {
    /// Print the report as pretty JSON to stdout.
    pub fn print(&self) -> anyhow::Result<()> {
        println!("{}", serde_json::to_string_pretty(self)?);
        Ok(())
    }
}
