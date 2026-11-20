use std::{cell::RefCell, rc::Rc};

use nama_pdk::{
    Permission, SubtitleDownload, SubtitleResults, SubtitleSearch, SubtitleSource, TitleId,
    TitleKind,
    nama_plugin_proto::nama::plugin::v1::SettingKind,
    services,
    testing::{Kernel, with},
};
use url::Url;

use super::{
    OpenSubtitles, Refused, check_settings,
    fake_opensubtitles::{API_KEY, FakeOpenSubtitles, FakeSubtitle, PASSWORD, USERNAME, srt},
    manifest,
};

/// A Kernel holding the fake's account and key, whose requests `fake` answers, not granting the
/// Store.
fn kernel(fake: &Rc<RefCell<FakeOpenSubtitles>>) -> Kernel {
    let fake = Rc::clone(fake);
    Kernel::new()
        .setting("api_key", API_KEY)
        .setting("username", USERNAME)
        .setting("password", PASSWORD)
        .http(move |request, body| Ok(fake.borrow_mut().answer(request, body)))
}

/// `subtitles` served by a fake with a fresh day's quota.
fn serving(subtitles: Vec<FakeSubtitle>) -> Rc<RefCell<FakeOpenSubtitles>> {
    Rc::new(RefCell::new(FakeOpenSubtitles {
        subtitles,
        ..FakeOpenSubtitles::default()
    }))
}

fn id(namespace: &str, value: &str) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

/// A search for Alien (1979), by its IMDb and TMDB ids, in `languages`.
fn alien(languages: &[&str]) -> SubtitleSearch {
    SubtitleSearch {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: vec![id("tmdb", "348"), id("imdb", "tt0078748")],
        file_name: "Alien.1979.1080p.BluRay.mkv".to_owned(),
        languages: languages.iter().map(|&tag| tag.to_owned()).collect(),
        ..SubtitleSearch::default()
    }
}

/// Each result's id, language, name and whether it's SDH.
fn summary(results: &SubtitleResults) -> Vec<(&str, &str, &str, bool)> {
    (results.results.iter())
        .map(|found| {
            let (id, language, name) = (&found.id, &found.language, &found.name);
            (id.as_str(), language.as_str(), name.as_str(), found.sdh)
        })
        .collect()
}

#[test]
fn manifest_serves_subtitles_from_an_api_key_and_an_account() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::SUBTITLE_SOURCE]);
    assert_eq!(manifest.allowed_hosts, ["*.opensubtitles.com"]);
    assert_eq!(manifest.permissions, [Permission::PERMISSION_STORE]);
    let settings: Vec<_> = (manifest.settings.iter())
        .map(|field| (field.key.as_str(), field.kind.as_known(), field.required))
        .collect();
    let secret = Some(SettingKind::SETTING_KIND_SECRET);
    assert_eq!(
        settings,
        [
            ("api_key", secret, true),
            ("username", Some(SettingKind::SETTING_KIND_TEXT), true),
            ("password", secret, true),
        ]
    );
}

#[test]
fn a_movie_is_searched_by_its_imdb_id_in_the_wanted_languages() {
    let fake = serving(vec![
        FakeSubtitle {
            sdh: true,
            ..FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")
        },
        FakeSubtitle {
            language: "pt-BR",
            ..FakeSubtitle::new(102, "Alien.1979.DVDRip")
        },
        FakeSubtitle {
            language: "fr",
            ..FakeSubtitle::new(103, "Alien.1979.Remastered")
        },
    ]);

    let found = with(&mut kernel(&fake), || {
        OpenSubtitles::search(alien(&["pt-BR", "en"]))
    })
    .unwrap();

    assert_eq!(
        summary(&found),
        [
            ("101", "en", "Alien.1979.1080p.BluRay", true),
            ("102", "pt-BR", "Alien.1979.DVDRip", false),
        ]
    );
}

#[test]
fn an_episode_is_searched_by_its_shows_id_season_and_episode() {
    let fake = serving(vec![FakeSubtitle {
        found_by: vec![
            ("episode_number", "2"),
            ("parent_tmdb_id", "1396"),
            ("season_number", "1"),
            ("type", "episode"),
        ],
        ..FakeSubtitle::new(201, "Breaking.Bad.S01E02")
    }]);
    let search = SubtitleSearch {
        kind: TitleKind::TITLE_KIND_SHOW.into(),
        ids: vec![id("tmdb", "1396"), id("tvdb", "81189")],
        season: Some(1),
        episode: Some(2),
        file_name: "Breaking.Bad.S01E02.mkv".to_owned(),
        languages: vec!["en".to_owned()],
        ..SubtitleSearch::default()
    };

    let found = with(&mut kernel(&fake), || OpenSubtitles::search(search)).unwrap();

    assert_eq!(
        summary(&found),
        [("201", "en", "Breaking.Bad.S01E02", false)]
    );
}

#[test]
fn a_version_without_ids_is_searched_by_its_file_name() {
    let fake = serving(vec![FakeSubtitle {
        found_by: vec![("query", "home movie 2003.mkv"), ("type", "movie")],
        ..FakeSubtitle::new(301, "Home.Movie.2003")
    }]);
    let search = SubtitleSearch {
        ids: Vec::new(),
        file_name: "Home Movie 2003.mkv".to_owned(),
        ..alien(&["en"])
    };

    let found = with(&mut kernel(&fake), || OpenSubtitles::search(search)).unwrap();

    assert_eq!(summary(&found), [("301", "en", "Home.Movie.2003", false)]);
}

#[test]
fn a_subtitle_in_several_files_is_left_out() {
    let fake = serving(vec![FakeSubtitle {
        files: 2,
        ..FakeSubtitle::new(401, "Alien.1979.2CD")
    }]);

    let found = with(&mut kernel(&fake), || OpenSubtitles::search(alien(&["en"]))).unwrap();

    assert_eq!(summary(&found), []);
}

/// Asks to download the file OpenSubtitles found as `id`.
fn download_of(id: &str) -> SubtitleDownload {
    SubtitleDownload {
        id: id.to_owned(),
        ..SubtitleDownload::default()
    }
}

#[test]
fn a_download_logs_in_and_answers_the_file_in_its_format() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);

    let downloaded = with(&mut kernel(&fake), || {
        OpenSubtitles::download(download_of("101"))
    })
    .unwrap();

    assert_eq!(
        (downloaded.data, downloaded.format.as_str()),
        (srt("Alien.1979.1080p.BluRay").into_bytes(), "srt")
    );
    assert_eq!(fake.borrow().remaining, 19);
}

/// The last segment of each URL the Plugin sent to, in order, such as `login`.
fn sent_to(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let url = Url::parse(&sent.request.url).unwrap();
            url.path_segments().unwrap().next_back().unwrap().to_owned()
        })
        .collect()
}

/// The file name a download link of the fake's subtitle 101 ends in.
const LINKED_FILE: &str = "Alien.1979.1080p.BluRay.srt";

#[test]
fn downloads_share_the_login_the_store_keeps() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    let mut kernel = kernel(&fake).store();

    for _ in 0..2 {
        with(&mut kernel, || OpenSubtitles::download(download_of("101"))).unwrap();
    }

    let sent = sent_to(&kernel);
    assert_eq!(
        sent,
        ["login", "download", LINKED_FILE, "download", LINKED_FILE]
    );
}

#[test]
fn a_refused_stored_token_logs_in_once_retries_once_and_keeps_the_new_token() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    let mut kernel = kernel(&fake).store();
    with(&mut kernel, || OpenSubtitles::download(download_of("101"))).unwrap();
    fake.borrow_mut().token = "renewed-token";

    let downloaded = with(&mut kernel, || OpenSubtitles::download(download_of("101"))).unwrap();
    with(&mut kernel, || OpenSubtitles::download(download_of("101"))).unwrap();

    assert_eq!(downloaded.data, srt("Alien.1979.1080p.BluRay").into_bytes());
    let sent = sent_to(&kernel);
    assert_eq!(
        sent[3..],
        [
            "download",
            "login",
            "download",
            LINKED_FILE,
            "download",
            LINKED_FILE
        ]
    );
}

#[test]
fn without_the_store_each_download_logs_in() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    let mut kernel = kernel(&fake);

    for _ in 0..2 {
        with(&mut kernel, || OpenSubtitles::download(download_of("101"))).unwrap();
    }

    let sent = sent_to(&kernel);
    assert_eq!(
        sent,
        [
            "login",
            "download",
            LINKED_FILE,
            "login",
            "download",
            LINKED_FILE
        ]
    );
}

#[test]
fn a_stored_login_for_another_username_is_not_used() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    let mut kernel = kernel(&fake).store();
    with(&mut kernel, || OpenSubtitles::download(download_of("101"))).unwrap();
    let mut kernel = kernel.setting("username", "someone-else");

    let _refused = with(&mut kernel, || OpenSubtitles::download(download_of("101")));

    assert_eq!(sent_to(&kernel)[3..], ["login"]);
}

#[test]
fn a_vip_accounts_download_goes_to_the_host_its_login_answers() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    fake.borrow_mut().base_url = "vip-api.opensubtitles.com";

    let downloaded = with(&mut kernel(&fake), || {
        OpenSubtitles::download(download_of("101"))
    })
    .unwrap();

    assert_eq!(downloaded.data, srt("Alien.1979.1080p.BluRay").into_bytes());
}

#[test]
fn a_refused_download_link_fails_with_its_status_not_as_a_wrong_api_key() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    fake.borrow_mut().links_expired = true;

    let err = with(&mut kernel(&fake), || {
        OpenSubtitles::download(download_of("101"))
    })
    .unwrap_err();

    assert_eq!(err.downcast_ref(), Some(&Refused::Status(403)));
}

#[test]
fn a_used_up_daily_quota_fails_the_download_saying_when_it_renews() {
    let fake = serving(vec![FakeSubtitle::new(101, "Alien.1979.1080p.BluRay")]);
    fake.borrow_mut().remaining = 0;

    let err = with(&mut kernel(&fake), || {
        OpenSubtitles::download(download_of("101"))
    })
    .unwrap_err();

    let renews_in = Some("3 hours and 12 minutes".to_owned());
    assert_eq!(err.downcast_ref(), Some(&Refused::Quota { renews_in }));
    assert_eq!(
        err.to_string(),
        "the OpenSubtitles account has used its daily download quota, which renews in 3 hours \
         and 12 minutes"
    );
}

#[test]
fn a_rate_limited_search_fails_saying_so_without_trying_again() {
    let fake = serving(Vec::new());
    fake.borrow_mut().rate_limited = true;
    let mut kernel = kernel(&fake);

    let err = with(&mut kernel, || OpenSubtitles::search(alien(&["en"]))).unwrap_err();

    let retry_after = Some(7);
    assert_eq!(
        err.downcast_ref(),
        Some(&Refused::RateLimit { retry_after })
    );
    assert_eq!(
        err.to_string(),
        "OpenSubtitles is limiting this Server's requests; try again in 7 s"
    );
    assert_eq!(kernel.sent().len(), 1);
}

#[test]
fn a_wrong_api_key_fails_the_settings_check() {
    let fake = serving(Vec::new());
    let mut kernel = kernel(&fake).setting("api_key", "wrong");

    let err = with(&mut kernel, check_settings).unwrap_err();

    assert_eq!(err.downcast_ref(), Some(&Refused::ApiKey));
    assert_eq!(err.to_string(), "OpenSubtitles refused the API key");
}

#[test]
fn a_wrong_password_fails_the_settings_check() {
    let fake = serving(Vec::new());
    let mut kernel = kernel(&fake).setting("password", "wrong");

    let err = with(&mut kernel, check_settings).unwrap_err();

    assert_eq!(err.downcast_ref(), Some(&Refused::Login));
    assert_eq!(
        err.to_string(),
        "OpenSubtitles refused the username or password"
    );
}

#[test]
fn the_settings_check_passes_with_the_right_account() {
    let fake = serving(Vec::new());

    with(&mut kernel(&fake), check_settings).unwrap();
}
