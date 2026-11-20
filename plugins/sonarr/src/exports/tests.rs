//! Sonarr's conversation, through `nama_pdk::testing`: each test scripts Sonarr's answers in the
//! order the Plugin asks, then reads back what it asked. What Sonarr shares with Radarr, the
//! settings check and a service that is down or refuses the key, Radarr's tests cover, but for
//! progress, which is tested here.

use std::{collections::HashMap, time::Duration};

use nama_pdk::{
    Download, Fulfiller, Request, Response, StopMonitoring, TitleId, TryAgainIn,
    extism_pdk::{
        Error,
        json::{self, Value, json},
    },
    nama_plugin_proto::nama::plugin::v1::{
        DownloadStatus, EpisodeNumber, EpisodeNumbers, RequestedSeason, ShowDeleted,
        show_deleted::Deleted,
    },
    testing::{Kernel, with},
};

use super::{SonarrFulfiller, needed_tvdb_id};

const DARK_TVDB: i64 = 334_824;
/// Sonarr's own id for Dark, once it has it.
const DARK_SONARR_ID: i64 = 7;

/// Sonarr's settings: the profile HD-1080p and the root folder `/tv`.
fn settings() -> Kernel {
    Kernel::new()
        .setting("url", "http://sonarr.test")
        .setting("api_key", "key")
        .setting("quality_profile", "4")
        .setting("root_folder", "/tv")
}

/// Sonarr, answering each request with the next of `answers`, a status and a JSON body.
fn sonarr(answers: Vec<(u16, Value)>) -> Kernel {
    let mut answers = answers.into_iter();
    settings().http(move |_request, _body| {
        let (status, body) = answers
            .next()
            .expect("Sonarr is asked no more than scripted");
        Ok(Response::new(status, body.to_string()))
    })
}

/// Sonarr while it is down: nothing answers.
fn sonarr_down() -> Kernel {
    settings().http(|_request, _body| Err(Error::msg("connection refused")))
}

/// What the Plugin asked Sonarr, as `<method> <API path>`.
fn asked(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let method = sent.request.method.as_deref().unwrap_or("GET");
            let path = (sent.request.url).trim_start_matches("http://sonarr.test/api/v3/");
            format!("{method} {path}")
        })
        .collect()
}

/// The body of the one request the Plugin sent as `request`, `<method> <API path>`.
fn body_of(kernel: &Kernel, request: &str) -> Value {
    let asked = asked(kernel);
    let [sent] = (kernel.sent().iter())
        .zip(&asked)
        .filter(|(_, asked)| *asked == request)
        .map(|(sent, _)| sent)
        .collect::<Vec<_>>()[..]
    else {
        panic!("not one {request} in {asked:?}");
    };
    json::from_slice(&sent.body).unwrap()
}

fn fixture(text: &str) -> (u16, Value) {
    (200, json::from_str(text).unwrap())
}

fn profiles() -> (u16, Value) {
    fixture(include_str!("../../fixtures/qualityprofile.json"))
}

/// Sonarr has no Show by the id asked, or finds none by it.
fn none() -> (u16, Value) {
    (200, json!([]))
}

/// Dark as Sonarr's lookup finds it, while Sonarr doesn't have it.
fn dark_looked_up() -> (u16, Value) {
    fixture(include_str!("../../fixtures/series-lookup.json"))
}

/// Dark as Sonarr has it, at the profile Any, monitoring Seasons 1 and 3.
fn dark_found() -> (u16, Value) {
    let seasons = json!([
        {"seasonNumber": 0, "monitored": false},
        {"seasonNumber": 1, "monitored": true},
        {"seasonNumber": 2, "monitored": false},
        {"seasonNumber": 3, "monitored": true}
    ]);
    (
        200,
        json!([{"id": DARK_SONARR_ID, "qualityProfileId": 1, "seasons": seasons}]),
    )
}

/// Sonarr refreshing no Show.
fn no_commands() -> (u16, Value) {
    (200, json!([]))
}

/// Sonarr taking a change.
fn accepted() -> (u16, Value) {
    (202, json!({}))
}

fn id(namespace: &str, value: &str) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

/// An Approved Request for the Season `season_number` of the Show `show_ids`.
fn season_of(show_ids: Vec<TitleId>, season_number: u32) -> Request {
    Request {
        movie_or_season: RequestedSeason {
            season_number,
            show_ids,
            ..RequestedSeason::default()
        }
        .into(),
        ..Request::default()
    }
}

/// An Approved Request for the Season `season_number` of Dark.
fn dark_season(season_number: u32) -> Request {
    season_of(vec![id("tvdb", &DARK_TVDB.to_string())], season_number)
}

/// [`dark_season`], asked for as the whole Show, which Sonarr then follows.
fn followed_dark_season(season_number: u32) -> Request {
    Request {
        followed_show_id: Some(1),
        ..dark_season(season_number)
    }
}

/// Sonarr told `deleted` of Dark was deleted.
fn dark_deleted(deleted: impl Into<Deleted>) -> StopMonitoring {
    StopMonitoring {
        title: ShowDeleted {
            show_ids: vec![id("tvdb", &DARK_TVDB.to_string())],
            deleted: Some(deleted.into()),
            ..ShowDeleted::default()
        }
        .into(),
        ..StopMonitoring::default()
    }
}

/// Dark's Episodes `numbers`, each a Season and Episode number.
fn dark_episodes(numbers: &[(u32, u32)]) -> EpisodeNumbers {
    let episodes = (numbers.iter())
        .map(|&(season_number, episode_number)| EpisodeNumber {
            season_number,
            episode_number,
            ..EpisodeNumber::default()
        })
        .collect();
    EpisodeNumbers {
        episodes,
        ..EpisodeNumbers::default()
    }
}

/// Dark's Episodes as Sonarr lists them, two per Season.
fn dark_episodes_listed() -> (u16, Value) {
    fixture(include_str!("../../fixtures/episode.json"))
}

#[test]
fn a_season_of_a_show_sonarr_lacks_adds_it_with_only_that_season_monitored_and_searched_for() {
    let mut kernel = sonarr(vec![profiles(), none(), dark_looked_up(), (201, json!({}))]);

    let fulfilled = with(&mut kernel, || SonarrFulfiller::fulfill(dark_season(1))).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "GET qualityprofile",
            "GET series?tvdbId=334824",
            "GET series/lookup?term=tvdb:334824",
            "POST series",
        ]
    );
    assert_eq!(
        body_of(&kernel, "POST series"),
        json!({
            "tvdbId": DARK_TVDB,
            "title": "Dark",
            "qualityProfileId": 4,
            "rootFolderPath": "/tv",
            "monitored": true,
            "monitorNewItems": "none",
            "seasonFolder": true,
            "seasons": [
                {"seasonNumber": 0, "monitored": false},
                {"seasonNumber": 1, "monitored": true},
                {"seasonNumber": 2, "monitored": false},
                {"seasonNumber": 3, "monitored": false}
            ],
            "addOptions": {"monitor": "skip", "searchForMissingEpisodes": true},
        })
    );
    let reported = (
        fulfilled.reference.as_str(),
        fulfilled.profile_name.as_str(),
    );
    assert_eq!(reported, ("334824/1", "HD-1080p"));
}

#[test]
fn a_whole_show_sonarr_lacks_is_added_with_its_new_seasons_monitored() {
    let mut kernel = sonarr(vec![profiles(), none(), dark_looked_up(), (201, json!({}))]);

    with(&mut kernel, || {
        SonarrFulfiller::fulfill(followed_dark_season(1))
    })
    .unwrap();

    assert_eq!(body_of(&kernel, "POST series")["monitorNewItems"], "all");
}

#[test]
fn a_later_season_is_monitored_and_searched_for_alone_and_reports_the_profile_sonarr_kept() {
    let season_2 = json!([
        {"id": 201, "seasonNumber": 2, "episodeNumber": 1},
        {"id": 202, "seasonNumber": 2, "episodeNumber": 2}
    ]);
    let mut kernel = sonarr(vec![
        profiles(),
        dark_found(),
        no_commands(),
        accepted(),
        (200, season_2),
        accepted(),
        (201, json!({})),
    ]);

    let fulfilled = with(&mut kernel, || SonarrFulfiller::fulfill(dark_season(2))).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "GET qualityprofile",
            "GET series?tvdbId=334824",
            "GET command",
            "POST seasonpass",
            "GET episode?seriesId=7&seasonNumber=2",
            "PUT episode/monitor",
            "POST command",
        ]
    );
    assert_eq!(
        body_of(&kernel, "POST seasonpass"),
        json!({"series": [{"id": DARK_SONARR_ID, "monitored": true, "seasons": [
            {"seasonNumber": 2, "monitored": true}
        ]}]})
    );
    assert_eq!(
        body_of(&kernel, "PUT episode/monitor"),
        json!({"episodeIds": [201, 202], "monitored": true})
    );
    assert_eq!(
        body_of(&kernel, "POST command"),
        json!({"name": "SeasonSearch", "seriesId": DARK_SONARR_ID, "seasonNumber": 2})
    );
    let reported = (
        fulfilled.profile_id.as_str(),
        fulfilled.profile_name.as_str(),
    );
    assert_eq!(reported, ("1", "Any"));
}

#[test]
fn a_whole_show_sonarr_has_gets_its_new_seasons_monitored() {
    let mut kernel = sonarr(vec![
        profiles(),
        dark_found(),
        no_commands(),
        accepted(),
        accepted(),
        (200, json!([])),
        accepted(),
        (201, json!({})),
    ]);

    with(&mut kernel, || {
        SonarrFulfiller::fulfill(followed_dark_season(2))
    })
    .unwrap();

    assert_eq!(
        asked(&kernel)[3..5],
        ["PUT series/editor", "POST seasonpass"]
    );
    assert_eq!(
        body_of(&kernel, "PUT series/editor"),
        json!({"seriesIds": [DARK_SONARR_ID], "monitorNewItems": "all"})
    );
}

#[test]
fn a_show_another_hand_off_added_meanwhile_gets_the_season_monitored_at_its_profile() {
    let lookup = json!([{"tvdbId": DARK_TVDB, "title": "Dark", "seasons": [
        {"seasonNumber": 1, "monitored": false},
        {"seasonNumber": 3, "monitored": false}
    ]}]);
    let exists = fixture(include_str!("../../fixtures/series-exists.json"));
    let episodes = json!([{"id": 301, "seasonNumber": 3, "episodeNumber": 1}]);
    let mut kernel = sonarr(vec![
        profiles(),
        none(),
        (200, lookup),
        (400, exists.1),
        dark_found(),
        no_commands(),
        accepted(),
        (200, episodes),
        accepted(),
        (201, json!({})),
    ]);

    let fulfilled = with(&mut kernel, || SonarrFulfiller::fulfill(dark_season(3))).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "GET qualityprofile",
            "GET series?tvdbId=334824",
            "GET series/lookup?term=tvdb:334824",
            "POST series",
            "GET series?tvdbId=334824",
            "GET command",
            "POST seasonpass",
            "GET episode?seriesId=7&seasonNumber=3",
            "PUT episode/monitor",
            "POST command",
        ]
    );
    assert_eq!(
        body_of(&kernel, "POST seasonpass"),
        json!({"series": [{"id": DARK_SONARR_ID, "monitored": true, "seasons": [
            {"seasonNumber": 3, "monitored": true}
        ]}]})
    );
    let reported = (
        fulfilled.reference.as_str(),
        fulfilled.profile_name.as_str(),
    );
    assert_eq!(reported, ("334824/3", "Any"));
}

#[test]
fn a_season_of_a_show_sonarr_is_refreshing_is_tried_again_in_5_seconds_changing_nothing() {
    let refreshing = json!([{"name": "RefreshSeries", "status": "started",
        "body": {"seriesIds": [DARK_SONARR_ID]}}]);
    let mut kernel = sonarr(vec![profiles(), dark_found(), (200, refreshing)]);

    let error = with(&mut kernel, || SonarrFulfiller::fulfill(dark_season(3))).unwrap_err();

    assert_eq!(
        error.downcast_ref::<TryAgainIn>(),
        Some(&TryAgainIn(Duration::from_secs(5)))
    );
    assert_eq!(
        asked(&kernel),
        [
            "GET qualityprofile",
            "GET series?tvdbId=334824",
            "GET command"
        ]
    );
}

#[test]
fn a_show_sonarr_cant_find_fails_in_plain_words() {
    let mut kernel = sonarr(vec![profiles(), none(), none()]);

    let error = with(&mut kernel, || SonarrFulfiller::fulfill(dark_season(1))).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Sonarr couldn't find the Show by its TVDB id, 334824"
    );
}

#[test]
fn a_show_without_a_tvdb_id_is_refused_without_asking_sonarr() {
    let mut kernel = settings();
    let unlisted = season_of(vec![id("tmdb", "1396")], 1);

    let error = with(&mut kernel, || SonarrFulfiller::fulfill(unlisted)).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Sonarr needs the Show's TVDB id, and none is known for it"
    );
    assert_eq!(kernel.sent(), []);
}

#[test]
fn cancelling_a_followed_show_turns_off_sonarrs_new_season_monitoring() {
    let mut kernel = sonarr(vec![dark_found(), no_commands(), accepted()]);

    with(&mut kernel, || {
        SonarrFulfiller::unfollow(followed_dark_season(1))
    })
    .unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "GET series?tvdbId=334824",
            "GET command",
            "PUT series/editor"
        ]
    );
    assert_eq!(
        body_of(&kernel, "PUT series/editor"),
        json!({"seriesIds": [DARK_SONARR_ID], "monitorNewItems": "none"})
    );
}

/// What Sonarr's queue in `queue.json` holds for Seasons 1 and 3 of Dark.
fn downloads_of_seasons_1_and_3(mut downloads: HashMap<String, Vec<Download>>) {
    let season_1_pack = Download {
        title: "Dark.S01.1080p.NF.WEB-DL.x264".to_owned(),
        percent: 25.0,
        size_bytes: Some(20_000_000_000),
        eta_seconds: Some(93_600),
        status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
        ..Download::default()
    };
    let season_3_episodes = vec![
        Download {
            title: "Dark.S03E01.1080p.NF.WEB-DL.x264".to_owned(),
            percent: 75.0,
            size_bytes: Some(2_000_000_000),
            eta_seconds: Some(600),
            status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
            ..Download::default()
        },
        Download {
            title: "Dark.S03E02.1080p.NF.WEB-DL.x264".to_owned(),
            percent: 100.0,
            size_bytes: Some(2_000_000_000),
            status: DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
            attention_reason: Some(
                "Sonarr said: Not an upgrade for existing episode file(s)".to_owned(),
            ),
            ..Download::default()
        },
    ];
    assert_eq!(downloads.remove("334824/1"), Some(vec![season_1_pack]));
    assert_eq!(downloads.remove("334824/3"), Some(season_3_episodes));
    // Season 2 is queued too, but wasn't asked about.
    assert!(downloads.is_empty(), "{downloads:?}");
}

#[test]
fn progress_reads_sonarrs_queue_once_for_every_season_asked_about() {
    let queue = fixture(include_str!("../../fixtures/queue.json"));
    let mut kernel = sonarr(vec![queue]);
    let references = vec!["334824/1".to_owned(), "334824/3".to_owned()];

    let downloads = with(&mut kernel, || SonarrFulfiller::progress(references)).unwrap();

    assert_eq!(
        asked(&kernel),
        ["GET queue?pageSize=1000&includeSeries=true"]
    );
    downloads_of_seasons_1_and_3(downloads);
}

#[test]
fn progress_with_sonarr_refusing_the_key_or_down_fails_in_plain_words() {
    let progress = || SonarrFulfiller::progress(vec!["334824/1".to_owned()]);

    let refused = with(&mut sonarr(vec![(401, Value::Null)]), progress);
    let down = with(&mut sonarr_down(), progress);

    assert_eq!(
        [refused, down].map(|failed| failed.unwrap_err().to_string()),
        ["Sonarr refused the API key", "Sonarr didn't answer"]
    );
}

#[test]
fn deleting_episodes_has_sonarr_unmonitor_those_episodes_only() {
    let mut kernel = sonarr(vec![
        dark_found(),
        no_commands(),
        dark_episodes_listed(),
        accepted(),
    ]);
    let secrets_and_beginnings = dark_episodes(&[(1, 1), (2, 1)]);

    with(&mut kernel, || {
        SonarrFulfiller::stop_monitoring(dark_deleted(secrets_and_beginnings))
    })
    .unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "GET series?tvdbId=334824",
            "GET command",
            "GET episode?seriesId=7",
            "PUT episode/monitor",
        ]
    );
    assert_eq!(
        body_of(&kernel, "PUT episode/monitor"),
        json!({"episodeIds": [101, 201], "monitored": false})
    );
}

#[test]
fn deleting_a_season_has_sonarr_unmonitor_it_and_its_episodes_and_leave_the_others() {
    let mut kernel = sonarr(vec![
        dark_found(),
        no_commands(),
        dark_episodes_listed(),
        accepted(),
        accepted(),
    ]);

    with(&mut kernel, || {
        SonarrFulfiller::stop_monitoring(dark_deleted(Deleted::SeasonNumber(1)))
    })
    .unwrap();

    assert_eq!(
        body_of(&kernel, "POST seasonpass"),
        json!({"series": [{"id": DARK_SONARR_ID, "seasons": [
            {"seasonNumber": 1, "monitored": false}
        ]}]})
    );
    assert_eq!(
        body_of(&kernel, "PUT episode/monitor"),
        json!({"episodeIds": [101, 102], "monitored": false})
    );
}

#[test]
fn deleting_a_show_has_sonarr_unmonitor_every_season_and_keep_following_new_ones() {
    let mut kernel = sonarr(vec![
        dark_found(),
        no_commands(),
        dark_episodes_listed(),
        accepted(),
        accepted(),
    ]);

    with(&mut kernel, || {
        SonarrFulfiller::stop_monitoring(dark_deleted(Deleted::EveryCurrentSeason(true)))
    })
    .unwrap();

    // Neither the Show's monitoring nor its new Seasons' (`PUT series/editor`) change.
    assert_eq!(
        asked(&kernel),
        [
            "GET series?tvdbId=334824",
            "GET command",
            "GET episode?seriesId=7",
            "POST seasonpass",
            "PUT episode/monitor",
        ]
    );
    let unmonitored =
        [0, 1, 2, 3].map(|number| json!({"seasonNumber": number, "monitored": false}));
    assert_eq!(
        body_of(&kernel, "POST seasonpass"),
        json!({"series": [{"id": DARK_SONARR_ID, "seasons": unmonitored}]})
    );
    assert_eq!(
        body_of(&kernel, "PUT episode/monitor"),
        json!({"episodeIds": [101, 102, 201, 202, 301, 302], "monitored": false})
    );
}

#[test]
fn deleting_a_show_sonarr_doesnt_have_changes_nothing() {
    let mut kernel = sonarr(vec![none()]);

    with(&mut kernel, || {
        SonarrFulfiller::stop_monitoring(dark_deleted(Deleted::EveryCurrentSeason(true)))
    })
    .unwrap();

    assert_eq!(asked(&kernel), ["GET series?tvdbId=334824"]);
}

#[test]
fn a_show_s_tvdb_id_is_its_number() {
    let ids = [id("tmdb", "1396"), id("tvdb", "81189")];

    assert_eq!(needed_tvdb_id(&ids).unwrap(), 81189);
}

#[test]
fn a_show_whose_tvdb_id_is_not_a_number_is_refused() {
    let error = needed_tvdb_id(&[id("tvdb", "breaking-bad")]).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Sonarr needs the Show's TVDB id, and none is known for it"
    );
}
