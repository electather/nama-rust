use std::{cell::RefCell, collections::HashMap, rc::Rc};

use nama_pdk::{
    ImportAsked, LinkPolled, PollLinkAsked, ScrobbleAsked, SettingKind, StartLinkAsked,
    SyncedWatchState, TitleId, WatchSync,
    extism_pdk::{
        Error,
        json::{self, Value, json},
    },
    nama_plugin_proto::nama::plugin::v1::TitleRefKind,
    testing::{Kernel, Sent, with},
};

use super::{
    Simkl,
    fake_simkl::{ACCESS_TOKEN, CLIENT_ID, FakeSimkl, Pin, USER_CODE},
    manifest,
};

const ADA: i64 = 7;

/// 2026-11-20T20:00:00Z, when Ada finished watching.
const EVENING_MS: i64 = 1_795_204_800_000;

fn shared(fake: FakeSimkl) -> Rc<RefCell<FakeSimkl>> {
    Rc::new(RefCell::new(fake))
}

/// The Admin's app setting, the Store granted, and `fake` answering every request.
fn kernel(fake: &Rc<RefCell<FakeSimkl>>) -> Kernel {
    let fake = Rc::clone(fake);
    Kernel::new()
        .setting("client_id", CLIENT_ID)
        .store()
        .http(move |request, body| Ok(fake.borrow_mut().answer(request, body)))
}

/// The Personal settings linking Simkl gave.
fn linked<M: FromIterator<(String, String)>>() -> M {
    std::iter::once(("access_token".to_owned(), ACCESS_TOKEN.to_owned())).collect()
}

fn title_ids(ids: &[(&str, &str)]) -> Vec<TitleId> {
    (ids.iter())
        .map(|&(namespace, value)| TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..TitleId::default()
        })
        .collect()
}

fn change(kind: TitleRefKind, ids: &[(&str, &str)], played: bool) -> SyncedWatchState {
    SyncedWatchState {
        kind: kind.into(),
        ids: title_ids(ids),
        played,
        changed_at_ms: EVENING_MS,
        ..SyncedWatchState::default()
    }
}

fn movie(ids: &[(&str, &str)], played: bool) -> SyncedWatchState {
    change(TitleRefKind::TITLE_REF_KIND_MOVIE, ids, played)
}

fn episode(ids: &[(&str, &str)], played: bool) -> SyncedWatchState {
    change(TitleRefKind::TITLE_REF_KIND_EPISODE, ids, played)
}

fn scrobbled(kernel: &mut Kernel, changes: Vec<SyncedWatchState>) -> Result<(), Error> {
    let asked = ScrobbleAsked {
        personal_settings: linked(),
        user_id: ADA,
        changes,
        ..ScrobbleAsked::default()
    };
    with(kernel, || Simkl::scrobble(asked)).map(drop)
}

fn imported(kernel: &mut Kernel, since: &str) -> Result<(Vec<SyncedWatchState>, String), Error> {
    let asked = ImportAsked {
        personal_settings: linked(),
        user_id: ADA,
        since: since.to_owned(),
        ..ImportAsked::default()
    };
    let imported = with(kernel, || Simkl::import(asked))?;
    Ok((imported.changes, imported.cursor))
}

/// Each request `kernel` sent, as `METHOD path`, its host and query left out.
fn asked(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let url = sent.request.url.trim_start_matches("https://api.simkl.com");
            let path = url.split_once('?').map_or(url, |(path, _query)| path);
            format!("{} {path}", sent.request.method.as_deref().unwrap_or("GET"))
        })
        .collect()
}

fn body(sent: &Sent) -> Value {
    json::from_slice(&sent.body).unwrap()
}

fn polled(kernel: &mut Kernel, device_code: &str) -> LinkPolled {
    let asked = PollLinkAsked {
        device_code: device_code.to_owned(),
        ..PollLinkAsked::default()
    };
    with(kernel, || Simkl::poll_link(asked)).unwrap()
}

#[test]
fn linking_gives_the_pin_waits_for_the_user_then_hands_back_the_token() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);
    let started = with(&mut kernel, || Simkl::start_link(StartLinkAsked::default())).unwrap();
    let waiting = polled(&mut kernel, &started.device_code);
    fake.borrow_mut().pin = Pin::Entered;
    let linked = polled(&mut kernel, &started.device_code);

    assert_eq!(
        (
            started.user_code.as_str(),
            started.verification_url.as_str(),
            started.device_code.as_str(),
            started.expires_in_seconds,
            started.interval_seconds,
        ),
        (USER_CODE, "https://simkl.com/pin", USER_CODE, 900, 5)
    );
    assert!(waiting.personal_settings.is_empty() && !waiting.expired);
    let linked: HashMap<_, _> = linked.personal_settings.into_iter().collect();
    assert_eq!(
        linked,
        HashMap::from([("access_token".to_owned(), ACCESS_TOKEN.to_owned())])
    );
}

#[test]
fn an_expired_pin_says_so() {
    let fake = shared(FakeSimkl::new());
    fake.borrow_mut().pin = Pin::Expired;
    let mut kernel = kernel(&fake);

    let expired = polled(&mut kernel, USER_CODE);

    assert!(expired.expired && expired.personal_settings.is_empty());
}

#[test]
fn a_device_code_that_isnt_a_pin_expires_without_asking_simkl() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);

    let expired = polled(&mut kernel, "../../sync/history");

    assert!(expired.expired);
    assert_eq!(kernel.sent(), []);
}

#[test]
fn a_revoked_token_asks_the_user_to_link_again() {
    let fake = shared(FakeSimkl::new());
    fake.borrow_mut().revoked = true;
    let mut kernel = kernel(&fake);

    let error = scrobbled(&mut kernel, vec![movie(&[("tmdb", "438631")], true)]).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Simkl no longer takes this User's link; they link Simkl again in Personal settings"
    );
}

#[test]
fn a_played_movie_and_episode_are_marked_watched_as_of_when_they_were_played() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);

    scrobbled(
        &mut kernel,
        vec![
            movie(&[("tmdb", "438631"), ("imdb", "tt1160419")], true),
            episode(&[("tvdb", "349232"), ("tmdb", "63056")], true),
            episode(&[("tmdb", "63057")], true),
            movie(&[("imdb", "tt0078748")], true),
        ],
    )
    .unwrap();

    let [added] = kernel.sent() else {
        panic!("sent {:?}", asked(&kernel));
    };
    assert_eq!(asked(&kernel), ["POST /sync/history"]);
    assert_eq!(
        body(added),
        json!({
            "movies": [{ "ids": { "tmdb": 438_631 }, "watched_at": "2026-11-20T20:00:00Z" }],
            "episodes": [{ "ids": { "tvdb": 349_232 }, "watched_at": "2026-11-20T20:00:00Z" }],
        })
    );
}

#[test]
fn position_reports_send_nothing_and_a_played_title_is_added_once_while_it_plays_on() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);
    let playing = SyncedWatchState {
        position_ms: 60_000,
        duration_ms: 7_200_000,
        ..movie(&[("tmdb", "438631")], false)
    };
    let credits = |seconds_later: i64| SyncedWatchState {
        position_ms: 6_600_000,
        duration_ms: 7_200_000,
        changed_at_ms: EVENING_MS + seconds_later * 1000,
        ..movie(&[("tmdb", "438631")], true)
    };

    for changes in [
        vec![playing],
        vec![credits(0)],
        vec![credits(10)],
        vec![credits(20)],
    ] {
        scrobbled(&mut kernel, changes).unwrap();
    }

    assert_eq!(asked(&kernel), ["POST /sync/history"]);
}

#[test]
fn a_title_marked_unplayed_is_unmarked_and_a_later_play_is_added_again() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);
    let dune = &[("tmdb", "438631")];

    scrobbled(&mut kernel, vec![movie(dune, true)]).unwrap();
    scrobbled(&mut kernel, vec![movie(dune, false)]).unwrap();
    let emptied = fake.borrow().watched.is_empty();
    scrobbled(&mut kernel, vec![movie(dune, true)]).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "POST /sync/history",
            "POST /sync/history/remove",
            "POST /sync/history"
        ]
    );
    assert_eq!(
        body(&kernel.sent()[1]),
        json!({ "movies": [{ "ids": { "tmdb": 438_631 } }], "episodes": [] })
    );
    assert!(emptied);
}

#[test]
fn a_failed_removal_still_keeps_the_play_added_beside_it() {
    let fake = shared(FakeSimkl::new());
    let answering = Rc::clone(&fake);
    let mut kernel =
        Kernel::new()
            .setting("client_id", CLIENT_ID)
            .store()
            .http(move |request, body| {
                if request.url.contains("/sync/history/remove") {
                    return Ok(nama_pdk::Response::new(500, ""));
                }
                Ok(answering.borrow_mut().answer(request, body))
            });
    let dune = &[("tmdb", "438631")];
    let reported_again = SyncedWatchState {
        changed_at_ms: EVENING_MS + 10_000,
        ..movie(dune, true)
    };

    let failed = scrobbled(
        &mut kernel,
        vec![movie(dune, true), movie(&[("tmdb", "329865")], false)],
    );
    scrobbled(&mut kernel, vec![reported_again]).unwrap();

    assert!(failed.is_err());
    assert_eq!(
        asked(&kernel),
        ["POST /sync/history", "POST /sync/history/remove"]
    );
}

#[test]
fn the_first_import_brings_everything_watched_and_later_ones_what_changed_since() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);
    {
        let mut fake = fake.borrow_mut();
        // Simkl writes some ids as strings and some as numbers.
        let dune = json!({ "simkl": 1, "tmdb": "438631", "imdb": "tt1160419" });
        fake.watch("movie", &dune, "2026-11-01T10:00:00Z");
        fake.watch(
            "episode",
            &json!({ "tvdb": 349_232 }),
            "2026-11-02T10:00:00Z",
        );
        fake.watch("movie", &json!({ "imdb": "tt3" }), "2026-11-03T10:00:00Z");
        fake.watch("episode", &json!({}), "2026-11-03T10:00:00Z");
    }
    let ids = |changes: &[SyncedWatchState]| -> Vec<(bool, Vec<String>, i64)> {
        (changes.iter())
            .map(|change| {
                let ids = change
                    .ids
                    .iter()
                    .map(|id| format!("{}-{}", id.namespace, id.value));
                let episode = change.kind == TitleRefKind::TITLE_REF_KIND_EPISODE;
                (episode, ids.collect(), change.changed_at_ms)
            })
            .collect()
    };

    let (first, cursor) = imported(&mut kernel, "").unwrap();
    let (unchanged, same_cursor) = imported(&mut kernel, &cursor).unwrap();
    fake.borrow_mut()
        .watch("movie", &json!({ "tmdb": 4 }), "2026-11-04T10:00:00Z");
    let (newer, newer_cursor) = imported(&mut kernel, &cursor).unwrap();

    assert_eq!(
        ids(&first),
        [
            (false, vec!["tmdb-438631".to_owned()], 1_793_527_200_000),
            (true, vec!["tvdb-349232".to_owned()], 1_793_613_600_000),
        ]
    );
    assert!(first.iter().all(|change| change.played));
    assert_eq!(cursor, "2026-11-03T10:00:00Z");
    assert_eq!(unchanged, []);
    assert_eq!(same_cursor, cursor);
    assert_eq!(
        ids(&newer),
        [(false, vec!["tmdb-4".to_owned()], 1_793_786_400_000)]
    );
    assert_eq!(newer_cursor, "2026-11-04T10:00:00Z");
    assert_eq!(
        asked(&kernel),
        [
            "GET /sync/activities",
            "GET /sync/all-items",
            "GET /sync/activities",
            "GET /sync/activities",
            "GET /sync/all-items",
        ]
    );
    let date_from = |sent: &Sent| sent.request.url.contains("date_from=2026-11-03T10:00:00Z");
    assert!(!date_from(&kernel.sent()[1]) && date_from(&kernel.sent()[4]));
}

#[test]
fn an_account_with_nothing_watched_imports_nothing() {
    let fake = shared(FakeSimkl::new());
    let mut kernel = kernel(&fake);

    let (changes, cursor) = imported(&mut kernel, "").unwrap();

    assert_eq!(changes, []);
    assert_eq!(cursor, "2026-11-01T00:00:00Z");
}

#[test]
fn a_rate_limited_ask_says_when_to_try_again() {
    let fake = shared(FakeSimkl::new());
    fake.borrow_mut().limiting = true;
    let mut kernel = kernel(&fake);

    let error = imported(&mut kernel, "").unwrap_err();

    assert_eq!(
        error.to_string(),
        "Simkl is limiting this Server's requests; try again in 30 s"
    );
}

#[test]
fn a_failure_names_its_status_but_never_a_token_or_the_body() {
    let mut kernel =
        Kernel::new()
            .setting("client_id", CLIENT_ID)
            .store()
            .http(|_request, _body| {
                Ok(nama_pdk::Response::new(
                    500,
                    r#"{"error":"bad token access-1"}"#,
                ))
            });

    let error = scrobbled(&mut kernel, vec![movie(&[("tmdb", "438631")], true)])
        .unwrap_err()
        .to_string();

    assert_eq!(error, "Simkl failed with HTTP status 500");
}

#[test]
fn without_the_store_a_scrobble_is_refused_rather_than_mark_each_report() {
    let mut kernel = Kernel::new().setting("client_id", CLIENT_ID);

    let error = scrobbled(&mut kernel, vec![movie(&[("tmdb", "438631")], true)]).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Simkl needs the Store, to tell a new play from the Server's reports"
    );
    assert_eq!(kernel.sent(), []);
}

#[test]
fn an_unlinked_user_is_refused_without_asking_simkl() {
    let mut kernel = Kernel::new();

    let error = with(&mut kernel, || {
        Simkl::import(ImportAsked {
            user_id: ADA,
            ..ImportAsked::default()
        })
    })
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "the User hasn't linked a Simkl account in Personal settings"
    );
}

#[test]
fn manifest_declares_the_app_and_each_users_token() {
    let manifest = manifest();
    let fields = |fields: &[nama_pdk::SettingField]| -> Vec<(String, Option<SettingKind>, bool)> {
        (fields.iter())
            .map(|field| (field.key.clone(), field.kind.as_known(), field.required))
            .collect()
    };
    let secret = Some(SettingKind::SETTING_KIND_SECRET);

    assert_eq!(
        fields(&manifest.settings),
        [("client_id".to_owned(), secret, true)]
    );
    assert_eq!(
        fields(&manifest.personal_settings),
        [("access_token".to_owned(), secret, false)]
    );
    assert_eq!(manifest.allowed_hosts, ["api.simkl.com"]);
    assert_eq!(
        manifest.points,
        [nama_pdk::point(nama_pdk::services::WATCH_SYNC)]
    );
}
