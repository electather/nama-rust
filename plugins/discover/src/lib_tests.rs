use nama_pdk::{
    HomeRowSource, HomeRows, Permission, Response, SettingKind, TitleKind,
    extism_pdk::{
        Error, HttpRequest,
        json::{self, Value, json},
    },
    nama_plugin_proto::nama::plugin::v1::Texts,
    services, setting_value,
    testing::{Kernel, with},
};
use url::Url;

use super::{API_KEY, Discover, REGION, SHARED, SHARED_ROWS, check_settings, manifest};

/// The API key the fake TMDB takes.
const VALID_KEY: &str = "fake-tmdb-key";

/// TMDB's answer to a GET of `request`'s URL: this week's trending titles, a person among them,
/// one Movie for a Movie list, one Show for a Show list, or its countries, refusing any key but [`VALID_KEY`] as TMDB does.
fn tmdb(request: &HttpRequest) -> Response {
    let url = Url::parse(&request.url).expect("the Plugin sends a URL");
    let key = url.query_pairs().find(|(name, _)| name == "api_key");
    if url.host_str() != Some("api.themoviedb.org") || key.is_none_or(|(_, key)| key != VALID_KEY) {
        return answer(
            401,
            &json!({ "status_code": 7, "status_message": "Invalid API key" }),
        );
    }
    match url.path() {
        "/3/trending/all/week" => answer(
            200,
            &json!({ "page": 1, "results": [
                { "id": 603, "media_type": "movie", "title": "The Matrix" },
                { "id": 287, "media_type": "person", "name": "Brad Pitt" },
                { "id": 1396, "media_type": "tv", "name": "Breaking Bad" },
            ] }),
        ),
        "/3/movie/popular"
        | "/3/movie/top_rated"
        | "/3/movie/now_playing"
        | "/3/movie/upcoming"
        | "/3/discover/movie" => answer(
            200,
            &json!({ "page": 1, "results": [{ "id": 550, "title": "Fight Club" }] }),
        ),
        "/3/tv/popular" | "/3/tv/top_rated" | "/3/tv/on_the_air" => answer(
            200,
            &json!({ "page": 1, "results": [{ "id": 1399, "name": "Game of Thrones" }] }),
        ),
        "/3/configuration/countries" => answer(
            200,
            &json!([
                { "iso_3166_1": "US", "english_name": "United States of America", "native_name": "United States" },
                { "iso_3166_1": "DE", "english_name": "Germany", "native_name": "Deutschland" },
            ]),
        ),
        _ => answer(404, &json!({ "status_code": 34 })),
    }
}

fn answer(status: u16, body: &Value) -> Response {
    Response::new(status, json::to_vec(body).unwrap_or_default())
}

/// A Kernel holding the fake's API key, Region's default and `picked` as the Shared rows, whose
/// requests the fake TMDB answers.
fn kernel(picked: &[&str]) -> Kernel {
    Kernel::new()
        .setting(API_KEY, VALID_KEY)
        .setting(REGION, "US")
        .setting(SHARED_ROWS, &json::to_string(picked).unwrap())
        .http(|request, _body| Ok(tmdb(request)))
}

fn shared_rows(kernel: &mut Kernel) -> Result<HomeRows, Error> {
    with(kernel, Discover::shared_rows)
}

/// A row's titles by kind and TMDB id.
type Titles<'a> = Vec<(Option<TitleKind>, &'a str)>;

/// Each row's key, heading, time to live, and titles.
fn summary(rows: &HomeRows) -> Vec<(&str, &str, u32, Titles<'_>)> {
    (rows.rows.iter())
        .map(|row| {
            let titles = (row.titles.iter())
                .map(|title| {
                    let id = title.id.as_option().expect("each title has an id");
                    assert_eq!(id.namespace, "tmdb");
                    (title.kind.as_known(), id.value.as_str())
                })
                .collect();
            (
                row.key.as_str(),
                row.heading.as_str(),
                row.ttl_seconds,
                titles,
            )
        })
        .collect()
}

#[test]
fn trending_this_week_picked_is_a_row_of_tmdbs_weekly_trending_movies_and_shows() {
    let rows = shared_rows(&mut kernel(&["trending-week"])).unwrap();

    assert_eq!(
        summary(&rows),
        [(
            "trending-week",
            "Trending this week",
            24 * 60 * 60,
            vec![
                (Some(TitleKind::TITLE_KIND_MOVIE), "603"),
                (Some(TitleKind::TITLE_KIND_SHOW), "1396"),
            ]
        )]
    );
}

/// What each request `kernel` sent asked TMDB: its path and query, leaving out the API key.
fn asked(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .map(|sent| {
            let url = Url::parse(&sent.request.url).expect("the Plugin sends a URL");
            let query: Vec<_> = (url.query_pairs())
                .filter(|(name, _)| name != "api_key")
                .map(|(name, value)| format!("{name}={value}"))
                .collect();
            format!("{}?{}", url.path(), query.join("&"))
        })
        .collect()
}

#[test]
fn every_tmdb_row_picked_is_answered_in_order_from_its_own_tmdb_list() {
    let every_row: Vec<_> = SHARED.iter().map(|(key, _)| *key).collect();
    let mut kernel = kernel(&every_row);

    let rows = shared_rows(&mut kernel).unwrap();

    let movie = vec![(Some(TitleKind::TITLE_KIND_MOVIE), "550")];
    let show = vec![(Some(TitleKind::TITLE_KIND_SHOW), "1399")];
    let day = 24 * 60 * 60;
    assert_eq!(
        summary(&rows)[1..],
        [
            ("popular-movies", "Popular movies", day, movie.clone()),
            ("popular-shows", "Popular shows", day, show.clone()),
            ("top-rated-movies", "Top rated movies", day, movie.clone()),
            ("top-rated-shows", "Top rated shows", day, show.clone()),
            ("now-in-theaters", "Now in theaters", day, movie.clone()),
            ("coming-soon", "Coming soon", day, movie.clone()),
            ("on-the-air", "On the air this week", day, show),
            (
                "critically-acclaimed",
                "Critically acclaimed",
                day,
                movie.clone()
            ),
            ("80s", "80s favourites", day, movie.clone()),
            ("90s", "90s favourites", day, movie.clone()),
            ("2000s", "2000s favourites", day, movie.clone()),
            ("under-90-minutes", "Under 90 minutes", day, movie.clone()),
            ("hidden-gems", "Hidden gems", day, movie),
        ]
    );
    let decade_favourites = "sort_by=vote_average.desc&vote_count.gte=1000";
    assert_eq!(
        asked(&kernel),
        [
            "/3/trending/all/week?".to_owned(),
            "/3/movie/popular?".to_owned(),
            "/3/tv/popular?".to_owned(),
            "/3/movie/top_rated?".to_owned(),
            "/3/tv/top_rated?".to_owned(),
            "/3/movie/now_playing?region=US".to_owned(),
            "/3/movie/upcoming?region=US".to_owned(),
            "/3/tv/on_the_air?".to_owned(),
            "/3/discover/movie?sort_by=popularity.desc&vote_average.gte=8&vote_count.gte=5000"
                .to_owned(),
            format!(
                "/3/discover/movie?primary_release_date.gte=1980-01-01&\
                 primary_release_date.lte=1989-12-31&{decade_favourites}"
            ),
            format!(
                "/3/discover/movie?primary_release_date.gte=1990-01-01&\
                 primary_release_date.lte=1999-12-31&{decade_favourites}"
            ),
            format!(
                "/3/discover/movie?primary_release_date.gte=2000-01-01&\
                 primary_release_date.lte=2009-12-31&{decade_favourites}"
            ),
            "/3/discover/movie?with_runtime.gte=60&with_runtime.lte=89&\
             sort_by=vote_average.desc&vote_count.gte=1000"
                .to_owned(),
            "/3/discover/movie?vote_average.gte=7.5&vote_count.gte=200&vote_count.lte=1000&\
             sort_by=vote_average.desc"
                .to_owned(),
        ]
    );
}

#[test]
fn now_in_theaters_and_coming_soon_ask_tmdb_for_the_region_set() {
    let mut kernel = kernel(&["now-in-theaters", "coming-soon"]).setting(REGION, "DE");

    let rows = shared_rows(&mut kernel).unwrap();

    assert_eq!(rows.rows.len(), 2);
    assert_eq!(
        asked(&kernel),
        [
            "/3/movie/now_playing?region=DE",
            "/3/movie/upcoming?region=DE"
        ]
    );
}

#[test]
fn only_the_picked_rows_are_answered_and_asked_of_tmdb() {
    let mut kernel = kernel(&["hidden-gems", "popular-shows"]);

    let rows = shared_rows(&mut kernel).unwrap();

    let keys: Vec<_> = rows.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["popular-shows", "hidden-gems"]);
    assert_eq!(kernel.sent().len(), 2);
}

#[test]
fn nothing_picked_is_no_row_and_asks_tmdb_nothing() {
    let mut kernel = kernel(&[]);

    let rows = shared_rows(&mut kernel).unwrap();

    assert_eq!(summary(&rows), []);
    assert_eq!(kernel.sent(), []);
}

#[test]
fn a_failed_tmdb_call_fails_the_refresh_so_the_kernel_keeps_the_old_rows() {
    let mut kernel =
        kernel(&["trending-week"]).http(|_request, _body| Ok(Response::new(503, Vec::new())));

    let err = shared_rows(&mut kernel).unwrap_err();

    assert_eq!(err.to_string(), "TMDB failed with HTTP status 503");
}

#[test]
fn the_settings_check_refuses_21_shared_rows_without_asking_tmdb() {
    let picked: Vec<_> = SHARED
        .iter()
        .cycle()
        .take(21)
        .map(|(key, _)| *key)
        .collect();
    let mut kernel = kernel(&picked);

    let err = with(&mut kernel, check_settings).unwrap_err();

    assert_eq!(err.to_string(), "pick at most 20 shared rows, not 21");
    assert_eq!(kernel.sent(), []);
}

#[test]
fn the_settings_check_refuses_an_api_key_tmdb_rejects() {
    let mut kernel = kernel(&["trending-week"]).setting(API_KEY, "wrong");

    let err = with(&mut kernel, check_settings).unwrap_err();

    assert_eq!(err.to_string(), "TMDB refused the API key");
}

#[test]
fn the_settings_check_offers_tmdbs_countries_by_name_for_region() {
    let checked = with(&mut kernel(&["trending-week"]), check_settings).unwrap();

    let regions: Vec<_> = (checked.choices[REGION].choices.iter())
        .map(|choice| (choice.value.as_str(), choice.label.as_str()))
        .collect();
    assert_eq!(
        regions,
        [("DE", "Germany"), ("US", "United States of America")]
    );
}

#[test]
fn manifest_serves_home_rows_with_its_permissions_api_key_region_and_shared_rows() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::HOME_ROW]);
    assert_eq!(
        manifest.permissions,
        [
            Permission::PERMISSION_READ_LIBRARIES,
            Permission::PERMISSION_READ_WATCH_STATES,
            Permission::PERMISSION_READ_WATCHLISTS,
            Permission::PERMISSION_STORE,
        ]
    );
    assert_eq!(manifest.allowed_hosts, ["api.themoviedb.org"]);
    let settings: Vec<_> = (manifest.settings.iter())
        .map(|field| {
            let default = field
                .default_value
                .as_option()
                .and_then(|value| value.value.clone());
            (
                field.key.as_str(),
                field.kind.as_known(),
                field.required,
                default,
            )
        })
        .collect();
    let picked = ["trending-week", "top-rated-movies", "top-rated-shows"].map(str::to_owned);
    assert_eq!(
        settings,
        [
            (
                "api_key",
                Some(SettingKind::SETTING_KIND_SECRET),
                true,
                None
            ),
            (
                "region",
                Some(SettingKind::SETTING_KIND_CHOICE),
                false,
                Some(setting_value::Value::TextValue("US".to_owned()))
            ),
            (
                "shared_rows",
                Some(SettingKind::SETTING_KIND_CHOICES),
                false,
                Some(
                    Texts {
                        values: picked.to_vec(),
                        ..Texts::default()
                    }
                    .into()
                )
            ),
        ]
    );
}

#[test]
fn manifest_offers_every_shared_row_of_discover_by_its_heading() {
    let manifest = manifest();

    let shared_rows = (manifest.settings.iter())
        .find(|field| field.key == "shared_rows")
        .expect("the manifest has Shared rows");
    let labels: Vec<_> = (shared_rows.choices.iter())
        .map(|choice| choice.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "Trending this week",
            "Popular movies",
            "Popular shows",
            "Top rated movies",
            "Top rated shows",
            "Now in theaters",
            "Coming soon",
            "On the air this week",
            "Critically acclaimed",
            "80s favourites",
            "90s favourites",
            "2000s favourites",
            "Under 90 minutes",
            "Hidden gems",
            "Best Picture winners",
            "Oscar nominees",
            "Palme d'Or winners",
            "Golden Globe best dramas",
        ]
    );
}

#[test]
fn manifest_says_where_to_get_a_tmdb_key_and_credits_tmdb() {
    let manifest = manifest();

    let setup = &manifest.setup_markdown;
    assert!(
        setup.contains("https://www.themoviedb.org/settings/api"),
        "{setup}"
    );
    assert!(
        manifest.description.contains("TMDB API"),
        "{}",
        manifest.description
    );
    assert_eq!(
        [manifest.homepage_url, manifest.source_url, manifest.license],
        [
            "https://www.themoviedb.org/",
            "https://github.com/electather/nama-rust/tree/main/plugins/discover",
            "AGPL-3.0-only",
        ]
    );
}
