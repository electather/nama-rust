use std::{env, process::Command, sync::Arc};

use jiff::{Timestamp, tz::TimeZone};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    job::{Job, JobRunner, RunJob},
};

use super::{log_zone, wall_clock_at};
use crate::{
    log_capture::CapturedLog,
    plugins::{Registry, cron::Cron},
};

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
