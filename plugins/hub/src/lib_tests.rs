use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use buffa::Message as _;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{DeviceKind as HubDeviceKind, TitleKind, ViewingEvent};
use nama_plugin_api::{
    BoxFuture, Observer, Registrar, Registration,
    automation::{
        Kernel, KernelFunction as _, KernelRefused,
        functions::{
            ListDevices, ListVersionTitles, ListWatchStates, StoreDelete, StoreGet, StorePut,
        },
    },
    contract::{
        Device, DeviceKind, Devices, Event, Playback, PlaybackEnded, PlaybackStarted, StoreEntry,
        StoreKey, StoreValue, Stored, TitleId, TitleRef, TitleRefKind, TitleWatchStates,
        VersionTitles, VersionTitlesAsked, WatchState, WatchStates, WatchStatesAsked,
        event::Event as Happened,
    },
    job::RunJob,
    points::ObservedEvents,
    server_link::{Link, ServerLinker},
};
use serde_json::json;

use super::*;
use crate::fake_hub::{EMAIL, FakeHub, USER_CODE, USER_LINK_CODE};

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
    let mut registrar = Registrar::new().with_kernel(Arc::clone(kernel));
    Hub::at(hub.url()).register(&mut registrar).unwrap();
    registrar.into_registrations()[0]
        .provider::<dyn ServerLinker>()
        .unwrap()
}

/// Links the Server through `linker` to [`EMAIL`]'s Account on `hub`.
async fn link(hub: &FakeHub, linker: &dyn ServerLinker) {
    linker.start_link("Living room").await.unwrap();
    hub.enter_code();
    assert_eq!(
        linker.link().await.unwrap(),
        Link::Linked {
            account: EMAIL.to_owned()
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
            account: EMAIL.to_owned()
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
            account: "an Account the Hub didn't confirm".to_owned()
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

#[test]
fn without_the_hubs_address_the_plugin_refuses_to_start() {
    let mut registrar = Registrar::new().with_kernel(store());

    let refused = Hub { url: None }.register(&mut registrar).unwrap_err();

    assert!(refused.to_string().contains("NAMA_HUB_URL"), "{refused}");
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
            account: EMAIL.to_owned()
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
            account: EMAIL.to_owned()
        })
    );
}

#[tokio::test]
async fn the_server_unlinking_a_user_unlinks_them_on_the_hub() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;

    linker.unlink_user(7).await.unwrap();

    assert!(!hub.user_linked(7));
    assert_eq!(linker.user_link(7).await.unwrap(), Some(Link::NotLinked));
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
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
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
async fn a_play_while_the_hub_is_down_is_sent_once_it_is_back_and_then_forgotten() {
    let hub = FakeHub::start().await;
    let Points {
        linker,
        observer,
        jobs,
    } = enabled_points(&hub, &playing());
    link(&hub, linker.as_ref()).await;
    link_user(&hub, linker.as_ref(), 7).await;
    hub.fail_recording(Some(ErrorCode::Unavailable));
    play(observer.as_ref(), 7, MATRIX_VERSION, 1).await;
    send(jobs.as_ref()).await.unwrap_err();
    assert_eq!(hub.viewing_events(), []);

    hub.fail_recording(None);
    send(jobs.as_ref()).await.unwrap();
    send(jobs.as_ref()).await.unwrap();

    assert_eq!(hub.viewing_events().len(), 1);
}
