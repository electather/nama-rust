//! Jobs: the Kernel runs each Job of an enabled Plugin on its schedule, one Run at a time per Job
//! (ADR 0079).

#[path = "support/installed_test_plugin.rs"]
mod installed_test_plugin;
#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::time::Duration;

use connectrpc::{ErrorCode, client::HttpClient};
use jiff::{ToSpan as _, Zoned};
use log::Log;
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, GetPluginRequest, Job, JobRunOutcome,
    JobServiceClient, ListJobRunsRequest, ListJobsRequest, Permission, PluginJob, RefusalReason,
    RunJobRequest,
};
use refused::assert_refused;
use server::TestServer;

const STARTED: &[&str] = &["test-plugin's Job wait started"];
const ENDED: &[&str] = &["test-plugin's Job wait ended"];
const ON_ASK_ENDED: &[&str] = &["test-plugin's Job on-ask ended"];
const DAY: Duration = Duration::from_hours(24);
/// Long enough for a firing loop that would fire to have run the Job.
const SETTLE: Duration = Duration::from_millis(500);

/// How long from now on the real clock, which a Server moving the clock starts on, until 04:00
/// next comes in the Server's time zone, when the test Plugin's `wait` is due.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn until_four() -> Duration {
    let now = Zoned::now();
    let today = now.with().hour(4).minute(0).second(0).subsec_nanosecond(0);
    let mut four = today.build().unwrap();
    if four <= now {
        four = four.checked_add(1.day()).unwrap();
    }
    now.duration_until(&four).unsigned_abs()
}

/// A Server moving the clock, with the test Plugin built with Jobs enabled.
async fn with_jobs() -> TestServer {
    let server = TestServer::builder().moves_clock().start().await;
    server.enable_test_plugin("jobs").await;
    server
}

#[tokio::test]
async fn a_job_fires_on_its_schedule_drops_a_firing_due_while_it_runs_and_stops_with_its_plugin() {
    let server = with_jobs().await;
    // Once the Server holds the clock alone, so no other test's Server logs Job lines meanwhile.
    let log = Log::global();
    let (started, ended) = (log.count(STARTED), log.count(ENDED));
    let on_ask = log.count(ON_ASK_ENDED);

    server.advance_clock(until_four().saturating_sub(Duration::from_secs(5)));
    tokio::time::sleep(SETTLE).await;
    assert_eq!(log.count(STARTED), started, "it fired before 04:00");

    server.advance_clock(Duration::from_secs(5));
    log.wait_for(STARTED, started + 1).await;
    // The next 04:00 comes due while the Run waits for the clock to move.
    server.advance_clock(DAY);
    log.wait_for(ENDED, ended + 1).await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(
        log.count(STARTED),
        started + 1,
        "the overlapping firing ran"
    );

    server.advance_clock(DAY);
    log.wait_for(STARTED, started + 2).await;
    // Ends the Run, which the runtime's drop would otherwise wait out to its time limit.
    server.advance_clock(Duration::from_mins(1));
    log.wait_for(ENDED, ended + 2).await;
    assert_eq!(
        log.count(ON_ASK_ENDED),
        on_ask,
        "a Job without a schedule fired"
    );

    server
        .plugin_service()
        .disable_plugin(DisablePluginRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    server.advance_clock(DAY);
    tokio::time::sleep(SETTLE).await;
    assert_eq!(log.count(STARTED), started + 2, "it fired once disabled");
}

/// The Jobs the test Plugin declares, as `server`'s Admin lists them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn test_plugin_jobs(server: &TestServer) -> Vec<Job> {
    let jobs = JobServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_jobs(ListJobsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .jobs;
    jobs.into_iter()
        .filter(|job| job.plugin_id == "test-plugin")
        .collect()
}

/// The outcome of the newest Run of the test Plugin's `wait`, once it isn't `running`.
async fn waits_last_outcome(server: &TestServer) -> JobRunOutcome {
    loop {
        let jobs = test_plugin_jobs(server).await;
        let wait = jobs.first().and_then(|job| job.last_run.as_option());
        let outcome = wait.map(|run| run.outcome);
        match outcome.and_then(|it| it.as_known()) {
            Some(JobRunOutcome::JOB_RUN_OUTCOME_RUNNING) | None => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Some(known) => return known,
        }
    }
}

#[tokio::test]
async fn an_admin_sees_each_job_and_its_runs_a_disabled_plugins_come_back_and_others_are_refused() {
    let server = with_jobs().await;
    // Once the Server holds the clock alone, so no other test's Server logs Job lines meanwhile.
    let log = Log::global();
    let (started, ended) = (log.count(STARTED), log.count(ENDED));

    // Before the clock moves, which would leave the Admin's sign-in not recent.
    let viewer = server
        .user("viewer", &[Permission::PERMISSION_MANAGE_SETTINGS])
        .await;
    let jobs = test_plugin_jobs(&server).await;
    let ids: Vec<&str> = jobs.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(ids, ["wait", "on-ask"]);
    assert_eq!(jobs[0].schedule, "0 4 * * *");
    let day_ms = i64::try_from(DAY.as_millis()).unwrap();
    let next = &jobs[0].next_runs_at_ms;
    assert_eq!(next.len(), 3);
    assert_eq!([next[1] - next[0], next[2] - next[1]], [day_ms, day_ms]);
    assert!(jobs[0].last_run.is_unset());
    assert!(
        jobs[1].next_runs_at_ms.is_empty(),
        "a Job without a schedule fires"
    );

    server.advance_clock(until_four());
    log.wait_for(STARTED, started + 1).await;
    // Ends the Run, which waits for the clock to move.
    server.advance_clock(Duration::from_mins(1));
    log.wait_for(ENDED, ended + 1).await;
    assert_eq!(
        waits_last_outcome(&server).await,
        JobRunOutcome::JOB_RUN_OUTCOME_SUCCEEDED
    );
    let service = JobServiceClient::new(HttpClient::plaintext(), server.client_config());
    let runs = service
        .list_job_runs(ListJobRunsRequest {
            plugin_id: "test-plugin".to_owned(),
            job_id: "wait".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .runs;
    let [run] = &runs[..] else {
        panic!("not one Run: {runs:?}");
    };
    assert!(run.ended_at_ms.is_some_and(|end| end > run.started_at_ms));
    assert_eq!(run.started_by, "", "a scheduled Run names who started it");

    let plugins = server.plugin_service();
    let id = "test-plugin".to_owned();
    (plugins.disable_plugin(DisablePluginRequest {
        id: id.clone(),
        ..Default::default()
    }))
    .await
    .unwrap();
    assert_eq!(test_plugin_jobs(&server).await, Vec::<Job>::new());
    (plugins.enable_plugin(EnablePluginRequest {
        id,
        ..Default::default()
    }))
    .await
    .unwrap();
    let back = test_plugin_jobs(&server).await;
    assert_eq!(back[0].last_run.as_option().map(|it| it.id), Some(run.id));

    let refused = JobServiceClient::new(HttpClient::plaintext(), viewer.client_config())
        .list_jobs(ListJobsRequest::default())
        .await
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn a_plugin_not_yet_enabled_shows_the_jobs_its_manifest_declares() {
    let server = TestServer::builder().start().await;
    server.install_test_plugin("jobs").await;

    let plugin = server
        .plugin_service()
        .get_plugin(GetPluginRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .plugin;
    let jobs: Vec<(&str, &str, &str)> = plugin
        .jobs
        .iter()
        .map(
            |PluginJob {
                 id, name, schedule, ..
             }| (id.as_str(), name.as_str(), schedule.as_str()),
        )
        .collect();
    assert_eq!(
        jobs,
        [("wait", "wait", "0 4 * * *"), ("on-ask", "on-ask", "")]
    );
}

fn run_request(job: &str) -> RunJobRequest {
    RunJobRequest {
        plugin_id: "test-plugin".to_owned(),
        job_id: job.to_owned(),
        ..Default::default()
    }
}

#[tokio::test]
async fn an_admin_runs_a_job_now_as_themselves_once_at_a_time_and_only_one_an_enabled_plugin_declares()
 {
    let server = with_jobs().await;
    // Once the Server holds the clock alone, so no other test's Server logs Job lines meanwhile.
    let log = Log::global();
    let (started, ended) = (log.count(STARTED), log.count(ENDED));
    let service = JobServiceClient::new(HttpClient::plaintext(), server.client_config());

    service.run_job(run_request("wait")).await.unwrap();
    assert_refused(
        service.run_job(run_request("wait")).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_JOB_RUNNING,
    );
    assert_refused(
        service.run_job(run_request("nap")).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_JOB_NOT_FOUND,
    );
    log.wait_for(STARTED, started + 1).await;
    // Ends the Run, which waits for the clock to move.
    server.advance_clock(Duration::from_mins(1));
    log.wait_for(ENDED, ended + 1).await;
    assert_eq!(
        waits_last_outcome(&server).await,
        JobRunOutcome::JOB_RUN_OUTCOME_SUCCEEDED
    );
    let wait = &test_plugin_jobs(&server).await[0];
    assert_eq!(
        wait.last_run.as_option().map(|run| run.started_by.as_str()),
        Some("Admin")
    );
    let on_ask = log.count(ON_ASK_ENDED);
    service.run_job(run_request("on-ask")).await.unwrap();
    log.wait_for(ON_ASK_ENDED, on_ask + 1).await;

    (server
        .plugin_service()
        .disable_plugin(DisablePluginRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        }))
    .await
    .unwrap();
    assert_refused(
        service.run_job(run_request("on-ask")).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_JOB_NOT_FOUND,
    );
}
