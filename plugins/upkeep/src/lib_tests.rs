use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, Plugin, Registrar, Registration,
    automation::{
        ExpiredSignInsDeleted, ForgetHomeRowsAsked, HomeRowsForgotten, Kernel, KernelRefused,
    },
    job::{JobRunner, RunJob},
    settings::{SettingValue, Settings},
};

use super::{DELETE_EXPIRED_SIGN_INS, FORGET_HOME_ROWS, HOME_ROWS_DAYS, Upkeep};

/// A Kernel function Upkeep called, with the request it sent.
type Called = (String, Vec<u8>);

/// Upkeep's Job runner, enabled with `settings` over a Kernel that records each function called
/// and answers it forgot 2 Users' Home rows and deleted 3 Devices and 1 Invite.
fn enabled(settings: Settings) -> (Arc<dyn JobRunner>, Arc<Mutex<Vec<Called>>>) {
    let called = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&called);
    let kernel = move |function: &str,
                       request: Vec<u8>|
          -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
        let answer = match function {
            "forget_home_rows" => HomeRowsForgotten {
                users: 2,
                ..HomeRowsForgotten::default()
            }
            .encode_to_vec(),
            "delete_expired_sign_ins" => ExpiredSignInsDeleted {
                devices: 3,
                invites: 1,
                ..ExpiredSignInsDeleted::default()
            }
            .encode_to_vec(),
            other => panic!("Upkeep called {other}"),
        };
        recorded
            .lock()
            .unwrap()
            .push((function.to_owned(), request));
        Box::pin(async move { Ok(answer) })
    };
    let settings = settings.resolve(&Upkeep.manifest().settings).unwrap();
    let kernel: Arc<dyn Kernel> = Arc::new(kernel);
    let mut registrar = Registrar::with_settings(settings).with_kernel(kernel);
    Upkeep.register(&mut registrar).unwrap();
    let runner = (registrar.into_registrations().iter())
        .find_map(Registration::provider::<dyn JobRunner>)
        .unwrap();
    (runner, called)
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
        let (runner, called) = enabled(settings);

        runner.run(&run(FORGET_HOME_ROWS)).await.unwrap();

        let called = called.lock().unwrap();
        let [(function, request)] = called.as_slice() else {
            panic!("{called:?}");
        };
        assert_eq!(function, "forget_home_rows");
        let asked_days = ForgetHomeRowsAsked::decode_from_slice(request)
            .unwrap()
            .days;
        assert_eq!(asked_days, days);
    }
}

#[tokio::test]
async fn deleting_expired_sign_ins_asks_the_kernel_once() {
    let (runner, called) = enabled(Settings::default());

    runner.run(&run(DELETE_EXPIRED_SIGN_INS)).await.unwrap();

    let functions = called
        .lock()
        .unwrap()
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<_>>();
    assert_eq!(functions, ["delete_expired_sign_ins"]);
}

#[tokio::test]
async fn a_job_upkeep_doesnt_declare_fails_without_asking_the_kernel() {
    let (runner, called) = enabled(Settings::default());

    let failed = runner.run(&run("vacuum")).await;

    assert_eq!(failed.unwrap_err().to_string(), "Upkeep has no Job vacuum");
    assert!(called.lock().unwrap().is_empty());
}

#[test]
fn it_forgets_home_rows_daily_at_four_and_deletes_expired_sign_ins_daily_at_ten_past() {
    let manifest = Upkeep.manifest();

    let schedules: Vec<_> = (manifest.jobs.iter())
        .map(|job| (job.id.as_str(), job.schedule.as_deref()))
        .collect();
    assert_eq!(
        schedules,
        [
            (FORGET_HOME_ROWS, Some("0 4 * * *")),
            (DELETE_EXPIRED_SIGN_INS, Some("10 4 * * *")),
        ]
    );
}
