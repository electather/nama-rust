//! Turns one CI run's test reports into history lines, a job summary and flaky-test issue
//! actions. `.github/workflows/test-report.yml` runs it and does the git and `gh` work around it.

mod report;
mod results;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use clap::Parser;
use serde::Serialize;

use crate::report::{HistoryLine, Issue, Run};

/// Reads the inputs, writes `history.jsonl`, `summary.md` and `issue-actions.jsonl` to `--out`.
#[derive(Debug, Parser)]
struct Args {
    /// The CI run: the `workflow_run` object of the webhook payload, as JSON.
    #[arg(long)]
    run: PathBuf,
    /// History lines from the 30 days before the run, as JSON Lines.
    #[arg(long)]
    history: PathBuf,
    /// The tree of the commit CI tested, as its jobs wrote it next to their reports.
    #[arg(long, value_parser = object_name)]
    tree: String,
    /// The flaky-test issues: `gh issue list --json number,state,title,body` output.
    #[arg(long)]
    issues: PathBuf,
    /// The directory to write the outputs to.
    #[arg(long)]
    out: PathBuf,
    /// Test reports: nextest JUnit (`.xml`) or vitest JSON (`.json`).
    reports: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut results = Vec::new();
    for path in &args.reports {
        let report = read(path)?;
        let parsed = if path.extension().is_some_and(|extension| extension == "xml") {
            results::parse_nextest_junit(&report)
        } else {
            results::parse_vitest_json(&report)
        };
        results.extend(parsed.with_context(|| format!("reading {}", path.display()))?);
    }
    let history: Vec<HistoryLine> = read(&args.history)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .context("reading the history")?;
    let run = Run {
        tree: args.tree,
        ..serde_json::from_str(&read(&args.run)?).context("reading the run")?
    };
    let issues: Vec<Issue> =
        serde_json::from_str(&read(&args.issues)?).context("reading the issues")?;

    let report = report::report(results, &run, &history, &issues)?;

    fs::write(args.out.join("history.jsonl"), json_lines(&report.lines)?)?;
    fs::write(args.out.join("summary.md"), report.summary)?;
    fs::write(
        args.out.join("issue-actions.jsonl"),
        json_lines(&report.actions)?,
    )?;
    Ok(())
}

/// A Git object name: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits. It's read from a PR's
/// reports, so nothing else gets into the history.
fn object_name(text: &str) -> Result<String, String> {
    let hex = text
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if hex && matches!(text.len(), 40 | 64) {
        Ok(text.to_owned())
    } else {
        Err(format!("{text:?} isn't one Git object name"))
    }
}

fn read(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

fn json_lines(items: &[impl Serialize]) -> serde_json::Result<String> {
    items
        .iter()
        .map(|item| Ok(serde_json::to_string(item)? + "\n"))
        .collect()
}
