//! A run's history lines as an OTLP/HTTP JSON logs request, the copy Grafana Cloud gets
//! (ADR 0073).

use anyhow::Context as _;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    report::{HistoryLine, unix_seconds},
    results::Outcome,
};

/// One log record per line, stamped with the run's start. The body is the line exactly as the
/// history holds it. `outcome` and `branch` are record attributes, which Loki keeps as structured
/// metadata: as resource attributes they'd be index labels, and every PR branch a new stream.
pub(crate) fn logs(lines: &[HistoryLine]) -> anyhow::Result<Value> {
    let records = lines
        .iter()
        .map(|line| {
            let seconds =
                unix_seconds(&line.time).with_context(|| format!("time {:?}", line.time))?;
            let (severity_number, severity_text) = severity(line.outcome);
            Ok(json!({
                "timeUnixNano": (i128::from(seconds) * 1_000_000_000).to_string(),
                "severityNumber": severity_number,
                "severityText": severity_text,
                "body": { "stringValue": serde_json::to_string(line)? },
                "attributes": [attribute("outcome", line.outcome), attribute("branch", &line.branch)],
            }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(json!({
        "resourceLogs": [{
            "resource": { "attributes": [attribute("service.name", "nama-ci-tests")] },
            "scopeLogs": [{ "scope": { "name": "nama-test-report" }, "logRecords": records }],
        }],
    }))
}

/// The OTLP severity for `outcome`, as its number and text. Without one, Loki guesses a record's
/// level from its body, and a passing test named for an error shows as one.
fn severity(outcome: Outcome) -> (u8, &'static str) {
    match outcome {
        Outcome::Pass => (9, "INFO"),
        Outcome::Flaky => (13, "WARN"),
        Outcome::Fail | Outcome::Hung => (17, "ERROR"),
    }
}

fn attribute(key: &str, value: impl Serialize) -> Value {
    json!({ "key": key, "value": { "stringValue": value } })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::logs;
    use crate::{
        report::HistoryLine,
        results::{Outcome, TestId, Tool},
    };

    #[test]
    fn each_line_is_a_record_with_its_json_as_body_its_outcomes_severity_and_attributes() {
        let line = HistoryLine {
            test: TestId {
                tool: Tool::Nextest,
                suite: "nama-server::api".to_owned(),
                name: "a \"quoted\" name".to_owned(),
            },
            outcome: Outcome::Flaky,
            seconds: 1.5,
            commit: "c0ffee".to_owned(),
            tree: "7ee".to_owned(),
            branch: "agent/issue-1".to_owned(),
            run_id: 7,
            run_attempt: 2,
            time: "2026-11-20T07:00:00Z".to_owned(),
        };

        let logs = logs(std::slice::from_ref(&line)).unwrap();

        let record = &logs["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert_eq!(
            logs["resourceLogs"][0]["resource"]["attributes"],
            json!([{ "key": "service.name", "value": { "stringValue": "nama-ci-tests" } }]),
        );
        assert_eq!(record["timeUnixNano"], "1795158000000000000");
        assert_eq!(record["severityNumber"], 13);
        assert_eq!(record["severityText"], "WARN");
        let body: HistoryLine =
            serde_json::from_str(record["body"]["stringValue"].as_str().unwrap()).unwrap();
        assert_eq!(body, line);
        assert_eq!(
            record["attributes"],
            json!([
                { "key": "outcome", "value": { "stringValue": "flaky" } },
                { "key": "branch", "value": { "stringValue": "agent/issue-1" } },
            ]),
        );
    }
}
