use std::{
    future,
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
        StartLinkAsked,
    },
    settings::{SettingField, SettingKind, SettingsSchema},
    watch_sync::WatchSync,
};
use nama_plugin_local_auth::LocalAuth;
use tempfile::TempDir;

use super::{LinkError, LinkState};
use crate::{
    config::Config,
    db::Database,
    plugin_settings::PluginSettings,
    plugins::Registry,
    users::{personal_settings_of, replace_personal_settings, testing::insert_user},
};

const TRACKER: PluginId = PluginId::from_static("tracker");
const DEVICE_CODE: &str = "device-1";

/// A Watch sync declaring a TEXT `handle` and a SECRET `token` as Personal settings, which
/// sends Users to `url` and answers each poll for `DEVICE_CODE` with what the test put in
/// `polled`.
#[derive(Clone)]
struct Tracker {
    url: Arc<Mutex<String>>,
    polled: Arc<Mutex<LinkPolled>>,
}

impl Tracker {
    /// Has the next poll answer `polled`.
    fn answer(&self, polled: LinkPolled) {
        *self.polled.lock().unwrap() = polled;
    }
}

fn field(key: &str, kind: SettingKind) -> SettingField {
    SettingField {
        key: key.to_owned(),
        label: key.to_owned(),
        description: String::new(),
        kind,
        required: false,
        default: None,
        min: None,
        max: None,
        choices: Vec::new(),
    }
}

impl Plugin for Tracker {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn WatchSync>()],
            personal_settings: SettingsSchema {
                fields: vec![
                    field("handle", SettingKind::Text),
                    field("token", SettingKind::Secret),
                ],
            },
            ..Manifest::new(TRACKER)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn WatchSync>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl WatchSync for Tracker {
    fn scrobble<'a>(
        &'a self,
        _asked: &'a ScrobbleAsked,
    ) -> BoxFuture<'a, Result<Scrobbled, PluginError>> {
        Box::pin(future::ready(Ok(Scrobbled::default())))
    }

    fn import<'a>(
        &'a self,
        _asked: &'a ImportAsked,
    ) -> BoxFuture<'a, Result<Imported, PluginError>> {
        Box::pin(future::ready(Ok(Imported::default())))
    }

    fn start_link<'a>(
        &'a self,
        _asked: &'a StartLinkAsked,
    ) -> BoxFuture<'a, Result<LinkStarted, PluginError>> {
        Box::pin(future::ready(Ok(LinkStarted {
            user_code: "ABCD-1234".to_owned(),
            verification_url: self.url.lock().unwrap().clone(),
            device_code: DEVICE_CODE.to_owned(),
            ..LinkStarted::default()
        })))
    }

    fn poll_link<'a>(
        &'a self,
        asked: &'a PollLinkAsked,
    ) -> BoxFuture<'a, Result<LinkPolled, PluginError>> {
        let answer = if asked.device_code == DEVICE_CODE {
            Ok(self.polled.lock().unwrap().clone())
        } else {
            Err("no such device code".into())
        };
        Box::pin(future::ready(answer))
    }
}

/// Plugin settings with `Tracker` enabled, the tracker, and a User's id.
async fn started() -> (TempDir, PluginSettings, Tracker, i64) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let tracker = Tracker {
        url: Arc::new(Mutex::new("https://tracker.example/activate".to_owned())),
        polled: Arc::default(),
    };
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(LocalAuth));
    let described = tracker.clone();
    registry.describe_builtin(move || Arc::new(described.clone()));
    let config = Config {
        data_dir: data_dir.path().to_owned(),
        ..Config::default()
    };
    let plugins = PluginSettings::start(Arc::clone(&db), registry, &config)
        .await
        .unwrap();
    let user = insert_user(&db, "ada").await;
    (data_dir, plugins, tracker, user)
}

#[tokio::test]
async fn a_link_is_pending_until_approved_then_keeps_the_tokens_as_secret_personal_settings() {
    let (_dir, plugins, tracker, ada) = started().await;
    let handle = ("handle".to_owned(), "ada".to_owned(), false);
    replace_personal_settings(&plugins.db, ada, &TRACKER, vec![handle])
        .await
        .unwrap();

    let started = plugins.start_watch_sync_link(&TRACKER).await.unwrap();
    assert_eq!(started.user_code, "ABCD-1234");
    let id = TRACKER;
    let poll = || plugins.poll_watch_sync_link(ada, &id, started.device_code.clone());
    assert_eq!(poll().await.unwrap(), LinkState::Pending);
    tracker.answer(LinkPolled {
        personal_settings: [("token", "s3cret"), ("handle", "mallory"), ("stray", "x")]
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .into_iter()
            .collect(),
        ..LinkPolled::default()
    });
    assert_eq!(poll().await.unwrap(), LinkState::Linked);

    // Only the SECRET token is taken: the User's own handle stays theirs.
    let stored = personal_settings_of(&plugins.db, ada, &TRACKER)
        .await
        .unwrap();
    assert_eq!(
        stored.into_iter().collect::<Vec<_>>(),
        [
            ("handle".to_owned(), ("ada".to_owned(), false)),
            ("token".to_owned(), ("s3cret".to_owned(), true)),
        ]
    );
}

#[tokio::test]
async fn an_expired_code_stores_nothing() {
    let (_dir, plugins, tracker, ada) = started().await;
    tracker.answer(LinkPolled {
        expired: true,
        ..LinkPolled::default()
    });

    let polled = (plugins.poll_watch_sync_link(ada, &TRACKER, DEVICE_CODE.to_owned())).await;

    assert_eq!(polled.unwrap(), LinkState::Expired);
    let stored = personal_settings_of(&plugins.db, ada, &TRACKER)
        .await
        .unwrap();
    assert!(stored.is_empty());
}

#[tokio::test]
async fn tokens_under_no_secret_personal_setting_fail_the_poll_rather_than_wait() {
    let (_dir, plugins, tracker, ada) = started().await;
    tracker.answer(LinkPolled {
        personal_settings: std::iter::once(("tokn".to_owned(), "s3cret".to_owned())).collect(),
        ..LinkPolled::default()
    });

    let polled = (plugins.poll_watch_sync_link(ada, &TRACKER, DEVICE_CODE.to_owned())).await;

    assert!(matches!(polled, Err(LinkError::Failed(_))), "{polled:?}");
    let stored = personal_settings_of(&plugins.db, ada, &TRACKER)
        .await
        .unwrap();
    assert!(stored.is_empty());
}

#[tokio::test]
async fn a_failed_poll_says_the_plugins_reason() {
    let (_dir, plugins, _tracker, ada) = started().await;

    let polled = (plugins.poll_watch_sync_link(ada, &TRACKER, "forged".to_owned())).await;

    assert!(matches!(polled, Err(LinkError::Failed(reason)) if reason.contains("no such device")));
}

#[tokio::test]
async fn a_verification_url_a_client_cant_safely_open_fails_the_start() {
    let (_dir, plugins, tracker, _ada) = started().await;
    *tracker.url.lock().unwrap() = "javascript:alert(1)".to_owned();

    let started = plugins.start_watch_sync_link(&TRACKER).await;

    assert!(matches!(started, Err(LinkError::Failed(_))), "{started:?}");
}

#[tokio::test]
async fn linking_at_a_plugin_that_is_no_watch_sync_is_refused() {
    let (_dir, plugins, _tracker, _ada) = started().await;

    let started = plugins
        .start_watch_sync_link(&PluginId::from_static("local-auth"))
        .await;

    assert!(matches!(started, Err(LinkError::NoWatchSync)));
}
