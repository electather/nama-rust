use nama_pdk::{
    ImageAsked, Response, SearchQuery, SeasonAsked, TitleAsked, TitleId, TitleKind, TitleSource,
    extism_pdk::{HttpRequest, json},
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
    testing::{Kernel, with},
};

use super::{
    Episodes, Found, Login, Lookup, Series, Tvdb, artwork_path, content_type, data, encoded,
    manifest, season, title_id, tvdb_id,
};

fn ids(pairs: &[(&str, &str)]) -> Vec<TitleId> {
    pairs
        .iter()
        .map(|&(namespace, value)| title_id(namespace, value))
        .collect()
}

#[test]
fn manifest_serves_the_title_source_under_tvdb_with_four_settings() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::TITLE_SOURCE]);
    assert_eq!(manifest.title_namespace, "tvdb");
    assert_eq!(manifest.title_lookup_namespaces, Vec::<String>::new());
    let settings: Vec<_> = manifest
        .settings
        .iter()
        .map(|field| {
            let default = field.default_value.as_option();
            let default = default.and_then(|default| default.value.clone());
            (
                field.key.as_str(),
                field.kind.as_known(),
                field.required,
                default,
            )
        })
        .collect();
    let secret = Some(SettingKind::SETTING_KIND_SECRET);
    let url = Some(SettingKind::SETTING_KIND_URL);
    let text = |text: &str| Some(Value::TextValue(text.to_owned()));
    assert_eq!(
        settings,
        [
            ("api_key", secret, true, None),
            ("pin", secret, false, None),
            ("api_url", url, false, text("https://api4.thetvdb.com/v4")),
            (
                "artwork_url",
                url,
                false,
                text("https://artworks.thetvdb.com")
            ),
        ]
    );
}

#[test]
fn manifest_says_where_to_get_an_api_key_and_gives_its_links_and_licence() {
    let manifest = manifest();

    let setup = &manifest.setup_markdown;
    assert!(
        setup.contains("https://thetvdb.com/dashboard/account/apikey"),
        "{setup}"
    );
    assert_eq!(
        [manifest.homepage_url, manifest.source_url, manifest.license],
        [
            "https://thetvdb.com/",
            "https://github.com/electather/nama-rust/tree/main/plugins/tvdb",
            "AGPL-3.0-only",
        ]
    );
}

#[test]
fn login_reads_the_token() {
    let login: Login = data(include_bytes!("../fixtures/login.json")).unwrap();

    assert_eq!(login.token, "fixture-token");
}

#[test]
fn search_results_carry_the_ids_tvdb_reports_and_the_english_name() {
    let found: Vec<Found> = data(include_bytes!("../fixtures/search.json")).unwrap();
    let candidates: Vec<_> = found.into_iter().filter_map(Found::candidate).collect();

    let [severance, hospital] = candidates.as_slice() else {
        panic!("{candidates:?}");
    };
    assert_eq!(severance.id.as_option(), Some(&title_id("tvdb", "371980")));
    assert_eq!(
        severance.ids,
        ids(&[
            ("tvdb", "371980"),
            ("imdb", "tt11280740"),
            ("tmdb", "95396")
        ])
    );
    assert_eq!(severance.title, "Severance");
    assert_eq!(severance.original_title, None);
    assert_eq!(severance.year, Some(2022));
    assert_eq!(severance.origin_country, ["US"]);
    assert_eq!(
        severance.poster.as_deref(),
        Some("/banners/v4/series/371980/posters/61f1b0b1a7a0c.jpg")
    );
    assert_eq!(hospital.title, "Severance Hospital");
    assert_eq!(hospital.original_title.as_deref(), Some("세브란스"));
    assert_eq!(hospital.origin_country, ["KR"]);
    assert_eq!(hospital.poster, None);
}

#[test]
fn a_show_has_its_ids_official_seasons_and_best_backdrop() {
    let series: Series = data(include_bytes!("../fixtures/series-extended.json")).unwrap();
    let show = series.show();

    assert_eq!(
        show.ids,
        ids(&[
            ("tvdb", "371980"),
            ("imdb", "tt11280740"),
            ("tmdb", "95396")
        ])
    );
    assert_eq!(show.title, "Severance");
    assert_eq!(show.first_air_date.as_deref(), Some("2022-02-18"));
    assert_eq!(show.seasons, [0, 1, 2]);
    assert_eq!(
        show.poster.as_deref(),
        Some("/banners/v4/series/371980/posters/61f1b0b1a7a0c.jpg")
    );
    assert_eq!(
        show.backdrop.as_deref(),
        Some("/banners/v4/series/371980/backgrounds/61f1b1b3d5f90.jpg")
    );
    assert!(show.overview.is_some());
}

#[test]
fn a_season_has_its_episodes_and_first_air_date() {
    let episodes: Episodes = data(include_bytes!("../fixtures/episodes-season-1.json")).unwrap();
    let season = season(1, episodes.episodes).unwrap();

    assert_eq!(season.title, "Season 1");
    assert_eq!(season.air_date.as_deref(), Some("2022-02-18"));
    let [first, second] = season.episodes.as_slice() else {
        panic!("{:?}", season.episodes);
    };
    assert_eq!(first.ids, ids(&[("tvdb", "8695386")]));
    assert_eq!((first.season, first.number), (1, 1));
    assert_eq!(first.title, "Good News About Hell");
    assert_eq!(first.runtime_minutes, Some(57));
    assert_eq!(
        first.still.as_deref(),
        Some("/banners/v4/episode/8695386/screencap/61f8c3c6e3d1a.jpg")
    );
    assert_eq!((second.number, second.overview.clone()), (2, None));
    assert_eq!(second.still, None);
}

#[test]
fn a_season_without_episodes_is_none_and_season_zero_is_specials() {
    let fixture = include_bytes!("../fixtures/episodes-season-1.json");
    let season_one: Episodes = data(fixture).unwrap();
    assert!(season(3, season_one.episodes).is_none());

    let mut specials: Episodes = data(fixture).unwrap();
    for episode in &mut specials.episodes {
        episode.season_number = 0;
    }
    assert_eq!(season(0, specials.episodes).unwrap().title, "Specials");
}

#[test]
fn ids_in_other_namespaces_are_unsupported_and_non_numbers_are_unknown() {
    assert_eq!(
        tvdb_id(Some(&title_id("tvdb", "371980"))).unwrap(),
        Lookup::Found(371_980)
    );
    assert_eq!(
        tvdb_id(Some(&title_id("tvdb", "x"))).unwrap(),
        Lookup::Missing
    );
    assert_eq!(
        tvdb_id(Some(&title_id("tmdb", "95396"))).unwrap(),
        Lookup::Unsupported
    );
}

#[test]
fn artwork_paths_and_their_types() {
    let path = |url: &str| artwork_path(Some(url.to_owned()));
    assert_eq!(
        path("https://artworks.thetvdb.com/banners/a.png").as_deref(),
        Some("/banners/a.png")
    );
    assert_eq!(path("/banners/b.jpg").as_deref(), Some("/banners/b.jpg"));
    assert_eq!(path(""), None);
    assert_eq!(content_type("/banners/a.png"), "image/png");
    assert_eq!(content_type("/banners/b.jpg"), "image/jpeg");
}

#[test]
fn a_query_is_percent_encoded() {
    assert_eq!(encoded("Dr. Who & Co"), "Dr.%20Who%20%26%20Co");
}

// TVDB's conversation, through `nama_pdk::testing`: each test calls a title source method as the
// Kernel would, against a TVDB answering with the recorded Severance responses.

const API: &str = "http://tvdb.test/v4";
const ARTWORK: &str = "http://artworks.tvdb.test";

/// TVDB's settings, against a TVDB answered by [`tvdb`].
fn kernel() -> Kernel {
    Kernel::new()
        .setting("api_key", "key")
        .setting("api_url", API)
        .setting("artwork_url", ARTWORK)
        .http(|request, _body| Ok(tvdb(request)))
}

/// TVDB, answering each request it has a recorded response for, and every artwork path with its
/// own path as the image's bytes. Any other request fails the test, so a Plugin can't pass by
/// handling an answer no test meant.
fn tvdb(request: &HttpRequest) -> Response {
    if let Some(path) = request.url.strip_prefix(ARTWORK) {
        return Response::new(200, path);
    }
    let body = match request.url.trim_start_matches(API) {
        "/login" => include_str!("../fixtures/login.json"),
        "/search?query=Severance&type=series&year=2022" => include_str!("../fixtures/search.json"),
        "/series/371980/extended?meta=translations" => {
            include_str!("../fixtures/series-extended.json")
        }
        "/series/371980/episodes/default?season=1" => {
            include_str!("../fixtures/episodes-season-1.json")
        }
        url => panic!("TVDB was asked for {url}, which the test didn't script"),
    };
    Response::new(200, body)
}

/// What the Plugin asked TVDB, as `<method> <API path>`.
fn asked(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let method = sent.request.method.as_deref().unwrap_or("GET");
            format!("{method} {}", sent.request.url.trim_start_matches(API))
        })
        .collect()
}

fn severance() -> TitleAsked {
    TitleAsked {
        id: title_id("tvdb", "371980").into(),
        ..TitleAsked::default()
    }
}

fn severance_season(season: u32) -> SeasonAsked {
    SeasonAsked {
        show: title_id("tvdb", "371980").into(),
        season,
        ..SeasonAsked::default()
    }
}

#[test]
fn a_call_logs_in_with_the_key_and_pin_then_sends_the_token() {
    let mut kernel = kernel().setting("pin", " 1234 ");

    with(&mut kernel, || Tvdb::show(severance())).unwrap();

    let [login, show] = kernel.sent() else {
        panic!("{:?}", asked(&kernel));
    };
    assert_eq!(login.request.method.as_deref(), Some("POST"));
    assert_eq!(login.request.url, format!("{API}/login"));
    let body: json::Value = json::from_slice(&login.body).unwrap();
    assert_eq!(body, json::json!({"apikey": "key", "pin": "1234"}));
    let token = show
        .request
        .headers
        .get("Authorization")
        .map(String::as_str);
    assert_eq!(token, Some("Bearer fixture-token"));
}

#[test]
fn a_search_finds_shows_with_the_ids_tvdb_reports_and_their_english_names() {
    let mut kernel = kernel();
    let query = SearchQuery {
        kind: TitleKind::TITLE_KIND_SHOW.into(),
        title: "Severance".to_owned(),
        year: Some(2022),
        ..SearchQuery::default()
    };

    let found = with(&mut kernel, || Tvdb::search(query)).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "POST /login",
            "GET /search?query=Severance&type=series&year=2022"
        ]
    );
    let hits: Vec<_> = (found.candidates.iter())
        .map(|hit| (hit.title.as_str(), hit.original_title.as_deref(), &hit.ids))
        .collect();
    let severance_ids = ids(&[
        ("tvdb", "371980"),
        ("imdb", "tt11280740"),
        ("tmdb", "95396"),
    ]);
    assert_eq!(
        hits,
        [
            ("Severance", None, &severance_ids),
            (
                "Severance Hospital",
                Some("세브란스"),
                &ids(&[("tvdb", "79157")])
            ),
        ]
    );
}

#[test]
fn a_show_comes_with_its_ids_official_seasons_and_best_backdrop() {
    let mut kernel = kernel();

    let answer = with(&mut kernel, || Tvdb::show(severance())).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "POST /login",
            "GET /series/371980/extended?meta=translations"
        ]
    );
    let show = answer.show.as_option().unwrap();
    assert_eq!(
        show.ids,
        ids(&[
            ("tvdb", "371980"),
            ("imdb", "tt11280740"),
            ("tmdb", "95396")
        ])
    );
    assert_eq!(show.title, "Severance");
    assert_eq!(show.seasons, [0, 1, 2]);
    assert_eq!(
        show.backdrop.as_deref(),
        Some("/banners/v4/series/371980/backgrounds/61f1b1b3d5f90.jpg")
    );
}

#[test]
fn a_season_comes_with_its_episodes_in_tvdbs_aired_order() {
    let mut kernel = kernel();

    let answer = with(&mut kernel, || Tvdb::season(severance_season(1))).unwrap();

    assert_eq!(
        asked(&kernel),
        [
            "POST /login",
            "GET /series/371980/episodes/default?season=1"
        ]
    );
    let season = answer.season.as_option().unwrap();
    assert_eq!(season.title, "Season 1");
    let episodes: Vec<_> = (season.episodes.iter())
        .map(|episode| (episode.number, episode.title.as_str(), &episode.ids))
        .collect();
    assert_eq!(
        episodes,
        [
            (1, "Good News About Hell", &ids(&[("tvdb", "8695386")])),
            (2, "Half Loop", &ids(&[("tvdb", "8695387")])),
        ]
    );
}

#[test]
fn an_id_in_another_namespace_is_unsupported_and_asks_tvdb_nothing() {
    let mut kernel = kernel();
    let tmdb = title_id("tmdb", "95396");
    let show = TitleAsked {
        id: tmdb.clone().into(),
        ..TitleAsked::default()
    };
    let season = SeasonAsked {
        show: tmdb.into(),
        season: 1,
        ..SeasonAsked::default()
    };

    let answers = with(&mut kernel, || {
        (Tvdb::show(show).unwrap(), Tvdb::season(season).unwrap())
    });

    assert!(
        answers.0.unsupported_id && answers.1.unsupported_id,
        "{answers:?}"
    );
    assert_eq!(asked(&kernel), Vec::<String>::new());
}

#[test]
fn an_image_is_fetched_from_the_artwork_url_with_its_type_from_its_path() {
    let mut kernel = kernel();
    let path = "/banners/v4/series/371980/posters/61f1b0b1a7a0c.jpg";
    let asked_for = ImageAsked {
        path: path.to_owned(),
        ..ImageAsked::default()
    };

    let answer = with(&mut kernel, || Tvdb::image(asked_for)).unwrap();

    let image = answer.image.as_option().unwrap();
    assert_eq!(
        (image.content_type.as_str(), image.bytes.as_slice()),
        ("image/jpeg", path.as_bytes())
    );
    assert_eq!(kernel.sent()[0].request.url, format!("{ARTWORK}{path}"));
}
