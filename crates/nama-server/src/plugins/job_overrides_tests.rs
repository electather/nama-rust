use std::sync::Arc;

use nama_plugin_api::{Manifest, PluginId, job::Job};
use nama_proto::nama::v1::PluginKind;
use tempfile::TempDir;

use super::{
    ConfigureError, JobOverride, configure_job, forget_undeclared_jobs, job_override,
    job_overrides, schedule,
};
use crate::{
    db::Database,
    plugins::{
        Listed,
        jobs::{Recording, RunLog, job_runs},
    },
};

fn job(id: &str, schedule: Option<&str>) -> Job {
    Job {
        id: id.to_owned(),
        name: id.to_owned(),
        description: String::new(),
        schedule: schedule.map(str::to_owned),
    }
}

/// The Plugin `chores`, declaring `tidy` daily at 04:00 and `sweep`, which only an Admin starts.
fn chores() -> Listed {
    declaring(
        "chores",
        vec![job("tidy", Some("0 4 * * *")), job("sweep", None)],
    )
}

fn declaring(id: &'static str, jobs: Vec<Job>) -> Listed {
    Listed {
        id: PluginId::from_static(id),
        kind: PluginKind::PLUGIN_KIND_BUILT_IN,
        manifest: Ok(Manifest {
            jobs,
            ..Manifest::new(PluginId::from_static(id))
        }),
        checks_settings: false,
        enabled: true,
    }
}

async fn database() -> (Arc<Database>, TempDir) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    (db, data_dir)
}

async fn tidy(db: &Database) -> Option<JobOverride> {
    job_override(db, "chores", "tidy").await.unwrap()
}

#[tokio::test]
async fn only_a_changed_job_has_a_row_and_turning_it_on_at_its_default_deletes_it() {
    let (db, _dir) = database().await;
    let configure =
        |enabled, schedule| configure_job(&db, Some(chores()), "tidy", enabled, schedule);

    configure(true, "").await.unwrap();
    assert_eq!(tidy(&db).await, None);
    configure(true, " 0 4 * * * ").await.unwrap();
    assert_eq!(
        tidy(&db).await,
        None,
        "its default, as written, is no change"
    );
    configure(true, "0  4\t* * *").await.unwrap();
    assert_eq!(
        tidy(&db).await,
        None,
        "its default, spaced otherwise, is no change"
    );

    configure(true, "*/15 * * * *").await.unwrap();
    let every_quarter = Some("*/15 * * * *".to_owned());
    let on = JobOverride {
        enabled: true,
        schedule: every_quarter.clone(),
    };
    assert_eq!(tidy(&db).await, Some(on));
    configure(false, "*/15 * * * *").await.unwrap();
    let off = JobOverride {
        enabled: false,
        schedule: every_quarter,
    };
    assert_eq!(tidy(&db).await, Some(off.clone()));
    let all = job_overrides(&db).await.unwrap();
    assert_eq!(
        all.into_iter().collect::<Vec<_>>(),
        [(("chores".to_owned(), "tidy".to_owned()), off)]
    );

    configure(true, "").await.unwrap();
    assert_eq!(tidy(&db).await, None);
}

#[tokio::test]
async fn a_schedule_that_doesnt_parse_one_for_a_job_without_one_or_an_unknown_job_is_refused() {
    let (db, _dir) = database().await;

    let unparsed = configure_job(&db, Some(chores()), "tidy", true, "0 4 * *").await;
    assert_eq!(
        unparsed.map_err(|it| it.to_string()),
        Err(
            "the schedule \"0 4 * *\" has 4 fields; a schedule has 5: minute, hour, day of month, \
             month and day of week"
                .to_owned()
        )
    );
    let unscheduled = configure_job(&db, Some(chores()), "sweep", true, "0 4 * * *").await;
    assert!(matches!(unscheduled, Err(ConfigureError::Unscheduled)));
    let undeclared = configure_job(&db, Some(chores()), "mop", false, "").await;
    assert!(matches!(undeclared, Err(ConfigureError::NotFound)));
    let unknown_plugin = configure_job(&db, None, "tidy", false, "").await;
    assert!(matches!(unknown_plugin, Err(ConfigureError::NotFound)));
    assert!(job_overrides(&db).await.unwrap().is_empty());

    configure_job(&db, Some(chores()), "sweep", false, "")
        .await
        .unwrap();
    let off = JobOverride {
        enabled: false,
        schedule: None,
    };
    let sweep = job_override(&db, "chores", "sweep").await.unwrap();
    assert_eq!(
        sweep,
        Some(off),
        "a Job without a schedule can still be off"
    );
}

/// Records one Run of `plugin`'s Job `job` in `db`.
async fn ran(db: &Arc<Database>, plugin: &PluginId, job: &str) {
    let log = Some(RunLog::new(Arc::clone(db)));
    let recording = Recording::start(log, plugin, job, None).await;
    drop(recording);
}

#[tokio::test]
async fn a_job_no_plugin_declares_loses_its_override_and_runs_unless_its_plugin_was_refused() {
    let (db, _dir) = database().await;
    let gone = declaring("gone", vec![job("tidy", Some("0 4 * * *"))]);
    let refused = declaring("refused", vec![job("tidy", Some("0 4 * * *"))]);
    for plugin in [chores(), gone, refused.clone()] {
        let id = plugin.id.clone();
        configure_job(&db, Some(plugin), "tidy", false, "")
            .await
            .unwrap();
        ran(&db, &id, "tidy").await;
    }
    // An update dropped `tidy`, `gone` is removed, and the Server refuses `refused` now.
    let updated = declaring("chores", vec![job("sweep", None)]);
    configure_job(&db, Some(updated.clone()), "sweep", false, "")
        .await
        .unwrap();
    ran(&db, &updated.id, "sweep").await;
    let refused = Listed {
        manifest: Err("its contract is too new".to_owned()),
        ..refused
    };

    forget_undeclared_jobs(&db, &[updated, refused]).await;

    let mut kept: Vec<_> = job_overrides(&db).await.unwrap().into_keys().collect();
    kept.sort();
    let key = |plugin: &str, job: &str| (plugin.to_owned(), job.to_owned());
    assert_eq!(kept, [key("chores", "sweep"), key("refused", "tidy")]);
    let runs = |plugin: &'static str, job: &'static str| {
        let db = Arc::clone(&db);
        async move {
            job_runs(&db, &PluginId::from_static(plugin), job)
                .await
                .unwrap()
                .len()
        }
    };
    assert_eq!(runs("chores", "tidy").await, 0);
    assert_eq!(runs("gone", "tidy").await, 0);
    assert_eq!(runs("chores", "sweep").await, 1);
    assert_eq!(runs("refused", "tidy").await, 1);
}

#[test]
fn an_admins_schedule_is_dropped_once_an_update_leaves_its_job_without_one() {
    let own = JobOverride {
        enabled: true,
        schedule: Some("*/15 * * * *".to_owned()),
    };
    let tidy = job("tidy", Some("0 4 * * *"));
    assert_eq!(schedule(&tidy, None), Some("0 4 * * *"));
    assert_eq!(schedule(&tidy, Some(&own)), Some("*/15 * * * *"));
    assert_eq!(schedule(&job("tidy", None), Some(&own)), None);
}
