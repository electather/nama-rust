//! Jobs: the Kernel runs each Job an enabled Plugin's Manifest declares when its schedule comes
//! due, in the Plugin's scope (ADR 0047, ADR 0079), so disabling the Plugin stops its Jobs, and
//! records each Job Run.

use std::{collections::HashMap, sync::Arc, time::Duration};

use jiff::{Timestamp, civil::DateTime, tz::TimeZone};
use nama_plugin_api::{
    Manifest, PluginError, PluginId, PointKey, Registration,
    job::{Job, JobRunner, RunJob},
};
use nama_plugin_proto::services;
use tokio::{runtime::Handle, task::JoinHandle};
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

use super::{
    ExtensionPoints, Failure, FailureKind, Listed, ProviderError,
    cron::Cron,
    job_overrides::{self, job_override},
    points::Asked,
    scope::Scope,
    wasm::TimedOut,
};
use crate::db::{self, Database};

/// How many of a Job's Runs are kept, whatever their outcome, so its history stays bounded.
const RUNS_KEPT: i64 = 50;

/// One Job Run, as recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRun {
    /// The Run's id, which orders a Job's Runs by when they started.
    pub id: i64,
    /// When it started, in milliseconds since the Unix epoch.
    pub started_ms: i64,
    /// When it ended, or `None` while it runs or if the Server stopped during it.
    pub ended_ms: Option<i64>,
    /// How it ended, or that it hasn't.
    pub outcome: JobRunOutcome,
}

/// A Job an enabled Plugin declares, as the Jobs page lists it.
#[derive(Debug, Clone)]
pub struct ListedJob {
    /// The Plugin declaring it.
    pub plugin: PluginId,
    /// The Plugin's name.
    pub plugin_name: String,
    /// The Job, as the Plugin's Manifest declares it.
    pub job: Job,
    /// The schedule it runs on: an Admin's in place of its Manifest's, or that.
    pub schedule: Option<String>,
    /// Whether it runs on its schedule, unless an Admin turned it off.
    pub enabled: bool,
    /// The next times its schedule fires, soonest first, in milliseconds since the Unix epoch:
    /// up to 3, and none without a schedule or while it is off.
    pub next_runs_ms: Vec<i64>,
    /// Its newest Run, if it has run.
    pub last_run: Option<JobRun>,
}

/// How many of a Job's next firings [`list_jobs`] gives.
const NEXT_RUNS: usize = 3;

/// How a Job Run ended, or that it hasn't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunOutcome {
    /// It is still running.
    Running,
    /// The Plugin ran the Job to its end.
    Succeeded,
    /// The Plugin failed it, or crashed.
    Failed {
        /// Why, in the Plugin's words with its secret settings blanked, or that it crashed.
        error: String,
    },
    /// It ran past its time limit.
    TimedOut,
    /// The Server stopped, or the Plugin was disabled, before it ended.
    Interrupted,
}

/// How a Job Run failed: past its time limit, which only an Installed Plugin has, or otherwise.
pub(super) enum RunFailed {
    TimedOut,
    Other,
}

impl FailureKind for RunFailed {
    fn is_failure(&self) -> bool {
        true
    }
}

/// A Job runner's error, which a Run's [`Failure`] tells the kind of.
struct RunError(PluginError);

impl ProviderError<RunFailed> for RunError {
    fn failure(self) -> Failure<RunFailed> {
        let Self(error) = self;
        let kind = if error.is::<TimedOut>() {
            RunFailed::TimedOut
        } else {
            RunFailed::Other
        };
        Failure {
            kind,
            message: error.to_string(),
        }
    }
}

/// How long a firing loop sleeps at most before it reads the Kernel's clock again. It sleeps to
/// the next minute, but a test moves the Kernel's clock without waking it, so there it looks often.
#[cfg(not(feature = "testing"))]
const LOOK_EVERY: Duration = Duration::from_secs(60);
#[cfg(feature = "testing")]
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// Why the Kernel refuses `manifest`'s Jobs, if it does: a Plugin running Jobs must declare at
/// least one, each with an id of its own of the characters `job.proto` allows, and a schedule, if
/// it has one, that parses.
pub(super) fn refusal(manifest: &Manifest) -> Option<String> {
    let runner = PointKey::provide::<dyn JobRunner>();
    if !manifest.extension_points.contains(&runner) {
        return None;
    }
    let jobs = &manifest.jobs;
    if jobs.is_empty() {
        return Some(format!(
            "its Manifest names {} and declares no Jobs",
            services::JOB
        ));
    }
    jobs.iter().enumerate().find_map(|(index, job)| {
        let id = &job.id;
        let allowed = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if id.is_empty() || !id.bytes().all(allowed) {
            Some(format!(
                "its Job {id:?} has an id that isn't one or more of a-z, 0-9 and -"
            ))
        } else if jobs.iter().take(index).any(|earlier| earlier.id == *id) {
            Some(format!("its Job {id} repeats an id"))
        } else {
            let why = Cron::parse(job.schedule.as_deref()?).err()?;
            Some(format!("its Job {id}'s schedule {why}"))
        }
    })
}

/// Starts in `scope` a loop firing each of `manifest`'s scheduled Jobs, if `registrations` provide
/// a [`JobRunner`]. The Registry calls it under its lock, so the loops are in the enablement's
/// scope before a disable can find it. Starts nothing outside a Tokio runtime, where no task can
/// run.
pub(super) fn schedule(
    scope: &Scope,
    points: &ExtensionPoints,
    manifest: &Manifest,
    registrations: &[Registration],
) {
    let runner = PointKey::provide::<dyn JobRunner>();
    if !registrations.iter().any(|it| it.point() == runner) || Handle::try_current().is_err() {
        return;
    }
    for job in &manifest.jobs {
        // Enabling refused a schedule that doesn't parse; a Job without one never fires.
        if let Some(cron) = job.schedule.as_deref().and_then(|it| Cron::parse(it).ok()) {
            let (points, plugin) = (points.clone(), manifest.id.clone());
            scope.spawn(firing(points, plugin, job.id.clone(), cron));
        }
    }
}

/// Runs the Job `job` each time its schedule comes due on the Server's wall clock, from when the
/// Kernel serves until its scope is cancelled or the Plugin no longer runs Jobs. The schedule is
/// `declared` unless an Admin turned the Job off or gave it another, which it reads each minute. It
/// awaits each Run and counts from its end, so a firing due while one runs is dropped, and from its
/// start, so firings missed before are not made up.
async fn firing(points: ExtensionPoints, plugin: PluginId, job: String, declared: Cron) {
    points.kernel_served().await;
    let mut seen = wall_clock();
    loop {
        tokio::time::sleep(until_next_minute().min(LOOK_EVERY)).await;
        let now = wall_clock();
        let due = configured(&points, &plugin, &job, declared)
            .await
            .is_some_and(|cron| cron.fires_between(seen, now));
        // Kept when the clocks go back, so the minutes they repeat don't fire again.
        seen = seen.max(now);
        if !due {
            continue;
        }
        match run_recorded(&points, &plugin, &job).await {
            Asked::Answered(answer) => {
                if let Err(failure) = answer.result {
                    warn!(plugin = %plugin, job, error = failure.message, "job run failed");
                }
            }
            // The Plugin is disabled, or panicked, which disabled it.
            Asked::Absent | Asked::Panicked => return,
        }
        seen = seen.max(wall_clock());
    }
}

/// The schedule `plugin`'s Job `job` fires on now: `declared`, unless an Admin turned it off or gave
/// it another. `None` while it is off, or when what an Admin changed can't be read, since it may
/// be off.
async fn configured(
    points: &ExtensionPoints,
    plugin: &PluginId,
    job: &str,
    declared: Cron,
) -> Option<Cron> {
    let Some(log) = points.read(|state| state.job_runs.clone()) else {
        return Some(declared);
    };
    match job_override(&log.db, plugin.as_str(), job).await {
        Ok(None) => Some(declared),
        Ok(Some(changed)) if !changed.enabled => None,
        // Saving it refused a schedule that doesn't parse.
        Ok(Some(changed)) => changed
            .schedule
            .map_or(Some(declared), |it| Cron::parse(&it).ok()),
        Err(err) => {
            warn!(plugin = %plugin, job, "couldn't read the job's schedule: {err}");
            None
        }
    }
}

/// Logs the time zone Jobs' schedules are read in, the Server's.
pub(crate) fn log_jobs_time_zone() {
    log_zone(TimeZone::try_system());
}

/// Logs `found`, the Server's time zone or why it has none, which leaves Jobs in UTC.
fn log_zone(found: Result<TimeZone, jiff::Error>) {
    match found {
        Ok(zone) => info!(
            zone = zone.iana_name().unwrap_or("unnamed"),
            "reading job schedules in this time zone"
        ),
        Err(error) => warn!(
            %error,
            "reading job schedules in UTC, as the server's time zone can't be found; \
             set TZ to one such as Europe/London or, in docker, mount /etc/localtime"
        ),
    }
}

/// The Kernel's clock as the Server's time zone reads it.
fn wall_clock() -> DateTime {
    wall_clock_at(Timestamp::from_millisecond(db::now_ms()).unwrap_or_default())
}

/// `now` as the Server's time zone reads it, asked of jiff each time so a change to the zone
/// reaches a firing once jiff's five-minute cache of it expires.
fn wall_clock_at(now: Timestamp) -> DateTime {
    TimeZone::system().to_datetime(now)
}

/// How long until the next minute starts on the Kernel's clock.
fn until_next_minute() -> Duration {
    let into_minute = db::now_ms().rem_euclid(60_000);
    Duration::from_millis((60_000 - into_minute).unsigned_abs())
}

/// Runs `plugin`'s Job `job` once, now, recording the Run if the Registry records them, and
/// returns how the Plugin was asked.
pub(super) async fn run_recorded(
    points: &ExtensionPoints,
    plugin: &PluginId,
    job: &str,
) -> Asked<(), RunFailed> {
    let recording =
        Recording::start(points.read(|state| state.job_runs.clone()), plugin, job).await;
    let run = RunJob {
        job_id: job.to_owned(),
        // db's clock, not the system's, so a test's clock reaches Plugins.
        now_ms: db::now_ms(),
        ..RunJob::default()
    };
    let run = &run;
    let asked = points
        .ask_one(plugin, |runner: Arc<dyn JobRunner>| async move {
            runner.run(run).await.map_err(RunError)
        })
        .await;
    recording.close(outcome(&asked)).await;
    asked
}

/// How a Run the Plugin was `asked` to make ended.
fn outcome(asked: &Asked<(), RunFailed>) -> JobRunOutcome {
    match asked {
        Asked::Answered(answer) => match &answer.result {
            Ok(()) => JobRunOutcome::Succeeded,
            Err(Failure {
                kind: RunFailed::TimedOut,
                ..
            }) => JobRunOutcome::TimedOut,
            Err(Failure {
                kind: RunFailed::Other,
                message,
            }) => JobRunOutcome::Failed {
                error: message.clone(),
            },
        },
        // The Registry logs the panic's message.
        Asked::Panicked => JobRunOutcome::Failed {
            error: "it crashed, which disabled it".to_owned(),
        },
        Asked::Absent => JobRunOutcome::Interrupted,
    }
}

/// Where Job Runs are recorded, and the writes ending them, which the Registry waits for when the
/// Server stops so none is cut off by the database closing.
#[derive(Clone)]
pub(super) struct RunLog {
    db: Arc<Database>,
    ending: TaskTracker,
}

impl RunLog {
    pub(super) fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            ending: TaskTracker::new(),
        }
    }

    /// Waits until every Run ending now has been written.
    pub(super) async fn ended(&self) {
        self.ending.close();
        self.ending.wait().await;
    }

    /// Ends the Run `id` as `outcome` in a task of its own, which cancelling the Run's caller
    /// doesn't stop.
    fn end(&self, runtime: &Handle, id: i64, outcome: JobRunOutcome) -> JoinHandle<()> {
        let db = Arc::clone(&self.db);
        self.ending
            .spawn_on(async move { end(&db, id, outcome).await }, runtime)
    }
}

/// A Job Run's row while it runs. [`Recording::close`] ends it as the Run ended; dropping it
/// unclosed, as cancelling the Plugin's scope mid-Run does, ends it as interrupted.
pub(super) struct Recording(Option<(RunLog, i64)>);

impl Recording {
    /// Writes a row for a Run of `plugin`'s Job `job` starting now, in `log` if there is one, and
    /// deletes the Job's Runs but the newest [`RUNS_KEPT`]. A Run whose row can't be written runs
    /// unrecorded.
    pub(super) async fn start(log: Option<RunLog>, plugin: &PluginId, job: &str) -> Self {
        let Some(log) = log else {
            return Self(None);
        };
        let (plugin, job, now) = (plugin.to_string(), job.to_owned(), db::now_ms());
        let started = log
            .db
            .write(move |tx| {
                Box::pin(async move {
                    let id = sqlx::query_scalar!(
                        "INSERT INTO job_runs (plugin, job, outcome, started_ms)
                         VALUES (?, ?, 'running', ?) RETURNING id",
                        plugin,
                        job,
                        now
                    )
                    .fetch_one(&mut **tx)
                    .await?;
                    sqlx::query!(
                        "DELETE FROM job_runs WHERE plugin = ? AND job = ? AND id <= (
                             SELECT id FROM job_runs WHERE plugin = ? AND job = ?
                             ORDER BY id DESC LIMIT 1 OFFSET ?
                         )",
                        plugin,
                        job,
                        plugin,
                        job,
                        RUNS_KEPT
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok::<_, sqlx::Error>(id)
                })
            })
            .await;
        match started {
            Ok(id) => Self(Some((log, id))),
            Err(err) => {
                warn!("couldn't record a job run: {err}");
                Self(None)
            }
        }
    }

    async fn close(mut self, outcome: JobRunOutcome) {
        if let Some((log, id)) = self.0.take() {
            // Its own task, as a Plugin's panic cancels this Run's scope while it closes.
            if let Err(err) = log.end(&Handle::current(), id, outcome).await {
                warn!(run = id, "couldn't record how a job run ended: {err}");
            }
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        // Outside a runtime no task can run, so startup closes the row instead.
        if let Some((log, id)) = self.0.take()
            && let Ok(runtime) = Handle::try_current()
        {
            log.end(&runtime, id, JobRunOutcome::Interrupted);
        }
    }
}

async fn end(db: &Database, id: i64, outcome: JobRunOutcome) {
    let (outcome, error) = match outcome {
        JobRunOutcome::Running => ("running", None),
        JobRunOutcome::Succeeded => ("succeeded", None),
        JobRunOutcome::Failed { error } => ("failed", Some(error)),
        JobRunOutcome::TimedOut => ("timed_out", None),
        JobRunOutcome::Interrupted => ("interrupted", None),
    };
    let now = db::now_ms();
    let ended = db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE job_runs SET outcome = ?, ended_ms = ?, error = ? WHERE id = ?",
                    outcome,
                    now,
                    error,
                    id
                )
                .execute(&mut **tx)
                .await
                .map(drop)
            })
        })
        .await;
    if let Err(err) = ended {
        warn!(run = id, "couldn't record how a job run ended: {err}");
    }
}

/// Closes as interrupted every Job Run left running, which only a Server that stopped during one
/// leaves. Its end stays unknown.
pub(super) async fn interrupt_unfinished(db: &Database) -> Result<(), sqlx::Error> {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query!("UPDATE job_runs SET outcome = 'interrupted' WHERE outcome = 'running'")
                .execute(&mut **tx)
                .await
                .map(drop)
        })
    })
    .await
}

/// The Jobs of each Plugin in `plugins` that is enabled, in that order and then as its Manifest
/// declares them, with what an Admin changed of each, when it next fires and its newest Run in
/// `db`.
///
/// # Errors
///
/// If the database can't be read.
pub(crate) async fn list_jobs(
    plugins: Vec<Listed>,
    db: &Database,
) -> Result<Vec<ListedJob>, sqlx::Error> {
    let mut last_runs = last_runs(db).await?;
    let changed = job_overrides::job_overrides(db).await?;
    let now = wall_clock();
    let manifests = (plugins.into_iter())
        .filter(|plugin| plugin.enabled)
        .filter_map(|plugin| plugin.manifest.ok());
    Ok(manifests
        .flat_map(|manifest| {
            manifest
                .jobs
                .into_iter()
                .map(move |job| (manifest.id.clone(), manifest.name.clone(), job))
        })
        .map(|(plugin, plugin_name, job)| {
            let key = (plugin.to_string(), job.id.clone());
            let changed = changed.get(&key);
            let schedule = job_overrides::schedule(&job, changed).map(str::to_owned);
            let enabled = job_overrides::enabled(changed);
            // Enabling, or saving an Admin's, refused a schedule that doesn't parse.
            let cron = schedule.as_deref().and_then(|it| Cron::parse(it).ok());
            ListedJob {
                next_runs_ms: cron
                    .filter(|_| enabled)
                    .map_or_default(|cron| next_runs_ms(cron, now)),
                last_run: last_runs.remove(&key),
                plugin,
                plugin_name,
                job,
                schedule,
                enabled,
            }
        })
        .collect())
}

/// When `schedule` would next fire, as [`list_jobs`] gives a Job's next Runs, for an Admin to see
/// before they save it.
///
/// # Errors
///
/// Why `schedule` doesn't parse.
pub(crate) fn preview_schedule(schedule: &str) -> Result<Vec<i64>, String> {
    let cron = Cron::parse(schedule.trim())?;
    Ok(next_runs_ms(cron, wall_clock()))
}

/// The next [`NEXT_RUNS`] times `cron` fires after the wall-clock time `now`, as instants.
fn next_runs_ms(cron: Cron, now: DateTime) -> Vec<i64> {
    let zone = TimeZone::system();
    let instant = |at| zone.to_ambiguous_timestamp(at).compatible().ok();
    cron.firings_after(now)
        .take(NEXT_RUNS)
        .filter_map(instant)
        .map(Timestamp::as_millisecond)
        .collect()
}

/// The newest Run of each Job that has one, by Plugin and Job id.
async fn last_runs(db: &Database) -> Result<HashMap<(String, String), JobRun>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT id, plugin, job, outcome, started_ms, ended_ms, error FROM job_runs AS run
         WHERE id = (SELECT MAX(id) FROM job_runs WHERE plugin = run.plugin AND job = run.job)"
    )
    .fetch_all(db.reader())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let run = JobRun {
                id: row.id,
                started_ms: row.started_ms,
                ended_ms: row.ended_ms,
                outcome: stored_outcome(&row.outcome, row.error),
            };
            ((row.plugin, row.job), run)
        })
        .collect())
}

/// The Runs of `plugin`'s Job `job`, newest first, enabled or not.
///
/// # Errors
///
/// If the database can't be read.
pub async fn job_runs(
    db: &Database,
    plugin: &PluginId,
    job: &str,
) -> Result<Vec<JobRun>, sqlx::Error> {
    let plugin = plugin.as_str();
    let rows = sqlx::query!(
        "SELECT id, outcome, started_ms, ended_ms, error FROM job_runs
         WHERE plugin = ? AND job = ? ORDER BY id DESC",
        plugin,
        job
    )
    .fetch_all(db.reader())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| JobRun {
            id: row.id,
            started_ms: row.started_ms,
            ended_ms: row.ended_ms,
            outcome: stored_outcome(&row.outcome, row.error),
        })
        .collect())
}

/// The outcome the `outcome` and `error` columns hold.
fn stored_outcome(outcome: &str, error: Option<String>) -> JobRunOutcome {
    match outcome {
        "running" => JobRunOutcome::Running,
        "succeeded" => JobRunOutcome::Succeeded,
        "failed" => JobRunOutcome::Failed {
            error: error.unwrap_or_default(),
        },
        "timed_out" => JobRunOutcome::TimedOut,
        // The table's CHECK leaves only 'interrupted'.
        _ => JobRunOutcome::Interrupted,
    }
}

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;
