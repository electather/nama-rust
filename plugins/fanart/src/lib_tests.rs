use nama_pdk::{
    ArtworkAnswer, ArtworkAsked, ArtworkKind, ImageAsked, Response, SeasonAsked, TitleId,
    TitleKind, TitleSource,
    extism_pdk::HttpRequest,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
    testing::{Kernel, with},
};

use nama_pdk::ArtworkKind::{
    ARTWORK_KIND_BACKDROP, ARTWORK_KIND_CLEAR_ART, ARTWORK_KIND_DISC, ARTWORK_KIND_LOGO,
    ARTWORK_KIND_POSTER, ARTWORK_KIND_THUMB,
};

use super::{
    Fanart, MOVIE, SHOW, artwork, asset_path, lookup, manifest, season_posters, show_path,
    unsupported_id,
};

fn id(namespace: &str, value: &str) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

#[test]
fn manifest_serves_the_title_source_looking_up_by_tmdb_imdb_and_tvdb() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::TITLE_SOURCE]);
    assert_eq!(manifest.points[0].version, "^1.7.0");
    assert_eq!(manifest.title_namespace, "fanart");
    assert_eq!(manifest.title_lookup_namespaces, ["tmdb", "imdb", "tvdb"]);
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
            ("client_key", secret, false, None),
            (
                "api_url",
                url,
                false,
                text("https://webservice.fanart.tv/v3")
            ),
            ("assets_url", url, false, text("https://assets.fanart.tv")),
        ]
    );
}

#[test]
fn manifest_says_where_to_get_an_api_key_and_gives_its_links_and_licence() {
    let manifest = manifest();

    let setup = &manifest.setup_markdown;
    assert!(
        setup.contains("https://fanart.tv/get-an-api-key/"),
        "{setup}"
    );
    assert_eq!(
        [manifest.homepage_url, manifest.source_url, manifest.license],
        [
            "https://fanart.tv/",
            "https://github.com/electather/nama-rust/tree/main/plugins/fanart",
            "AGPL-3.0-only",
        ]
    );
}

#[test]
fn a_movies_lists_map_to_kinds_with_hd_first_languages_and_votes() {
    let artwork = artwork(MOVIE, include_bytes!("../fixtures/movie.json")).unwrap();

    let listed: Vec<_> = artwork
        .iter()
        .map(|art| {
            (
                art.kind.as_known(),
                art.path.as_str(),
                art.language.as_deref(),
                art.votes,
            )
        })
        .collect();
    let path = "/fanart/movies/949";
    assert_eq!(
        listed,
        [
            (
                Some(ARTWORK_KIND_LOGO),
                &*format!("{path}/hdmovielogo/heat-5e1a.png"),
                Some("en"),
                Some(7)
            ),
            (
                Some(ARTWORK_KIND_LOGO),
                &format!("{path}/hdmovielogo/heat-5e1b.png"),
                None,
                Some(2)
            ),
            (
                Some(ARTWORK_KIND_LOGO),
                &format!("{path}/movielogo/heat-4f2a.png"),
                Some("en"),
                Some(1)
            ),
            (
                Some(ARTWORK_KIND_POSTER),
                &format!("{path}/movieposter/heat-52c1.jpg"),
                Some("de"),
                Some(0)
            ),
            (
                Some(ARTWORK_KIND_BACKDROP),
                &format!("{path}/moviebackground/heat-50d3.jpg"),
                None,
                Some(4)
            ),
            (
                Some(ARTWORK_KIND_CLEAR_ART),
                &format!("{path}/hdmovieclearart/heat-53aa.png"),
                Some("en"),
                Some(3)
            ),
            (
                Some(ARTWORK_KIND_DISC),
                &format!("{path}/moviedisc/heat-51bb.png"),
                Some("en"),
                Some(1)
            ),
        ]
    );
    assert_eq!(
        (artwork[0].width, artwork[0].height),
        (Some(800), Some(310))
    );
    assert_eq!(
        (artwork[2].width, artwork[2].height),
        (Some(400), Some(155))
    );
}

#[test]
fn a_shows_lists_map_to_kinds_leaving_out_its_seasons() {
    let artwork = artwork(SHOW, include_bytes!("../fixtures/tv.json")).unwrap();

    let listed: Vec<_> = artwork
        .iter()
        .map(|art| (art.kind.as_known(), art.path.rsplit('/').next().unwrap()))
        .collect();
    assert_eq!(
        listed,
        [
            (Some(ARTWORK_KIND_LOGO), "severance-61b.png"),
            (Some(ARTWORK_KIND_LOGO), "severance-61a.png"),
            (Some(ARTWORK_KIND_BACKDROP), "severance-61c.jpg"),
            (Some(ARTWORK_KIND_THUMB), "severance-61d.jpg"),
        ]
    );
    assert_eq!(
        (artwork[3].width, artwork[3].height),
        (Some(500), Some(281))
    );
}

#[test]
fn a_season_lists_only_its_own_season_posters_never_those_for_all() {
    let body = include_bytes!("../fixtures/tv.json");
    let listed = |season| {
        season_posters(body, season)
            .unwrap()
            .into_iter()
            .map(|art| {
                (
                    art.kind.as_known(),
                    art.path,
                    art.language,
                    art.votes,
                    (art.width, art.height),
                )
            })
            .collect::<Vec<_>>()
    };
    let poster = |file: &str, language: Option<&str>, votes| {
        (
            Some(ARTWORK_KIND_POSTER),
            format!("/fanart/tv/371980/seasonposter/{file}"),
            language.map(str::to_owned),
            Some(votes),
            (Some(1000), Some(1426)),
        )
    };

    assert_eq!(listed(1), [poster("severance-61e.jpg", Some("en"), 1)]);
    assert_eq!(listed(2), [poster("severance-61f.jpg", None, 3)]);
    assert_eq!(listed(3), []);
}

fn season_looked_up(namespace: &str, value: &str) -> Option<String> {
    let asked = SeasonAsked {
        show: id(namespace, value).into(),
        season: 1,
        ..SeasonAsked::default()
    };
    show_path(&asked).unwrap()
}

// `None` is what the `season_artwork` export answers `unsupported_id` for, before any request.
#[test]
fn a_seasons_show_is_looked_up_by_tvdb_id_alone() {
    assert_eq!(
        season_looked_up("tvdb", "371980").as_deref(),
        Some("tv/371980")
    );
    assert_eq!(season_looked_up("tvdb", "a/b").as_deref(), Some("tv/a%2Fb"));
    assert_eq!(season_looked_up("tmdb", "95396"), None);
    assert_eq!(season_looked_up("imdb", "tt11280740"), None);
}

fn looked_up(kind: TitleKind, namespace: &str, value: &str) -> Option<String> {
    let asked = ArtworkAsked {
        kind: kind.into(),
        id: id(namespace, value).into(),
        ..ArtworkAsked::default()
    };
    lookup(&asked).unwrap().map(|(path, _)| path)
}

#[test]
fn movies_are_looked_up_by_tmdb_or_imdb_id_and_shows_by_tvdb_id() {
    use TitleKind::*;

    assert_eq!(
        looked_up(TITLE_KIND_MOVIE, "tmdb", "949").as_deref(),
        Some("movies/949")
    );
    assert_eq!(
        looked_up(TITLE_KIND_MOVIE, "imdb", "tt0113277").as_deref(),
        Some("movies/tt0113277")
    );
    assert_eq!(
        looked_up(TITLE_KIND_SHOW, "tvdb", "371980").as_deref(),
        Some("tv/371980")
    );
    assert_eq!(
        looked_up(TITLE_KIND_SHOW, "tvdb", "a/b").as_deref(),
        Some("tv/a%2Fb")
    );
}

// `None` is what the `artwork` export answers `unsupported_id` for.
#[test]
fn a_foreign_namespace_or_kind_is_unsupported() {
    use TitleKind::*;

    assert_eq!(looked_up(TITLE_KIND_MOVIE, "anidb", "1"), None);
    assert_eq!(looked_up(TITLE_KIND_MOVIE, "tvdb", "1"), None);
    assert_eq!(looked_up(TITLE_KIND_SHOW, "tmdb", "1"), None);
    assert_eq!(looked_up(TITLE_KIND_UNSPECIFIED, "tmdb", "1"), None);
}

// The export itself calls into the Extism runtime, so it can't link outside wasm32.
#[test]
fn an_id_fanart_cant_look_up_answers_unsupported_id_and_no_artwork() {
    assert_eq!(
        unsupported_id(),
        ArtworkAnswer {
            unsupported_id: true,
            artwork: vec![],
            ..ArtworkAnswer::default()
        }
    );
}

#[test]
fn a_malformed_item_or_list_is_skipped_and_the_rest_kept() {
    let body = br#"{
        "hdmovielogo": [
            {"url": "https://assets.fanart.tv/fanart/movies/1/hdmovielogo/good.png", "lang": "en", "likes": "3"},
            {"lang": "en", "likes": "5"},
            {"url": "https://assets.fanart.tv/fanart/movies/1/hdmovielogo/bad.png", "likes": 9}
        ],
        "movielogo": "not a list"
    }"#;

    let artwork = artwork(MOVIE, body).unwrap();

    let listed: Vec<_> = artwork
        .iter()
        .map(|art| (art.path.as_str(), art.votes))
        .collect();
    assert_eq!(listed, [("/fanart/movies/1/hdmovielogo/good.png", Some(3))]);
}

#[test]
fn an_answer_that_isnt_an_object_fails() {
    artwork(MOVIE, b"[]").unwrap_err();
}

#[test]
fn an_asset_url_becomes_its_path() {
    assert_eq!(
        asset_path("http://assets.fanart.tv/fanart/tv/1/hdtvlogo/a.png").as_deref(),
        Some("/fanart/tv/1/hdtvlogo/a.png")
    );
    assert_eq!(asset_path("not a url"), None);
}

// fanart.tv's conversation, tested here rather than in the Server (ADR 0065), against the Heat and
// Severance lists recorded in `fixtures/`.

const API: &str = "http://fanart.test/v3";
const ASSETS: &str = "http://assets.fanart.test";

/// fanart.tv's settings, against a fanart.tv answered by [`fanart`].
fn kernel() -> Kernel {
    Kernel::new()
        .setting("api_key", "key")
        .setting("api_url", API)
        .setting("assets_url", ASSETS)
        .http(|request, _body| Ok(fanart(request)))
}

/// fanart.tv, answering Heat by its TMDB and IMDb ids and Severance by its TVDB id, having nothing
/// for the TMDB id 1, and answering every asset path with its own path as the image's bytes. Any
/// other request fails the test, so a Plugin can't pass by handling an answer no test meant.
fn fanart(request: &HttpRequest) -> Response {
    if let Some(path) = request.url.strip_prefix(ASSETS) {
        return Response::new(200, path);
    }
    let body = match request.url.trim_start_matches(API) {
        "/movies/949?api_key=key" | "/movies/tt0113277?api_key=key" => {
            include_str!("../fixtures/movie.json")
        }
        "/tv/371980?api_key=key" => include_str!("../fixtures/tv.json"),
        "/movies/1?api_key=key" => return Response::new(404, ""),
        url => panic!("fanart.tv was asked for {url}, which the test didn't script"),
    };
    Response::new(200, body)
}

/// The API paths the Plugin asked fanart.tv for.
fn asked(kernel: &Kernel) -> Vec<&str> {
    (kernel.sent().iter())
        .map(|sent| sent.request.url.trim_start_matches(API))
        .collect()
}

fn artwork_of(kind: TitleKind, namespace: &str, value: &str) -> ArtworkAsked {
    ArtworkAsked {
        kind: kind.into(),
        id: id(namespace, value).into(),
        ..ArtworkAsked::default()
    }
}

/// Each piece of `answer`'s artwork as its kind and file name.
fn listed(answer: &ArtworkAnswer) -> Vec<(Option<ArtworkKind>, &str)> {
    (answer.artwork.iter())
        .map(|art| (art.kind.as_known(), art.path.rsplit('/').next().unwrap()))
        .collect()
}

#[test]
fn a_movie_is_looked_up_by_its_tmdb_or_imdb_id() {
    for (namespace, value) in [("tmdb", "949"), ("imdb", "tt0113277")] {
        let mut kernel = kernel();
        let asked_for = artwork_of(TitleKind::TITLE_KIND_MOVIE, namespace, value);

        let answer = with(&mut kernel, || Fanart::artwork(asked_for)).unwrap();

        assert_eq!(asked(&kernel), [format!("/movies/{value}?api_key=key")]);
        assert_eq!(answer.artwork.len(), 7, "{namespace}");
    }
}

#[test]
fn a_movies_artwork_list_maps_to_kinds_its_title_logos_first() {
    let mut kernel = kernel();
    let heat = artwork_of(TitleKind::TITLE_KIND_MOVIE, "tmdb", "949");

    let answer = with(&mut kernel, || Fanart::artwork(heat)).unwrap();

    assert_eq!(
        listed(&answer),
        [
            (Some(ARTWORK_KIND_LOGO), "heat-5e1a.png"),
            (Some(ARTWORK_KIND_LOGO), "heat-5e1b.png"),
            (Some(ARTWORK_KIND_LOGO), "heat-4f2a.png"),
            (Some(ARTWORK_KIND_POSTER), "heat-52c1.jpg"),
            (Some(ARTWORK_KIND_BACKDROP), "heat-50d3.jpg"),
            (Some(ARTWORK_KIND_CLEAR_ART), "heat-53aa.png"),
            (Some(ARTWORK_KIND_DISC), "heat-51bb.png"),
        ]
    );
}

#[test]
fn a_show_is_looked_up_by_its_tvdb_id_leaving_out_its_seasons_posters() {
    let mut kernel = kernel();
    let severance = artwork_of(TitleKind::TITLE_KIND_SHOW, "tvdb", "371980");

    let answer = with(&mut kernel, || Fanart::artwork(severance)).unwrap();

    assert_eq!(asked(&kernel), ["/tv/371980?api_key=key"]);
    assert_eq!(
        listed(&answer),
        [
            (Some(ARTWORK_KIND_LOGO), "severance-61b.png"),
            (Some(ARTWORK_KIND_LOGO), "severance-61a.png"),
            (Some(ARTWORK_KIND_BACKDROP), "severance-61c.jpg"),
            (Some(ARTWORK_KIND_THUMB), "severance-61d.jpg"),
        ]
    );
}

#[test]
fn a_season_gets_only_its_own_posters() {
    let mut kernel = kernel();
    let season = SeasonAsked {
        show: id("tvdb", "371980").into(),
        season: 2,
        ..SeasonAsked::default()
    };

    let answer = with(&mut kernel, || Fanart::season_artwork(season)).unwrap();

    assert_eq!(asked(&kernel), ["/tv/371980?api_key=key"]);
    assert_eq!(
        listed(&answer),
        [(Some(ARTWORK_KIND_POSTER), "severance-61f.jpg")]
    );
}

#[test]
fn an_id_fanart_doesnt_have_answers_no_artwork() {
    let mut kernel = kernel();
    let unknown = artwork_of(TitleKind::TITLE_KIND_MOVIE, "tmdb", "1");

    let answer = with(&mut kernel, || Fanart::artwork(unknown)).unwrap();

    assert_eq!(asked(&kernel), ["/movies/1?api_key=key"]);
    assert_eq!(answer, ArtworkAnswer::default());
}

#[test]
fn an_id_fanart_cant_look_up_by_is_unsupported_and_asks_nothing() {
    let mut kernel = kernel();
    let movie = artwork_of(TitleKind::TITLE_KIND_MOVIE, "tvdb", "1");
    let season = SeasonAsked {
        show: id("tmdb", "95396").into(),
        season: 1,
        ..SeasonAsked::default()
    };

    let answers = with(&mut kernel, || {
        [
            Fanart::artwork(movie).unwrap(),
            Fanart::season_artwork(season).unwrap(),
        ]
    });

    assert_eq!(answers, [unsupported_id(), unsupported_id()]);
    assert_eq!(asked(&kernel), Vec::<&str>::new());
}

#[test]
fn an_image_is_fetched_from_the_assets_url_with_its_type_from_its_path() {
    let mut kernel = kernel();
    let path = "/fanart/movies/949/hdmovielogo/heat-5e1a.png";
    let asked_for = ImageAsked {
        path: path.to_owned(),
        ..ImageAsked::default()
    };

    let answer = with(&mut kernel, || Fanart::image(asked_for)).unwrap();

    let image = answer.image.as_option().unwrap();
    assert_eq!(
        (image.content_type.as_str(), image.bytes.as_slice()),
        ("image/png", path.as_bytes())
    );
    assert_eq!(kernel.sent()[0].request.url, format!("{ASSETS}{path}"));
}

#[test]
fn a_refused_key_names_the_api_key() {
    let mut kernel = kernel().http(|_request, _body| Ok(Response::new(401, "")));
    let heat = artwork_of(TitleKind::TITLE_KIND_MOVIE, "tmdb", "949");

    let err = with(&mut kernel, || Fanart::artwork(heat)).unwrap_err();

    assert_eq!(err.to_string(), "fanart.tv refused the API key");
}
