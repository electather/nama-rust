//! Jobs: the Kernel runs each Job an enabled Plugin's Manifest declares when its schedule comes
//! due, in the Plugin's scope (ADR 0047, ADR 0079), so disabling the Plugin stops its Jobs, and
//! records each Job Run.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

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
    Answer, ExtensionPoints, Failure, FailureKind, Listed, ProviderError, cron::Cron,
    points::Asked, scope::Scope, wasm::TimedOut,
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
    /// The name of the Admin who started it, as it was then, or `None` if its schedule did.
    pub started_by: Option<String>,
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
    /// The next times its schedule fires, soonest first, in milliseconds since the Unix epoch:
    /// up to 3, and none without a schedule.
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

/// Runs the Job `job` each time `cron` comes due on the Server's wall clock, from when the Kernel
/// serves until its scope is cancelled or the Plugin no longer runs Jobs. A firing due while a Run
/// of the Job goes, its own or an Admin's, is dropped, and it counts from its start, so firings
/// missed before are not made up.
async fn firing(points: ExtensionPoints, plugin: PluginId, job: String, cron: Cron) {
    points.kernel_served().await;
    let mut seen = wall_clock();
    loop {
        tokio::time::sleep(until_next_minute().min(LOOK_EVERY)).await;
        let now = wall_clock();
        let due = cron.fires_between(seen, now);
        // Kept when the clocks go back, so the minutes they repeat don't fire again.
        seen = seen.max(now);
        if !due {
            continue;
        }
        // `None` while an Admin's Run of it goes, which drops this firing.
        let Some(run) = Run::start(&points, &plugin, &job, None).await else {
            continue;
        };
        if let Asked::Absent | Asked::Panicked = run.finish(&points).await {
            // The Plugin is disabled, or panicked, which disabled it.
            return;
        }
        seen = seen.max(wall_clock());
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

/// Why an Admin's Run of a Job didn't start.
#[derive(Debug, PartialEq, Eq)]
pub enum RunRefused {
    /// No enabled Plugin with the id declares a Job with the id.
    NotDeclared,
    /// The Job has a Run going.
    Running,
}

/// Starts a Run of `plugin`'s Job `job` now in the Plugin's scope, recorded as started by the
/// Admin named `admin`, and returns once its row is written, without waiting for it to end.
///
/// # Errors
///
/// [`RunRefused`] says why it didn't start.
pub(crate) async fn run_now(
    points: &ExtensionPoints,
    plugin: &PluginId,
    job: &str,
    admin: String,
) -> Result<(), RunRefused> {
    if !declares(points, plugin, job) {
        return Err(RunRefused::NotDeclared);
    }
    let run = Run::start(points, plugin, job, Some(admin))
        .await
        .ok_or(RunRefused::Running)?;
    let running = points.clone();
    // Its row is written, so it started: a Plugin disabled since drops it unasked, which records
    // it interrupted, as a disable mid-Run does.
    points.spawn_in(plugin, async move {
        run.finish(&running).await;
    });
    Ok(())
}

/// Whether `plugin` is enabled and its Manifest declares the Job `job`.
fn declares(points: &ExtensionPoints, plugin: &PluginId, job: &str) -> bool {
    points.read(|state| {
        let manifest = state
            .known
            .get(plugin)
            .and_then(|it| it.plugin.as_ref().ok());
        state.plugins.contains_key(plugin)
            && manifest.is_some_and(|(_, manifest)| manifest.jobs.iter().any(|it| it.id == job))
    })
}

/// The Jobs with a Run going, by Plugin and Job id, so a Job runs once at a time whether its
/// schedule or an Admin started it.
#[derive(Clone, Default)]
pub(super) struct RunningJobs(Arc<Mutex<HashSet<(PluginId, String)>>>);

impl RunningJobs {
    /// Marks `plugin`'s Job `job` running until the returned [`Claim`] drops, or `None` if it
    /// already is.
    fn claim(&self, plugin: &PluginId, job: &str) -> Option<Claim> {
        let key = (plugin.clone(), job.to_owned());
        self.lock().insert(key.clone()).then(|| Claim {
            running: self.clone(),
            key,
        })
    }

    // Nothing panics while it is held, so a poisoned set is still whole.
    fn lock(&self) -> MutexGuard<'_, HashSet<(PluginId, String)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A Job marked running in [`RunningJobs`] until this drops.
struct Claim {
    running: RunningJobs,
    key: (PluginId, String),
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.running.lock().remove(&self.key);
    }
}

/// A Job Run under way: its Job claimed, so no other Run of it starts, and its row written.
pub(super) struct Run {
    plugin: PluginId,
    job: String,
    recording: Recording,
    claim: Claim,
}

impl Run {
    /// Claims `plugin`'s Job `job` and records a Run of it starting now, by the Admin named
    /// `started_by` or else its schedule. `None` while the Job has a Run going.
    pub(super) async fn start(
        points: &ExtensionPoints,
        plugin: &PluginId,
        job: &str,
        started_by: Option<String>,
    ) -> Option<Self> {
        let claim = points.read(|state| state.running_jobs.claim(plugin, job))?;
        let log = points.read(|state| state.job_runs.clone());
        Some(Self {
            plugin: plugin.clone(),
            job: job.to_owned(),
            recording: Recording::start(log, plugin, job, started_by).await,
            claim,
        })
    }

    /// Asks the Plugin to run the Job, records how the Run ended and logs a failure. Returns how
    /// the Plugin was asked.
    pub(super) async fn finish(self, points: &ExtensionPoints) -> Asked<(), RunFailed> {
        let Self {
            plugin,
            job,
            recording,
            claim,
        } = self;
        let run = RunJob {
            job_id: job.clone(),
            // db's clock, not the system's, so a test's clock reaches Plugins.
            now_ms: db::now_ms(),
            ..RunJob::default()
        };
        let run = &run;
        let asked = points
            .ask_one(&plugin, |runner: Arc<dyn JobRunner>| async move {
                runner.run(run).await.map_err(RunError)
            })
            .await;
        recording.close(outcome(&asked)).await;
        drop(claim);
        if let Asked::Answered(Answer {
            result: Err(failure),
            ..
        }) = &asked
        {
            warn!(plugin = %plugin, job, error = failure.message, "job run failed");
        }
        asked
    }
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
    /// Writes a row for a Run of `plugin`'s Job `job` starting now, which the Admin named
    /// `started_by` started or else its schedule, in `log` if there is one, and deletes the Job's
    /// Runs but the newest [`RUNS_KEPT`]. A Run whose row can't be written runs unrecorded.
    pub(super) async fn start(
        log: Option<RunLog>,
        plugin: &PluginId,
        job: &str,
        started_by: Option<String>,
    ) -> Self {
        let Some(log) = log else {
            return Self(None);
        };
        let (plugin, job, now) = (plugin.to_string(), job.to_owned(), db::now_ms());
        let started = log
            .db
            .write(move |tx| {
                Box::pin(async move {
                    let id = sqlx::query_scalar!(
                        "INSERT INTO job_runs (plugin, job, outcome, started_ms, started_by)
                         VALUES (?, ?, 'running', ?, ?) RETURNING id",
                        plugin,
                        job,
                        now,
                        started_by
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
/// declares them, with when each next fires and its newest Run in `db`.
///
/// # Errors
///
/// If the database can't be read.
pub(crate) async fn list_jobs(
    plugins: Vec<Listed>,
    db: &Database,
) -> Result<Vec<ListedJob>, sqlx::Error> {
    let mut last_runs = last_runs(db).await?;
    let now = wall_clock();
    let enabled = plugins.into_iter().filter(|plugin| plugin.enabled);
    let manifests = enabled.filter_map(|plugin| plugin.manifest.ok());
    Ok(manifests
        .flat_map(|manifest| {
            manifest
                .jobs
                .into_iter()
                .map(move |job| (manifest.id.clone(), manifest.name.clone(), job))
        })
        .map(|(plugin, plugin_name, job)| ListedJob {
            next_runs_ms: next_runs_ms(job.schedule.as_deref(), now),
            last_run: last_runs.remove(&(plugin.to_string(), job.id.clone())),
            plugin,
            plugin_name,
            job,
        })
        .collect())
}

/// The next [`NEXT_RUNS`] times `schedule` fires after the wall-clock time `now`, as instants.
fn next_runs_ms(schedule: Option<&str>, now: DateTime) -> Vec<i64> {
    // Enabling refused a schedule that doesn't parse.
    let Some(cron) = schedule.and_then(|it| Cron::parse(it).ok()) else {
        return Vec::new();
    };
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
        "SELECT id, plugin, job, outcome, started_ms, ended_ms, error, started_by
         FROM job_runs AS run
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
                started_by: row.started_by,
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
        "SELECT id, outcome, started_ms, ended_ms, error, started_by FROM job_runs
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
            started_by: row.started_by,
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
