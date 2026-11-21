//! Provide: a runner of the Jobs its Plugin's [`Manifest`](crate::Manifest) declares in `jobs`,
//! which the Kernel runs on their schedules (ADR 0079).
//!
//! A Plugin serves it by registering an `Arc<dyn JobRunner>`:
//!
//! ```ignore
//! registrar.provide::<dyn JobRunner>(Priority::DEFAULT, Arc::new(my_runner));
//! ```

pub use crate::contract::RunJob;
use crate::{BoxFuture, PluginError};

/// One Job a Plugin's Manifest declares, as `job.proto`'s `Job` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// Unique among the Plugin's Jobs: one or more of a-z, 0-9 and `-`.
    pub id: String,
    /// What admins know the Job by.
    pub name: String,
    /// What the Job does.
    pub description: String,
    /// A 5-field cron expression read in the Server's time zone; `None` runs it only when asked.
    pub schedule: Option<String>,
}

/// A runner of its Plugin's Jobs. It speaks the contract's `JobService` messages, as an Installed
/// Plugin's export does.
pub trait JobRunner: Send + Sync + 'static {
    /// Runs the Job `run.job_id`. The Kernel runs one Job at most once at a time.
    ///
    /// # Errors
    ///
    /// When the Run failed: the Kernel logs why, and the Job's next firing is its retry.
    fn run<'a>(&'a self, run: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>>;
}
