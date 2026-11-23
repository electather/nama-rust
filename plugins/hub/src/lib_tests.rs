use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use buffa::Message as _;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{DeviceKind as HubDeviceKind, ViewingEvent};
use nama_plugin_api::{
    BoxFuture, ObservePoint, Observer, Registrar, Registration,
    automation::{
        Kernel, KernelFunction as _, KernelRefused,
        functions::{
            ListDevices, ListLibraries, ListTitles, ListVersionTitles, ListWatchStates,
            StoreDelete, StoreGet, StorePut,
        },
    },
    contract::{
        Device, DeviceKind, Devices, Event, Libraries, Library, Playback, PlaybackEnded,
        PlaybackStarted, StoreEntry, StoreKey, StoreValue, Stored, Title, TitleId, TitleRef,
        TitleRefKind, TitleWatchStates, Titles, VersionTitles, VersionTitlesAsked, WatchState,
        WatchStates, WatchStatesAsked, event::Event as Happened,
    },
    contract::{
        FeaturedSuggestions, HomeRow, HomeRowTitle, HomeRows, LibraryKind, SharedRowsAsked,
        SimilarAsked, SimilarTitle, SimilarTitles, TitleKind, TitlesAsked, UserFeaturedAsked,
        UserRowsAsked,
    },
    diagnostics_sharer::{DiagnosticsSharer, Signal},
    home_row::{HomeRowMethod, HomeRowSource},
    job::RunJob,
    points::{
        ObservedEvents, TrackDescription, TrackPreference, TrackPreferenceChange,
        TrackPreferenceFor, WatchStateChange, WatchStateChanges,
    },
    server_link::{Link, Reachability, ServerAddresses, ServerLinker, WatchlistEntry},
    similar_source::SimilarSource,
};
use serde_json::json;

use nama_hub_proto::nama::hub::v1::WatchStateChange as HubChange;

use super::*;
use crate::fake_hub::{
    EMAIL, FakeHub, OPEN_PORT, PUBLIC_IP, SIGN_IN_ID, USER_CODE, USER_LINK_CODE,
};

/// A Kernel whose only functions are the Plugin's Store, kept in memory.
fn store() -> Arc<dyn Kernel> {
    let entries: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let mut entries = entries.lock().unwrap();
            let answer = if function == StoreGet::NAME {
                let key = StoreKey::decode_from_slice(&request).unwrap();
                StoreValue {
                    value: entries.get(&key.key).cloned(),
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == StorePut::NAME {
                let entry = StoreEntry::decode_from_slice(&request).unwrap();
                entries.insert(entry.key, entry.value);
                Stored::default().encode_to_vec()
            } else if function == StoreDelete::NAME {
                let key = StoreKey::decode_from_slice(&request).unwrap();
                entries.remove(&key.key);
                Stored::default().encode_to_vec()
            } else {
                return Box::pin(future::ready(Err(KernelRefused(function.to_owned()))));
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

/// The Plugin enabled against `hub`, keeping its Store in `kernel`.
fn enabled(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Arc<dyn ServerLinker> {
    provided(hub, kernel)
}

/// What the Plugin enabled against `hub`, keeping its Store in `kernel`, provides as `T`.
fn provided<T: ?Sized + Send + Sync + 'static>(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Arc<T> {
    provider(&registered(hub, kernel))
}

/// The Plugin enabled against `hub`, keeping its Store in `kernel`, as the Kernel holds it.
fn registered(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Vec<Registration> {
    let mut registrar = Registrar::new().with_kernel(Arc::clone(kernel));
    Hub::at(hub.url()).register(&mut registrar).unwrap();
    registrar.into_registrations()
}

/// What `registrations` provide as `T`.
fn provider<T: ?Sized + Send + Sync + 'static>(registrations: &[Registration]) -> Arc<T> {
    (registrations.iter())
        .find_map(Registration::provider::<T>)
        .unwrap()
}

/// What observes `P` among `registrations`.
fn observer<P: ObservePoint>(registrations: &[Registration]) -> Arc<dyn Observer<P>> {
    (registrations.iter())
        .find_map(Registration::observer::<P>)
        .unwrap()
}

/// What a Similar source is asked about a Movie known by `ids`.
fn movie(ids: &[(&str, &str)]) -> SimilarAsked {
    SimilarAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: ids
            .iter()
            .map(|&(namespace, value)| TitleId {
                namespace: namespace.to_owned(),
                value: value.to_owned(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// `similar`'s titles, each as its id's namespace and value and its weight.
fn answered(similar: &SimilarTitles) -> Vec<(&str, &str, f32)> {
    (similar.titles.iter())
        .map(|title: &SimilarTitle| {
            (
                title.id.namespace.as_str(),
                title.id.value.as_str(),
                title.weight,
            )
        })
        .collect()
}

/// Links the Server through `linker` to [`EMAIL`]'s Account on `hub`.
async fn link(hub: &FakeHub, linker: &dyn ServerLinker) {
    linker.start_link("Living room").await.unwrap();
    hub.enter_code();
    assert_eq!(
        linker.link().await.unwrap(),
        Link::Linked {
            account: EMAIL.to_owned(),
            reachability: None,
        }
    );
}

/// Links the User `user_id` through `linker` to [`EMAIL`]'s Account on `hub`.
async fn link_user(hub: &FakeHub, linker: &dyn ServerLinker, user_id: i64) {
    linker.start_user_link(user_id, "Grace").await.unwrap();
    hub.enter_user_code(user_id);
    assert_eq!(
        linker.user_link(user_id).await.unwrap(),
        Some(Link::Linked {
            account: EMAIL.to_owned(),
            reachability: None,
        })
    );
}

#[tokio::test]
async fn the_server_shows_a_code_and_is_linked_once_someone_enters_it() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);

    let started = linker.start_link("Living room").await.unwrap();

    let waiting = Link::Waiting {
        code: USER_CODE.to_owned(),
        url: hub.url(),
    };
    assert_eq!(started, waiting);
    assert_eq!(hub.starting().as_deref(), Some("Living room"));
    assert_eq!(linker.link().await.unwrap(), waiting);
    hub.enter_code();
    let linked_to_ada = Link::Linked {
        account: EMAIL.to_owned(),
        reachability: None,
    };
    assert_eq!(linker.link().await.unwrap(), linked_to_ada);
    assert!(hub.linked());
    // Enabled again, such as after a restart, it is still Linked.
    assert_eq!(enabled(&hub, &kernel).link().await.unwrap(), linked_to_ada);
}

#[tokio::test]
async fn a_server_that_isnt_linked_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let link = enabled(&hub, &store()).link().await.unwrap();

    assert_eq!(link, Link::NotLinked);
}

#[tokio::test]
async fn the_code_keeps_showing_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    let started = linker.start_link("Living room").await.unwrap();
    hub.stop();

    let link = linker.link().await.unwrap();

    assert_eq!(link, started);
}

#[tokio::test]
async fn a_linked_server_cant_start_linking_again() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    let refused = linker.start_link("Living room").await.unwrap_err();

    assert!(refused.to_string().contains("Unlink it first"), "{refused}");
}

#[tokio::test]
async fn unlinking_revokes_the_servers_credential_on_the_hub() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;

    linker.unlink().await.unwrap();

    assert!(!hub.linked());
    assert_eq!(linker.link().await.unwrap(), Link::NotLinked);
}

#[tokio::test]
async fn unlinking_while_the_hub_is_unreachable_keeps_the_link() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    hub.stop();

    linker.unlink().await.unwrap_err();

    assert!(matches!(linker.link().await.unwrap(), Link::Linked { .. }));
}

#[tokio::test]
async fn a_credential_whose_token_cant_be_checked_is_kept_and_can_be_unlinked() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    linker.start_link("Living room").await.unwrap();
    hub.enter_code();
    hub.break_key_set();

    linker.link().await.unwrap_err();

    assert_eq!(
        linker.link().await.unwrap(),
        Link::Linked {
            account: "an Account the Hub didn't confirm".to_owned(),
            reachability: None,
        }
    );
    linker.unlink().await.unwrap();
    assert!(!hub.linked());
}

#[tokio::test]
async fn a_hub_signed_token_verifies_against_the_kept_key_set_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    assert_eq!(hub.key_set_fetches(), 1);
    let token = hub.sign(&json!({ "sub": "server", "email": "bea@example.com", "exp": u64::MAX }));
    hub.stop();

    let linker = Linker::new(HubApi::new(&hub.url()).unwrap(), kernel);
    let claims = linker.verify(&token).await.unwrap();

    assert_eq!(claims.email, "bea@example.com");
    assert_eq!(hub.key_set_fetches(), 1);
}

/// The Plugin's sharer, enabled against `hub` with the Store in `kernel`.
fn sharer(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Arc<dyn DiagnosticsSharer> {
    provided(hub, kernel)
}

#[tokio::test]
async fn a_linked_server_sends_each_shared_batch_to_the_hub_unchanged_with_its_credential() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let sharer = sharer(&hub, &kernel);
    sharer
        .share(Signal::Traces, b"spans".to_vec())
        .await
        .unwrap();
    sharer.share(Signal::Logs, b"lines".to_vec()).await.unwrap();

    assert_eq!(
        hub.shared(),
        [
            ("traces".to_owned(), b"spans".to_vec()),
            ("logs".to_owned(), b"lines".to_vec())
        ]
    );
}

#[tokio::test]
async fn a_server_that_isnt_linked_sends_nothing() {
    let hub = FakeHub::start().await;
    let kernel = store();

    sharer(&hub, &kernel)
        .share(Signal::Traces, b"spans".to_vec())
        .await
        .unwrap();

    assert_eq!(hub.shared(), []);
}

#[test]
fn without_the_hubs_address_the_plugin_refuses_to_start() {
    let mut registrar = Registrar::new().with_kernel(store());

    let refused = Hub { url: None }.register(&mut registrar).unwrap_err();

    assert!(refused.to_string().contains("NAMA_HUB_URL"), "{refused}");
}

#[tokio::test]
async fn a_linked_server_gets_the_titles_the_hub_finds_alike() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    let similar = provided::<dyn SimilarSource>(&hub, &kernel);

    let the_matrix = similar
        .similar(&movie(&[("imdb", "tt0133093"), ("tmdb", "603")]))
        .await
        .unwrap();

    assert_eq!(
        answered(&the_matrix),
        [("imdb", "tt0234215", 0.9), ("imdb", "tt0078748", 0.35)]
    );
}

/// The User `user_id`'s rows and Featured suggestions from the Plugin enabled against `hub`,
/// keeping its Store in `kernel`.
async fn recommended_for(
    hub: &FakeHub,
    kernel: &Arc<dyn Kernel>,
    user_id: i64,
) -> Result<(HomeRows, FeaturedSuggestions), PluginError> {
    let source = provided::<dyn HomeRowSource>(hub, kernel);
    let rows = (source.user_rows(&UserRowsAsked {
        user_id,
        ..Default::default()
    }))
    .await?;
    let featured = (source.user_featured(&UserFeaturedAsked {
        user_id,
        ..Default::default()
    }))
    .await?;
    Ok((rows, featured))
}

/// Each row as its key, heading, time to live and titles, each title as its kind and External ID,
/// such as `recommended: Recommended for you, 3600 s: movie tmdb-604`.
fn rows_of(rows: &HomeRows) -> Vec<String> {
    (rows.rows.iter())
        .map(|row| {
            let titles: Vec<String> = (row.titles.iter())
                .map(|title| {
                    let kind = match title.kind.as_known() {
                        Some(TitleKind::TITLE_KIND_SHOW) => "show",
                        _ => "movie",
                    };
                    format!("{kind} {}-{}", title.id.namespace, title.id.value)
                })
                .collect();
            let (key, heading, ttl) = (&row.key, &row.heading, row.ttl_seconds);
            format!("{key}: {heading}, {ttl} s: {}", titles.join(", "))
        })
        .collect()
}

/// Each suggestion's kind, External ID and weight.
fn suggested(featured: &FeaturedSuggestions) -> Vec<(TitleKind, String, f32)> {
    (featured.suggestions.iter())
        .map(|suggestion| {
            let id = &suggestion.id;
            let kind = suggestion.kind.as_known().unwrap();
            (
                kind,
                format!("{}-{}", id.namespace, id.value),
                suggestion.weight,
            )
        })
        .collect()
}

/// A Kernel keeping the Store as [`store`] does, whose one Library holds an Episode of the Show
/// `tmdb-1396`, and no Movie.
fn with_a_show() -> Arc<dyn Kernel> {
    let store = store();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let answer = if function == ListLibraries::NAME {
                let shows = Library {
                    id: 1,
                    ..Default::default()
                };
                Libraries {
                    libraries: vec![shows],
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == ListTitles::NAME {
                let episode = Title {
                    ids: vec![tmdb("62085")],
                    show_ids: vec![tmdb("1396")],
                    ..Default::default()
                };
                Titles {
                    titles: vec![episode],
                    ..Default::default()
                }
                .encode_to_vec()
            } else {
                let (store, function) = (Arc::clone(&store), function.to_owned());
                return Box::pin(async move { store.call(&function, request).await });
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

fn tmdb(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_linked_user_gets_the_hubs_recommendations_as_a_row_and_those_on_the_server_featured() {
    let hub = FakeHub::start().await;
    let kernel = with_a_show();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;

    let (rows, featured) = recommended_for(&hub, &kernel, 7).await.unwrap();

    assert_eq!(
        rows_of(&rows),
        ["recommended: Recommended for you, 3600 s: movie tmdb-604, show tmdb-1396"]
    );
    // Not the Movie, which no Library has, so Featured couldn't Play it (ADR 0072).
    assert_eq!(
        suggested(&featured),
        [(TitleKind::TITLE_KIND_SHOW, "tmdb-1396".to_owned(), 0.4)]
    );
    assert_eq!(featured.ttl_seconds, 3600);
    let source = provided::<dyn HomeRowSource>(&hub, &kernel);
    assert!(source.gives(HomeRowMethod::User));
    assert!(source.gives(HomeRowMethod::Featured));
}

#[tokio::test]
async fn a_user_the_hub_recommends_nothing_gets_no_row_or_suggestions() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let (rows, featured) = recommended_for(&hub, &kernel, 8).await.unwrap();

    assert_eq!(rows_of(&rows), Vec::<String>::new());
    assert_eq!(suggested(&featured), []);
}

#[tokio::test]
async fn a_server_that_isnt_linked_gets_no_recommendations_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let (rows, featured) = recommended_for(&hub, &store(), 7).await.unwrap();

    assert_eq!(rows_of(&rows), Vec::<String>::new());
    assert_eq!(suggested(&featured), []);
}

#[tokio::test]
async fn recommendations_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    hub.stop();

    recommended_for(&hub, &kernel, 7).await.unwrap_err();
}

#[tokio::test]
async fn a_user_shows_a_code_and_is_linked_once_it_is_entered() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));

    let started = linker.start_user_link(7, "Grace").await.unwrap();

    let waiting = Link::Waiting {
        code: USER_LINK_CODE.to_owned(),
        url: hub.url(),
    };
    assert_eq!(started, waiting);
    assert_eq!(hub.user_starting(7).as_deref(), Some("Grace"));
    assert_eq!(linker.user_link(7).await.unwrap(), Some(waiting));
    hub.enter_user_code(7);
    assert_eq!(
        linker.user_link(7).await.unwrap(),
        Some(Link::Linked {
            account: EMAIL.to_owned(),
            reachability: None,
        })
    );
    assert_eq!(linker.user_link(8).await.unwrap(), Some(Link::NotLinked));
}

#[tokio::test]
async fn users_cant_link_until_the_server_is() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());

    assert_eq!(linker.user_link(7).await.unwrap(), None);
    linker.start_user_link(7, "Grace").await.unwrap_err();
}

#[tokio::test]
async fn a_linked_user_shows_linked_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    hub.stop();

    // Enabled again, it asks the Hub, which doesn't answer.
    let link = enabled(&hub, &kernel).user_link(7).await.unwrap();

    assert_eq!(
        link,
        Some(Link::Linked {
            account: EMAIL.to_owned(),
            reachability: None,
        })
    );
}

#[tokio::test]
async fn a_title_the_hub_doesnt_know_gets_no_similar_titles() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let unknown = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "1")]))
        .await;

    assert_eq!(answered(&unknown.unwrap()), []);
}

#[tokio::test]
async fn a_server_that_isnt_linked_gets_no_similar_titles_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let similar = provided::<dyn SimilarSource>(&hub, &store())
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    assert_eq!(answered(&similar.unwrap()), []);
}

#[tokio::test]
async fn similar_titles_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    hub.stop();

    let similar = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    similar.unwrap_err();
}

#[tokio::test]
async fn the_server_unlinking_a_user_unlinks_them_on_the_hub() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;

    linker.unlink_user(7).await.unwrap();

    assert!(!hub.user_linked(7));
    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));
    let kept = kernel
        .ask::<StoreGet>(StoreKey {
            key: "user-link/7".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(kept.value, None);
}

#[tokio::test]
async fn the_account_unlinking_a_user_on_the_hub_shows_on_the_server() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;

    hub.unlink_user_on_hub(7);

    // Enabled again, as a minute later, it asks the Hub again.
    let link = enabled(&hub, &kernel).user_link(7).await.unwrap();
    assert_eq!(link, Some(Link::NotLinked));
}

#[tokio::test]
async fn a_linked_users_settings_dont_ask_the_hub_each_time() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    let asks = hub.user_link_asks();

    linker.user_link(7).await.unwrap();
    linker.user_link(7).await.unwrap();

    assert_eq!(hub.user_link_asks(), asks);
}

#[tokio::test]
async fn a_user_linked_already_who_starts_linking_sees_their_link() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));
    hub.link_user_on_hub(7);

    let started = linker.start_user_link(7, "Grace").await.unwrap();

    let their_link = Link::Linked {
        account: EMAIL.to_owned(),
        reachability: None,
    };
    assert_eq!(started, their_link);
    assert_eq!(linker.user_link(7).await.unwrap(), Some(their_link));
}

#[tokio::test]
async fn a_server_linked_again_doesnt_show_its_users_old_links_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    linker.unlink().await.unwrap();
    link(&hub, linker.as_ref()).await;
    hub.stop();

    let link = enabled(&hub, &kernel).user_link(7).await.unwrap();

    assert_eq!(link, Some(Link::NotLinked));
}

#[tokio::test]
async fn a_server_linked_again_doesnt_show_its_users_old_links() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    linker.start_user_link(8, "Ida").await.unwrap();

    linker.unlink().await.unwrap();
    link(&hub, linker.as_ref()).await;

    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));
    assert_eq!(linker.user_link(8).await.unwrap(), Some(Link::NotLinked));
}

/// The Kernel's side of the Plugin: its observer, its Job runner and its linker.
type Syncing = (
    Arc<dyn Observer<WatchStatesChanged>>,
    Arc<dyn JobRunner>,
    Arc<dyn ServerLinker>,
);

/// The Plugin enabled against `hub`, the Server and its User 7 Linked, as the Kernel tells it of
/// Watch State changes, runs its Job and pulls.
async fn syncing_user_7(hub: &FakeHub) -> Syncing {
    let kernel = store();
    let linker = enabled(hub, &kernel);
    link(hub, linker.as_ref()).await;
    link_user(hub, linker.as_ref(), 7).await;
    let (observer, jobs, _) = syncing(hub, &kernel);
    (observer, jobs, linker)
}

/// The Plugin enabled against `hub`, the Server Linked and its User 7 Linked on the Hub though
/// Settings never asked, as the Kernel tells it of Watch State changes and runs its Job.
async fn syncing_user_7_linked_on_hub(
    hub: &FakeHub,
) -> (Arc<dyn Observer<WatchStatesChanged>>, Arc<dyn JobRunner>) {
    let kernel = store();
    link(hub, enabled(hub, &kernel).as_ref()).await;
    hub.link_user_on_hub(7);
    let (observer, jobs, _) = syncing(hub, &kernel);
    (observer, jobs)
}

/// The Plugin enabled against `hub`, keeping its Store in `kernel`, as the Kernel tells it of
/// Watch State changes and runs its Job.
fn syncing(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Syncing {
    let mut registrar = Registrar::new().with_kernel(Arc::clone(kernel));
    Hub::at(hub.url()).register(&mut registrar).unwrap();
    let registrations = registrar.into_registrations();
    let observer = (registrations.iter())
        .find_map(Registration::observer::<WatchStatesChanged>)
        .unwrap();
    let jobs = (registrations.iter())
        .find_map(Registration::provider::<dyn JobRunner>)
        .unwrap();
    let linker = (registrations.iter())
        .find_map(Registration::provider::<dyn ServerLinker>)
        .unwrap();
    (observer, jobs, linker)
}

/// The Movie `tmdb-<id>` at `position_ms`, changed at `changed_at_ms`.
fn movie_at(id: u64, position_ms: u64, changed_at_ms: i64) -> WatchStateChange {
    WatchStateChange {
        episode: false,
        ids: vec![format!("tmdb-{id}")],
        played: false,
        position_ms,
        duration_ms: 7000,
        changed_at_ms,
    }
}

/// Tells `observer` of the User `user_id`'s `changes`, in one write.
async fn tell(
    observer: &dyn Observer<WatchStatesChanged>,
    user_id: i64,
    changes: Vec<WatchStateChange>,
) {
    let write = WatchStateChanges { user_id, changes };
    observer.observe(&write).await.unwrap();
}

async fn run_sync(jobs: &dyn JobRunner) -> Result<(), PluginError> {
    let run = RunJob {
        job_id: "sync".to_owned(),
        ..Default::default()
    };
    jobs.run(&run).await
}

/// What the Hub was pushed, in order, as each change's first id, position and when it changed.
fn pushed(hub: &FakeHub) -> Vec<Vec<(String, u64, i64)>> {
    let changes = |push: Vec<HubChange>| {
        (push.into_iter())
            .map(|change| {
                (
                    change.ids[0].clone(),
                    change.position_ms,
                    change.changed_at_ms,
                )
            })
            .collect()
    };
    hub.pushes().into_iter().map(changes).collect()
}

#[tokio::test]
async fn changes_made_while_the_hub_is_down_reach_it_in_order_once_it_is_back() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_user_7(&hub).await;
    hub.set_down(true);

    tell(observer.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    tell(observer.as_ref(), 7, vec![movie_at(679, 2000, 2_000)]).await;
    assert!(run_sync(jobs.as_ref()).await.is_err());
    tell(observer.as_ref(), 7, vec![movie_at(348, 3000, 3_000)]).await;
    hub.set_down(false);
    run_sync(jobs.as_ref()).await.unwrap();
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(
        pushed(&hub),
        [[
            ("tmdb-679".to_owned(), 2000, 2_000),
            ("tmdb-348".to_owned(), 3000, 3_000)
        ]]
    );
}

/// The Plugin enabled against `hub`, the Server and its User 7 Linked, as the Kernel tells it of
/// Track Preference changes, runs its Job and pulls.
async fn syncing_preferences_of_user_7(
    hub: &FakeHub,
) -> (
    Arc<dyn Observer<TrackPreferenceChanged>>,
    Arc<dyn JobRunner>,
    Arc<dyn ServerLinker>,
) {
    let kernel = store();
    let linker = enabled(hub, &kernel);
    link(hub, linker.as_ref()).await;
    link_user(hub, linker.as_ref(), 7).await;
    let mut registrar = Registrar::new().with_kernel(kernel);
    Hub::at(hub.url()).register(&mut registrar).unwrap();
    let registrations = registrar.into_registrations();
    let observer = (registrations.iter())
        .find_map(Registration::observer::<TrackPreferenceChanged>)
        .unwrap();
    let jobs = (registrations.iter())
        .find_map(Registration::provider::<dyn JobRunner>)
        .unwrap();
    (observer, jobs, linker)
}

/// The User `user_id`'s Subtitle Preference for `kept_for`, subtitles in `language`, changed at
/// `changed_at_ms`.
fn subtitles_in(
    user_id: i64,
    kept_for: TrackPreferenceFor,
    language: &str,
    changed_at_ms: i64,
) -> TrackPreferenceChange {
    TrackPreferenceChange {
        user_id,
        preference: TrackPreference {
            kept_for,
            like: Some(TrackDescription {
                language: Some(language.to_owned()),
                ..TrackDescription::default()
            }),
            changed_at_ms,
            ..TrackPreference::default()
        },
    }
}

/// What the Hub was pushed of Track Preferences, in order, as each change's title and language.
fn pushed_preferences(hub: &FakeHub) -> Vec<Vec<(String, String)>> {
    (hub.preference_pushes().into_iter())
        .map(|push| {
            (push.into_iter())
                .map(|change| (change.title_id, change.language))
                .collect()
        })
        .collect()
}

#[tokio::test]
async fn preferences_changed_while_the_hub_is_down_reach_it_once_back_the_latest_of_each() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_preferences_of_user_7(&hub).await;
    let alien = || TrackPreferenceFor::Movie(vec!["tmdb-348".to_owned()]);
    hub.set_down(true);

    let told = [
        subtitles_in(7, alien(), "eng", 1_000),
        subtitles_in(7, alien(), "fre", 3_000),
        subtitles_in(7, TrackPreferenceFor::User, "ger", 2_000),
        // Told late, though made before the French.
        subtitles_in(7, alien(), "jpn", 2_500),
    ];
    for change in &told {
        observer.observe(change).await.unwrap();
    }
    assert!(run_sync(jobs.as_ref()).await.is_err());
    hub.set_down(false);
    run_sync(jobs.as_ref()).await.unwrap();
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(
        pushed_preferences(&hub),
        [[
            ("tmdb-348".to_owned(), "fre".to_owned()),
            (String::new(), "ger".to_owned())
        ]]
    );
}

#[tokio::test]
async fn a_pull_sends_the_queued_preferences_first_and_answers_them_under_each_id_as_descriptions()
{
    let hub = FakeHub::start().await;
    let (observer, _, linker) = syncing_preferences_of_user_7(&hub).await;
    let show =
        |ids: &[&str]| TrackPreferenceFor::Show(ids.iter().map(|&id| id.to_owned()).collect());
    let mut sdh = subtitles_in(7, show(&["tmdb-1396", "tvdb-81189"]), "eng", 1_000);
    sdh.preference.like = Some(TrackDescription {
        language: Some("eng".to_owned()),
        sdh: true,
        title: Some("English (SDH)".to_owned()),
        ..TrackDescription::default()
    });
    let not_linked = subtitles_in(8, TrackPreferenceFor::User, "ger", 1_000);

    observer.observe(&sdh).await.unwrap();
    observer.observe(&not_linked).await.unwrap();

    let under = |id| TrackPreference {
        kept_for: show(&[id]),
        ..sdh.preference.clone()
    };
    assert_eq!(
        linker.track_preferences(7).await.unwrap(),
        [under("tmdb-1396"), under("tvdb-81189")]
    );
    assert_eq!(linker.track_preferences(8).await.unwrap(), []);
}

#[tokio::test]
async fn an_account_approving_on_the_hub_signs_in_as_its_user() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;

    let started = linker
        .start_account_sign_in("https://nama.example/sign-in")
        .await
        .unwrap();

    assert_eq!(
        started.url,
        format!("{}/server-sign-in/{SIGN_IN_ID}", hub.url())
    );
    assert_eq!(
        hub.sign_in_returns_to().as_deref(),
        Some("https://nama.example/sign-in")
    );
    assert_eq!(
        linker
            .finish_account_sign_in(&started.handle, "")
            .await
            .unwrap(),
        None
    );
    let code = hub.approve_sign_in(7);
    assert_eq!(
        linker
            .finish_account_sign_in(&started.handle, &code)
            .await
            .unwrap(),
        Some(7)
    );
    assert_eq!(
        linker
            .finish_account_sign_in(&started.handle, &code)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn finishing_an_approved_sign_in_with_a_wrong_code_ends_it() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    let started = linker
        .start_account_sign_in("https://nama.example/sign-in")
        .await
        .unwrap();
    let code = hub.approve_sign_in(7);

    assert_eq!(
        linker
            .finish_account_sign_in(&started.handle, "wrong")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        linker
            .finish_account_sign_in(&started.handle, &code)
            .await
            .unwrap(),
        None
    );
}

fn addresses(url: &str, lan: &[&str]) -> ServerAddresses {
    ServerAddresses {
        url: url.to_owned(),
        lan: lan.iter().map(|&address| address.to_owned()).collect(),
        public_port: None,
    }
}

#[tokio::test]
async fn the_server_reports_its_addresses_once_linked_and_then_only_when_they_change() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    let lan_only = addresses("", &["http://192.168.1.10:7980"]);
    linker.set_addresses(lan_only.clone()).await.unwrap();
    assert_eq!(hub.reported_addresses(), []);
    link(&hub, linker.as_ref()).await;

    linker.set_addresses(lan_only.clone()).await.unwrap();
    linker.set_addresses(lan_only.clone()).await.unwrap();
    linker
        .set_addresses(addresses(
            "https://nama.example.com",
            &["http://192.168.1.10:7980"],
        ))
        .await
        .unwrap();

    let lan = vec!["http://192.168.1.10:7980".to_owned()];
    assert_eq!(
        hub.reported_addresses(),
        [
            (String::new(), lan.clone()),
            ("https://nama.example.com".to_owned(), lan),
        ]
    );
}

#[tokio::test]
async fn a_change_told_after_a_later_one_of_the_same_title_is_dropped() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_user_7(&hub).await;

    tell(observer.as_ref(), 7, vec![movie_at(348, 3000, 2_000)]).await;
    tell(observer.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(pushed(&hub), [[("tmdb-348".to_owned(), 3000, 2_000)]]);
}

#[tokio::test]
async fn a_user_who_isnt_linked_keeps_their_watch_state_on_the_server() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_user_7(&hub).await;
    let asks = hub.user_link_asks();

    tell(observer.as_ref(), 8, vec![movie_at(348, 1000, 1_000)]).await;
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(hub.user_link_asks(), asks + 1);
    assert_eq!(hub.pushes(), Vec::<Vec<HubChange>>::new());
}

#[tokio::test]
async fn a_user_linked_on_the_hub_has_their_changes_pushed_though_settings_never_asked() {
    let hub = FakeHub::start().await;
    let (observer, jobs) = syncing_user_7_linked_on_hub(&hub).await;

    tell(observer.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(pushed(&hub), [[("tmdb-348".to_owned(), 1000, 1_000)]]);
}

#[tokio::test]
async fn a_user_whose_link_the_hub_cant_be_asked_about_has_nothing_queued_and_isnt_asked_again() {
    let hub = FakeHub::start().await;
    let (observer, jobs) = syncing_user_7_linked_on_hub(&hub).await;
    hub.fail_user_links();
    let asks = hub.user_link_asks();

    tell(observer.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    tell(observer.as_ref(), 7, vec![movie_at(348, 2000, 2_000)]).await;
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(hub.user_link_asks(), asks + 1);
    assert_eq!(hub.pushes(), Vec::<Vec<HubChange>>::new());
}

#[tokio::test]
async fn changes_dont_ask_the_hub_again_about_a_user_it_was_just_asked_about() {
    let hub = FakeHub::start().await;
    let (observer, _) = syncing_user_7_linked_on_hub(&hub).await;
    let asks = hub.user_link_asks();

    tell(observer.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    tell(observer.as_ref(), 7, vec![movie_at(348, 2000, 2_000)]).await;

    assert_eq!(hub.user_link_asks(), asks + 1);
}

#[tokio::test]
async fn a_write_of_many_changes_is_pushed_a_page_at_a_time_in_order() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_user_7(&hub).await;
    let changes: Vec<_> = (1..=250).map(|id| movie_at(id, 1000, 1_000)).collect();

    tell(observer.as_ref(), 7, changes).await;
    run_sync(jobs.as_ref()).await.unwrap();

    let pushes = hub.pushes();
    let sizes: Vec<_> = pushes.iter().map(Vec::len).collect();
    let ids: Vec<_> = (pushes.into_iter().flatten())
        .map(|change| change.ids[0].clone())
        .collect();
    let expected: Vec<_> = (1..=250).map(|id| format!("tmdb-{id}")).collect();
    assert_eq!(sizes, [100, 100, 50]);
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn a_history_told_on_linking_is_pushed_whole_in_pages_behind_a_newer_change_queued() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_user_7(&hub).await;
    let history: Vec<_> = (1..=250).map(|id| movie_at(id, 1000, 1_000)).collect();

    tell(observer.as_ref(), 7, vec![movie_at(1, 3000, 5_000)]).await;
    tell(observer.as_ref(), 7, history).await;
    run_sync(jobs.as_ref()).await.unwrap();

    let pushes = pushed(&hub);
    let sizes: Vec<_> = pushes.iter().map(Vec::len).collect();
    assert_eq!(sizes, [100, 100, 50]);
    let mut expected = vec![("tmdb-1".to_owned(), 3000, 5_000)];
    expected.extend((2..=250).map(|id| (format!("tmdb-{id}"), 1000, 1_000)));
    assert_eq!(pushes.concat(), expected);
}

#[tokio::test]
async fn ids_the_hub_wouldnt_take_are_left_out_and_a_change_with_none_stays_behind() {
    let hub = FakeHub::start().await;
    let (observer, jobs, _) = syncing_user_7(&hub).await;
    let long = format!("anidb-{}", "1".repeat(100));
    let with_a_long_id = WatchStateChange {
        ids: vec![long.clone(), "tmdb-348".to_owned()],
        ..movie_at(348, 1000, 1_000)
    };
    let with_only_a_long_id = WatchStateChange {
        ids: vec![long],
        ..movie_at(679, 1000, 1_000)
    };

    tell(
        observer.as_ref(),
        7,
        vec![with_a_long_id, with_only_a_long_id],
    )
    .await;
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(pushed(&hub), [[("tmdb-348".to_owned(), 1000, 1_000)]]);
}

#[tokio::test]
async fn signing_in_with_an_account_fails_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;

    hub.stop();

    let started = linker
        .start_account_sign_in("https://nama.example/sign-in")
        .await;
    assert!(started.is_err());
}

#[tokio::test]
async fn a_server_that_isnt_linked_cant_sign_in_with_accounts_or_invite() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());

    let started = linker
        .start_account_sign_in("https://nama.example/sign-in")
        .await;
    assert!(started.is_err());
    linker
        .invite_user(7, "Grace", "grace@example.com")
        .await
        .unwrap_err();
    assert_eq!(hub.invite(7), None);
}

#[tokio::test]
async fn a_user_created_from_an_account_is_invited_on_the_hub() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;

    linker
        .invite_user(7, "Grace", "grace@example.com")
        .await
        .unwrap();

    assert_eq!(
        hub.invite(7),
        Some(("Grace".to_owned(), "grace@example.com".to_owned()))
    );
}

#[tokio::test]
async fn a_server_linked_again_reports_its_addresses_again() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    let lan_only = addresses("", &["http://192.168.1.10:7980"]);
    link(&hub, linker.as_ref()).await;
    linker.set_addresses(lan_only.clone()).await.unwrap();
    linker.unlink().await.unwrap();
    link(&hub, linker.as_ref()).await;

    linker.set_addresses(lan_only).await.unwrap();

    assert_eq!(hub.reported_addresses().len(), 2);
}

#[tokio::test]
async fn addresses_the_hub_couldnt_take_are_reported_again() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    let lan_only = addresses("", &["http://192.168.1.10:7980"]);
    hub.fail_next_addresses();
    assert!(linker.set_addresses(lan_only.clone()).await.is_err());

    linker.set_addresses(lan_only).await.unwrap();

    assert_eq!(hub.reported_addresses().len(), 1);
}

/// What the Plugin keeps in `kernel`'s Store of the Server's Link.
async fn kept_link(kernel: &Arc<dyn Kernel>) -> Option<Vec<u8>> {
    let key = StoreKey {
        key: "link".to_owned(),
        ..Default::default()
    };
    kernel.ask::<StoreGet>(key).await.unwrap().value
}

#[tokio::test]
async fn a_server_the_hub_dropped_is_unlinked_by_its_next_report() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    hub.forget_server();

    linker
        .set_addresses(addresses("", &["http://192.168.1.10:7980"]))
        .await
        .unwrap();

    assert_eq!(linker.link().await.unwrap(), Link::NotLinked);
    assert_eq!(kept_link(&kernel).await, None);
}

#[tokio::test]
async fn a_server_the_hub_dropped_is_unlinked_once_a_users_link_is_asked() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    hub.forget_server();

    // Enabled again, as a minute later, it asks the Hub again.
    let user_link = enabled(&hub, &kernel).user_link(7).await.unwrap();

    assert_eq!(user_link, None);
    assert_eq!(linker.link().await.unwrap(), Link::NotLinked);
    assert_eq!(kept_link(&kernel).await, None);
}

#[tokio::test]
async fn a_server_the_hub_dropped_links_again_and_so_do_its_users() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    hub.forget_server();
    linker
        .set_addresses(addresses("", &["http://192.168.1.10:7980"]))
        .await
        .unwrap();

    link(&hub, linker.as_ref()).await;

    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));
    link_user(&hub, linker.as_ref(), 7).await;
}

#[tokio::test]
async fn a_hub_that_doesnt_answer_or_fails_keeps_the_link() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    let lan_only = addresses("", &["http://192.168.1.10:7980"]);
    hub.fail_next_addresses();
    linker.set_addresses(lan_only.clone()).await.unwrap_err();
    hub.stop();

    linker.set_addresses(lan_only).await.unwrap_err();
    let user_link = enabled(&hub, &kernel).user_link(7).await.unwrap();

    assert_eq!(user_link, Some(Link::NotLinked));
    assert!(matches!(linker.link().await.unwrap(), Link::Linked { .. }));
    assert!(kept_link(&kernel).await.is_some());
}

/// The Plugin enabled against `hub` with `kernel`, its Server and its User 7 Linked.
async fn linked_user_7(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Vec<Registration> {
    let registrations = registered(hub, kernel);
    let linker = provider::<dyn ServerLinker>(&registrations);
    link(hub, linker.as_ref()).await;
    link_user(hub, linker.as_ref(), 7).await;
    registrations
}

/// [`linked_user_7`], once the Hub forgot the Server, as when the Account it Linked with is
/// deleted.
async fn dropped(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Vec<Registration> {
    let registrations = linked_user_7(hub, kernel).await;
    hub.forget_server();
    registrations
}

/// Checks that the Server shows Not Linked, and Links again at once, and so does its User 7.
async fn links_again(hub: &FakeHub, registrations: &[Registration]) {
    let linker = provider::<dyn ServerLinker>(registrations);
    assert_eq!(linker.link().await.unwrap(), Link::NotLinked);
    link(hub, linker.as_ref()).await;
    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));
    link_user(hub, linker.as_ref(), 7).await;
}

#[tokio::test]
async fn a_sync_of_changes_queued_for_a_server_the_hub_forgot_succeeds_and_drops_them() {
    let hub = FakeHub::start().await;
    let points = linked_user_7(&hub, &store()).await;
    let jobs = provider::<dyn JobRunner>(&points);
    let watch_states = observer::<WatchStatesChanged>(&points);
    tell(watch_states.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    let preference = subtitles_in(7, TrackPreferenceFor::User, "eng", 1_000);
    (observer::<TrackPreferenceChanged>(&points).observe(&preference))
        .await
        .unwrap();
    hub.forget_server();

    run_sync(jobs.as_ref()).await.unwrap();
    let calls = hub.server_calls();
    run_sync(jobs.as_ref()).await.unwrap();

    assert_eq!(
        hub.server_calls(),
        calls,
        "the next run asks the Hub nothing"
    );
    assert_eq!(hub.pushes(), Vec::<Vec<HubChange>>::new());
    assert_eq!(pushed_preferences(&hub), Vec::<Vec<_>>::new());
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn a_watch_state_pull_for_a_server_the_hub_forgot_answers_empty() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    let pulled = provider::<dyn ServerLinker>(&points).watch_states(7).await;

    assert_eq!(pulled.unwrap(), []);
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn a_preference_pull_for_a_server_the_hub_forgot_answers_empty() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    let pulled = provider::<dyn ServerLinker>(&points)
        .track_preferences(7)
        .await;

    assert_eq!(pulled.unwrap(), []);
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn a_watchlist_sync_for_a_server_the_hub_forgot_answers_empty() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    let synced = provider::<dyn ServerLinker>(&points)
        .sync_watchlist(7, vec![watchlisted(348, 1_000)])
        .await;

    assert_eq!(synced.unwrap(), []);
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn a_play_for_a_server_the_hub_forgot_sends_no_viewing_event() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &playing()).await;

    play(observer::<Events>(&points).as_ref(), 7, MATRIX_VERSION, 1).await;
    send(provider::<dyn JobRunner>(&points).as_ref())
        .await
        .unwrap();

    assert_eq!(hub.viewing_events(), []);
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn similar_titles_for_a_server_the_hub_forgot_are_none() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    let similar = provider::<dyn SimilarSource>(&points)
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    assert_eq!(answered(&similar.unwrap()), []);
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn the_recommended_row_for_a_server_the_hub_forgot_is_empty() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &with_a_show()).await;

    let rows = provider::<dyn HomeRowSource>(&points)
        .user_rows(&UserRowsAsked {
            user_id: 7,
            ..Default::default()
        })
        .await;

    assert_eq!(rows_of(&rows.unwrap()), Vec::<String>::new());
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn featured_suggestions_for_a_server_the_hub_forgot_are_none() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &with_a_show()).await;

    let featured = provider::<dyn HomeRowSource>(&points)
        .user_featured(&UserFeaturedAsked {
            user_id: 7,
            ..Default::default()
        })
        .await;

    assert_eq!(suggested(&featured.unwrap()), []);
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn nama_charts_for_a_server_the_hub_forgot_are_no_rows() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &with_movies(&["603"])).await;

    let rows = provider::<dyn HomeRowSource>(&points)
        .shared_rows(&SharedRowsAsked::default())
        .await;

    assert_eq!(rows.unwrap().rows, []);
    links_again(&hub, &points).await;
}

/// Checks that `refused` is the Server not being Linked.
fn says_not_linked<T>(refused: Result<T, PluginError>) {
    let Err(refused) = refused else {
        panic!("refused");
    };
    assert!(refused.to_string().contains("isn't Linked"), "{refused}");
}

#[tokio::test]
async fn a_user_cant_start_linking_once_the_hub_forgot_the_server() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    says_not_linked(
        provider::<dyn ServerLinker>(&points)
            .start_user_link(8, "Grace")
            .await,
    );
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn a_user_cant_be_invited_once_the_hub_forgot_the_server() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    says_not_linked(
        provider::<dyn ServerLinker>(&points)
            .invite_user(8, "Grace", "grace@example.com")
            .await,
    );
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn no_one_signs_in_with_an_account_once_the_hub_forgot_the_server() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    says_not_linked(
        provider::<dyn ServerLinker>(&points)
            .start_account_sign_in("https://nama.example/sign-in")
            .await,
    );
    links_again(&hub, &points).await;
}

#[tokio::test]
async fn every_call_while_the_hub_is_unreachable_fails_and_keeps_the_link() {
    let hub = FakeHub::start().await;
    let points = linked_user_7(&hub, &playing()).await;
    let linker = provider::<dyn ServerLinker>(&points);
    let rows = provider::<dyn HomeRowSource>(&points);
    let preference = subtitles_in(7, TrackPreferenceFor::User, "eng", 1_000);
    (observer::<TrackPreferenceChanged>(&points).observe(&preference))
        .await
        .unwrap();
    hub.stop();

    run_sync(provider::<dyn JobRunner>(&points).as_ref())
        .await
        .unwrap_err();
    linker.watch_states(7).await.unwrap_err();
    linker.track_preferences(7).await.unwrap_err();
    (linker.sync_watchlist(7, vec![watchlisted(348, 1_000)]))
        .await
        .unwrap_err();
    play(observer::<Events>(&points).as_ref(), 7, MATRIX_VERSION, 1).await;
    send(provider::<dyn JobRunner>(&points).as_ref())
        .await
        .unwrap_err();
    (provider::<dyn SimilarSource>(&points).similar(&movie(&[("tmdb", "603")])))
        .await
        .unwrap_err();
    let user = UserRowsAsked {
        user_id: 7,
        ..Default::default()
    };
    rows.user_rows(&user).await.unwrap_err();
    let featured = UserFeaturedAsked {
        user_id: 7,
        ..Default::default()
    };
    rows.user_featured(&featured).await.unwrap_err();
    (rows.shared_rows(&SharedRowsAsked::default()))
        .await
        .unwrap_err();
    linker.start_user_link(8, "Grace").await.unwrap_err();
    (linker.invite_user(8, "Grace", "grace@example.com"))
        .await
        .unwrap_err();
    let signing_in = linker.start_account_sign_in("https://nama.example/sign-in");
    assert!(signing_in.await.is_err());

    assert!(matches!(linker.link().await.unwrap(), Link::Linked { .. }));
    assert!(matches!(
        linker.user_link(7).await.unwrap(),
        Some(Link::Linked { .. })
    ));
}

#[tokio::test]
async fn a_hub_failing_the_syncs_pushes_keeps_the_link() {
    let hub = FakeHub::start().await;
    let points = linked_user_7(&hub, &store()).await;
    let watch_states = observer::<WatchStatesChanged>(&points);
    tell(watch_states.as_ref(), 7, vec![movie_at(348, 1000, 1_000)]).await;
    let preference = subtitles_in(7, TrackPreferenceFor::User, "eng", 1_000);
    (observer::<TrackPreferenceChanged>(&points).observe(&preference))
        .await
        .unwrap();
    hub.set_down(true);

    run_sync(provider::<dyn JobRunner>(&points).as_ref())
        .await
        .unwrap_err();

    let linker = provider::<dyn ServerLinker>(&points);
    assert!(matches!(linker.link().await.unwrap(), Link::Linked { .. }));
}

#[tokio::test]
async fn unlinking_a_server_the_hub_forgot_succeeds() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    provider::<dyn ServerLinker>(&points)
        .unlink()
        .await
        .unwrap();

    links_again(&hub, &points).await;
}

#[tokio::test]
async fn unlinking_a_user_of_a_server_the_hub_forgot_succeeds() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store()).await;

    provider::<dyn ServerLinker>(&points)
        .unlink_user(7)
        .await
        .unwrap();

    links_again(&hub, &points).await;
}

#[tokio::test]
async fn a_user_linked_on_the_hub_has_their_plays_sent_though_settings_never_asked() {
    let hub = FakeHub::start().await;
    let points = registered(&hub, &playing());
    link(&hub, provider::<dyn ServerLinker>(&points).as_ref()).await;
    hub.link_user_on_hub(7);

    play(observer::<Events>(&points).as_ref(), 7, MATRIX_VERSION, 1).await;

    assert_eq!(hub.viewing_events().len(), 1);
}

#[tokio::test]
async fn a_user_the_hub_says_isnt_linked_has_nothing_sent() {
    let hub = FakeHub::start().await;
    let points = linked_user_7(&hub, &playing()).await;
    let jobs = provider::<dyn JobRunner>(&points);

    tell(
        observer::<WatchStatesChanged>(&points).as_ref(),
        8,
        vec![movie_at(348, 1000, 1_000)],
    )
    .await;
    let preference = subtitles_in(8, TrackPreferenceFor::User, "eng", 1_000);
    (observer::<TrackPreferenceChanged>(&points).observe(&preference))
        .await
        .unwrap();
    let synced = provider::<dyn ServerLinker>(&points)
        .sync_watchlist(8, vec![watchlisted(348, 1_000)])
        .await;
    play(observer::<Events>(&points).as_ref(), 8, MATRIX_VERSION, 1).await;
    run_sync(jobs.as_ref()).await.unwrap();
    send(jobs.as_ref()).await.unwrap();

    assert_eq!(synced.unwrap(), []);
    assert_eq!(hub.pushes(), Vec::<Vec<HubChange>>::new());
    assert_eq!(pushed_preferences(&hub), Vec::<Vec<_>>::new());
    assert_eq!(hub.watchlist_syncs(), Vec::<i64>::new());
    assert_eq!(hub.viewing_events(), []);
}

#[tokio::test]
async fn a_pull_sends_the_queue_first_and_answers_the_accounts_watch_state() {
    let hub = FakeHub::start().await;
    let (observer, _, linker) = syncing_user_7(&hub).await;
    tell(observer.as_ref(), 7, vec![movie_at(348, 3000, 2_000)]).await;

    let pulled = linker.watch_states(7).await.unwrap();

    assert_eq!(pushed(&hub), [[("tmdb-348".to_owned(), 3000, 2_000)]]);
    assert_eq!(pulled, [movie_at(348, 3000, 2_000)]);
    assert_eq!(
        linker.watch_states(8).await.unwrap(),
        [],
        "User 8 isn't Linked"
    );
}

/// The Version holding the Movie `tmdb-603`.
const MATRIX_VERSION: i64 = 1;

/// A Version whose Item isn't Matched.
const UNMATCHED_VERSION: i64 = 2;

/// The TV each play is on.
const TV: i64 = 3;

/// How far into `tmdb-603` each play stops, and how long it is: past 90%, so finished.
const STOPPED_MS: u64 = 7_500_000;
const MATRIX_MS: u64 = 8_160_000;

/// A Kernel keeping the Store as [`store`] does, which answers that [`MATRIX_VERSION`] holds the
/// Movie `tmdb-603`, that any User asked about stopped it at [`STOPPED_MS`] just now, and that
/// [`TV`] is a TV.
fn playing() -> Arc<dyn Kernel> {
    let store = store();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let answer = if function == ListVersionTitles::NAME {
                let asked = VersionTitlesAsked::decode_from_slice(&request).unwrap();
                let matrix = TitleRef {
                    library_id: 1,
                    kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
                    id: TitleId {
                        namespace: "tmdb".to_owned(),
                        value: "603".to_owned(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                };
                let titles = Vec::from_iter((asked.version_id == MATRIX_VERSION).then_some(matrix));
                VersionTitles {
                    titles,
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == ListWatchStates::NAME {
                let asked = WatchStatesAsked::decode_from_slice(&request).unwrap();
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                let state = WatchState {
                    user_id: asked.user_id.unwrap(),
                    position_ms: STOPPED_MS,
                    duration_ms: MATRIX_MS,
                    updated_at_ms: i64::try_from(now.as_millis()).unwrap(),
                    ..Default::default()
                };
                let titles = (asked.titles.into_iter())
                    .map(|title_ref| TitleWatchStates {
                        title_ref: title_ref.into(),
                        states: vec![state.clone()],
                        ..Default::default()
                    })
                    .collect();
                WatchStates {
                    titles,
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == ListDevices::NAME {
                let tv = Device {
                    id: TV,
                    user_id: 7,
                    kind: DeviceKind::DEVICE_KIND_TV.into(),
                    ..Default::default()
                };
                Devices {
                    devices: vec![tv],
                    ..Default::default()
                }
                .encode_to_vec()
            } else {
                let (store, function) = (Arc::clone(&store), function.to_owned());
                return Box::pin(async move { store.call(&function, request).await });
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

/// The Extension Points the Plugin serves.
struct Points {
    linker: Arc<dyn ServerLinker>,
    observer: Arc<dyn Observer<Events>>,
    jobs: Arc<dyn JobRunner>,
}

/// The Plugin enabled against `hub` with `kernel`.
fn enabled_points(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Points {
    let mut registrar = Registrar::new().with_kernel(Arc::clone(kernel));
    Hub::at(hub.url()).register(&mut registrar).unwrap();
    let registrations = registrar.into_registrations();
    let mut found = registrations.iter();
    Points {
        linker: (found.clone())
            .find_map(Registration::provider::<dyn ServerLinker>)
            .unwrap(),
        observer: (found.clone())
            .find_map(Registration::observer::<Events>)
            .unwrap(),
        jobs: found
            .find_map(Registration::provider::<dyn JobRunner>)
            .unwrap(),
    }
}

/// The User `user_id` plays `version` on [`TV`] as the plan `plan_id`, and stops.
async fn play(observer: &dyn Observer<Events>, user_id: i64, version: i64, plan_id: u64) {
    let playback = Playback {
        user_id,
        device_id: TV,
        version_id: version,
        plan_id,
        ..Default::default()
    };
    let event = |happened: Happened| Event {
        event: Some(happened),
        ..Default::default()
    };
    let started = PlaybackStarted {
        playback: playback.clone().into(),
        ..Default::default()
    };
    let ended = PlaybackEnded {
        playback: playback.into(),
        ..Default::default()
    };
    for happened in [started.into(), ended.into()] {
        let batch = ObservedEvents {
            events: vec![event(happened)],
            ..Default::default()
        };
        observer.observe(&batch).await.unwrap();
    }
}

/// Runs the Job that sends the Viewing Events the Hub hasn't had yet.
async fn send(jobs: &dyn JobRunner) -> Result<(), PluginError> {
    let run = RunJob {
        job_id: "send-viewing-events".to_owned(),
        ..Default::default()
    };
    jobs.run(&run).await
}

#[tokio::test]
async fn a_linked_users_play_of_a_matched_title_goes_to_the_hub() {
    let hub = FakeHub::start().await;
    let Points {
        linker, observer, ..
    } = enabled_points(&hub, &playing());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    play(observer.as_ref(), 7, MATRIX_VERSION, 1).await;

    let sent = hub.viewing_events();
    let [sent] = sent.as_slice() else {
        panic!("one Viewing Event: {sent:?}");
    };
    assert_eq!(sent.user_id, 7);
    assert_ne!(sent.key, "");
    let event = sent.event.as_option().unwrap();
    let started_at = u64::try_from(event.started_at).unwrap();
    assert!(started_at >= before.as_secs(), "{started_at}");
    assert!((0..24 * 60).contains(&event.time_of_day_minutes));
    assert_eq!(
        event,
        &ViewingEvent {
            external_id: "tmdb-603".to_owned(),
            started_at: event.started_at,
            time_of_day_minutes: event.time_of_day_minutes,
            stop_position_ms: 7_500_000,
            duration_ms: 8_160_000,
            finished: true,
            device_kind: HubDeviceKind::DEVICE_KIND_TV.into(),
            ..Default::default()
        }
    );
}

/// The Movie `tmdb-<id>` on the Watchlist, changed at `changed_at_ms`.
fn watchlisted(id: u64, changed_at_ms: i64) -> WatchlistEntry {
    WatchlistEntry {
        show: false,
        id: format!("tmdb-{id}"),
        removed: false,
        changed_at_ms,
    }
}

#[tokio::test]
async fn a_watchlist_sync_answers_the_accounts_watchlist_and_a_users_leaves_only_once_linked() {
    let hub = FakeHub::start().await;
    let (_, _, linker) = syncing_user_7(&hub).await;
    let too_long = WatchlistEntry {
        id: format!("tmdb-{}", "1".repeat(100)),
        ..watchlisted(0, 3_000)
    };
    let removed = WatchlistEntry {
        show: true,
        removed: true,
        ..watchlisted(1399, 2_000)
    };
    let watchlist = vec![watchlisted(348, 1_000), removed.clone(), too_long];

    let synced = linker.sync_watchlist(7, watchlist).await.unwrap();

    assert_eq!(synced, [removed, watchlisted(348, 1_000)]);
    assert_eq!(
        linker
            .sync_watchlist(8, vec![watchlisted(603, 1_000)])
            .await
            .unwrap(),
        [],
        "User 8 isn't Linked"
    );
    assert_eq!(hub.watchlist_syncs(), [7]);
}

#[tokio::test]
async fn a_server_that_isnt_linked_syncs_no_watchlist() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());

    let synced = linker.sync_watchlist(7, vec![watchlisted(348, 1_000)]);

    assert_eq!(synced.await.unwrap(), []);
}

#[tokio::test]
async fn a_server_that_isnt_linked_pulls_nothing() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());

    assert_eq!(linker.watch_states(7).await.unwrap(), []);
}

#[tokio::test]
async fn plays_by_users_who_arent_linked_or_of_unmatched_items_send_nothing() {
    let hub = FakeHub::start().await;
    let Points {
        linker,
        observer,
        jobs,
    } = enabled_points(&hub, &playing());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;

    play(observer.as_ref(), 8, MATRIX_VERSION, 1).await;
    play(observer.as_ref(), 7, UNMATCHED_VERSION, 2).await;
    send(jobs.as_ref()).await.unwrap();

    assert_eq!(hub.viewing_events(), []);
}

#[tokio::test]
async fn a_play_while_the_hub_is_down_is_sent_once_it_is_back_after_a_restart_and_then_forgotten() {
    let hub = FakeHub::start().await;
    let kernel = playing();
    let Points {
        linker,
        observer,
        jobs,
    } = enabled_points(&hub, &kernel);
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    hub.fail_recording(Some(ErrorCode::Unavailable));
    play(observer.as_ref(), 7, MATRIX_VERSION, 1).await;
    send(jobs.as_ref()).await.unwrap_err();
    assert_eq!(hub.viewing_events(), []);
    // The Server restarts: a new Plugin over the same Store.
    let restarted = enabled_points(&hub, &kernel);

    hub.fail_recording(None);
    send(restarted.jobs.as_ref()).await.unwrap();
    send(restarted.jobs.as_ref()).await.unwrap();

    assert_eq!(hub.viewing_events().len(), 1);
}

#[tokio::test]
async fn the_server_shows_whether_the_hub_reached_its_public_port() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    let with_port = |port| ServerAddresses {
        public_port: port,
        ..addresses("", &["http://192.168.1.10:7980"])
    };
    let shown = async || {
        let Link::Linked { reachability, .. } = linker.link().await.unwrap() else {
            panic!("not Linked");
        };
        reachability
    };

    linker
        .set_addresses(with_port(Some(OPEN_PORT)))
        .await
        .unwrap();
    let reached = shown().await;
    linker
        .set_addresses(with_port(Some(OPEN_PORT + 1)))
        .await
        .unwrap();
    let missed = shown().await;
    linker.set_addresses(with_port(None)).await.unwrap();

    assert_eq!(
        reached,
        Some(Reachability {
            address: format!("http://{PUBLIC_IP}:{OPEN_PORT}"),
            reachable: true,
        })
    );
    assert_eq!(
        missed,
        Some(Reachability {
            address: format!("http://{PUBLIC_IP}:{}", OPEN_PORT + 1),
            reachable: false,
        })
    );
    assert_eq!(shown().await, None);
}

/// The Movies Library that [`with_movies`]'s Kernel has.
const MOVIES_LIBRARY: i64 = 1;

/// A Kernel keeping the Store as [`store`] does, with a Movies Library holding the Movies whose
/// TMDB ids are `tmdb`, and a Shows Library.
fn with_movies(tmdb: &'static [&'static str]) -> Arc<dyn Kernel> {
    let store = store();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let answer = if function == ListLibraries::NAME {
                let library = |id, kind: LibraryKind| Library {
                    id,
                    kind: kind.into(),
                    ..Default::default()
                };
                Libraries {
                    libraries: vec![
                        library(MOVIES_LIBRARY, LibraryKind::LIBRARY_KIND_MOVIES),
                        library(2, LibraryKind::LIBRARY_KIND_SHOWS),
                    ],
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == ListTitles::NAME {
                let asked = TitlesAsked::decode_from_slice(&request).unwrap();
                assert_eq!(
                    asked.library_id, MOVIES_LIBRARY,
                    "only Movies are asked for"
                );
                let movie = |value: &str| Title {
                    title_ref: TitleRef {
                        library_id: MOVIES_LIBRARY,
                        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
                        id: TitleId {
                            namespace: "tmdb".to_owned(),
                            value: value.to_owned(),
                            ..Default::default()
                        }
                        .into(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                };
                Titles {
                    titles: tmdb.iter().map(|&value| movie(value)).collect(),
                    ..Default::default()
                }
                .encode_to_vec()
            } else {
                let (store, function) = (Arc::clone(&store), function.to_owned());
                return Box::pin(async move { store.call(&function, request).await });
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

/// The titles of `rows`' row keyed `key`, each as its id's namespace and value.
fn row_titles<'a>(rows: &'a HomeRows, key: &str) -> Vec<(&'a str, &'a str)> {
    let Some(row) = rows.rows.iter().find(|row| row.key == key) else {
        panic!("a row keyed {key}: {rows:?}");
    };
    (row.titles.iter())
        .map(|title: &HomeRowTitle| (title.id.namespace.as_str(), title.id.value.as_str()))
        .collect()
}

#[tokio::test]
async fn a_linked_server_shares_the_trending_movies_its_libraries_have_in_the_hubs_order() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["862", "603", "1"]);
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    assert_eq!(
        row_titles(&rows, "trending"),
        [("tmdb", "603"), ("tmdb", "862")]
    );
    let HomeRow {
        key,
        heading,
        ttl_seconds,
        titles,
        ..
    } = &rows.rows[0];
    assert_eq!(
        (key.as_str(), heading.as_str()),
        ("trending", "Trending on Nama")
    );
    assert_eq!(*ttl_seconds, 24 * 60 * 60);
    assert!(
        (titles.iter()).all(|title| title.kind.as_known() == Some(TitleKind::TITLE_KIND_MOVIE))
    );
}

#[tokio::test]
async fn trending_movies_no_library_has_leave_the_row_empty() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["1"]);
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    assert_eq!(row_titles(&rows, "trending"), []);
}

#[tokio::test]
async fn a_linked_server_shares_the_most_watchlisted_titles_in_the_hubs_order_had_or_not() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["603"]);
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    let Some(row) = (rows.rows.iter()).find(|row| row.key == "most-watchlisted") else {
        panic!("a most watchlisted row: {rows:?}");
    };
    let titles: Vec<_> = (row.titles.iter())
        .map(|title| (title.kind.as_known(), title.id.value.as_str()))
        .collect();
    let (movie, show) = (
        Some(TitleKind::TITLE_KIND_MOVIE),
        Some(TitleKind::TITLE_KIND_SHOW),
    );
    assert_eq!(titles, [(show, "70523"), (movie, "603"), (movie, "78")]);
    assert_eq!(row.heading, "Most watchlisted on Nama");
    assert_eq!(row.ttl_seconds, 24 * 60 * 60);
}

#[tokio::test]
async fn a_server_that_isnt_linked_has_no_trending_row_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let rows = provided::<dyn HomeRowSource>(&hub, &with_movies(&["603"]))
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    assert_eq!(rows.rows, []);
}

#[tokio::test]
async fn whats_trending_fails_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["603"]);
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    hub.stop();

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await;

    rows.unwrap_err();
}

#[tokio::test]
async fn the_trending_row_is_shared() {
    let hub = FakeHub::start().await;

    let rows = provided::<dyn HomeRowSource>(&hub, &store());

    assert!(rows.gives(HomeRowMethod::Shared));
}
