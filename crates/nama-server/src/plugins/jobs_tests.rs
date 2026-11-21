use std::sync::Arc;

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    job::{Job, JobRunner, RunJob},
};

use crate::plugins::Registry;

const ID: PluginId = PluginId::from_static("chores");

/// A Built-in Plugin running the Jobs it declares, each of which does nothing.
struct Declaring(Vec<Job>);

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
        registrar.provide::<dyn JobRunner>(Priority::DEFAULT, Arc::new(Idle));
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
    let enabled = registry.enable(Arc::new(Declaring(jobs)));
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
