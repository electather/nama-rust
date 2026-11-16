use nama_pdk::{
    ArtworkAnswer, ArtworkAsked, TitleId, TitleKind,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
};

use nama_pdk::ArtworkKind::{
    ARTWORK_KIND_BACKDROP, ARTWORK_KIND_CLEAR_ART, ARTWORK_KIND_DISC, ARTWORK_KIND_LOGO,
    ARTWORK_KIND_POSTER, ARTWORK_KIND_THUMB,
};

use super::{MOVIE, SHOW, artwork, asset_path, lookup, manifest, unsupported_id};

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
    assert_eq!(manifest.points[0].version, "^1.2.0");
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
