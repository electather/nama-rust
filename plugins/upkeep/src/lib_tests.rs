use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    Plugin, Registrar, Registration,
    automation::{
        Kernel, KernelFunction, Permission, answering,
        functions::{
            BackUp, DeleteExpiredSignIns, ForgetGonePlugins, ForgetHomeRows, RefreshMetadata,
        },
    },
    contract::{
        BackedUp, ExpiredSignInsDeleted, GonePluginsForgotten, HomeRowsForgotten, MetadataRefreshed,
    },
    job::{JobRunner, RunJob},
    settings::{SettingValue, Settings},
};

use super::{
    BACK_UP, DELETE_EXPIRED_SIGN_INS, FORGET_GONE_PLUGINS, FORGET_HOME_ROWS, GONE_PLUGINS_DAYS,
    HOME_ROWS_DAYS, REFRESH_METADATA, Upkeep,
};

/// The requests a Kernel function was asked, in order.
type Asked<F> = Arc<Mutex<Vec<<F as KernelFunction>::Request>>>;

/// A Kernel answering `F` alone with `response`, and what it was asked.
fn answering_with<F: KernelFunction + 'static>(
    response: F::Response,
) -> (Arc<dyn Kernel>, Asked<F>) {
    let asked = Asked::<F>::default();
    let recorded = Arc::clone(&asked);
    let kernel = answering::<F>(move |request| {
        recorded.lock().unwrap().push(request);
        Ok(response.clone())
    });
    (kernel, asked)
}

/// Upkeep's Job runner, enabled with `settings` over `kernel`.
fn enabled(settings: Settings, kernel: Arc<dyn Kernel>) -> Arc<dyn JobRunner> {
    let settings = settings.resolve(&Upkeep.manifest().settings).unwrap();
    let mut registrar = Registrar::with_settings(settings).with_kernel(kernel);
    Upkeep.register(&mut registrar).unwrap();
    (registrar.into_registrations().iter())
        .find_map(Registration::provider::<dyn JobRunner>)
        .unwrap()
}

fn run(job_id: &str) -> RunJob {
    RunJob {
        job_id: job_id.to_owned(),
        ..RunJob::default()
    }
}

#[tokio::test]
async fn forgetting_home_rows_asks_the_kernel_with_the_days_set_30_unless_the_admin_set_others() {
    let set = Settings::new(BTreeMap::from([(
        HOME_ROWS_DAYS.to_owned(),
        SettingValue::Integer(7),
    )]));
    for (settings, days) in [(Settings::default(), 30), (set, 7)] {
        let forgotten = HomeRowsForgotten {
            users: 2,
            ..HomeRowsForgotten::default()
        };
        let (kernel, asked) = answering_with::<ForgetHomeRows>(forgotten);

        enabled(settings, kernel)
            .run(&run(FORGET_HOME_ROWS))
            .await
            .unwrap();

        let asked: Vec<_> = asked
            .lock()
            .unwrap()
            .iter()
            .map(|asked| asked.days)
            .collect();
        assert_eq!(asked, [days]);
    }
}

#[tokio::test]
async fn deleting_expired_sign_ins_asks_the_kernel_once() {
    let deleted = ExpiredSignInsDeleted {
        devices: 3,
        invites: 1,
        ..ExpiredSignInsDeleted::default()
    };
    let (kernel, asked) = answering_with::<DeleteExpiredSignIns>(deleted);

    enabled(Settings::default(), kernel)
        .run(&run(DELETE_EXPIRED_SIGN_INS))
        .await
        .unwrap();

    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn forgetting_gone_plugins_asks_the_kernel_with_the_days_set_30_unless_the_admin_set_others()
{
    let set = Settings::new(BTreeMap::from([(
        GONE_PLUGINS_DAYS.to_owned(),
        SettingValue::Integer(90),
    )]));
    for (settings, days) in [(Settings::default(), 30), (set, 90)] {
        let forgotten = GonePluginsForgotten {
            plugin_ids: vec!["radarr".to_owned()],
            ..GonePluginsForgotten::default()
        };
        let (kernel, asked) = answering_with::<ForgetGonePlugins>(forgotten);

        enabled(settings, kernel)
            .run(&run(FORGET_GONE_PLUGINS))
            .await
            .unwrap();

        let asked: Vec<_> = asked
            .lock()
            .unwrap()
            .iter()
            .map(|asked| asked.days)
            .collect();
        assert_eq!(asked, [days]);
    }
}

#[tokio::test]
async fn refreshing_metadata_asks_the_kernel_once() {
    let refreshed = MetadataRefreshed {
        titles: 3,
        ..MetadataRefreshed::default()
    };
    let (kernel, asked) = answering_with::<RefreshMetadata>(refreshed);

    enabled(Settings::default(), kernel)
        .run(&run(REFRESH_METADATA))
        .await
        .unwrap();

    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn backing_up_asks_the_kernel_once() {
    let (kernel, asked) = answering_with::<BackUp>(backed_up());

    enabled(Settings::default(), kernel)
        .run(&run(BACK_UP))
        .await
        .unwrap();

    assert_eq!(asked.lock().unwrap().len(), 1);
}

fn backed_up() -> BackedUp {
    BackedUp {
        name: "nama-20261121T030000Z.tar".to_owned(),
        size_bytes: 4096,
        ..BackedUp::default()
    }
}

#[test]
fn it_requests_the_back_up_permission() {
    assert!(
        (Upkeep.manifest().permissions).contains(&Permission::PERMISSION_BACK_UP),
        "the Back up Job needs it"
    );
}

#[tokio::test]
async fn a_job_upkeep_doesnt_declare_fails_without_asking_the_kernel() {
    let (kernel, asked) = answering_with::<BackUp>(backed_up());

    let failed = enabled(Settings::default(), kernel)
        .run(&run("vacuum"))
        .await;

    assert_eq!(failed.unwrap_err().to_string(), "Upkeep has no Job vacuum");
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn it_backs_up_runs_home_rows_and_expired_sign_ins_daily_and_the_others_on_sunday_mornings() {
    let manifest = Upkeep.manifest();

    let jobs: Vec<_> = (manifest.jobs.iter())
        .map(|job| (job.id.as_str(), job.schedule.as_deref()))
        .collect();
    assert_eq!(
        jobs,
        [
            (BACK_UP, Some("0 3 * * *")),
            (FORGET_HOME_ROWS, Some("0 4 * * *")),
            (DELETE_EXPIRED_SIGN_INS, Some("10 4 * * *")),
            (FORGET_GONE_PLUGINS, Some("20 4 * * 0")),
            (REFRESH_METADATA, Some("30 4 * * 0")),
        ]
    );
}
