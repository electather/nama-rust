//! The tool's pure core: one CI run's results plus the recent history in; history lines, a
//! summary and flaky-test issue actions out.

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::results::{Outcome, TestId, TestResult};

/// The CI run being reported: the fields used from the webhook's `workflow_run` object.
#[derive(Debug, Deserialize)]
pub(crate) struct Run {
    pub(crate) head_sha: String,
    pub(crate) head_branch: String,
    pub(crate) id: u64,
    #[serde(rename = "run_attempt")]
    pub(crate) attempt: u32,
    /// UTC, as `YYYY-MM-DDTHH:MM:SSZ`.
    #[serde(rename = "run_started_at")]
    pub(crate) started_at: String,
    pub(crate) html_url: String,
    /// The tree of the commit CI tested, which the webhook doesn't carry. It's the code that
    /// flakes are told apart by (ADR 0022).
    #[serde(skip)]
    pub(crate) tree: String,
}

/// One test's result in one run, as kept in the history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct HistoryLine {
    #[serde(flatten)]
    pub(crate) test: TestId,
    pub(crate) outcome: Outcome,
    pub(crate) seconds: f64,
    pub(crate) commit: String,
    /// As [`Run::tree`].
    pub(crate) tree: String,
    pub(crate) branch: String,
    pub(crate) run_id: u64,
    pub(crate) run_attempt: u32,
    /// When the run started, as [`Run::started_at`].
    pub(crate) time: String,
}

/// An issue as `gh issue list --json number,state,title,body` lists it.
#[derive(Debug, Deserialize)]
pub(crate) struct Issue {
    pub(crate) number: u64,
    pub(crate) state: IssueState,
    pub(crate) title: String,
    pub(crate) body: String,
}

/// Whether an [`Issue`] is open, as `gh` writes it.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum IssueState {
    Open,
    Closed,
}

/// A change to make to the flaky-test issues.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub(crate) enum IssueAction {
    /// Opens an issue, labelled `needs-triage`.
    Open {
        title: String,
        body: String,
    },
    Comment {
        issue: u64,
        body: String,
    },
    Reopen {
        issue: u64,
    },
    Retitle {
        issue: u64,
        title: String,
    },
}

/// What [`report`] produces for one run.
#[derive(Debug)]
pub(crate) struct Report {
    /// To append to the history.
    pub(crate) lines: Vec<HistoryLine>,
    /// Markdown for the run's job summary.
    pub(crate) summary: String,
    pub(crate) actions: Vec<IssueAction>,
}

/// Reports one run's results against the history. `history` needs the 30 days before the run,
/// in the order it was recorded; older lines are ignored. If `history` already holds this run,
/// as when `test-report` is re-run, the summary is the same and nothing is recorded or acted on.
///
/// # Errors
///
/// If the run's start time isn't `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn report(
    results: Vec<TestResult>,
    run: &Run,
    history: &[HistoryLine],
    issues: &[Issue],
) -> anyhow::Result<Report> {
    let now = unix_seconds(&run.started_at)
        .with_context(|| format!("run_started_at {:?}", run.started_at))?;
    let (recorded, history): (Vec<&HistoryLine>, Vec<&HistoryLine>) = history
        .iter()
        .partition(|line| line.run_id == run.id && line.run_attempt == run.attempt);
    let lines: Vec<HistoryLine> = results
        .into_iter()
        .map(|result| HistoryLine {
            test: result.id,
            outcome: result.outcome,
            seconds: result.seconds,
            commit: run.head_sha.clone(),
            tree: run.tree.clone(),
            branch: run.head_branch.clone(),
            run_id: run.id,
            run_attempt: run.attempt,
            time: run.started_at.clone(),
        })
        .collect();
    let earlier = by_test(
        history
            .into_iter()
            .filter(|line| unix_seconds(&line.time).is_some_and(|time| now - time <= LOOKBACK)),
    );
    let flakes = flakes(&lines, &earlier);
    let slowdowns = slowdowns(&lines, &earlier, now);
    let summary = summary(&lines, &flakes, &slowdowns);
    if !recorded.is_empty() {
        return Ok(Report {
            lines: Vec::new(),
            summary,
            actions: Vec::new(),
        });
    }
    let actions = issue_actions(&flakes, run, issues)?;
    Ok(Report {
        lines,
        summary,
        actions,
    })
}

const DAY: i64 = 24 * 60 * 60;
/// How far back the history counts, for flakes and for timings.
const LOOKBACK: i64 = 30 * DAY;

/// A test that flaked in this run.
struct Flake<'a> {
    line: &'a HistoryLine,
    /// How many times it flaked in the last 30 days, this run included.
    count: usize,
}

fn by_test<'a>(
    lines: impl Iterator<Item = &'a HistoryLine>,
) -> HashMap<&'a TestId, Vec<&'a HistoryLine>> {
    let mut by_test: HashMap<_, Vec<_>> = HashMap::new();
    for line in lines {
        by_test.entry(&line.test).or_default().push(line);
    }
    by_test
}

fn history_of<'a, 'b>(
    earlier: &'b HashMap<&TestId, Vec<&'a HistoryLine>>,
    test: &TestId,
) -> &'b [&'a HistoryLine] {
    earlier.get(test).map(Vec::as_slice).unwrap_or_default()
}

/// The tests that flaked in this run, each once: a report can name a test twice, and each test
/// has one issue.
fn flakes<'a>(
    lines: &'a [HistoryLine],
    earlier: &HashMap<&TestId, Vec<&'a HistoryLine>>,
) -> Vec<Flake<'a>> {
    let this_run = by_test(lines.iter());
    let mut seen = HashSet::new();
    lines
        .iter()
        .filter(|line| seen.insert(&line.test))
        .filter_map(|line| {
            let before = history_of(earlier, &line.test);
            let now = history_of(&this_run, &line.test);
            let flaked = flake_results(before.iter().chain(now).copied());
            // Results are judged in order, so the history's flakes come first.
            let first_new = flaked.get(flake_results(before.iter().copied()).len())?;
            Some(Flake {
                line: first_new,
                count: flaked.len(),
            })
        })
        .collect()
}

/// The results among one test's `lines`, in order, that were a flake: a pass on retry, or the
/// first result that gave a tree both a pass and a failure. Later results on that tree don't flake
/// it again (ADR 0022).
fn flake_results<'a>(lines: impl Iterator<Item = &'a HistoryLine>) -> Vec<&'a HistoryLine> {
    let mut by_tree: HashMap<&str, (bool, bool)> = HashMap::new();
    let mut flaked = Vec::new();
    for line in lines {
        let (passed, failed) = by_tree.entry(&line.tree).or_default();
        let was_flaky = *passed && *failed;
        *passed |= line.outcome.passed();
        *failed |= line.outcome.failed();
        if line.outcome == Outcome::Flaky || (!was_flaky && *passed && *failed) {
            flaked.push(line);
        }
    }
    flaked
}

/// A week of timings to compare against the rest of the 30 days: long enough to take a median
/// over, short enough to show a test slowing down soon after.
const RECENT: i64 = 7 * DAY;
/// Tests faster than this are left out of slowdowns: on a loaded runner their times are noise.
const SLOWDOWN_FLOOR_SECONDS: f64 = 1.0;

/// A test whose median time this week is at least double its median before.
struct Slowdown<'a> {
    test: &'a TestId,
    recent: f64,
    before: f64,
}

fn slowdowns<'a>(
    lines: &'a [HistoryLine],
    earlier: &HashMap<&TestId, Vec<&HistoryLine>>,
    now: i64,
) -> Vec<Slowdown<'a>> {
    lines
        .iter()
        .filter_map(|line| {
            let history = history_of(earlier, &line.test);
            // Only passing times: a failure can stop early, and a hung test ran into the limit.
            let (recent, older): (Vec<_>, Vec<_>) = history
                .iter()
                .copied()
                .chain([line])
                .filter(|timed| timed.outcome.passed())
                .partition(|timed| {
                    unix_seconds(&timed.time).is_some_and(|time| now - time < RECENT)
                });
            let recent = median(recent.iter().map(|timed| timed.seconds))?;
            let before = median(older.iter().map(|timed| timed.seconds))?;
            (recent >= SLOWDOWN_FLOOR_SECONDS && recent >= 2.0 * before).then_some(Slowdown {
                test: &line.test,
                recent,
                before,
            })
        })
        .collect()
}

fn median(times: impl Iterator<Item = f64>) -> Option<f64> {
    let mut times: Vec<f64> = times.collect();
    times.sort_by(f64::total_cmp);
    let middle = times.len() / 2;
    if times.len() % 2 == 1 {
        times.get(middle).copied()
    } else {
        Some((times.get(middle.checked_sub(1)?)? + times.get(middle)?) / 2.0)
    }
}

fn issue_actions(
    flakes: &[Flake<'_>],
    run: &Run,
    issues: &[Issue],
) -> anyhow::Result<Vec<IssueAction>> {
    let mut actions = Vec::new();
    for flake in flakes {
        let test = &flake.line.test;
        // Found by this line, not by title, which the count changes and a person may edit. `>`
        // is escaped so a test name holding `-->` can't end the comment and render the rest.
        let marker = format!(
            "<!-- flaky-test {} -->",
            serde_json::to_string(test)?.replace('>', "\\u003e")
        );
        let title = format!("Flaky: {test} ({}× / {}d)", flake.count, LOOKBACK / DAY);
        let note = format!(
            "Flaked in [run {id}, attempt {attempt}]({url}/attempts/{attempt}) on `{commit}` ({branch}).",
            id = run.id,
            attempt = run.attempt,
            url = run.html_url,
            commit = run.head_sha,
            branch = code(&run.head_branch),
        );
        let Some(issue) = issues
            .iter()
            .find(|issue| issue.body.lines().any(|line| line == marker))
        else {
            actions.push(IssueAction::Open {
                title,
                body: format!(
                    "{marker}\n{} passed and failed on the same commit.\n\n{note}",
                    code(test)
                ),
            });
            continue;
        };
        if issue.state == IssueState::Closed {
            actions.push(IssueAction::Reopen {
                issue: issue.number,
            });
        }
        if issue.title != title {
            actions.push(IssueAction::Retitle {
                issue: issue.number,
                title,
            });
        }
        actions.push(IssueAction::Comment {
            issue: issue.number,
            body: note,
        });
    }
    Ok(actions)
}

/// A test slower than this is slow (nextest's `slow-timeout` in `.config/nextest.toml`).
const SLOW_SECONDS: f64 = 10.0;
/// nextest kills a test as hung after this long (`terminate-after` in `.config/nextest.toml`).
const HUNG_SECONDS: f64 = 60.0;
/// How many tests a summary lists as the run's slowest.
const SLOWEST_SHOWN: usize = 10;

fn summary(lines: &[HistoryLine], flakes: &[Flake<'_>], slowdowns: &[Slowdown<'_>]) -> String {
    let count = |outcome| lines.iter().filter(|line| line.outcome == outcome).count();
    let headline = format!(
        "## Tests\n\n{} tests: {} passed, {} failed, {} flaky, {} hung.\n",
        lines.len(),
        count(Outcome::Pass),
        count(Outcome::Fail),
        count(Outcome::Flaky),
        count(Outcome::Hung),
    );
    let flaky: Vec<String> = flakes
        .iter()
        .map(|flake| {
            let how = if flake.line.outcome == Outcome::Flaky {
                "passed on retry"
            } else {
                "passed and failed on this commit"
            };
            format!("- {}: {how}", code(&flake.line.test))
        })
        .collect();
    let hung: Vec<String> = lines
        .iter()
        .filter(|line| line.outcome == Outcome::Hung)
        .map(|line| format!("- {}", code(&line.test)))
        .collect();
    // A hung test's time is the limit it was killed at, so it's left out of the timings.
    let mut by_time: Vec<&HistoryLine> = lines
        .iter()
        .filter(|line| line.outcome != Outcome::Hung)
        .collect();
    by_time.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
    let slow: Vec<String> = by_time
        .iter()
        .take_while(|line| line.seconds > SLOW_SECONDS)
        .map(|line| format!("- {}", timed(&line.test, line.seconds)))
        .collect();
    let slowing: Vec<String> = slowdowns
        .iter()
        .map(|slowdown| {
            format!(
                "- {}: median {:.1} s this week, {:.1} s the {} days before",
                code(slowdown.test),
                slowdown.recent,
                slowdown.before,
                (LOOKBACK - RECENT) / DAY,
            )
        })
        .collect();
    let slowest: Vec<String> = by_time
        .iter()
        .take(SLOWEST_SHOWN)
        .enumerate()
        .map(|(rank, line)| format!("{}. {}", rank + 1, timed(&line.test, line.seconds)))
        .collect();
    [
        Some(headline),
        section("Flaky", &flaky),
        section(&format!("Hung, killed at {HUNG_SECONDS} s"), &hung),
        section(&format!("Slow, over {SLOW_SECONDS} s"), &slow),
        section("Slowing down", &slowing),
        section(&format!("{SLOWEST_SHOWN} slowest"), &slowest),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

/// A Markdown section listing `items`, or nothing when there are none.
fn section(title: &str, items: &[String]) -> Option<String> {
    (!items.is_empty()).then(|| format!("### {title}\n\n{}\n", items.join("\n")))
}

fn timed(test: &TestId, seconds: f64) -> String {
    format!("{}: {seconds:.1} s", code(test))
}

/// `text` as Markdown code. Test and branch names come from a PR, so a backtick in one must not
/// end the code span and let the rest through as Markdown, such as an @mention.
fn code(text: impl Display) -> String {
    format!("`{}`", text.to_string().replace('`', "'"))
}

/// Seconds since the Unix epoch of a UTC `YYYY-MM-DDTHH:MM:SSZ` time, as GitHub writes them.
fn unix_seconds(time: &str) -> Option<i64> {
    let (date, clock) = time.strip_suffix('Z')?.split_once('T')?;
    let numbers = |text: &str, separator| -> Option<Vec<i64>> {
        text.split(separator)
            .map(|number| number.parse().ok())
            .collect()
    };
    let [year, month, day] = numbers(date, '-')?[..] else {
        return None;
    };
    let [hour, minute, second] = numbers(clock, ':')?[..] else {
        return None;
    };
    // Days from 1970-01-01 to a proleptic Gregorian date: Howard Hinnant's `days_from_civil`.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * DAY + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::{HistoryLine, Issue, IssueAction, IssueState, Report, Run, report};
    use crate::results::{Outcome, TestId, Tool, parse_nextest_junit, parse_vitest_json};

    const COMMIT: &str = "c0ffee";
    /// The tree this run tested.
    const TREE: &str = "7ee";
    const NOW: &str = "2026-11-08T12:00:00Z";

    fn run() -> Run {
        Run {
            head_sha: COMMIT.to_owned(),
            head_branch: "main".to_owned(),
            id: 7,
            attempt: 1,
            started_at: NOW.to_owned(),
            html_url: "https://github.com/o/r/actions/runs/7".to_owned(),
            tree: TREE.to_owned(),
        }
    }

    /// A nextest JUnit report of `nama-server`'s unit tests holding `cases`.
    fn junit(cases: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run"><testsuite name="nama-server">{cases}</testsuite></testsuites>"#
        )
    }

    fn case(name: &str, seconds: f64, children: &str) -> String {
        format!(
            r#"<testcase name="{name}" classname="nama-server" time="{seconds}">{children}</testcase>"#
        )
    }

    fn test(name: &str) -> TestId {
        TestId {
            tool: Tool::Nextest,
            suite: "nama-server".to_owned(),
            name: name.to_owned(),
        }
    }

    /// An earlier result of `name` from a PR run that tested `tree`, `days_ago` (up to 38) days
    /// before this run.
    fn earlier(
        name: &str,
        outcome: Outcome,
        seconds: f64,
        tree: &str,
        days_ago: u32,
    ) -> HistoryLine {
        HistoryLine {
            test: test(name),
            outcome,
            seconds,
            commit: "pr-head".to_owned(),
            tree: tree.to_owned(),
            branch: "feature".to_owned(),
            run_id: 1,
            run_attempt: 1,
            time: if days_ago < 8 {
                format!("2026-11-{:02}T12:00:00Z", 8 - days_ago)
            } else {
                format!("2026-10-{:02}T12:00:00Z", 39 - days_ago)
            },
        }
    }

    fn report_junit(cases: &str, history: &[HistoryLine], issues: &[Issue]) -> Report {
        let results = parse_nextest_junit(&junit(cases)).expect("valid JUnit");
        report(results, &run(), history, issues).expect("a valid run")
    }

    #[test]
    fn a_clean_run_appends_a_passing_line_per_test_and_opens_no_issues() {
        let report = report_junit(&(case("a", 0.5, "") + &case("b", 1.3, "")), &[], &[]);

        assert_eq!(
            report.lines,
            vec![
                HistoryLine {
                    test: test("a"),
                    outcome: Outcome::Pass,
                    seconds: 0.5,
                    commit: COMMIT.to_owned(),
                    tree: TREE.to_owned(),
                    branch: "main".to_owned(),
                    run_id: 7,
                    run_attempt: 1,
                    time: NOW.to_owned(),
                },
                HistoryLine {
                    test: test("b"),
                    seconds: 1.3,
                    ..report.lines[0].clone()
                },
            ]
        );
        assert_eq!(
            report.summary,
            "## Tests\n\n2 tests: 2 passed, 0 failed, 0 flaky, 0 hung.\n\n\
             ### 10 slowest\n\n1. `nama-server b`: 1.3 s\n2. `nama-server a`: 0.5 s\n"
        );
        assert_eq!(report.actions, vec![]);
    }

    /// The body nextest writes for a failed attempt before a passing retry.
    const FLAKY_FAILURE: &str =
        r#"<flakyFailure type="test failure with exit code 101">panicked</flakyFailure>"#;

    #[test]
    fn a_test_that_passed_on_retry_is_flaky_and_gets_an_issue() {
        let report = report_junit(&case("a", 0.5, FLAKY_FAILURE), &[], &[]);

        assert_eq!(report.lines[0].outcome, Outcome::Flaky);
        assert!(
            report
                .summary
                .contains("### Flaky\n\n- `nama-server a`: passed on retry\n")
        );
        assert_eq!(
            report.actions,
            vec![IssueAction::Open {
                title: "Flaky: nama-server a (1× / 30d)".to_owned(),
                body: "<!-- flaky-test {\"tool\":\"nextest\",\"suite\":\"nama-server\",\"name\":\"a\"} -->\n\
                       `nama-server a` passed and failed on the same commit.\n\n\
                       Flaked in [run 7, attempt 1](https://github.com/o/r/actions/runs/7/attempts/1) \
                       on `c0ffee` (`main`)."
                    .to_owned(),
            }]
        );
    }

    /// A failed attempt with no passing retry.
    const FAILURE: &str = r#"<failure type="test failure with exit code 101">panicked</failure>"#;

    #[test]
    fn a_failure_on_main_after_a_pass_on_its_pr_with_the_same_tree_is_flaky() {
        let history = [earlier("a", Outcome::Pass, 0.5, TREE, 1)];

        let report = report_junit(&case("a", 0.5, FAILURE), &history, &[]);

        assert_eq!(report.lines[0].outcome, Outcome::Fail);
        assert!(
            report
                .summary
                .contains("- `nama-server a`: passed and failed on this commit\n")
        );
        assert!(matches!(report.actions[..], [IssueAction::Open { .. }]));
    }

    #[test]
    fn a_test_that_failed_on_another_tree_is_not_flaky() {
        let history = [earlier("a", Outcome::Fail, 0.5, "broken", 1)];

        let report = report_junit(&case("a", 0.5, ""), &history, &[]);

        assert!(!report.summary.contains("Flaky"));
        assert_eq!(report.actions, vec![]);
    }

    #[test]
    fn a_pass_older_than_30_days_does_not_make_a_failure_flaky() {
        let history = [earlier("a", Outcome::Pass, 0.5, TREE, 31)];

        let report = report_junit(&case("a", 0.5, FAILURE), &history, &[]);

        assert_eq!(report.actions, vec![]);
    }

    #[test]
    fn a_tree_already_flaky_does_not_flake_again_when_it_passes() {
        let history = [
            earlier("a", Outcome::Pass, 0.5, TREE, 1),
            earlier("a", Outcome::Fail, 0.5, TREE, 1),
        ];

        let report = report_junit(&case("a", 0.5, ""), &history, &[]);

        assert_eq!(report.actions, vec![]);
    }

    #[test]
    fn a_hung_test_is_listed_as_hung_and_not_as_slow() {
        let hung = case(
            "h",
            60.0,
            r#"<failure type="test timeout"/><rerunFailure type="test timeout"/>"#,
        );

        let report = report_junit(&(hung + &case("a", 0.5, "")), &[], &[]);

        assert_eq!(report.lines[0].outcome, Outcome::Hung);
        assert!(
            report
                .summary
                .contains("### Hung, killed at 60 s\n\n- `nama-server h`\n")
        );
        assert!(!report.summary.contains("### Slow"));
        assert!(
            report
                .summary
                .contains("### 10 slowest\n\n1. `nama-server a`: 0.5 s\n")
        );
    }

    #[test]
    fn a_test_over_10_seconds_is_slow() {
        let report = report_junit(&(case("s", 12.0, "") + &case("a", 10.0, "")), &[], &[]);

        assert!(
            report
                .summary
                .contains("### Slow, over 10 s\n\n- `nama-server s`: 12.0 s\n\n###")
        );
    }

    #[test]
    fn the_summary_lists_only_the_10_slowest_tests() {
        let cases: String = (1..=11)
            .map(|n| case(&format!("t{n}"), f64::from(n), ""))
            .collect();

        let report = report_junit(&cases, &[], &[]);

        assert!(report.summary.contains("10. `nama-server t2`: 2.0 s\n"));
        assert!(!report.summary.contains("`nama-server t1`"));
    }

    #[test]
    fn a_test_whose_median_time_doubled_is_slowing_down() {
        let history = [
            earlier("s", Outcome::Pass, 0.9, "old", 20),
            earlier("s", Outcome::Pass, 1.0, "old", 15),
            earlier("s", Outcome::Pass, 1.1, "old", 10),
            earlier("s", Outcome::Pass, 2.2, "new", 2),
        ];

        let report = report_junit(&case("s", 2.4, ""), &history, &[]);

        assert!(report.summary.contains(
            "### Slowing down\n\n- `nama-server s`: median 2.3 s this week, 1.0 s the 23 days before\n"
        ));
    }

    #[test]
    fn a_test_under_a_second_whose_time_doubled_is_not_slowing_down() {
        let history = [earlier("f", Outcome::Pass, 0.2, "old", 10)];

        let report = report_junit(&case("f", 0.5, ""), &history, &[]);

        assert!(!report.summary.contains("Slowing down"));
    }

    /// The issue for test `a`, as `gh issue list` gives it.
    fn issue_for_a(state: IssueState) -> Issue {
        Issue {
            number: 42,
            state,
            title: "Flaky: nama-server a (1× / 30d)".to_owned(),
            body: "<!-- flaky-test {\"tool\":\"nextest\",\"suite\":\"nama-server\",\"name\":\"a\"} -->\n\
                   `nama-server a` passed and failed on the same commit."
                .to_owned(),
        }
    }

    /// Test `a` flaked on an earlier commit and, through a retry, flakes again in this run.
    fn flakes_again(issue: Issue) -> Vec<IssueAction> {
        let history = [
            earlier("a", Outcome::Fail, 0.5, "older", 3),
            earlier("a", Outcome::Pass, 0.5, "older", 3),
        ];
        report_junit(&case("a", 0.5, FLAKY_FAILURE), &history, &[issue]).actions
    }

    const FLAKED_AGAIN: &str = "Flaked in [run 7, attempt 1](https://github.com/o/r/actions/runs/7/attempts/1) \
                                on `c0ffee` (`main`).";

    #[test]
    fn a_known_flaky_test_flaking_again_gets_a_comment_and_a_new_count() {
        assert_eq!(
            flakes_again(issue_for_a(IssueState::Open)),
            vec![
                IssueAction::Retitle {
                    issue: 42,
                    title: "Flaky: nama-server a (2× / 30d)".to_owned()
                },
                IssueAction::Comment {
                    issue: 42,
                    body: FLAKED_AGAIN.to_owned()
                },
            ]
        );
    }

    #[test]
    fn a_closed_issue_is_reopened_when_its_test_flakes_again() {
        assert_eq!(
            flakes_again(issue_for_a(IssueState::Closed)),
            vec![
                IssueAction::Reopen { issue: 42 },
                IssueAction::Retitle {
                    issue: 42,
                    title: "Flaky: nama-server a (2× / 30d)".to_owned()
                },
                IssueAction::Comment {
                    issue: 42,
                    body: FLAKED_AGAIN.to_owned()
                },
            ]
        );
    }

    #[test]
    fn a_vitest_and_a_nextest_report_are_recorded_together() {
        let vitest = r#"{"testResults": [{
            "name": "/_work/jit-runner-3/nama-rust/nama-rust/web/src/lib/access.test.ts",
            "assertionResults": [
                {"ancestorTitles": ["whoIsSignedIn"], "title": "reads the session", "status": "passed",
                 "duration": 1500, "failureMessages": []},
                {"ancestorTitles": [], "title": "retried", "status": "passed",
                 "duration": 20, "failureMessages": ["Error: first attempt"]},
                {"ancestorTitles": [], "title": "broken", "status": "failed",
                 "duration": 5, "failureMessages": ["Error: no"]},
                {"ancestorTitles": [], "title": "later", "status": "skipped", "failureMessages": []}
            ]}]}"#;
        let mut results = parse_vitest_json(vitest).expect("a vitest report");
        results.extend(parse_nextest_junit(&junit(&case("a", 0.5, ""))).expect("valid JUnit"));

        let report = report(results, &run(), &[], &[]).expect("a valid run");

        let web = |name: &str| TestId {
            tool: Tool::Vitest,
            suite: "web/src/lib/access.test.ts".to_owned(),
            name: name.to_owned(),
        };
        let recorded: Vec<_> = report
            .lines
            .into_iter()
            .map(|line| (line.test, line.outcome, line.seconds))
            .collect();
        assert_eq!(
            recorded,
            vec![
                (web("whoIsSignedIn > reads the session"), Outcome::Pass, 1.5),
                (web("retried"), Outcome::Flaky, 0.02),
                (web("broken"), Outcome::Fail, 0.005),
                (test("a"), Outcome::Pass, 0.5),
            ]
        );
    }

    #[test]
    fn a_backtick_in_a_test_name_cannot_end_its_code_span() {
        let report = report_junit(&case("a` @someone `b", 0.5, FLAKY_FAILURE), &[], &[]);

        assert!(
            report
                .summary
                .contains("- `nama-server a' @someone 'b`: passed on retry\n")
        );
    }

    #[test]
    fn a_test_that_errored_then_passed_on_retry_is_flaky() {
        let report = report_junit(
            &case("a", 0.5, r#"<flakyError type="test abort"/>"#),
            &[],
            &[],
        );

        assert_eq!(report.lines[0].outcome, Outcome::Flaky);
    }

    #[test]
    fn the_issue_title_counts_each_flake_not_each_tree() {
        let history = [
            earlier("a", Outcome::Flaky, 0.5, TREE, 3),
            earlier("a", Outcome::Flaky, 0.5, TREE, 2),
        ];

        let report = report_junit(&case("a", 0.5, FLAKY_FAILURE), &history, &[]);

        assert!(matches!(
            &report.actions[..],
            [IssueAction::Open { title, .. }] if title == "Flaky: nama-server a (3× / 30d)"
        ));
    }

    #[test]
    fn a_test_named_twice_in_a_run_gets_one_issue() {
        let cases = case("a", 0.5, FLAKY_FAILURE) + &case("a", 0.5, FLAKY_FAILURE);

        let report = report_junit(&cases, &[], &[]);

        assert!(matches!(report.actions[..], [IssueAction::Open { .. }]));
    }

    #[test]
    fn a_run_already_in_the_history_is_not_recorded_or_acted_on_again() {
        let first = report_junit(&case("a", 0.5, FLAKY_FAILURE), &[], &[]);

        let again = report_junit(&case("a", 0.5, FLAKY_FAILURE), &first.lines, &[]);

        assert_eq!(again.lines, vec![]);
        assert_eq!(again.actions, vec![]);
        assert_eq!(again.summary, first.summary);
    }

    #[test]
    fn a_test_name_cannot_end_the_issue_marker() {
        let report = report_junit(&case("a --&gt; @someone", 0.5, FLAKY_FAILURE), &[], &[]);

        let [IssueAction::Open { body, .. }] = &report.actions[..] else {
            panic!("expected one issue, got {:?}", report.actions);
        };
        assert!(body.starts_with(
            r#"<!-- flaky-test {"tool":"nextest","suite":"nama-server","name":"a --\u003e @someone"} -->"#
        ));
    }

    #[test]
    fn a_backtick_in_a_branch_name_cannot_end_its_code_span() {
        let run = Run {
            head_branch: "x` @someone `y".to_owned(),
            ..run()
        };
        let results =
            parse_nextest_junit(&junit(&case("a", 0.5, FLAKY_FAILURE))).expect("valid JUnit");

        let report = report(results, &run, &[], &[]).expect("a valid run");

        let [IssueAction::Open { body, .. }] = &report.actions[..] else {
            panic!("expected one issue, got {:?}", report.actions);
        };
        assert!(body.ends_with("(`x' @someone 'y`)."));
    }
}
