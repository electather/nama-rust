//! A run's history lines as an OTLP/HTTP JSON logs request, the copy Grafana Cloud gets
//! (ADR 0073).

use anyhow::Context as _;
use serde::Serialize;
use serde_json::{Value, json};

use crate::report::{HistoryLine, unix_seconds};

/// One log record per line, stamped with the run's start. The body is the line exactly as the
/// history holds it. `outcome` and `branch` are record attributes, which Loki keeps as structured
/// metadata: as resource attributes they'd be index labels, and every PR branch a new stream.
pub(crate) fn logs(lines: &[HistoryLine]) -> anyhow::Result<Value> {
    let records = lines
        .iter()
        .map(|line| {
            let seconds =
                unix_seconds(&line.time).with_context(|| format!("time {:?}", line.time))?;
            Ok(json!({
                "timeUnixNano": (i128::from(seconds) * 1_000_000_000).to_string(),
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
    fn each_line_is_a_record_with_its_json_as_body_and_outcome_and_branch_as_attributes() {
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
