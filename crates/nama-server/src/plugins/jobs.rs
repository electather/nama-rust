//! Jobs: the Kernel runs each Job an enabled Plugin's Manifest declares when its schedule comes
//! due, in the Plugin's scope (ADR 0047, ADR 0079), so disabling the Plugin stops its Jobs.

use std::{sync::Arc, time::Duration};

use jiff::{Timestamp, civil::DateTime, tz::TimeZone};
use nama_plugin_api::{
    Manifest, PluginId, PointKey, Registration,
    job::{JobRunner, RunJob},
};
use nama_plugin_proto::services;
use tracing::{info, warn};

use super::{ExtensionPoints, Failure, cron::Cron, scope::Scope};
use crate::db;

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
/// a [`JobRunner`]. The Registry calls it under its lock, as it does `tick::scope`, so the loops
/// are in the enablement's scope before a disable can find it. Starts nothing outside a Tokio
/// runtime, where no task can run.
pub(super) fn schedule(
    scope: &Scope,
    points: &ExtensionPoints,
    manifest: &Manifest,
    registrations: &[Registration],
) {
    let runner = PointKey::provide::<dyn JobRunner>();
    if !registrations.iter().any(|it| it.point() == runner)
        || tokio::runtime::Handle::try_current().is_err()
    {
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
/// serves until its scope is cancelled or the Plugin no longer runs Jobs. It awaits each Run and
/// counts from its end, so a firing due while one runs is dropped, and from its start, so firings
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
        // None once the Plugin is disabled, or panicked, which disabled it.
        let Some(ran) = run(&points, &plugin, &job).await else {
            return;
        };
        if let Err(failure) = ran {
            warn!(plugin = %plugin, job, error = failure.message, "job run failed");
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

/// `now` as the Server's time zone reads it, looked up each time so a change to the zone reaches
/// the next firing.
fn wall_clock_at(now: Timestamp) -> DateTime {
    TimeZone::system().to_datetime(now)
}

/// How long until the next minute starts on the Kernel's clock.
fn until_next_minute() -> Duration {
    let into_minute = db::now_ms().rem_euclid(60_000);
    Duration::from_millis((60_000 - into_minute).unsigned_abs())
}

/// Runs `plugin`'s Job `job` once, now: how the Run ended, or `None` if the Plugin runs no Jobs,
/// such as one disabled since or one whose Run panicked, which disabled it.
async fn run(
    points: &ExtensionPoints,
    plugin: &PluginId,
    job: &str,
) -> Option<Result<(), Failure>> {
    let run = RunJob {
        job_id: job.to_owned(),
        // db's clock, not the system's, so a test's clock reaches Plugins.
        now_ms: db::now_ms(),
        ..RunJob::default()
    };
    let run = &run;
    let asked = points
        .ask_one(plugin, |runner: Arc<dyn JobRunner>| async move {
            runner.run(run).await
        })
        .await;
    asked.answered().map(|answer| answer.result)
}

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;
