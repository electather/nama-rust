//! A runner of the Jobs its Plugin's Manifest declares, which the Kernel runs on their schedules.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{Job, RunJob};

/// A runner of its Plugin's Jobs (ADR 0079). Its Manifest's `points` name
/// [`services::JOB`](crate::services::JOB), its `jobs` declare at least one Job, and
/// [`job_runner!`](crate::job_runner!) exports it:
///
/// ```
/// use nama_pdk::{JobRunner, RunJob, extism_pdk::Error};
///
/// struct Tidy;
///
/// impl JobRunner for Tidy {
///     fn run(run: RunJob) -> Result<(), Error> {
///         match run.job_id.as_str() {
///             "tidy" => Ok(()),
///             other => Err(Error::msg(format!("there is no Job {other}"))),
///         }
///     }
/// }
///
/// nama_pdk::job_runner!(Tidy);
/// ```
pub trait JobRunner {
    /// Runs the Job `run.job_id`. `run.now_ms` is the Kernel's clock. The Kernel runs one Job at
    /// most once at a time, and allows a Run 60 seconds.
    ///
    /// # Errors
    ///
    /// When the Run failed, such as a Kernel function refused: the Kernel logs the error's message,
    /// and the Job's next firing is its retry.
    fn run(run: RunJob) -> Result<(), Error>;
}

/// Exports a [`JobRunner`] as the Plugin's `run_job`, taking the `nama.plugin.v1.RunJob` the
/// Kernel sends it and returning a `nama.plugin.v1.JobRan`.
#[macro_export]
macro_rules! job_runner {
    ($runner:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, JobRunner, RunJob, extism_pdk,
                nama_plugin_proto::nama::plugin::v1::JobRan, plugin_fn,
            };

            #[plugin_fn]
            pub fn run_job(Buffa(run): Buffa<RunJob>) -> FnResult<Buffa<JobRan>> {
                <$runner as JobRunner>::run(run)?;
                Ok(Buffa(JobRan::default()))
            }
        };
    };
}
