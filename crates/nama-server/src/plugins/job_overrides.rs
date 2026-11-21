//! What an Admin changed of a Job (ADR 0079): turned it off, or gave it a schedule in place of its
//! Manifest's. Only a changed Job has a row, so a Plugin update's new schedule reaches the rest,
//! and a Job no Plugin declares any more loses its row and its Runs.

use std::collections::HashMap;

use nama_plugin_api::job::Job;
use thiserror::Error;
use tracing::warn;

use super::{Listed, cron::Cron};
use crate::db::Database;

/// What an Admin changed of a Job, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JobOverride {
    /// Whether it runs on its schedule.
    pub(super) enabled: bool,
    /// The schedule it runs on in place of its Manifest's, if an Admin gave it one.
    pub(super) schedule: Option<String>,
}

/// The schedule `job` runs on as `changed` leaves it, whether or not it is on: none once its
/// Manifest declares none, as only a Job with a schedule fires on its own, even one an Admin gave
/// another before an update dropped it.
pub(super) fn schedule<'a>(job: &'a Job, changed: Option<&'a JobOverride>) -> Option<&'a str> {
    let declared = job.schedule.as_deref()?;
    Some(
        changed
            .and_then(|it| it.schedule.as_deref())
            .unwrap_or(declared),
    )
}

/// Whether `job` runs on its schedule as `changed` leaves it.
pub(super) fn enabled(changed: Option<&JobOverride>) -> bool {
    changed.is_none_or(|it| it.enabled)
}

/// Why an Admin's change to a Job was refused.
#[derive(Debug, Error)]
pub enum ConfigureError {
    /// No Plugin the Server has declares the Job.
    #[error("no plugin the server has declares that job")]
    NotFound,
    /// The schedule doesn't parse; it says why.
    #[error("the schedule {0}")]
    Invalid(String),
    /// The Job has no schedule of its own, so it runs only when an Admin starts it.
    #[error("this job has no schedule to replace: it runs only when an admin starts it")]
    Unscheduled,
    /// The change couldn't be saved.
    #[error("could not save the job's schedule: {0}")]
    Database(#[from] sqlx::Error),
}

/// Turns `plugin`'s Job `job` on or off, and has it run on `schedule` in place of its Manifest's,
/// or on that if `schedule` is blank or the same. A Job left on and on its Manifest's schedule
/// keeps no row. A firing loop reads it at its next minute, so it takes effect without a restart.
///
/// # Errors
///
/// If `plugin` isn't a Plugin the Server has or doesn't declare `job`, `schedule` doesn't parse or
/// is set for a Job without one, or the change can't be saved.
pub(crate) async fn configure_job(
    db: &Database,
    plugin: Option<Listed>,
    job: &str,
    enabled: bool,
    schedule: &str,
) -> Result<(), ConfigureError> {
    let manifest = plugin.and_then(|it| it.manifest.ok());
    let (plugin_id, declared) = manifest
        .and_then(|it| {
            let declared = it.jobs.into_iter().find(|declared| declared.id == job)?;
            Some((it.id.to_string(), declared))
        })
        .ok_or(ConfigureError::NotFound)?;
    let schedule = Some(schedule.trim())
        .filter(|it| !it.is_empty() && Some(*it) != declared.schedule.as_deref())
        .map(str::to_owned);
    if let Some(schedule) = &schedule {
        if declared.schedule.is_none() {
            return Err(ConfigureError::Unscheduled);
        }
        Cron::parse(schedule).map_err(ConfigureError::Invalid)?;
    }
    let job_id = declared.id;
    db.write(move |tx| {
        Box::pin(async move {
            if enabled && schedule.is_none() {
                sqlx::query!(
                    "DELETE FROM job_overrides WHERE plugin = ? AND job = ?",
                    plugin_id,
                    job_id
                )
                .execute(&mut **tx)
                .await?;
            } else {
                sqlx::query!(
                    "INSERT INTO job_overrides (plugin, job, enabled, schedule) VALUES (?, ?, ?, ?)
                     ON CONFLICT (plugin, job)
                     DO UPDATE SET enabled = excluded.enabled, schedule = excluded.schedule",
                    plugin_id,
                    job_id,
                    enabled,
                    schedule
                )
                .execute(&mut **tx)
                .await?;
            }
            Ok::<_, sqlx::Error>(())
        })
    })
    .await?;
    Ok(())
}

/// What an Admin changed of `plugin`'s Job `job`, if anything.
pub(super) async fn job_override(
    db: &Database,
    plugin: &str,
    job: &str,
) -> Result<Option<JobOverride>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT enabled AS \"enabled: bool\", schedule FROM job_overrides
         WHERE plugin = ? AND job = ?",
        plugin,
        job
    )
    .fetch_optional(db.reader())
    .await?;
    Ok(row.map(|row| JobOverride {
        enabled: row.enabled,
        schedule: row.schedule,
    }))
}

/// What an Admin changed of each Job they changed, by Plugin and Job id.
pub(super) async fn job_overrides(
    db: &Database,
) -> Result<HashMap<(String, String), JobOverride>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT plugin, job, enabled AS \"enabled: bool\", schedule FROM job_overrides"
    )
    .fetch_all(db.reader())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let changed = JobOverride {
                enabled: row.enabled,
                schedule: row.schedule,
            };
            ((row.plugin, row.job), changed)
        })
        .collect())
}

/// Deletes the overrides and Runs of each Job no Plugin in `plugins` declares: a Plugin update
/// dropped it, or the Plugin is gone. A Plugin the Server refused keeps its Jobs', as which Jobs it
/// declares can't be read. Logs a failure, which leaves them until the next listing.
pub(crate) async fn forget_undeclared_jobs(db: &Database, plugins: &[Listed]) {
    let refused: Vec<&str> = plugins
        .iter()
        .filter(|plugin| plugin.manifest.is_err())
        .map(|plugin| plugin.id.as_str())
        .collect();
    let declared: Vec<[&str; 2]> = plugins
        .iter()
        .filter_map(|plugin| plugin.manifest.as_ref().ok())
        .flat_map(|manifest| {
            let plugin = manifest.id.as_str();
            manifest
                .jobs
                .iter()
                .map(move |job| [plugin, job.id.as_str()])
        })
        .collect();
    // Plain strings, so serializing them can't fail.
    let refused = serde_json::to_string(&refused).unwrap_or_default();
    let declared = serde_json::to_string(&declared).unwrap_or_default();
    let forgot = db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM job_overrides
                     WHERE plugin NOT IN (SELECT value FROM json_each(?))
                     AND (plugin, job) NOT IN (SELECT value ->> 0, value ->> 1 FROM json_each(?))",
                    refused,
                    declared
                )
                .execute(&mut **tx)
                .await?;
                sqlx::query!(
                    "DELETE FROM job_runs
                     WHERE plugin NOT IN (SELECT value FROM json_each(?))
                     AND (plugin, job) NOT IN (SELECT value ->> 0, value ->> 1 FROM json_each(?))",
                    refused,
                    declared
                )
                .execute(&mut **tx)
                .await
                .map(drop)
            })
        })
        .await;
    if let Err(err) = forgot {
        warn!("couldn't forget the jobs no plugin declares: {err}");
    }
}

#[cfg(test)]
#[path = "job_overrides_tests.rs"]
mod tests;
