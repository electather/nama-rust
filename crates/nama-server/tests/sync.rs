//! A User's Watch State and Track Preferences on their other Servers coming back through the
//! enabled Plugin that links the Server (ADR 0008, ADR 0092), and their Watchlist syncing with them
//! (ADR 0056), here a stand-in for the Hub that two Servers share.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use browsing::{ALIEN_1080P, Fixture, tmdb_id};
use connectrpc::client::{ClientConfig, HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    points::{
        TrackPreference, TrackPreferenceChange, TrackPreferenceChanged, WatchStateChange,
        WatchStateChanges, WatchStatesChanged,
    },
    server_link::{AccountSignIn, Link, ServerAddresses, ServerLinker, WatchlistEntry},
};
use nama_proto::nama::v1::{
    AddToWatchlistRequest, BrowseServiceClient, GetMovieRequest, GetServerLinkRequest,
    GetTrackPreferencesRequest, GetTrackPreferencesResponse, GetUserLinkRequest, LibraryKind,
    ListContinueWatchingRequest, PlaybackServiceClient, RemoveFromWatchlistRequest,
    ReportProgressRequest, ServerServiceClient, SetAudioPreferenceRequest,
    SetSubtitlePreferenceRequest, SubtitleKind, TitleKind, UserServiceClient, UsersAudioPreference,
    UsersSubtitlePreference, WatchStateServiceClient, WatchlistServiceClient,
    continue_watching_entry::MovieOrEpisode,
};
use nama_server::plugins::Registry;
use server::{TestServer, WAIT, tmdb::ALIEN};

/// A stand-in for the Hub, shared by every Server it's enabled on: it keeps each Watch State and
/// Track Preference change a Server tells of, and hands them all back to every User's pull, and
/// merges every Watchlist synced, the change made last winning, all as one Account's.
#[derive(Clone, Default)]
struct Hub {
    watch_states: Arc<Mutex<Vec<WatchStateChange>>>,
    preferences: Arc<Mutex<Vec<TrackPreference>>>,
    watchlist: Arc<Mutex<HashMap<(bool, String), WatchlistEntry>>>,
}

impl Hub {
    fn kept(&self) -> MutexGuard<'_, Vec<WatchStateChange>> {
        self.watch_states
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn kept_preferences(&self) -> MutexGuard<'_, Vec<TrackPreference>> {
        self.preferences
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn watchlist(&self) -> MutexGuard<'_, HashMap<(bool, String), WatchlistEntry>> {
        self.watchlist
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// A Server the Hub is enabled on, as `builder` starts it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn on(&self, builder: server::Builder) -> server::Builder {
        let plugins = Registry::new();
        plugins.enable(Arc::new(self.clone())).unwrap();
        builder.plugins(plugins)
    }
}

impl Plugin for Hub {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![
                PointKey::provide::<dyn ServerLinker>(),
                PointKey::observe::<WatchStatesChanged>(),
                PointKey::observe::<TrackPreferenceChanged>(),
            ],
            ..Manifest::new(PluginId::from_static("hub"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(self.clone()));
        registrar.observe::<WatchStatesChanged>(self.clone());
        registrar.observe::<TrackPreferenceChanged>(self.clone());
        Ok(())
    }
}

impl Observer<WatchStatesChanged> for Hub {
    fn observe<'a>(
        &'a self,
        event: &'a WatchStateChanges,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        self.kept().extend(event.changes.iter().cloned());
        Box::pin(future::ready(Ok(())))
    }
}

impl Observer<TrackPreferenceChanged> for Hub {
    fn observe<'a>(
        &'a self,
        event: &'a TrackPreferenceChange,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        self.kept_preferences().push(event.preference.clone());
        Box::pin(future::ready(Ok(())))
    }
}

/// Only what pulling and syncing ask; the rest is `tests/server_link.rs`'s.
impl ServerLinker for Hub {
    fn link(&self) -> BoxFuture<'_, Result<Link, PluginError>> {
        Box::pin(future::ready(Ok(linked())))
    }

    fn start_link<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(future::ready(Ok(linked())))
    }

    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn user_link(&self, _: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>> {
        Box::pin(future::ready(Ok(Some(linked()))))
    }

    fn start_user_link<'a>(
        &'a self,
        _: i64,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(future::ready(Ok(linked())))
    }

    fn unlink_user(&self, _: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn invite_user<'a>(
        &'a self,
        _: i64,
        _: &'a str,
        _: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn start_account_sign_in<'a>(
        &'a self,
        _: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>> {
        Box::pin(future::ready(Err("no sign-in with an Account here".into())))
    }

    fn finish_account_sign_in<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>> {
        Box::pin(future::ready(Ok(None)))
    }

    fn set_addresses(&self, _: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn watch_states(&self, _: i64) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>> {
        Box::pin(future::ready(Ok(self.kept().clone())))
    }

    fn track_preferences(
        &self,
        _: i64,
    ) -> BoxFuture<'_, Result<Vec<TrackPreference>, PluginError>> {
        Box::pin(future::ready(Ok(self.kept_preferences().clone())))
    }

    fn sync_watchlist(
        &self,
        _: i64,
        watchlist: Vec<WatchlistEntry>,
    ) -> BoxFuture<'_, Result<Vec<WatchlistEntry>, PluginError>> {
        let mut kept = self.watchlist();
        for entry in watchlist {
            let title = (entry.show, entry.id.clone());
            if (kept.get(&title)).is_none_or(|old| old.changed_at_ms < entry.changed_at_ms) {
                kept.insert(title, entry);
            }
        }
        Box::pin(future::ready(Ok(kept.values().cloned().collect())))
    }
}

fn linked() -> Link {
    Link::Linked {
        account: "ada@example.com".to_owned(),
        reachability: None,
    }
}

/// Watches Alien a minute in on `fx`'s Server as its Admin, and waits until the Hub has it.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn watch_alien(fx: &Fixture, hub: &Hub) {
    let watch_states =
        WatchStateServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let report = ReportProgressRequest {
        version_id: fx.version_ids(ALIEN).await[0],
        position_ms: 60_000,
        duration_ms: 7_200_000,
        ..Default::default()
    };
    watch_states.report_progress(report).await.unwrap();
    until("the Hub has the progress", async || !hub.kept().is_empty()).await;
}

/// The position of Alien in the Continue Watching of the User signed in as `who`, if it's there.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_in_continue_watching(who: &ClientConfig) -> Option<u64> {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), who.clone());
    let listed = browse
        .list_continue_watching(ListContinueWatchingRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .entries
        .into_iter()
        .find_map(|entry| match entry.movie_or_episode {
            Some(MovieOrEpisode::Movie(movie)) => Some(movie.watch_state.position_ms),
            _ => None,
        })
}

/// Waits until `done`, as a pull happens in the background.
async fn until(what: &str, mut done: impl AsyncFnMut() -> bool) {
    let waited = tokio::time::timeout(WAIT, async {
        while !done().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "timed out waiting until {what}");
}

#[tokio::test]
async fn progress_watched_on_one_server_shows_once_signed_in_on_another() {
    let hub = Hub::default();
    let first = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let second = Fixture::alien_on(hub.on(TestServer::builder())).await;

    watch_alien(&first, &hub).await;
    let ada = second.server.user_with_default_permissions("Ada").await;

    until("Continue Watching shows the progress", async || {
        alien_in_continue_watching(&ada.client_config()).await == Some(60_000)
    })
    .await;
}

#[tokio::test]
async fn progress_on_a_title_the_server_lacks_shows_once_a_scan_adds_it() {
    let hub = Hub::default();
    let first = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let second = Fixture::start_on(
        LibraryKind::LIBRARY_KIND_MOVIES,
        hub.on(TestServer::builder()),
    )
    .await;
    // Signed in before anything is watched, so only the scan's pull can bring the progress.
    let ada = second.server.user_with_default_permissions("Ada").await;
    watch_alien(&first, &hub).await;
    assert_eq!(alien_in_continue_watching(&ada.client_config()).await, None);

    second.write(ALIEN_1080P, b"video");
    second.scan_and_match().await;

    until("Continue Watching shows the progress", async || {
        alien_in_continue_watching(&ada.client_config()).await == Some(60_000)
    })
    .await;
}

/// Watches Alien on `fx`'s Server, then has the Hub forget it, as if it was watched before the
/// Link, and answers the change the Server told of, as of the second the Server stores.
async fn alien_watched_before_linking(fx: &Fixture, hub: &Hub) -> WatchStateChange {
    watch_alien(fx, hub).await;
    let made = hub.kept().remove(0);
    WatchStateChange {
        changed_at_ms: made.changed_at_ms.div_euclid(1000) * 1000,
        ..made
    }
}

/// The own Track Preferences of whoever `who` calls as, as the Server gives them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn own_preferences(who: &ClientConfig) -> GetTrackPreferencesResponse {
    PlaybackServiceClient::new(HttpClient::plaintext(), who.clone())
        .get_track_preferences(GetTrackPreferencesRequest::default())
        .await
        .unwrap()
        .into_owned()
}

#[tokio::test]
async fn subtitle_and_audio_preferences_set_on_one_server_are_the_users_once_signed_in_on_another()
{
    let hub = Hub::default();
    let first = hub.on(TestServer::builder()).start().await;
    let second = hub.on(TestServer::builder()).start().await;
    let english_sdh = UsersSubtitlePreference {
        language: Some("eng".to_owned()),
        kind: SubtitleKind::SUBTITLE_KIND_SDH.into(),
        ..Default::default()
    };
    let japanese = UsersAudioPreference {
        language: Some("jpn".to_owned()),
        ..Default::default()
    };

    let playback = PlaybackServiceClient::new(HttpClient::plaintext(), first.client_config());
    let subtitle = SetSubtitlePreferenceRequest {
        subtitle: english_sdh.clone().into(),
        ..Default::default()
    };
    playback.set_subtitle_preference(subtitle).await.unwrap();
    let audio = SetAudioPreferenceRequest {
        audio: japanese.clone().into(),
        ..Default::default()
    };
    playback.set_audio_preference(audio).await.unwrap();
    until("the Hub has both", async || {
        hub.kept_preferences().len() == 2
    })
    .await;
    let ada = second.user_with_default_permissions("Ada").await;

    until("the second Server has them", async || {
        let preferences = own_preferences(&ada.client_config()).await;
        (
            preferences.subtitle.into_option(),
            preferences.audio.into_option(),
        ) == (Some(english_sdh.clone()), Some(japanese.clone()))
    })
    .await;
}

/// Asks for the Link of `fx`'s Admin, as Settings does once they Link.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn look_at_admins_link(fx: &Fixture) {
    let users = UserServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    users
        .get_user_link(GetUserLinkRequest::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn linking_a_user_pushes_their_watch_state_from_before_dated_as_stored() {
    let hub = Hub::default();
    let fx = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let stored = alien_watched_before_linking(&fx, &hub).await;

    look_at_admins_link(&fx).await;

    until("the Hub has the progress", async || !hub.kept().is_empty()).await;
    assert_eq!(*hub.kept(), [stored]);
}

#[tokio::test]
async fn linking_a_user_leaves_a_title_the_account_has_newer_as_the_account_has_it() {
    let hub = Hub::default();
    let fx = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let stored = alien_watched_before_linking(&fx, &hub).await;
    let watched_on_another_server = WatchStateChange {
        position_ms: 90_000,
        changed_at_ms: stored.changed_at_ms + 60_000,
        ..stored.clone()
    };
    hub.kept().push(watched_on_another_server.clone());

    look_at_admins_link(&fx).await;

    until("the Hub has the push", async || hub.kept().len() == 2).await;
    let newest = (hub.kept().iter())
        .max_by_key(|change| change.changed_at_ms)
        .cloned();
    assert_eq!(newest, Some(watched_on_another_server));
    until(
        "Continue Watching shows the Account's progress",
        async || alien_in_continue_watching(&fx.server.client_config()).await == Some(90_000),
    )
    .await;
}

#[tokio::test]
async fn linking_the_server_pushes_every_users_watch_state_dated_as_stored() {
    let hub = Hub::default();
    let fx = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let stored = alien_watched_before_linking(&fx, &hub).await;

    let server = ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    server
        .get_server_link(GetServerLinkRequest::default())
        .await
        .unwrap();

    until("the Hub has the progress", async || !hub.kept().is_empty()).await;
    assert_eq!(*hub.kept(), [stored]);
}

/// Whether Alien is on the Watchlist of the User signed in as `who` on `fx`'s Server.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_on_watchlist(fx: &Fixture, who: &ClientConfig) -> bool {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), who.clone());
    let asked = GetMovieRequest {
        library_id: fx.library,
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    };
    let got = browse.get_movie(asked).await.unwrap().into_owned();
    got.movie.into_option().unwrap().on_watchlist
}

#[tokio::test]
async fn a_title_added_on_one_server_and_removed_on_another_ends_off_both() {
    let hub = Hub::default();
    let first = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let second = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let admin = first.server.client_config();
    let alien = (TitleKind::TITLE_KIND_MOVIE.into(), tmdb_id(ALIEN));
    let on_first = WatchlistServiceClient::new(HttpClient::plaintext(), admin.clone());
    let added = AddToWatchlistRequest {
        kind: alien.0,
        id: alien.1.clone().into(),
        ..Default::default()
    };
    on_first.add_to_watchlist(added).await.unwrap();
    until("the Hub has the title", async || {
        !hub.watchlist().is_empty()
    })
    .await;

    let ada = second.server.user_with_default_permissions("Ada").await;
    until("Ada's Watchlist on the other Server has it", async || {
        alien_on_watchlist(&second, &ada.client_config()).await
    })
    .await;
    let on_second = WatchlistServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let removed = RemoveFromWatchlistRequest {
        kind: alien.0,
        id: alien.1.into(),
        ..Default::default()
    };
    on_second.remove_from_watchlist(removed).await.unwrap();
    until("the Hub has the removal", async || {
        hub.watchlist().values().all(|entry| entry.removed)
    })
    .await;
    first.scan_and_match().await;

    until("the first Server took the removal", async || {
        !alien_on_watchlist(&first, &admin).await
    })
    .await;
}
