//! The Jobs Plugins declare and their Job Runs: `nama.v1.JobService` (ADR 0079).

use std::sync::Arc;

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    self as proto, JobService, ListJobRunsRequest, ListJobRunsResponse, ListJobsRequest,
    ListJobsResponse,
};

use crate::{
    db::Database,
    plugin_settings::PluginSettings,
    plugins::{JobRun, JobRunOutcome, ListedJob, job_runs},
};

use super::database_failed;

/// `nama.v1.JobService`: the Jobs of the enabled Plugins and their Runs, for Admins.
pub(super) struct JobServiceHandler {
    pub(super) plugin_settings: PluginSettings,
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
        Response::ok(ListJobRunsResponse {
            runs: runs
                .map_err(database_failed)?
                .into_iter()
                .map(run_to_proto)
                .collect(),
            ..Default::default()
        })
    }
}

fn job_to_proto(listed: ListedJob) -> proto::Job {
    let ListedJob {
        plugin,
        plugin_name,
        job,
        next_runs_ms,
        last_run,
    } = listed;
    proto::Job {
        plugin_id: plugin.to_string(),
        plugin_name,
        id: job.id,
        name: job.name,
        description: job.description,
        schedule: job.schedule.unwrap_or_default(),
        next_runs_at_ms: next_runs_ms,
        last_run: last_run.map(run_to_proto).into(),
        ..Default::default()
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
        ..Default::default()
    }
}
