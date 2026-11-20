//! Radarr's conversation, through `nama_pdk::testing`: each test scripts Radarr's answers in the
//! order the Plugin asks, then reads back what it asked. What Radarr and Sonarr share, the
//! settings check and a Radarr that is down or refuses the key, is tested here once.

use nama_pdk::{
    Fulfiller, Request, Response, StopMonitoring, TitleId,
    extism_pdk::{
        Error,
        json::{self, Value, json},
    },
    nama_plugin_proto::nama::plugin::v1::{
        CheckedSettings, Download, DownloadStatus, MovieDeleted, RequestedMovie, RequestedQuality,
    },
    testing::{Kernel, with},
};

use super::{RadarrFulfiller, radarr, tmdb_id};

const HEAT: i64 = 949;
const ALIEN: i64 = 348;
const DUNE: i64 = 438_631;
/// Radarr's own id for Alien, which it has.
const ALIEN_RADARR_ID: i64 = 2;
const API_KEY: &str = "radarr-key";

/// Radarr's settings: the profile HD-1080p and the root folder `/movies`.
fn settings() -> Kernel {
    Kernel::new()
        .setting("url", "http://radarr.test/")
        .setting("api_key", API_KEY)
        .setting("quality_profile", "4")
        .setting("root_folder", "/movies")
}

/// Radarr, answering each request with the next of `answers`, a status and a JSON body.
fn radarr_answering(answers: Vec<(u16, Value)>) -> Kernel {
    let mut answers = answers.into_iter();
    settings().http(move |_request, _body| {
        let (status, body) = answers
            .next()
            .expect("Radarr is asked no more than scripted");
        Ok(Response::new(status, body.to_string()))
    })
}

/// Radarr while it is down: nothing answers.
fn radarr_down() -> Kernel {
    settings().http(|_request, _body| Err(Error::msg("connection refused")))
}

/// What the Plugin asked Radarr, as `<method> <API path>`.
fn asked(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let method = sent.request.method.as_deref().unwrap_or("GET");
            let path = (sent.request.url).trim_start_matches("http://radarr.test/api/v3/");
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

/// Radarr's quality profiles once an Admin removed HD-1080p.
fn profiles_without_hd() -> (u16, Value) {
    (
        200,
        json!([{"id": 1, "name": "Any"}, {"id": 5, "name": "Ultra-HD"}]),
    )
}

/// Alien as Radarr has it, at Ultra-HD.
fn alien_found() -> (u16, Value) {
    fixture(include_str!("../../fixtures/movie-alien.json"))
}

fn id(namespace: &str, value: &str) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

/// An Approved Request for the Movie `title`, whose TMDB id is `tmdb_id`, at no Quality.
fn movie(tmdb_id: i64, title: &str) -> Request {
    Request {
        movie_or_season: RequestedMovie {
            title: title.to_owned(),
            ids: vec![id("imdb", "tt0000001"), id("tmdb", &tmdb_id.to_string())],
            ..RequestedMovie::default()
        }
        .into(),
        ..Request::default()
    }
}

/// `request` at the Quality `name`, of the Radarr profile `profile_id`.
fn at_quality(request: Request, name: &str, profile_id: &str) -> Request {
    Request {
        quality: RequestedQuality {
            name: name.to_owned(),
            profile_id: profile_id.to_owned(),
            ..RequestedQuality::default()
        }
        .into(),
        ..request
    }
}

/// Radarr told the Movie `tmdb_id` was deleted.
fn deleted(tmdb_id: i64) -> StopMonitoring {
    StopMonitoring {
        title: MovieDeleted {
            ids: vec![id("tmdb", &tmdb_id.to_string())],
            ..MovieDeleted::default()
        }
        .into(),
        ..StopMonitoring::default()
    }
}

fn check_settings(kernel: &mut Kernel) -> Result<CheckedSettings, Error> {
    with(kernel, || radarr()?.checked_settings())
}

/// The `(value, label)` of each choice `checked` offers for the setting `key`.
fn choices<'a>(checked: &'a CheckedSettings, key: &str) -> Vec<(&'a str, &'a str)> {
    (checked.choices[key].choices.iter())
        .map(|choice| (choice.value.as_str(), choice.label.as_str()))
        .collect()
}

#[test]
fn the_settings_check_offers_radarrs_quality_profiles_and_root_folders() {
    let folders = fixture(include_str!("../../fixtures/rootfolder.json"));
    let mut kernel = radarr_answering(vec![profiles(), folders]);

    let checked = check_settings(&mut kernel).unwrap();

    assert_eq!(asked(&kernel), ["GET qualityprofile", "GET rootfolder"]);
    let keys: Vec<_> = (kernel.sent().iter())
        .map(|sent| sent.request.headers.get("X-Api-Key").map(String::as_str))
        .collect();
    assert_eq!(keys, [Some(API_KEY); 2]);
    assert_eq!(
        choices(&checked, "quality_profile"),
        [("1", "Any"), ("4", "HD-1080p"), ("5", "Ultra-HD")]
    );
    assert_eq!(
        choices(&checked, "root_folder"),
        [("/movies", "/movies"), ("/movies-4k", "/movies-4k")]
    );
}

// The Plugin page flags a Quality whose profile the check stops offering.
#[test]
fn the_settings_check_stops_offering_a_profile_removed_from_radarr() {
    let mut kernel = radarr_answering(vec![profiles_without_hd(), (200, json!([]))]);

    let checked = check_settings(&mut kernel).unwrap();

    assert_eq!(
        choices(&checked, "quality_profile"),
        [("1", "Any"), ("5", "Ultra-HD")]
    );
}

#[test]
fn the_settings_check_with_a_wrong_key_or_radarr_down_fails_in_plain_words() {
    let wrong_key = check_settings(&mut radarr_answering(vec![(401, Value::Null)]));
    let down = check_settings(&mut radarr_down());

    assert_eq!(
        [wrong_key, down].map(|failed| failed.unwrap_err().to_string()),
        ["Radarr refused the API key", "Radarr didn't answer"]
    );
}

#[test]
fn a_movie_is_added_at_the_settings_profile_and_folder_and_searched_for() {
    let added = fixture(include_str!("../../fixtures/movie.json"));
    let mut kernel = radarr_answering(vec![profiles(), (201, added.1)]);

    let fulfilled = with(&mut kernel, || {
        RadarrFulfiller::fulfill(movie(HEAT, "Heat"))
    })
    .unwrap();

    assert_eq!(asked(&kernel), ["GET qualityprofile", "POST movie"]);
    assert_eq!(
        body_of(&kernel, "POST movie"),
        json!({
            "tmdbId": HEAT,
            "title": "Heat",
            "qualityProfileId": 4,
            "rootFolderPath": "/movies",
            "monitored": true,
            "minimumAvailability": "released",
            "addOptions": {"searchForMovie": true},
        })
    );
    let reported = (
        fulfilled.reference.as_str(),
        fulfilled.profile_name.as_str(),
    );
    assert_eq!(reported, ("949", "HD-1080p"));
}

#[test]
fn a_movie_is_added_at_its_qualitys_profile() {
    let added = fixture(include_str!("../../fixtures/movie.json"));
    let mut kernel = radarr_answering(vec![profiles(), (201, added.1)]);
    let heat_in_4k = at_quality(movie(HEAT, "Heat"), "4K", "5");

    let fulfilled = with(&mut kernel, || RadarrFulfiller::fulfill(heat_in_4k)).unwrap();

    assert_eq!(body_of(&kernel, "POST movie")["qualityProfileId"], 5);
    assert_eq!(fulfilled.profile_name, "Ultra-HD");
}

#[test]
fn a_movie_radarr_already_has_reports_the_profile_radarr_has_it_at() {
    let exists = fixture(include_str!("../../fixtures/movie-exists.json"));
    let mut kernel = radarr_answering(vec![profiles(), (400, exists.1), alien_found()]);

    let fulfilled = with(&mut kernel, || {
        RadarrFulfiller::fulfill(movie(ALIEN, "Alien"))
    })
    .unwrap();

    assert_eq!(
        asked(&kernel),
        ["GET qualityprofile", "POST movie", "GET movie?tmdbId=348"]
    );
    let reported = (
        fulfilled.profile_id.as_str(),
        fulfilled.profile_name.as_str(),
    );
    assert_eq!(reported, ("5", "Ultra-HD"));
}

#[test]
fn a_movie_radarr_cant_find_fails_with_radarrs_reason() {
    let not_found = fixture(include_str!("../../fixtures/movie-not-found.json"));
    let mut kernel = radarr_answering(vec![profiles(), (400, not_found.1)]);

    let failed = with(&mut kernel, || {
        RadarrFulfiller::fulfill(movie(DUNE, "Dune"))
    })
    .unwrap_err();

    assert_eq!(
        failed.to_string(),
        "Radarr said: A movie with this ID was not found."
    );
}

#[test]
fn a_movie_at_a_profile_removed_from_radarr_fails_naming_it_and_isnt_added() {
    let mut kernel = radarr_answering(vec![profiles_without_hd(), (200, json!([]))]);
    let heat_in_hd = at_quality(movie(HEAT, "Heat"), "HD", "4");

    let failed = with(&mut kernel, || RadarrFulfiller::fulfill(heat_in_hd)).unwrap_err();

    assert_eq!(
        failed.to_string(),
        "Radarr has no quality profile with the id 4, which HD uses"
    );
    assert_eq!(
        asked(&kernel),
        ["GET qualityprofile", "GET movie?tmdbId=949"]
    );
}

#[test]
fn a_movie_radarr_already_has_is_handed_off_at_its_own_profile_though_the_asked_one_was_removed() {
    let mut kernel = radarr_answering(vec![profiles_without_hd(), alien_found()]);
    let alien_in_hd = at_quality(movie(ALIEN, "Alien"), "HD", "4");

    let fulfilled = with(&mut kernel, || RadarrFulfiller::fulfill(alien_in_hd)).unwrap();

    assert_eq!(
        asked(&kernel),
        ["GET qualityprofile", "GET movie?tmdbId=348"]
    );
    let reported = (
        fulfilled.profile_id.as_str(),
        fulfilled.profile_name.as_str(),
    );
    assert_eq!(reported, ("5", "Ultra-HD"));
}

#[test]
fn a_hand_off_with_radarr_down_fails_in_plain_words() {
    let failed = with(&mut radarr_down(), || {
        RadarrFulfiller::fulfill(movie(HEAT, "Heat"))
    });

    assert_eq!(failed.unwrap_err().to_string(), "Radarr didn't answer");
}

#[test]
fn progress_reads_radarrs_queue_once_for_every_movie_asked_about() {
    let queue = fixture(include_str!("../../fixtures/queue.json"));
    let mut kernel = radarr_answering(vec![queue]);
    let references = vec![HEAT.to_string(), ALIEN.to_string()];

    let mut downloads = with(&mut kernel, || RadarrFulfiller::progress(references)).unwrap();

    assert_eq!(
        asked(&kernel),
        ["GET queue?pageSize=1000&includeMovie=true"]
    );
    let heat = Download {
        title: "Heat.1995.1080p.BluRay.x264".to_owned(),
        percent: 25.0,
        size_bytes: Some(8_000_000_000),
        eta_seconds: Some(93_600),
        status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
        ..Download::default()
    };
    let alien_blocked = Download {
        title: "Alien.1979.Directors.Cut.1080p".to_owned(),
        percent: 100.0,
        size_bytes: Some(4_000_000_000),
        status: DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
        attention_reason: Some(
            "Radarr said: Found matching movie via grab history, but release was matched to \
             movie by ID. Automatic import is not possible."
                .to_owned(),
        ),
        ..Download::default()
    };
    assert_eq!(downloads.remove("949"), Some(vec![heat]));
    assert_eq!(downloads.remove("348"), Some(vec![alien_blocked]));
    // The Matrix is queued too, but wasn't asked about.
    assert!(downloads.is_empty(), "{downloads:?}");
}

#[test]
fn deleting_a_movie_has_radarr_unmonitor_it_and_keep_it() {
    let mut kernel = radarr_answering(vec![alien_found(), (202, json!([]))]);

    with(&mut kernel, || {
        RadarrFulfiller::stop_monitoring(deleted(ALIEN))
    })
    .unwrap();

    assert_eq!(asked(&kernel), ["GET movie?tmdbId=348", "PUT movie/editor"]);
    assert_eq!(
        body_of(&kernel, "PUT movie/editor"),
        json!({"movieIds": [ALIEN_RADARR_ID], "monitored": false})
    );
}

#[test]
fn deleting_a_movie_radarr_doesnt_have_changes_nothing() {
    let mut kernel = radarr_answering(vec![(200, json!([]))]);

    with(&mut kernel, || {
        RadarrFulfiller::stop_monitoring(deleted(DUNE))
    })
    .unwrap();

    assert_eq!(asked(&kernel), ["GET movie?tmdbId=438631"]);
}

#[test]
fn deleting_a_movie_with_radarr_down_fails_in_plain_words() {
    let failed = with(&mut radarr_down(), || {
        RadarrFulfiller::stop_monitoring(deleted(ALIEN))
    });

    assert_eq!(failed.unwrap_err().to_string(), "Radarr didn't answer");
}

#[test]
fn a_movie_without_a_tmdb_id_is_refused() {
    let error = tmdb_id(&[id("imdb", "tt0078748")]).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Radarr needs the Movie's TMDB id, and none is known for it"
    );
}

#[test]
fn a_movie_s_tmdb_id_is_its_number() {
    let ids = [id("imdb", "tt0078748"), id("tmdb", "348")];

    assert_eq!(tmdb_id(&ids).unwrap(), 348);
}
