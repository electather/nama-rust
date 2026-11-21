use std::{env, future::pending, process::Command, sync::Arc, time::Duration};

use jiff::{Timestamp, tz::TimeZone};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    job::{Job, JobRunner, RunJob},
};
use tempfile::TempDir;

use super::{
    JobRun, JobRunOutcome, Recording, Run, RunLog, RunRefused, job_runs, log_zone, run_now,
    wall_clock_at,
};
use crate::{
    db::Database,
    log_capture::CapturedLog,
    plugins::{ExtensionPoints, Registry, cron::Cron, wasm::TimedOut},
};

const ID: PluginId = PluginId::from_static("chores");

/// A Built-in Plugin running the Jobs it declares with its runner.
struct Declaring(Vec<Job>, Arc<dyn JobRunner>);

impl Plugin for Declaring {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn JobRunner>()],
            jobs: self.0.clone(),
            ..Manifest::new(ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn JobRunner>(Priority::DEFAULT, Arc::clone(&self.1));
        Ok(())
    }
}

struct Idle;

impl JobRunner for Idle {
    fn run<'a>(&'a self, _: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }
}

fn job(schedule: Option<&str>) -> Job {
    Job {
        id: "tidy".to_owned(),
        name: "Tidy".to_owned(),
        description: String::new(),
        schedule: schedule.map(str::to_owned),
    }
}

fn enabling(jobs: Vec<Job>) -> Result<(), String> {
    let registry = Registry::new();
    let enabled = registry.enable(Arc::new(Declaring(jobs, Arc::new(Idle))));
    enabled.map(drop).map_err(|refused| refused.to_string())
}

#[test]
fn a_builtin_declaring_no_jobs_or_a_bad_schedule_is_refused_as_an_installed_one_is() {
    assert_eq!(
        enabling(Vec::new()),
        Err(
            "the Server refused plugin chores: its Manifest names nama.plugin.v1.JobService and \
             declares no Jobs"
                .to_owned()
        )
    );
    assert_eq!(
        enabling(vec![job(Some("0 4 * *"))]),
        Err(
            "the Server refused plugin chores: its Job tidy's schedule \"0 4 * *\" has 4 fields; \
             a schedule has 5: minute, hour, day of month, month and day of week"
                .to_owned()
        )
    );
}

#[test]
fn a_builtin_declaring_jobs_with_and_without_a_schedule_is_enabled() {
    assert_eq!(enabling(vec![job(Some("0 4 * * *"))]), Ok(()));
    assert_eq!(enabling(vec![job(None)]), Ok(()));
}

/// A Job runner that ends each Run as its function does, which may panic.
struct Ending(fn() -> Result<(), PluginError>);

impl JobRunner for Ending {
    fn run<'a>(&'a self, _: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { (self.0)() })
    }
}

/// A Job runner whose Runs never end.
struct Endless;

impl JobRunner for Endless {
    fn run<'a>(&'a self, _: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(pending())
    }
}

/// A Registry recording Job Runs in a fresh database, with `runner` running the Job `tidy`.
async fn recording(runner: Arc<dyn JobRunner>) -> (Registry, Arc<Database>, TempDir) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let registry = Registry::new();
    registry.record_job_runs_in(Arc::clone(&db)).await.unwrap();
    registry
        .enable(Arc::new(Declaring(vec![job(None)], runner)))
        .unwrap();
    (registry, db, data_dir)
}

/// Runs the Job `tidy` once, as its schedule would.
async fn run_tidy(points: &ExtensionPoints) {
    let run = Run::start(points, &ID, "tidy", None).await;
    run.expect("tidy isn't running").finish(points).await;
}

async fn runs(db: &Database) -> Vec<JobRun> {
    job_runs(db, &ID, "tidy").await.unwrap()
}

async fn outcome_of_running(ends: fn() -> Result<(), PluginError>) -> JobRunOutcome {
    let (registry, db, _dir) = recording(Arc::new(Ending(ends))).await;
    run_tidy(&registry).await;
    let [run] = &runs(&db).await[..] else {
        panic!("not one run recorded");
    };
    assert!(run.ended_ms.is_some_and(|ended| ended >= run.started_ms));
    run.outcome.clone()
}

#[tokio::test]
async fn a_run_is_recorded_as_succeeded_failed_with_the_plugins_message_or_timed_out() {
    assert_eq!(
        outcome_of_running(|| Ok(())).await,
        JobRunOutcome::Succeeded
    );
    assert_eq!(
        outcome_of_running(|| Err("the disk is full".into())).await,
        JobRunOutcome::Failed {
            error: "the disk is full".to_owned()
        }
    );
    assert_eq!(
        outcome_of_running(|| Err(TimedOut("it didn't answer within 60s".to_owned()).into())).await,
        JobRunOutcome::TimedOut
    );
}

#[tokio::test]
async fn a_run_whose_plugin_panics_is_recorded_as_failed() {
    let outcome = outcome_of_running(|| panic!("tidying went wrong")).await;
    assert!(
        matches!(outcome, JobRunOutcome::Failed { .. }),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_run_its_plugin_is_disabled_during_is_recorded_as_interrupted() {
    let (registry, db, _dir) = recording(Arc::new(Endless)).await;
    let points = registry.extension_points();
    assert!(registry.spawn_in(&ID, async move {
        run_tidy(&points).await;
    }));
    while runs(&db).await.is_empty() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    registry.disable(&ID).await;

    while runs(&db).await[0].outcome == JobRunOutcome::Running {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(runs(&db).await[0].outcome, JobRunOutcome::Interrupted);
}

#[tokio::test]
async fn a_run_the_server_stops_during_is_recorded_as_interrupted_before_shutdown_returns() {
    let (registry, db, _dir) = recording(Arc::new(Endless)).await;
    let points = registry.extension_points();
    assert!(registry.spawn_in(&ID, async move {
        run_tidy(&points).await;
    }));
    while runs(&db).await.is_empty() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    registry.shutdown().await;

    assert_eq!(runs(&db).await[0].outcome, JobRunOutcome::Interrupted);
}

#[tokio::test]
async fn an_admin_cant_run_a_job_its_schedule_is_running_until_that_run_ends() {
    let (registry, _db, _dir) = recording(Arc::new(Idle)).await;
    // As startup does, so the Server has its Manifest.
    registry.describe_builtin(|| Arc::new(Declaring(vec![job(None)], Arc::new(Idle))));
    let scheduled = Run::start(&registry, &ID, "tidy", None).await.unwrap();

    let refused = run_now(&registry, &ID, "tidy", "Ada".to_owned()).await;
    scheduled.finish(&registry).await;
    let started = run_now(&registry, &ID, "tidy", "Ada".to_owned()).await;

    assert_eq!((refused, started), (Err(RunRefused::Running), Ok(())));
}

#[tokio::test]
async fn a_run_a_stopped_server_left_running_is_interrupted_at_the_next_start() {
    let (_registry, db, _dir) = recording(Arc::new(Idle)).await;
    let recording = Recording::start(Some(RunLog::new(Arc::clone(&db))), &ID, "tidy", None).await;
    #[expect(
        clippy::mem_forget,
        reason = "a Server that stops mid-Run never drops its Recording, leaving its row running"
    )]
    std::mem::forget(recording);

    Registry::new()
        .record_job_runs_in(Arc::clone(&db))
        .await
        .unwrap();

    let [run] = &runs(&db).await[..] else {
        panic!("not one run recorded");
    };
    assert_eq!(run.outcome, JobRunOutcome::Interrupted);
}

#[tokio::test]
async fn a_job_keeps_its_newest_50_runs() {
    let (registry, db, _dir) = recording(Arc::new(Idle)).await;
    for _ in 0..51 {
        run_tidy(&registry).await;
    }

    let kept = runs(&db).await;

    assert_eq!(kept.len(), 50);
    let ids: Vec<i64> = kept.iter().map(|run| run.id).collect();
    assert_eq!(
        ids,
        (2..=51).rev().collect::<Vec<_>>(),
        "not the newest, newest first"
    );
}

#[test]
fn with_tz_set_to_london_a_job_at_three_comes_due_at_three_london_time() {
    const LONDON: &str = "Europe/London";
    // TZ is process-wide and jiff caches the zone it read for five minutes, so the test runs again
    // in a child with TZ set rather than set it under the other tests running in this process.
    if env::var_os("TZ").is_none_or(|tz| tz != LONDON) {
        let name = "plugins::jobs::tests::with_tz_set_to_london_a_job_at_three_comes_due_at_three_london_time";
        let child = Command::new(env::current_exe().unwrap())
            .args(["--exact", name])
            .env("TZ", LONDON)
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&child.stdout);
        assert!(child.status.success() && out.contains("1 passed"), "{out}");
        return;
    }
    let three = Cron::parse("0 3 * * *").unwrap();
    // British Summer Time, when London's 03:00 is 02:00 UTC.
    let at = |utc: &str| wall_clock_at(utc.parse::<Timestamp>().unwrap());
    assert!(three.matches(at("2026-07-01T02:00:00Z")));
    assert!(!three.matches(at("2026-07-01T03:00:00Z")));
}

#[test]
fn the_time_zone_job_schedules_are_read_in_is_logged_by_name() {
    let log = CapturedLog::start();
    log_zone(TimeZone::get("Europe/London"));

    let logged = log.finish();
    assert!(logged.contains("INFO"), "{logged}");
    assert!(logged.contains("Europe/London"), "{logged}");
}

#[test]
fn no_time_zone_to_read_job_schedules_in_warns_of_utc_and_names_tz() {
    let log = CapturedLog::start();
    log_zone(TimeZone::get("Nowhere/Atlantis"));

    let logged = log.finish();
    assert!(logged.contains("WARN"), "{logged}");
    for named in ["UTC", "TZ", "/etc/localtime"] {
        assert!(logged.contains(named), "{named} in {logged}");
    }
}
