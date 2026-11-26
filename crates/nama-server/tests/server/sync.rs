//! A Linked User's Watch State, Subtitle and Audio Preferences and Watchlist syncing with the Hub
//! the Server's config names, which the Kernel does (ADR 0008, ADR 0056, ADR 0092, ADR 0105), here
//! a fake whose one Account two Servers share, which Links every User but those its test names.
//! Their plays go to it as Viewing Events (ADR 0093).

use crate::support::{browsing, fake_hub, server};

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use browsing::{ALIEN_1080P, Fixture, tmdb_id};
use connectrpc::client::{ClientConfig, HttpClient};
use fake_hub::FakeHub;
use nama_hub_proto::nama::hub::v1::{
    self as hub, TrackPreferenceChange, WatchStateChange, WatchlistEntry,
};
use nama_proto::nama::v1::{
    AddToWatchlistRequest, AudioCodec, BrowseServiceClient, Container, DeviceDecoding,
    GetMovieRequest, GetTrackPreferencesRequest, GetTrackPreferencesResponse, LibraryKind,
    ListContinueWatchingRequest, PlanPlaybackRequest, PlaybackServiceClient,
    RemoveFromWatchlistRequest, ReportProgressRequest, SetAudioPreferenceRequest,
    SetSubtitlePreferenceRequest, StartUserLinkRequest, StopPlaybackRequest, SubtitleKind,
    TitleKind, UserServiceClient, UsersAudioPreference, UsersSubtitlePreference, VideoCodec,
    WatchStateServiceClient, WatchlistServiceClient, continue_watching_entry::MovieOrEpisode,
};
use server::{TestServer, WAIT, tmdb::ALIEN};

/// A Server whose config names `hub`, as `builder` starts it.
fn on(hub: &FakeHub, builder: server::Builder) -> server::Builder {
    builder.config(hub.config())
}

/// A Server with Alien, Linked with `hub`, all its Users Linked.
async fn alien_linked_with(hub: &FakeHub) -> Fixture {
    let fx = Fixture::alien_on(on(hub, TestServer::builder())).await;
    hub.link_with_users(&fx.server).await;
    fx
}

/// Has `server` send what waits for the Hub, rather than wait for its minute's send: signing its
/// Admin in again pulls, and a pull sends first.
async fn send_waiting(server: &TestServer) {
    server.sign_in(1, "Admin").await;
}

/// Reports Alien `position_ms` in on `fx`'s Server as whoever `who` calls as.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn watch_alien(fx: &Fixture, who: &ClientConfig, position_ms: u64) {
    let watch_states = WatchStateServiceClient::new(HttpClient::plaintext(), who.clone());
    let report = ReportProgressRequest {
        version_id: fx.version_ids(ALIEN).await[0],
        position_ms,
        duration_ms: 7_200_000,
        ..Default::default()
    };
    watch_states.report_progress(report).await.unwrap();
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

/// The positions of every Watch State the Hub keeps.
fn positions(hub: &FakeHub) -> Vec<u64> {
    (hub.account().watch_states.iter())
        .map(|change| change.position_ms)
        .collect()
}

/// Waits until `done`, as a sync happens in the background.
async fn until(what: &str, mut done: impl AsyncFnMut() -> bool) {
    let waited = tokio::time::timeout(WAIT, async {
        while !done().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "timed out waiting until {what}");
}

/// A minute from now, in milliseconds since the Unix epoch: later than anything the test does.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn in_a_minute_ms() -> i64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(now.as_millis()).unwrap() + 60_000
}

#[tokio::test]
async fn progress_watched_on_one_server_shows_once_signed_in_on_another() {
    let hub = FakeHub::start().await;
    let other_hub = hub.for_another_server().await;
    let first = alien_linked_with(&hub).await;
    let second = alien_linked_with(&other_hub).await;

    watch_alien(&first, &first.server.client_config(), 60_000).await;
    send_waiting(&first.server).await;
    until("the Hub has the progress", async || {
        positions(&hub) == [60_000]
    })
    .await;
    let ada = second.server.user_with_default_permissions("Ada").await;

    until("Continue Watching shows the progress", async || {
        alien_in_continue_watching(&ada.client_config()).await == Some(60_000)
    })
    .await;
}

#[tokio::test]
async fn progress_made_while_the_hub_is_down_reaches_it_once_it_is_back() {
    let hub = FakeHub::start().await;
    let fx = alien_linked_with(&hub).await;
    // The Admin's Link is known before the Hub goes down: while it's unknown, nothing is pushed.
    start_admins_link(&fx.server).await;
    hub.set_down(true);

    watch_alien(&fx, &fx.server.client_config(), 60_000).await;
    send_waiting(&fx.server).await;
    until("the Server tried to push", async || {
        hub.refused("PushWatchStatesRequest") > 0
    })
    .await;
    hub.set_down(false);
    send_waiting(&fx.server).await;

    until("the Hub has the progress", async || {
        positions(&hub) == [60_000]
    })
    .await;
}

#[tokio::test]
async fn an_unlinked_users_progress_never_reaches_the_hub() {
    let hub = FakeHub::start().await;
    let fx = Fixture::alien_on(on(&hub, TestServer::builder())).await;
    hub.link_with_users(&fx.server).await;
    // Unlinked before she signs in, as her sign-in's pull asks the Hub whether she is Linked and
    // the Server keeps that answer for a minute.
    let id = fx.server.create_user("Ada").await;
    hub.unlink_user_on_hub(id);
    let ada = fx.server.sign_in(id, "Ada").await;

    // In the order they're queued, so a page holds Ada's before the Admin's.
    watch_alien(&fx, &ada.client_config(), 60_000).await;
    watch_alien(&fx, &fx.server.client_config(), 30_000).await;
    send_waiting(&fx.server).await;

    until("the Hub has the Admin's progress", async || {
        !positions(&hub).is_empty()
    })
    .await;
    assert_eq!(positions(&hub), [30_000]);
}

#[tokio::test]
async fn progress_on_a_title_the_server_lacks_shows_once_a_scan_adds_it() {
    let hub = FakeHub::start().await;
    let first = alien_linked_with(&hub).await;
    let other_hub = hub.for_another_server().await;
    let second = Fixture::start_on(
        LibraryKind::LIBRARY_KIND_MOVIES,
        on(&other_hub, TestServer::builder()),
    )
    .await;
    other_hub.link_with_users(&second.server).await;
    // Signed in before anything is watched, so only the scan's pull can bring the progress.
    let ada = second.server.user_with_default_permissions("Ada").await;
    watch_alien(&first, &first.server.client_config(), 60_000).await;
    send_waiting(&first.server).await;
    until("the Hub has the progress", async || {
        positions(&hub) == [60_000]
    })
    .await;
    assert_eq!(alien_in_continue_watching(&ada.client_config()).await, None);

    second.write(ALIEN_1080P, b"video");
    second.scan_and_match().await;

    until("Continue Watching shows the progress", async || {
        alien_in_continue_watching(&ada.client_config()).await == Some(60_000)
    })
    .await;
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

fn english_sdh() -> UsersSubtitlePreference {
    UsersSubtitlePreference {
        language: Some("eng".to_owned()),
        kind: SubtitleKind::SUBTITLE_KIND_SDH.into(),
        ..Default::default()
    }
}

fn audio_in(language: &str) -> UsersAudioPreference {
    UsersAudioPreference {
        language: Some(language.to_owned()),
        ..Default::default()
    }
}

/// Sets English SDH subtitles and Japanese audio as the own Preferences of whoever `who` calls as.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn prefer_english_sdh_and_japanese(who: &ClientConfig) {
    let playback = PlaybackServiceClient::new(HttpClient::plaintext(), who.clone());
    let subtitle = SetSubtitlePreferenceRequest {
        subtitle: english_sdh().into(),
        ..Default::default()
    };
    playback.set_subtitle_preference(subtitle).await.unwrap();
    let audio = SetAudioPreferenceRequest {
        audio: audio_in("jpn").into(),
        ..Default::default()
    };
    playback.set_audio_preference(audio).await.unwrap();
}

/// The language of each Preference the Hub keeps, the Subtitle Preference first.
fn languages(hub: &FakeHub) -> Vec<(bool, String)> {
    let mut kept: Vec<_> = (hub.account().preferences.iter())
        .map(|change| (change.audio, change.language.clone()))
        .collect();
    kept.sort();
    kept
}

#[tokio::test]
async fn subtitle_and_audio_preferences_set_on_one_server_are_the_users_once_signed_in_on_another()
{
    let hub = FakeHub::start().await;
    let first = on(&hub, TestServer::builder()).start().await;
    hub.link_with_users(&first).await;
    let other_hub = hub.for_another_server().await;
    let second = on(&other_hub, TestServer::builder()).start().await;
    other_hub.link_with_users(&second).await;

    prefer_english_sdh_and_japanese(&first.client_config()).await;
    send_waiting(&first).await;
    until("the Hub has both", async || languages(&hub).len() == 2).await;
    let ada = second.user_with_default_permissions("Ada").await;

    until("the second Server has them", async || {
        let preferences = own_preferences(&ada.client_config()).await;
        (
            preferences.subtitle.into_option(),
            preferences.audio.into_option(),
        ) == (Some(english_sdh()), Some(audio_in("jpn")))
    })
    .await;
}

#[tokio::test]
async fn linking_the_server_pushes_its_users_watch_state_and_preferences_from_before_dated_as_stored()
 {
    let hub = FakeHub::start().await;
    let fx = Fixture::alien_on(on(&hub, TestServer::builder())).await;
    let admin = fx.server.client_config();
    // Before the Server is Linked, so nothing waits for the Hub but what Linking pushes.
    watch_alien(&fx, &admin, 60_000).await;
    prefer_english_sdh_and_japanese(&admin).await;

    hub.link_with_users(&fx.server).await;
    send_waiting(&fx.server).await;

    until("the Hub has them", async || {
        positions(&hub) == [60_000] && languages(&hub).len() == 2
    })
    .await;
    let changed_at_ms = hub.account().watch_states[0].changed_at_ms;
    assert_eq!(changed_at_ms % 1000, 0, "dated to the second, as stored");
    assert_eq!(
        languages(&hub),
        [(false, "eng".to_owned()), (true, "jpn".to_owned())]
    );
}

/// Alien as another Server left it for the Account: `position_ms` in, `changed_at_ms`.
fn alien_watched_elsewhere(position_ms: u64, changed_at_ms: i64) -> WatchStateChange {
    WatchStateChange {
        ids: vec![format!("tmdb-{ALIEN}")],
        position_ms,
        duration_ms: 7_200_000,
        changed_at_ms,
        ..Default::default()
    }
}

#[tokio::test]
async fn linking_leaves_a_title_the_account_has_newer_as_the_account_has_it() {
    let hub = FakeHub::start().await;
    let fx = Fixture::alien_on(on(&hub, TestServer::builder())).await;
    let admin = fx.server.client_config();
    watch_alien(&fx, &admin, 60_000).await;
    (hub.account()).keep_watch_state(alien_watched_elsewhere(90_000, in_a_minute_ms()));

    hub.link_with_users(&fx.server).await;
    send_waiting(&fx.server).await;

    until(
        "Continue Watching shows the Account's progress",
        async || alien_in_continue_watching(&admin).await == Some(90_000),
    )
    .await;
}

#[tokio::test]
async fn linking_leaves_a_preference_the_account_has_newer_as_the_account_has_it() {
    let hub = FakeHub::start().await;
    let server = on(&hub, TestServer::builder()).start().await;
    prefer_english_sdh_and_japanese(&server.client_config()).await;
    hub.account().keep_preference(TrackPreferenceChange {
        audio: true,
        language: "fre".to_owned(),
        changed_at_ms: in_a_minute_ms(),
        ..Default::default()
    });

    hub.link_with_users(&server).await;
    send_waiting(&server).await;

    until("the Server has the Account's audio", async || {
        let preferences = own_preferences(&server.client_config()).await;
        preferences.audio.into_option() == Some(audio_in("fre"))
    })
    .await;
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

/// Starts Linking `server`'s Admin, as Settings does, which is Linked already, so the Server
/// knows it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_admins_link(server: &TestServer) {
    let users = UserServiceClient::new(HttpClient::plaintext(), server.client_config());
    users
        .start_user_link(StartUserLinkRequest::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn starting_a_link_already_made_pushes_then_pulls_and_syncs_the_watchlist() {
    let hub = FakeHub::start().await;
    let fx = Fixture::alien_on(on(&hub, TestServer::builder())).await;
    let admin = fx.server.client_config();
    watch_alien(&fx, &admin, 60_000).await;
    (hub.account()).keep_watch_state(alien_watched_elsewhere(90_000, in_a_minute_ms()));
    hub.account().keep_entry(WatchlistEntry {
        kind: hub::TitleKind::TITLE_KIND_MOVIE.into(),
        id: format!("tmdb-{ALIEN}"),
        changed_at_ms: in_a_minute_ms(),
        ..Default::default()
    });
    hub.link_with_users(&fx.server).await;

    start_admins_link(&fx.server).await;

    until(
        "Continue Watching shows the Account's progress",
        async || alien_in_continue_watching(&admin).await == Some(90_000),
    )
    .await;
    until("the Watchlist has the Account's title", async || {
        alien_on_watchlist(&fx, &admin).await
    })
    .await;
    // Later sends may push again after a pull, such as the Server Link's push of the same title.
    let asked = hub.account().asked.clone();
    let first = |step: &str| asked.iter().position(|asked| *asked == step);
    assert!(
        first("push").is_some() && first("push") < first("pull"),
        "pulled before pushing: {asked:?}"
    );
}

#[tokio::test]
async fn a_title_added_on_one_server_and_removed_on_another_ends_off_both() {
    let hub = FakeHub::start().await;
    let other_hub = hub.for_another_server().await;
    let first = alien_linked_with(&hub).await;
    let second = alien_linked_with(&other_hub).await;
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
        !hub.account().watchlist.is_empty()
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
        hub.account().watchlist.iter().all(|entry| entry.removed)
    })
    .await;
    first.scan_and_match().await;

    until("the first Server took the removal", async || {
        !alien_on_watchlist(&first, &admin).await
    })
    .await;
}

/// Plays Alien to `position_ms` as whoever `who` calls as, from planning it to stopping it.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn play_alien(fx: &Fixture, who: &ClientConfig, position_ms: u64) {
    let playback = PlaybackServiceClient::new(HttpClient::plaintext(), who.clone());
    let planned = playback
        .plan_playback(PlanPlaybackRequest {
            version_id: fx.version_ids(ALIEN).await[0],
            device_decoding: DeviceDecoding {
                containers: vec![Container::CONTAINER_MP4.into()],
                video_codecs: vec![VideoCodec::VIDEO_CODEC_H264.into()],
                audio_codecs: vec![AudioCodec::AUDIO_CODEC_AAC.into()],
                mse: true,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    watch_alien(fx, who, position_ms).await;
    let stopped = StopPlaybackRequest {
        plan_id: planned.plan.into_option().unwrap().id,
        ..Default::default()
    };
    playback.stop_playback(stopped).await.unwrap();
}

/// Each Viewing Event the Hub has, by its User and where it stopped.
fn viewing_events(hub: &FakeHub) -> Vec<(i64, String, i64)> {
    (hub.account().viewing_events.iter())
        .map(|recorded| {
            let event = recorded.event.as_option().cloned().unwrap_or_default();
            (recorded.user_id, event.external_id, event.stop_position_ms)
        })
        .collect()
}

#[tokio::test]
async fn a_linked_users_play_reaches_the_hub_as_a_viewing_event() {
    let hub = FakeHub::start().await;
    let fx = alien_linked_with(&hub).await;
    start_admins_link(&fx.server).await;

    play_alien(&fx, &fx.server.client_config(), 60_000).await;

    until("the Hub has the Viewing Event", async || {
        send_waiting(&fx.server).await;
        !viewing_events(&hub).is_empty()
    })
    .await;
    assert_eq!(viewing_events(&hub), [(1, format!("tmdb-{ALIEN}"), 60_000)]);
}

#[tokio::test]
async fn an_unlinked_users_play_never_reaches_the_hub() {
    let hub = FakeHub::start().await;
    let fx = alien_linked_with(&hub).await;
    // Unlinked before she signs in, as her sign-in's pull asks the Hub whether she is Linked and
    // the Server keeps that answer for a minute.
    let id = fx.server.create_user("Ada").await;
    hub.unlink_user_on_hub(id);
    let ada = fx.server.sign_in(id, "Ada").await;

    play_alien(&fx, &ada.client_config(), 60_000).await;
    play_alien(&fx, &fx.server.client_config(), 30_000).await;

    until("the Hub has the Admin's Viewing Event", async || {
        send_waiting(&fx.server).await;
        !viewing_events(&hub).is_empty()
    })
    .await;
    assert_eq!(viewing_events(&hub), [(1, format!("tmdb-{ALIEN}"), 30_000)]);
}
