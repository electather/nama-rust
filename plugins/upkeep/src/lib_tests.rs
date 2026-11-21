use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, Plugin, Registrar, Registration,
    automation::{ForgetHomeRowsAsked, HomeRowsForgotten, Kernel, KernelRefused},
    job::{JobRunner, RunJob},
    settings::{SettingValue, Settings},
};

use super::{FORGET_HOME_ROWS, HOME_ROWS_DAYS, Upkeep};

/// Upkeep's Job runner, enabled with `settings` over a Kernel that records the days each
/// `forget_home_rows` asks with, and answers that it forgot 2 Users.
fn enabled(settings: Settings) -> (Arc<dyn JobRunner>, Arc<Mutex<Vec<u32>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&asked);
    let kernel = move |function: &str,
                       request: Vec<u8>|
          -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
        assert_eq!(function, "forget_home_rows");
        let days = ForgetHomeRowsAsked::decode_from_slice(&request)
            .unwrap()
            .days;
        recorded.lock().unwrap().push(days);
        let forgotten = HomeRowsForgotten {
            users: 2,
            ..HomeRowsForgotten::default()
        };
        Box::pin(async move { Ok(forgotten.encode_to_vec()) })
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

        assert_eq!(*asked.lock().unwrap(), [days]);
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
fn it_runs_forget_home_rows_daily_at_four() {
    let manifest = Upkeep.manifest();

    let [job] = manifest.jobs.as_slice() else {
        panic!("{:?}", manifest.jobs);
    };
    assert_eq!(job.id, FORGET_HOME_ROWS);
    assert_eq!(job.schedule.as_deref(), Some("0 4 * * *"));
}
