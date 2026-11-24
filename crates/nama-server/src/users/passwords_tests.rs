use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use futures_util::future::join_all;
use nama_plugin_api::{Manifest, Plugin, PointKey, Priority, Registrar};

use super::*;
use crate::{db::Database, plugins::Registry};

const PLUGIN: PluginId = PluginId::from_static("strict");

/// An Authenticator that rejects a password shorter than 8 characters and fails to hash any other.
struct Strict;

impl Authenticator for Strict {
    fn hash_password(&self, password: &str) -> Result<String, PasswordError> {
        if password.len() < 8 {
            return Err(PasswordError::Rejected("too short".to_owned()));
        }
        Err(PasswordError::Failed("out of memory".into()))
    }

    fn verify_password(&self, _password: &str, _credential: &str) -> Result<bool, PluginError> {
        Ok(false)
    }

    fn verify_unknown_user(&self, _password: &str) {}
}

impl Plugin for Strict {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Authenticator>()],
            ..Manifest::new(PLUGIN)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Authenticator>(Priority::DEFAULT, Arc::new(Self));
        Ok(())
    }
}

#[tokio::test]
async fn an_authenticators_rejected_password_records_no_failure_while_a_failed_hash_records_one() {
    let data_dir = tempfile::tempdir().unwrap();
    let plugins = Registry::new();
    plugins.enable(Arc::new(Strict)).unwrap();
    let points = plugins.extension_points();
    let users = Users::new(
        Arc::new(Database::open(data_dir.path()).await.unwrap()),
        points.clone(),
    );
    let authenticator = PointKey::provide::<dyn Authenticator>();

    let failed = users.hash_password("password", "long enough").await;
    let failing = points.failing(&PLUGIN, authenticator);
    let rejected = users.hash_password("password", "short").await;
    let answered = points.failing(&PLUGIN, authenticator);

    assert!(matches!(failed, Err(Error::Authenticator(_))));
    assert_eq!(failing.map(|failing| failing.in_a_row), Some(1));
    assert!(matches!(rejected, Err(Error::PasswordRejected { .. })));
    assert_eq!(answered, None);
}

/// An Authenticator that takes a while to hash, recording how many of its hashes ran at once.
#[derive(Clone, Default)]
struct Counting(Arc<Hashes>);

#[derive(Default)]
struct Hashes {
    running: AtomicUsize,
    most_at_once: AtomicUsize,
}

impl Authenticator for Counting {
    fn hash_password(&self, _password: &str) -> Result<String, PasswordError> {
        let running = self.0.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.0.most_at_once.fetch_max(running, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(20));
        self.0.running.fetch_sub(1, Ordering::SeqCst);
        Ok("hashed".to_owned())
    }

    fn verify_password(&self, _password: &str, _credential: &str) -> Result<bool, PluginError> {
        Ok(false)
    }

    fn verify_unknown_user(&self, _password: &str) {}
}

impl Plugin for Counting {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Authenticator>()],
            ..Manifest::new(PLUGIN)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Authenticator>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

#[tokio::test]
async fn twenty_hashes_at_once_run_no_more_than_the_bound_together_and_all_finish() {
    let data_dir = tempfile::tempdir().unwrap();
    let plugins = Registry::new();
    let counting = Counting::default();
    plugins.enable(Arc::new(counting.clone())).unwrap();
    let users = Users::new(
        Arc::new(Database::open(data_dir.path()).await.unwrap()),
        plugins.extension_points(),
    );

    let hashed = join_all((0..20).map(|_| users.hash_password("password", "long enough"))).await;

    assert!(hashed.iter().all(Result::is_ok));
    assert!(counting.0.most_at_once.load(Ordering::SeqCst) <= CONCURRENT_HASHES);
}
