use nama_pdk::{
    Permission, Response, SegmentKind, SegmentResults, SegmentSearch, SegmentSource, TitleId,
    TitleKind,
    extism_pdk::Error,
    nama_plugin_proto::nama::plugin::v1::SettingKind,
    services,
    testing::{Kernel, with},
};

use super::{TheIntroDb, manifest};

fn id(namespace: &str, value: &str) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

/// Breaking Bad's first Episode, whose Show has every id TheIntroDB takes.
fn breaking_bad_pilot() -> SegmentSearch {
    SegmentSearch {
        kind: TitleKind::TITLE_KIND_SHOW.into(),
        ids: vec![
            id("tmdb", "1396"),
            id("tvdb", "81189"),
            id("imdb", "tt0903747"),
        ],
        season: Some(1),
        episode: Some(1),
        duration_ms: 3_480_000,
        ..SegmentSearch::default()
    }
}

/// Fight Club, which has no TVDB id.
fn fight_club() -> SegmentSearch {
    SegmentSearch {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: vec![id("imdb", "tt0137523"), id("tmdb", "550")],
        duration_ms: 8_340_000,
        ..SegmentSearch::default()
    }
}

/// A Kernel answering every request with `status` and `body`.
fn answering(status: u16, body: &'static [u8]) -> Kernel {
    Kernel::new().http(move |_request, _body| Ok(Response::new(status, body)))
}

fn search(kernel: &mut Kernel, search: SegmentSearch) -> Result<SegmentResults, Error> {
    with(kernel, || TheIntroDb::search(search))
}

/// Each Segment's kind, start and end.
fn found(results: &SegmentResults) -> Vec<(Option<SegmentKind>, u64, Option<u64>)> {
    (results.segments.iter())
        .map(|segment| (segment.kind.as_known(), segment.start_ms, segment.end_ms))
        .collect()
}

fn sent_urls(kernel: &Kernel) -> Vec<&str> {
    (kernel.sent().iter())
        .map(|sent| sent.request.url.as_str())
        .collect()
}

#[test]
fn manifest_serves_the_segment_source_on_theintrodbs_api_with_an_optional_secret_key() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::SEGMENT_SOURCE]);
    assert_eq!(manifest.allowed_hosts, ["api.theintrodb.org"]);
    assert_eq!(manifest.permissions, [Permission::PERMISSION_STORE]);
    let [setting] = manifest.settings.as_slice() else {
        panic!("{:?}", manifest.settings);
    };
    assert_eq!(setting.key, "api_key");
    assert!(!setting.required);
    assert_eq!(setting.kind, SettingKind::SETTING_KIND_SECRET);
}

#[test]
fn an_episode_is_asked_for_by_its_shows_tvdb_id_season_episode_and_duration() {
    let mut kernel = answering(200, include_bytes!("../fixtures/episode.json"));

    let results = search(&mut kernel, breaking_bad_pilot()).unwrap();

    assert_eq!(
        sent_urls(&kernel),
        [
            "https://api.theintrodb.org/v4/media?tvdb_id=81189&season=1&episode=1&duration_ms=3480000"
        ]
    );
    assert_eq!(kernel.sent()[0].request.headers.get("Authorization"), None);
    assert_eq!(
        found(&results),
        [
            (
                Some(SegmentKind::SEGMENT_KIND_INTRO),
                228_665,
                Some(246_498)
            ),
            (Some(SegmentKind::SEGMENT_KIND_CREDITS), 3_432_884, None),
        ]
    );
}

#[test]
fn a_movie_without_a_tvdb_id_is_asked_for_by_its_tmdb_id_and_an_unset_start_is_its_start() {
    let mut kernel = answering(200, include_bytes!("../fixtures/movie.json"));

    let results = search(&mut kernel, fight_club()).unwrap();

    assert_eq!(
        sent_urls(&kernel),
        ["https://api.theintrodb.org/v4/media?tmdb_id=550&duration_ms=8340000"]
    );
    assert_eq!(
        found(&results),
        [
            (Some(SegmentKind::SEGMENT_KIND_INTRO), 0, Some(119_000)),
            (
                Some(SegmentKind::SEGMENT_KIND_CREDITS),
                8_177_000,
                Some(8_348_000)
            ),
        ]
    );
}

#[test]
fn a_title_theintrodb_doesnt_know_has_no_segments() {
    let mut kernel = answering(404, include_bytes!("../fixtures/unknown.json"));

    let unknown = SegmentSearch {
        ids: vec![id("tmdb", "9999999")],
        ..fight_club()
    };

    let results = search(&mut kernel, unknown).unwrap();

    assert_eq!(
        sent_urls(&kernel),
        ["https://api.theintrodb.org/v4/media?tmdb_id=9999999&duration_ms=8340000"]
    );
    assert_eq!(found(&results), []);
}

#[test]
fn a_refused_401_names_the_api_key() {
    let mut kernel = answering(401, b"{}").setting("api_key", "revoked");

    let failed = search(&mut kernel, fight_club()).unwrap_err();

    assert_eq!(failed.to_string(), "TheIntroDB refused the API key");
}

#[test]
fn an_id_theintrodb_cant_place_has_no_segments() {
    let mut kernel = answering(422, b"{}");

    let results = search(&mut kernel, fight_club()).unwrap();

    assert_eq!(found(&results), []);
}

#[test]
fn of_several_stretches_of_a_kind_the_earliest_is_kept_and_no_segment_answers_are_dropped() {
    let body =
        br#"{"intro":[{"start_ms":120000,"end_ms":150000},{"start_ms":30000,"end_ms":90000}],
        "recap":[{"start_ms":null,"end_ms":0}],
        "credits":[{"start_ms":0,"end_ms":null},{"start_ms":null,"end_ms":null}],
        "preview":[{"start_ms":1680000,"end_ms":1740000}]}"#;
    let mut kernel = answering(200, body);

    let results = search(&mut kernel, breaking_bad_pilot()).unwrap();

    assert_eq!(
        found(&results),
        [(Some(SegmentKind::SEGMENT_KIND_INTRO), 30_000, Some(90_000))]
    );
}

#[test]
fn an_episode_tvdb_doesnt_have_has_no_segments() {
    let body = br#"{"error":"Season 99 Episode 99 does not exist on TVDB for this show"}"#;
    let mut kernel = answering(400, body);
    let no_such_episode = SegmentSearch {
        season: Some(99),
        episode: Some(99),
        ..breaking_bad_pilot()
    };

    let results = search(&mut kernel, no_such_episode).unwrap();

    assert_eq!(found(&results), []);
}

#[test]
fn the_api_key_is_sent_as_a_bearer_token() {
    let mut kernel = answering(404, b"{}").setting("api_key", " secret ");

    search(&mut kernel, fight_club()).unwrap();

    let headers = &kernel.sent()[0].request.headers;
    assert_eq!(
        headers.get("Authorization").map(String::as_str),
        Some("Bearer secret")
    );
}

#[test]
fn a_title_with_no_id_theintrodb_takes_or_an_episode_without_numbers_is_not_asked_for() {
    let mut kernel = Kernel::new();
    let anidb_only = SegmentSearch {
        ids: vec![id("anidb", "69")],
        ..fight_club()
    };
    let no_numbers = SegmentSearch {
        episode: None,
        ..breaking_bad_pilot()
    };
    let odd_id = SegmentSearch {
        ids: vec![id("tmdb", "550&tvdb_id=1")],
        ..fight_club()
    };

    for asked in [anidb_only, no_numbers, odd_id] {
        assert_eq!(found(&search(&mut kernel, asked).unwrap()), []);
    }
}

#[test]
fn without_an_api_key_a_granted_store_refuses_a_31st_request_within_10_seconds() {
    let mut kernel = answering(404, b"{}").store();

    let answered: Vec<_> = (0..31)
        .map(|_| search(&mut kernel, fight_club()).is_ok())
        .collect();

    assert_eq!(kernel.sent().len(), 30);
    assert!(answered[..30].iter().all(|&ok| ok));
    assert!(!answered[30]);
}
