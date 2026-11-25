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
    Trakt,
    fake_trakt::{CLIENT_ID, CLIENT_SECRET, Code, DEVICE_CODE, FakeTrakt},
    manifest,
};

const ADA: i64 = 7;

/// 2026-11-20T20:00:00Z, when Ada finished watching.
const EVENING_MS: i64 = 1_795_204_800_000;

fn shared(fake: FakeTrakt) -> Rc<RefCell<FakeTrakt>> {
    Rc::new(RefCell::new(fake))
}

/// The Admin's app settings, the Store granted, and `fake` answering every request.
fn kernel(fake: &Rc<RefCell<FakeTrakt>>) -> Kernel {
    let fake = Rc::clone(fake);
    Kernel::new()
        .setting("client_id", CLIENT_ID)
        .setting("client_secret", CLIENT_SECRET)
        .store()
        .http(move |request, body| Ok(fake.borrow_mut().answer(request, body)))
}

/// The Personal settings linking `fake`'s account gave.
fn linked<M: FromIterator<(String, String)>>(fake: &Rc<RefCell<FakeTrakt>>) -> M {
    let fake = fake.borrow();
    tokens(&fake.access_token, &fake.refresh_token)
}

fn tokens<M: FromIterator<(String, String)>>(access_token: &str, refresh_token: &str) -> M {
    [
        ("access_token".to_owned(), access_token.to_owned()),
        ("refresh_token".to_owned(), refresh_token.to_owned()),
    ]
    .into_iter()
    .collect()
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

fn scrobbled(
    kernel: &mut Kernel,
    fake: &Rc<RefCell<FakeTrakt>>,
    changes: Vec<SyncedWatchState>,
) -> Result<(), Error> {
    let asked = ScrobbleAsked {
        personal_settings: linked(fake),
        user_id: ADA,
        changes,
        ..ScrobbleAsked::default()
    };
    with(kernel, || Trakt::scrobble(asked)).map(drop)
}

fn imported(
    kernel: &mut Kernel,
    fake: &Rc<RefCell<FakeTrakt>>,
    since: &str,
) -> Result<(Vec<SyncedWatchState>, String), Error> {
    let asked = ImportAsked {
        personal_settings: linked(fake),
        user_id: ADA,
        since: since.to_owned(),
        ..ImportAsked::default()
    };
    let imported = with(kernel, || Trakt::import(asked))?;
    Ok((imported.changes, imported.cursor))
}

/// Each request `kernel` sent, as `METHOD path`, its host and query left out.
fn asked(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let url = sent.request.url.trim_start_matches("https://api.trakt.tv");
            let path = url.split_once('?').map_or(url, |(path, _query)| path);
            format!("{} {path}", sent.request.method.as_deref().unwrap_or("GET"))
        })
        .collect()
}

fn polled(kernel: &mut Kernel) -> LinkPolled {
    let asked = PollLinkAsked {
        device_code: DEVICE_CODE.to_owned(),
        ..PollLinkAsked::default()
    };
    with(kernel, || Trakt::poll_link(asked)).unwrap()
}

fn body(sent: &Sent) -> Value {
    json::from_slice(&sent.body).unwrap()
}

#[test]
fn linking_gives_the_code_waits_for_the_user_then_hands_back_the_tokens() {
    let fake = shared(FakeTrakt::new());
    let mut kernel = kernel(&fake);
    let started = with(&mut kernel, || Trakt::start_link(StartLinkAsked::default())).unwrap();
    let waiting = polled(&mut kernel);
    fake.borrow_mut().code = Code::Entered;
    let linked = polled(&mut kernel);

    assert_eq!(
        (
            started.user_code.as_str(),
            started.verification_url.as_str(),
            started.device_code.as_str(),
            started.expires_in_seconds,
            started.interval_seconds,
        ),
        ("5055CC52", "https://trakt.tv/activate", DEVICE_CODE, 600, 5)
    );
    assert!(waiting.personal_settings.is_empty() && !waiting.expired);
    let linked: HashMap<_, _> = linked.personal_settings.into_iter().collect();
    assert_eq!(linked, tokens("access-1", "refresh-1"));
}

#[test]
fn an_expired_device_code_says_so() {
    let fake = shared(FakeTrakt::new());
    fake.borrow_mut().code = Code::Expired;
    let mut kernel = kernel(&fake);

    let expired = polled(&mut kernel);

    assert!(expired.expired && expired.personal_settings.is_empty());
}

#[test]
fn an_expired_access_token_is_renewed_and_the_ask_made_again() {
    let fake = shared(FakeTrakt::linked());
    let mut kernel = kernel(&fake);
    let personal: HashMap<String, String> = linked(&fake);
    fake.borrow_mut().expire_access_token();

    let import = ImportAsked {
        personal_settings: personal.into_iter().collect(),
        user_id: ADA,
        ..ImportAsked::default()
    };
    with(&mut kernel, || Trakt::import(import)).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "GET /sync/last_activities",
            "POST /oauth/token",
            "GET /sync/last_activities",
            "GET /sync/history",
        ]
    );
}

#[test]
fn a_refused_refresh_token_asks_the_user_to_link_again() {
    let fake = shared(FakeTrakt::linked());
    let mut kernel = kernel(&fake);
    let personal = linked(&fake);
    fake.borrow_mut().expire_access_token();
    fake.borrow_mut().refresh_token = "revoked".to_owned();

    let asked = ImportAsked {
        personal_settings: personal,
        user_id: ADA,
        ..ImportAsked::default()
    };
    let error = with(&mut kernel, || Trakt::import(asked)).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Trakt no longer takes this User's link; they link Trakt again in Personal settings"
    );
}

#[test]
fn a_played_movie_and_episode_go_to_the_history_as_of_when_they_were_played() {
    let fake = shared(FakeTrakt::linked());
    let mut kernel = kernel(&fake);

    scrobbled(
        &mut kernel,
        &fake,
        vec![
            movie(&[("tmdb", "438631"), ("imdb", "tt1160419")], true),
            episode(&[("tvdb", "349232"), ("tmdb", "63056")], true),
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
            "episodes": [
                { "ids": { "tmdb": 63056, "tvdb": 349_232 }, "watched_at": "2026-11-20T20:00:00Z" },
            ],
        })
    );
}

#[test]
fn position_reports_send_nothing_and_a_played_title_is_added_once_while_it_plays_on() {
    let fake = shared(FakeTrakt::linked());
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
        scrobbled(&mut kernel, &fake, changes).unwrap();
    }

    assert_eq!(asked(&kernel), ["POST /sync/history"]);
}

#[test]
fn a_title_marked_unplayed_leaves_the_history_and_a_later_play_is_added_again() {
    let fake = shared(FakeTrakt::linked());
    let mut kernel = kernel(&fake);
    let dune = &[("tmdb", "438631")];

    scrobbled(&mut kernel, &fake, vec![movie(dune, true)]).unwrap();
    scrobbled(&mut kernel, &fake, vec![movie(dune, false)]).unwrap();
    let emptied = fake.borrow().history.is_empty();
    scrobbled(&mut kernel, &fake, vec![movie(dune, true)]).unwrap();

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
    let fake = shared(FakeTrakt::linked());
    let answering = Rc::clone(&fake);
    let mut kernel = Kernel::new()
        .setting("client_id", CLIENT_ID)
        .setting("client_secret", CLIENT_SECRET)
        .store()
        .http(move |request, body| {
            if request.url.ends_with("/sync/history/remove") {
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
        &fake,
        vec![movie(dune, true), movie(&[("tmdb", "329865")], false)],
    );
    scrobbled(&mut kernel, &fake, vec![reported_again]).unwrap();

    assert!(failed.is_err());
    assert_eq!(
        asked(&kernel),
        ["POST /sync/history", "POST /sync/history/remove"]
    );
}

#[test]
fn the_first_import_brings_the_whole_history_page_by_page_and_later_ones_what_changed_since() {
    let fake = shared(FakeTrakt::linked());
    let mut kernel = kernel(&fake);
    {
        let mut fake = fake.borrow_mut();
        fake.page_size = 2;
        fake.watch(
            "movie",
            &json!({ "tmdb": 1, "imdb": "tt1" }),
            "2026-11-01T10:00:00.000Z",
        );
        fake.watch(
            "episode",
            &json!({ "tvdb": 2, "tmdb": null }),
            "2026-11-02T10:00:00.000Z",
        );
        fake.watch(
            "movie",
            &json!({ "imdb": "tt3" }),
            "2026-11-03T10:00:00.000Z",
        );
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

    let (first, cursor) = imported(&mut kernel, &fake, "").unwrap();
    let (unchanged, same_cursor) = imported(&mut kernel, &fake, &cursor).unwrap();
    fake.borrow_mut()
        .watch("movie", &json!({ "tmdb": 4 }), "2026-11-04T10:00:00.000Z");
    let (newer, newer_cursor) = imported(&mut kernel, &fake, &cursor).unwrap();

    assert_eq!(
        ids(&first),
        [
            (true, vec!["tvdb-2".to_owned()], 1_793_613_600_000),
            (false, vec!["tmdb-1".to_owned()], 1_793_527_200_000),
        ]
    );
    assert!(first.iter().all(|change| change.played));
    assert_eq!(cursor, "2026-11-03T10:00:00.000Z");
    assert_eq!(unchanged, []);
    assert_eq!(same_cursor, cursor);
    assert_eq!(
        ids(&newer),
        [(false, vec!["tmdb-4".to_owned()], 1_793_786_400_000)]
    );
    assert_eq!(newer_cursor, "2026-11-04T10:00:00.000Z");
    let history_asks: Vec<_> = (kernel.sent().iter())
        .map(|sent| sent.request.url.as_str())
        .filter(|url| url.contains("/sync/history"))
        .collect();
    assert_eq!(
        history_asks,
        [
            "https://api.trakt.tv/sync/history?page=1&limit=100",
            "https://api.trakt.tv/sync/history?page=2&limit=100",
            "https://api.trakt.tv/sync/history?page=1&limit=100&start_at=2026-11-03T10:00:00Z",
        ]
    );
}

#[test]
fn a_rate_limited_ask_says_when_to_try_again() {
    let fake = shared(FakeTrakt::linked());
    fake.borrow_mut().limiting = true;
    let mut kernel = kernel(&fake);

    let error = imported(&mut kernel, &fake, "").unwrap_err();

    assert_eq!(
        error.to_string(),
        "Trakt is limiting this Server's requests; try again in 30 s"
    );
}

#[test]
fn a_failure_names_its_status_but_never_a_token_or_the_body() {
    let mut kernel = Kernel::new()
        .setting("client_id", CLIENT_ID)
        .setting("client_secret", CLIENT_SECRET)
        .store()
        .http(|_request, _body| {
            Ok(nama_pdk::Response::new(
                500,
                r#"{"error":"bad token access-1 for refresh-1"}"#,
            ))
        });

    let error = with(&mut kernel, || {
        Trakt::scrobble(ScrobbleAsked {
            personal_settings: tokens("access-1", "refresh-1"),
            user_id: ADA,
            changes: vec![movie(&[("tmdb", "438631")], true)],
            ..ScrobbleAsked::default()
        })
    })
    .unwrap_err()
    .to_string();

    assert_eq!(error, "Trakt failed with HTTP status 500");
}

#[test]
fn without_the_store_a_scrobble_is_refused_rather_than_add_a_play_each_report() {
    let fake = shared(FakeTrakt::linked());
    let mut kernel = Kernel::new()
        .setting("client_id", CLIENT_ID)
        .setting("client_secret", CLIENT_SECRET);

    let error =
        scrobbled(&mut kernel, &fake, vec![movie(&[("tmdb", "438631")], true)]).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Trakt needs the Store, to tell a new play from the Server's reports"
    );
    assert_eq!(kernel.sent(), []);
}

#[test]
fn an_unlinked_user_is_refused_without_asking_trakt() {
    let mut kernel = Kernel::new();

    let error = with(&mut kernel, || {
        Trakt::import(ImportAsked {
            user_id: ADA,
            ..ImportAsked::default()
        })
    })
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "the User hasn't linked a Trakt account in Personal settings"
    );
}

#[test]
fn manifest_declares_the_app_secrets_and_each_users_tokens() {
    let manifest = manifest();
    let fields = |fields: &[nama_pdk::SettingField]| -> Vec<(String, Option<SettingKind>, bool)> {
        (fields.iter())
            .map(|field| (field.key.clone(), field.kind.as_known(), field.required))
            .collect()
    };
    let secret = Some(SettingKind::SETTING_KIND_SECRET);

    assert_eq!(
        fields(&manifest.settings),
        [
            ("client_id".to_owned(), secret, true),
            ("client_secret".to_owned(), secret, true),
        ]
    );
    assert_eq!(
        fields(&manifest.personal_settings),
        [
            ("access_token".to_owned(), secret, false),
            ("refresh_token".to_owned(), secret, false),
        ]
    );
    assert_eq!(manifest.allowed_hosts, ["api.trakt.tv"]);
}
