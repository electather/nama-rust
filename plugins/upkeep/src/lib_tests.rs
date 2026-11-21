use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, Plugin, Registrar, Registration,
    automation::{
        ForgetGonePluginsAsked, ForgetHomeRowsAsked, GonePluginsForgotten, HomeRowsForgotten,
        Kernel, KernelRefused,
    },
    job::{JobRunner, RunJob},
    settings::{SettingValue, Settings},
};

use super::{FORGET_GONE_PLUGINS, FORGET_HOME_ROWS, GONE_PLUGINS_DAYS, HOME_ROWS_DAYS, Upkeep};

/// Each Kernel function a Job called, with the days it asked with.
type Asked = Arc<Mutex<Vec<(String, u32)>>>;

/// Upkeep's Job runner, enabled with `settings` over a Kernel that records each function called
/// with the days it asks with, and answers as if it forgot something.
fn enabled(settings: Settings) -> (Arc<dyn JobRunner>, Asked) {
    let asked = Asked::default();
    let recorded = Arc::clone(&asked);
    let kernel = move |function: &str,
                       request: Vec<u8>|
          -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
        let (days, answer) = match function {
            "forget_home_rows" => (
                ForgetHomeRowsAsked::decode_from_slice(&request)
                    .unwrap()
                    .days,
                HomeRowsForgotten {
                    users: 2,
                    ..HomeRowsForgotten::default()
                }
                .encode_to_vec(),
            ),
            "forget_gone_plugins" => (
                ForgetGonePluginsAsked::decode_from_slice(&request)
                    .unwrap()
                    .days,
                GonePluginsForgotten {
                    plugin_ids: vec!["radarr".to_owned()],
                    ..GonePluginsForgotten::default()
                }
                .encode_to_vec(),
            ),
            other => panic!("Upkeep called {other}"),
        };
        recorded.lock().unwrap().push((function.to_owned(), days));
        Box::pin(async move { Ok(answer) })
    };
    let settings = settings.resolve(&Upkeep.manifest().settings).unwrap();
    let kernel: Arc<dyn Kernel> = Arc::new(kernel);
    let mut registrar = Registrar::with_settings(settings).with_kernel(kernel);
    Upkeep.register(&mut registrar).unwrap();
    let runner = (registrar.into_registrations().iter())
        .find_map(Registration::provider::<dyn JobRunner>)
        .unwrap();
    (runner, asked)
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
        let (runner, asked) = enabled(settings);

        runner.run(&run(FORGET_HOME_ROWS)).await.unwrap();

        assert_eq!(
            *asked.lock().unwrap(),
            [("forget_home_rows".to_owned(), days)]
        );
    }
}

#[tokio::test]
async fn forgetting_gone_plugins_asks_the_kernel_with_the_days_set_30_unless_the_admin_set_others()
{
    let set = Settings::new(BTreeMap::from([(
        GONE_PLUGINS_DAYS.to_owned(),
        SettingValue::Integer(90),
    )]));
    for (settings, days) in [(Settings::default(), 30), (set, 90)] {
        let (runner, asked) = enabled(settings);

        runner.run(&run(FORGET_GONE_PLUGINS)).await.unwrap();

        assert_eq!(
            *asked.lock().unwrap(),
            [("forget_gone_plugins".to_owned(), days)]
        );
    }
}

#[tokio::test]
async fn a_job_upkeep_doesnt_declare_fails_without_asking_the_kernel() {
    let (runner, asked) = enabled(Settings::default());

    let failed = runner.run(&run("vacuum")).await;

    assert_eq!(failed.unwrap_err().to_string(), "Upkeep has no Job vacuum");
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn it_runs_forget_home_rows_daily_at_four_and_forget_gone_plugins_weekly() {
    let manifest = Upkeep.manifest();

    let jobs: Vec<_> = (manifest.jobs.iter())
        .map(|job| (job.id.as_str(), job.schedule.as_deref()))
        .collect();
    assert_eq!(
        jobs,
        [
            (FORGET_HOME_ROWS, Some("0 4 * * *")),
            (FORGET_GONE_PLUGINS, Some("20 4 * * 0")),
        ]
    );
}
