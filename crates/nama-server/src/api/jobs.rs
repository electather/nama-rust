//! The Jobs Plugins declare and their Job Runs: `nama.v1.JobService` (ADR 0079).

use std::sync::Arc;

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    self as proto, ConfigureJobRequest, ConfigureJobResponse, JobService, ListJobRunsRequest,
    ListJobRunsResponse, ListJobsRequest, ListJobsResponse, PreviewJobScheduleRequest,
    PreviewJobScheduleResponse, RefusalReason, RunJobRequest, RunJobResponse,
};

use crate::{
    db::Database,
    diagnostics::{PluginLine, plugin_lines},
    plugin_settings::PluginSettings,
    plugins::{
        ConfigureError, ExtensionPoints, JobRun, JobRunOutcome, ListedJob, RunRefused,
        configure_job, job_runs, preview_schedule, run_now,
    },
};

use super::{caller, database_failed, plugins::plugin_line_to_proto, refuse};

/// `nama.v1.JobService`: the Jobs of the enabled Plugins and their Runs, for Admins.
pub(super) struct JobServiceHandler {
    pub(super) plugin_settings: PluginSettings,
    pub(super) plugins: ExtensionPoints,
    pub(super) db: Arc<Database>,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl JobService for JobServiceHandler {
    async fn list_jobs(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListJobsRequest>,
    ) -> ServiceResult<ListJobsResponse> {
        let jobs = self.plugin_settings.jobs().await.map_err(database_failed)?;
        Response::ok(ListJobsResponse {
            jobs: jobs.into_iter().map(job_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn list_job_runs(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListJobRunsRequest>,
    ) -> ServiceResult<ListJobRunsResponse> {
        let plugin = PluginId::from(request.plugin_id.to_owned());
        let runs = job_runs(&self.db, &plugin, request.job_id).await;
        let lines = plugin_lines(&plugin);
        Response::ok(ListJobRunsResponse {
            runs: runs
                .map_err(database_failed)?
                .into_iter()
                .map(|run| run_with_lines(run, &lines))
                .collect(),
            ..Default::default()
        })
    }

    async fn configure_job(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ConfigureJobRequest>,
    ) -> ServiceResult<ConfigureJobResponse> {
        let id = PluginId::from(request.plugin_id.to_owned());
        let stored = self.plugin_settings.get(&id).await;
        let plugin = stored.map_err(database_failed)?.map(|it| it.plugin);
        let (job, enabled) = (request.job_id, request.enabled);
        match configure_job(&self.db, plugin, job, enabled, request.schedule).await {
            Ok(()) => Response::ok(ConfigureJobResponse::default()),
            Err(ConfigureError::Database(err)) => Err(database_failed(err)),
            Err(refused @ ConfigureError::NotFound) => {
                let refused = refuse(
                    RefusalReason::REFUSAL_REASON_JOB_NOT_FOUND,
                    refused.to_string(),
                );
                Err(refused.field("job_id").into())
            }
            Err(refused @ (ConfigureError::Invalid(_) | ConfigureError::Unscheduled)) => {
                Err(schedule_invalid(&refused, request.schedule))
            }
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "reading a schedule needs no I/O, but the generated trait's methods are async"
    )]
    async fn preview_job_schedule(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, PreviewJobScheduleRequest>,
    ) -> ServiceResult<PreviewJobScheduleResponse> {
        let next_runs_at_ms = preview_schedule(request.schedule)
            .map_err(|why| schedule_invalid(&why, request.schedule))?;
        Response::ok(PreviewJobScheduleResponse {
            next_runs_at_ms,
            ..Default::default()
        })
    }

    async fn run_job(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RunJobRequest>,
    ) -> ServiceResult<RunJobResponse> {
        let admin = caller(&ctx)?.user.name.clone();
        let plugin = PluginId::from(request.plugin_id.to_owned());
        let started = run_now(&self.plugins, &plugin, request.job_id, admin).await;
        started.map_err(|refused| match refused {
            RunRefused::NotDeclared => refuse(
                RefusalReason::REFUSAL_REASON_JOB_NOT_FOUND,
                "no enabled Plugin with that id declares that Job",
            )
            .field("job_id"),
            RunRefused::Running => refuse(
                RefusalReason::REFUSAL_REASON_JOB_RUNNING,
                "it is running already; run it again once that Run ends",
            ),
        })?;
        Response::ok(RunJobResponse::default())
    }
}

/// Refuses `schedule`, which `why` says can't be used.
fn schedule_invalid(why: &ConfigureError, schedule: &str) -> connectrpc::ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_SCHEDULE_INVALID,
        why.to_string(),
    )
    .field("schedule")
    .param("schedule", &schedule)
    .into()
}

fn job_to_proto(listed: ListedJob) -> proto::Job {
    let ListedJob {
        plugin,
        plugin_name,
        job,
        schedule,
        enabled,
        next_runs_ms,
        last_run,
    } = listed;
    proto::Job {
        plugin_id: plugin.to_string(),
        plugin_name,
        id: job.id,
        name: job.name,
        description: job.description,
        schedule: schedule.unwrap_or_default(),
        default_schedule: job.schedule.unwrap_or_default(),
        enabled,
        next_runs_at_ms: next_runs_ms,
        last_run: last_run.map(run_to_proto).into(),
        ..Default::default()
    }
}

/// `run` with the Plugin Log lines written during it, oldest first, of its Plugin's kept `lines`,
/// which are newest first.
fn run_with_lines(run: JobRun, lines: &[PluginLine]) -> proto::JobRun {
    let written = lines.iter().rev().filter(|line| line.run == Some(run.id));
    proto::JobRun {
        log_lines: written.cloned().map(plugin_line_to_proto).collect(),
        ..run_to_proto(run)
    }
}

fn run_to_proto(run: JobRun) -> proto::JobRun {
    use proto::JobRunOutcome as P;
    let (outcome, error) = match run.outcome {
        JobRunOutcome::Running => (P::JOB_RUN_OUTCOME_RUNNING, String::new()),
        JobRunOutcome::Succeeded => (P::JOB_RUN_OUTCOME_SUCCEEDED, String::new()),
        JobRunOutcome::Failed { error } => (P::JOB_RUN_OUTCOME_FAILED, error),
        JobRunOutcome::TimedOut => (P::JOB_RUN_OUTCOME_TIMED_OUT, String::new()),
        JobRunOutcome::Interrupted => (P::JOB_RUN_OUTCOME_INTERRUPTED, String::new()),
    };
    proto::JobRun {
        id: run.id,
        outcome: outcome.into(),
        started_at_ms: run.started_ms,
        ended_at_ms: run.ended_ms,
        error,
        started_by: run.started_by.unwrap_or_default(),
        ..Default::default()
    }
}
