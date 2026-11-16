use nama_pdk::{
    TitleId,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
};

use super::{
    Episodes, Found, Login, Lookup, Series, artwork_path, content_type, data, encoded, manifest,
    season, title_id, tvdb_id,
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
