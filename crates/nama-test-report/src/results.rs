//! Test results read from nextest's JUnit report and vitest's JSON report.

use std::fmt;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

/// The test runner a result came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tool {
    Nextest,
    Vitest,
}

/// One test, the same in every run: its tool, suite (a nextest binary such as
/// `nama-server::api`, or a vitest file such as `web/src/lib/access.test.ts`) and full name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct TestId {
    pub(crate) tool: Tool,
    pub(crate) suite: String,
    pub(crate) name: String,
}

impl fmt::Display for TestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.suite, self.name)
    }
}

/// How a test ended in one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Outcome {
    Pass,
    Fail,
    /// Failed, then passed on a retry.
    Flaky,
    /// Killed for running too long.
    Hung,
}

impl Outcome {
    /// Whether the test passed at least once.
    pub(crate) fn passed(self) -> bool {
        matches!(self, Self::Pass | Self::Flaky)
    }

    /// Whether the test failed at least once.
    pub(crate) fn failed(self) -> bool {
        matches!(self, Self::Fail | Self::Flaky | Self::Hung)
    }
}

/// How one test did in one run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TestResult {
    pub(crate) id: TestId,
    pub(crate) outcome: Outcome,
    pub(crate) seconds: f64,
}

/// Reads a nextest JUnit report. Skipped tests are left out.
///
/// # Errors
///
/// If the XML is malformed or a test case lacks a `name`, `classname` or `time`.
pub(crate) fn parse_nextest_junit(xml: &str) -> anyhow::Result<Vec<TestResult>> {
    let report = roxmltree::Document::parse(xml)?;
    report
        .descendants()
        .filter(|node| node.has_tag_name("testcase") && !has_child(*node, "skipped"))
        .map(|case| {
            let attribute = |name| {
                case.attribute(name)
                    .with_context(|| format!("a JUnit testcase without `{name}`"))
            };
            Ok(TestResult {
                id: TestId {
                    tool: Tool::Nextest,
                    suite: attribute("classname")?.to_owned(),
                    name: attribute("name")?.to_owned(),
                },
                outcome: nextest_outcome(case),
                seconds: attribute("time")?.parse()?,
            })
        })
        .collect()
}

fn nextest_outcome(case: roxmltree::Node<'_, '_>) -> Outcome {
    let last_failure = case
        .children()
        .find(|node| node.has_tag_name("failure") || node.has_tag_name("error"));
    match last_failure {
        // nextest's type for a test killed by `slow-timeout`'s `terminate-after`.
        Some(failure) if failure.attribute("type") == Some("test timeout") => Outcome::Hung,
        Some(_) => Outcome::Fail,
        None if has_child(case, "flakyFailure") => Outcome::Flaky,
        None => Outcome::Pass,
    }
}

fn has_child(node: roxmltree::Node<'_, '_>, tag: &str) -> bool {
    node.children().any(|child| child.has_tag_name(tag))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VitestReport {
    test_results: Vec<VitestFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VitestFile {
    /// The test file's absolute path.
    name: String,
    assertion_results: Vec<VitestTest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VitestTest {
    ancestor_titles: Vec<String>,
    title: String,
    status: String,
    /// Milliseconds, retries included.
    duration: Option<f64>,
    /// Kept from failed attempts even when a retry passed.
    failure_messages: Vec<String>,
}

/// Reads a vitest JSON report. Tests that didn't run (skipped, todo) are left out.
///
/// vitest's JUnit report doesn't show a test that passed on retry, so this reads its JSON one.
///
/// # Errors
///
/// If the JSON isn't a vitest report.
pub(crate) fn parse_vitest_json(json: &str) -> anyhow::Result<Vec<TestResult>> {
    let report: VitestReport = serde_json::from_str(json)?;
    Ok(report
        .test_results
        .into_iter()
        .flat_map(|file| {
            let suite = web_relative(&file.name);
            file.assertion_results.into_iter().filter_map(move |test| {
                let outcome = match (test.status.as_str(), test.failure_messages.is_empty()) {
                    ("passed", true) => Outcome::Pass,
                    ("passed", false) => Outcome::Flaky,
                    ("failed", _) => Outcome::Fail,
                    _ => return None,
                };
                let mut name = test.ancestor_titles;
                name.push(test.title);
                Some(TestResult {
                    id: TestId {
                        tool: Tool::Vitest,
                        suite: suite.clone(),
                        name: name.join(" > "),
                    },
                    outcome,
                    seconds: test.duration.unwrap_or_default() / 1000.0,
                })
            })
        })
        .collect())
}

/// `web/…` from an absolute path, which differs between runners' checkouts.
fn web_relative(path: &str) -> String {
    path.split_once("/web/")
        .map_or_else(|| path.to_owned(), |(_, rest)| format!("web/{rest}"))
}
