use std::time::{Duration, UNIX_EPOCH};

use nama_pdk::{
    HomeRowSource, HomeRows, Library, Permission, Response, SettingKind, Title, TitleId, TitleKind,
    TitleRef, TitleRefKind, UserRowsAsked, WatchState, WatchlistTitle,
    extism_pdk::{
        Error, HttpRequest,
        json::{self, Value, json},
    },
    kernel, services, setting_value,
    testing::{Kernel, with},
};
use url::Url;

use super::{
    API_KEY, Discover, PER_USER, REGION, SHARED, SHARED_ROWS, USER_ROWS, check_settings, manifest,
};

/// The API key the fake TMDB takes.
const VALID_KEY: &str = "fake-tmdb-key";

/// TMDB's answer to a GET of `request`'s URL: this week's trending titles, a person among them,
/// one Movie for a Movie list, one Show for a Show list, or its countries, refusing any key but
/// [`VALID_KEY`] as TMDB does.
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
        | "/3/movie/upcoming" => answer(
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
        "/3/genre/movie/list" => genres(&[
            (28, "Action"),
            (35, "Comedy"),
            (18, "Drama"),
            (27, "Horror"),
        ]),
        "/3/genre/tv/list" => genres(&[(35, "Comedy"), (18, "Drama"), (10765, "Sci-Fi & Fantasy")]),
        "/3/discover/movie"
            if url
                .query_pairs()
                .any(|(name, _)| name.starts_with("with_c")) =>
        {
            answer(200, &movies_of(&url))
        }
        "/3/discover/movie" | "/3/discover/tv" => {
            let asked = |name: &str| {
                url.query_pairs()
                    .find(|(asked_name, _)| asked_name == name)
                    .map(|(_, value)| value.into_owned())
            };
            // A search without a genre is a fixed row's, answered with the one Movie.
            let Some(genre) = asked("with_genres") else {
                return answer(
                    200,
                    &json!({ "page": 1, "results": [{ "id": 550, "title": "Fight Club" }] }),
                );
            };
            assert_eq!(asked("sort_by").as_deref(), Some("vote_average.desc"));
            page_after(&genre)
        }
        path => match path.split('/').collect::<Vec<_>>()[..] {
            ["", "3", "movie" | "tv", id, "recommendations"] => page_after(id),
            ["", "3", media @ ("movie" | "tv"), id] => match credits(media, id) {
                Some(credits) => answer(200, &credits),
                None => answer(404, &json!({ "status_code": 34 })),
            },
            ["", "3", "person", id, "tv_credits"] => answer(200, &shows_of(id)),
            _ => answer(404, &json!({ "status_code": 34 })),
        },
    }
}

const KEANU: u64 = 6384;
/// Billed 6th in each of The Matrix films.
const AGENT: u64 = 77;
const LANA: u64 = 9340;
const BRYAN: u64 = 17419;
const VINCE: u64 = 66633;

fn name(person: u64) -> String {
    match person {
        KEANU => "Keanu Reeves".to_owned(),
        AGENT => "Agent Brown".to_owned(),
        LANA => "Lana Wachowski".to_owned(),
        BRYAN => "Bryan Cranston".to_owned(),
        VINCE => "Vince Gilligan, who made Breaking Bad and more".to_owned(),
        _ => format!("Person {person}"),
    }
}

/// TMDB's answer for a Movie's or Show's details with its Cast and Crew: its Cast in order,
/// a producer, its Directors, and a Show's Creators; `None` for a title TMDB doesn't have.
fn credits(media: &str, title: &str) -> Option<Value> {
    let (cast, directors, creators): (&[u64], &[u64], &[u64]) = match (media, title) {
        ("movie", "603") => (&[KEANU, 1, 2, 3, 4, AGENT], &[LANA], &[]),
        ("movie", "604") => (&[KEANU, 5, 6, 7, 8, AGENT], &[LANA], &[]),
        ("movie", "605") => (&[9, 10, 11, 12, 13, AGENT], &[14], &[]),
        ("tv", "1396") => (&[BRYAN, 15], &[], &[VINCE]),
        _ => return None,
    };
    let person = |id: &u64| json!({ "id": id, "name": name(*id) });
    let cast: Vec<_> = (cast.iter().enumerate())
        .map(|(order, id)| json!({ "id": id, "name": name(*id), "character": "", "order": order }))
        .collect();
    let crew: Vec<_> = (directors.iter())
        .map(|id| json!({ "id": id, "name": name(*id), "job": "Director" }))
        .chain([json!({ "id": 99, "name": "A Producer", "job": "Producer" })])
        .collect();
    let mut details = json!({ "id": title, "credits": { "cast": cast, "crew": crew } });
    if media == "tv" {
        details["created_by"] = creators.iter().map(person).collect();
    }
    Some(details)
}

/// TMDB's answer to a search for the Movies a Person is in, by `with_cast` or `with_crew`.
fn movies_of(url: &Url) -> Value {
    let ids: &[u64] = match url
        .query_pairs()
        .find(|(name, _)| name.starts_with("with_"))
    {
        Some((role, id)) => match (role.as_ref(), id.as_ref()) {
            ("with_cast", "6384") => &[603, 624],
            ("with_crew", "9340") => &[604, 2000],
            _ => &[],
        },
        None => &[],
    };
    let results: Vec<_> = ids.iter().map(|id| json!({ "id": id })).collect();
    json!({ "page": 1, "results": results })
}

/// A talk show, on which a guest appears as themself.
const TALK_SHOW: u64 = 22980;

/// Shows by TMDB id, each with how many Episodes a Person has a Role in.
type Shows<'a> = &'a [(u64, u32)];

/// TMDB's answer for the Shows a Person is in the Cast or Crew of, with how many Episodes.
fn shows_of(person: &str) -> Value {
    let (cast, crew): (Shows<'_>, Shows<'_>) = match person {
        "6384" => (&[(TALK_SHOW, 9), (1400, 2), (1396, 1), (2100, 10)], &[]),
        "9340" => (&[], &[(70523, 8)]),
        _ => (&[], &[]),
    };
    let shows = |shows: Shows<'_>| -> Vec<_> {
        (shows.iter())
            .map(|&(id, episodes)| {
                let genres: &[u64] = if id == TALK_SHOW { &[10767, 35] } else { &[18] };
                json!({ "id": id, "genre_ids": genres, "episode_count": episodes })
            })
            .collect()
    };
    json!({ "cast": shows(cast), "crew": shows(crew) })
}

/// A page of two titles, numbered `id` with a 1 and then a 2 after it, without their media as
/// TMDB answers for a page of one media.
fn page_after(id: &str) -> Response {
    let results: Vec<_> = (1..=2)
        .map(|n| json!({ "id": format!("{id}{n}").parse::<u64>().unwrap() }))
        .collect();
    answer(200, &json!({ "page": 1, "results": results }))
}

fn genres(genres: &[(u64, &str)]) -> Response {
    let genres: Vec<_> = (genres.iter())
        .map(|(id, name)| json!({ "id": id, "name": name }))
        .collect();
    answer(200, &json!({ "genres": genres }))
}

/// Wikidata's answer to a SPARQL query of `request`'s URL: each award's Movies as TMDB ids, newest
/// first, a Best Picture winner Wikidata has no TMDB id for among them.
fn wikidata(request: &HttpRequest) -> Response {
    let url = Url::parse(&request.url).expect("the Plugin sends a URL");
    let query = (url.query_pairs())
        .find_map(|(name, query)| (name == "query").then(|| query.into_owned()))
        .unwrap_or_default();
    let movies: &[&str] = if query.contains("P1411") {
        &["1064213", "872585"]
    } else if query.contains("Q102427") {
        &["1054867", "", "776503"]
    } else if query.contains("Q179808") {
        &["1401459", "1456349"]
    } else if query.contains("Q1011509") {
        &["549509"]
    } else {
        return answer(400, &json!({}));
    };
    let bindings: Vec<_> = (movies.iter())
        .map(|&tmdb| match tmdb {
            "" => json!({ "newest": { "type": "literal", "value": "2023-01-01T00:00:00Z" } }),
            tmdb => json!({ "tmdb": { "type": "literal", "value": tmdb } }),
        })
        .collect();
    answer(
        200,
        &json!({ "head": { "vars": ["tmdb", "newest"] }, "results": { "bindings": bindings } }),
    )
}

fn answer(status: u16, body: &Value) -> Response {
    Response::new(status, json::to_vec(body).unwrap_or_default())
}

/// A Kernel holding the fake's API key, Region's default and `picked` as the Shared rows, granting
/// the Store, whose requests the fake TMDB and Wikidata answer.
fn kernel(picked: &[&str]) -> Kernel {
    Kernel::new()
        .setting(API_KEY, VALID_KEY)
        .setting(REGION, "US")
        .setting_picked(SHARED_ROWS, picked)
        .store()
        .http(|request, _body| {
            let host = Url::parse(&request.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned));
            Ok(match host.as_deref() {
                Some("query.wikidata.org") => wikidata(request),
                _ => tmdb(request),
            })
        })
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
    let tmdb_rows: Vec<_> = (SHARED.iter())
        .map(|(key, _)| *key)
        .filter(|key| crate::award_query(key).is_none())
        .collect();
    let mut kernel = kernel(&tmdb_rows);

    let rows = shared_rows(&mut kernel).unwrap();

    let movie = vec![(Some(TitleKind::TITLE_KIND_MOVIE), "550")];
    let show = vec![(Some(TitleKind::TITLE_KIND_SHOW), "1399")];
    let day = 24 * 60 * 60;
    let trending = vec![
        (Some(TitleKind::TITLE_KIND_MOVIE), "603"),
        (Some(TitleKind::TITLE_KIND_SHOW), "1396"),
    ];
    assert_eq!(
        summary(&rows),
        [
            ("trending-week", "Trending this week", day, trending),
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

/// An award Movie with the TMDB id `id`, as a row summary lists it.
fn award_movie(id: &str) -> (Option<TitleKind>, &str) {
    (Some(TitleKind::TITLE_KIND_MOVIE), id)
}

#[test]
fn award_rows_are_only_those_picked_each_of_wikidatas_movies_in_its_order() {
    let mut kernel = kernel(&["palme-dor", "best-picture"]);

    let rows = shared_rows(&mut kernel).unwrap();

    let day = 24 * 60 * 60;
    assert_eq!(
        summary(&rows),
        [
            (
                "best-picture",
                "Best Picture winners",
                day,
                vec![award_movie("1054867"), award_movie("776503")]
            ),
            (
                "palme-dor",
                "Palme d'Or winners",
                day,
                vec![award_movie("1401459"), award_movie("1456349")]
            ),
        ]
    );
    assert_eq!(kernel.sent().len(), 2);
}

#[test]
fn award_movies_wikidata_has_no_tmdb_id_for_are_left_out() {
    let rows = shared_rows(&mut kernel(&["best-picture"])).unwrap();

    let movies: Vec<_> = rows.rows[0]
        .titles
        .iter()
        .map(|title| &title.id.value)
        .collect();
    assert_eq!(movies, ["1054867", "776503"]);
}

#[test]
fn wikidata_is_asked_with_a_user_agent_naming_nama() {
    let mut kernel = kernel(&["golden-globe-dramas"]);

    shared_rows(&mut kernel).unwrap();

    let user_agent = &kernel.sent()[0].request.headers["User-Agent"];
    assert!(user_agent.starts_with("Nama-Discover/"), "{user_agent}");
}

#[test]
fn a_refresh_while_award_movies_are_fresh_reads_the_store_and_asks_wikidata_nothing() {
    let mut kernel = kernel(&["oscar-nominees"]);
    let first = shared_rows(&mut kernel).unwrap();
    let asked = kernel.sent().len();

    let again = shared_rows(&mut kernel).unwrap();

    assert_eq!(summary(&again), summary(&first));
    assert_eq!(kernel.sent().len(), asked);
}

#[test]
fn a_refresh_a_month_on_asks_wikidata_again_and_keeps_its_new_answer() {
    let mut kernel = kernel(&["palme-dor"]);
    let month_ago = nama_pdk::now() - Duration::from_hours(31 * 24);
    let asked_at = month_ago.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let kept = json!({ "asked_at": asked_at, "movies": [550] });
    with(&mut kernel, || {
        kernel::store_put("award:palme-dor", None, &json::to_vec(&kept).unwrap())
    })
    .unwrap();

    let refreshed = shared_rows(&mut kernel).unwrap();
    let again = shared_rows(&mut kernel).unwrap();

    let movies = vec![award_movie("1401459"), award_movie("1456349")];
    assert_eq!(summary(&refreshed)[0].3, movies);
    assert_eq!(summary(&again)[0].3, movies);
    assert_eq!(kernel.sent().len(), 1);
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
fn the_settings_check_refuses_21_rows_for_each_user_without_asking_tmdb() {
    let picked: Vec<_> = PER_USER
        .iter()
        .cycle()
        .take(21)
        .map(|(key, _)| *key)
        .collect();
    let mut kernel = kernel(&["trending-week"]).setting_picked(USER_ROWS, picked);

    let err = with(&mut kernel, check_settings).unwrap_err();

    assert_eq!(
        err.to_string(),
        "pick at most 20 rows for each User, not 21"
    );
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
fn manifest_serves_home_rows_with_its_permissions_api_key_region_shared_and_user_rows() {
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
    assert_eq!(
        manifest.allowed_hosts,
        ["api.themoviedb.org", "query.wikidata.org"]
    );
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
    let picked = |values: [&str; 3]| nama_pdk::picked_default(values).into_option()?.value;
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
                picked(["trending-week", "top-rated-movies", "top-rated-shows"])
            ),
            (
                "user_rows",
                Some(SettingKind::SETTING_KIND_CHOICES),
                false,
                picked(["because", "more-genre", "watchlist"])
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
fn manifest_offers_every_row_for_each_user_by_its_heading() {
    let manifest = manifest();

    let user_rows = (manifest.settings.iter())
        .find(|field| field.key == "user_rows")
        .expect("the manifest has Your rows");
    let labels: Vec<_> = (user_rows.choices.iter())
        .map(|choice| choice.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "Because you watched",
            "More genre",
            "Starring",
            "From",
            "Watch it again",
            "Your Watchlist",
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

const ADA: i64 = 1;
const BEN: i64 = 2;

/// `days` days before now, in milliseconds since the Unix epoch, as a Watch State's time.
fn days_ago(days: u64) -> i64 {
    let then = nama_pdk::now() - Duration::from_hours(24 * days);
    let since_epoch = then.duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(since_epoch.as_millis()).unwrap()
}

fn tmdb_id(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

/// A Movie in Library 1, by its TMDB id, named `Movie <id>`.
fn movie(value: &str, genres: &[&str]) -> Title {
    Title {
        title: format!("Movie {value}"),
        title_ref: TitleRef {
            library_id: 1,
            kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
            id: tmdb_id(value).into(),
            ..TitleRef::default()
        }
        .into(),
        genres: genres.iter().map(|&genre| genre.to_owned()).collect(),
        ..Title::default()
    }
}

/// An Episode in Library 1 of the Show with the TMDB id `show`, named `Show <show>`.
fn episode(value: &str, show: &str, genres: &[&str]) -> Title {
    Title {
        show_title: Some(format!("Show {show}")),
        title_ref: TitleRef {
            kind: TitleRefKind::TITLE_REF_KIND_EPISODE.into(),
            ..(*movie(value, genres).title_ref).clone()
        }
        .into(),
        show_id: tmdb_id(show).into(),
        ..movie(value, genres)
    }
}

fn tvdb_id(value: &str) -> TitleId {
    TitleId {
        namespace: "tvdb".to_owned(),
        ..tmdb_id(value)
    }
}

/// An Episode in Library 1 of the Show listed under `show`, which also has the id `also`.
fn episode_also_known_as(value: &str, show: TitleId, also: TitleId) -> Title {
    Title {
        show_ids: vec![show.clone(), also],
        show_id: show.into(),
        ..episode(value, "", &[])
    }
}

fn watchlisted(kind: TitleKind, value: &str) -> WatchlistTitle {
    WatchlistTitle {
        kind: kind.into(),
        id: tmdb_id(value).into(),
        ..WatchlistTitle::default()
    }
}

/// A Kernel holding `picked` as Your rows and `titles` in Library 1, granting Read Watch States
/// and Read Watchlists, with no Watch State and every Watchlist empty yet.
fn user_kernel(picked: &[&str], titles: &[Title]) -> Kernel {
    let library = Library {
        id: 1,
        ..Library::default()
    };
    Kernel::new()
        .setting(API_KEY, VALID_KEY)
        .setting_picked(USER_ROWS, picked)
        .http(|request, _body| Ok(tmdb(request)))
        .libraries([library])
        .titles(1, titles.iter().cloned())
        .watch_states(&TitleRef::default(), [])
        .watchlist(ADA, [])
}

/// `kernel` with the User `user_id`'s Watch State of `title`: `played`, or stopped `position_ms`
/// in, last changed at `changed_at_ms`.
fn watching(
    kernel: Kernel,
    title: &Title,
    user_id: i64,
    (played, position_ms): (bool, u64),
    changed_at_ms: i64,
) -> Kernel {
    let state = WatchState {
        user_id,
        played,
        position_ms,
        updated_at_ms: changed_at_ms,
        ..WatchState::default()
    };
    kernel.watch_states(&title.title_ref, [state])
}

const PLAYED: (bool, u64) = (true, 0);
const STARTED: (bool, u64) = (false, 60_000);

fn user_rows(kernel: &mut Kernel, user_id: i64) -> Result<HomeRows, Error> {
    let asked = UserRowsAsked {
        user_id,
        ..UserRowsAsked::default()
    };
    with(kernel, || Discover::user_rows(asked))
}

#[test]
fn your_watchlist_leaves_out_started_and_played_titles_newest_added_first() {
    let titles = [
        movie("1", &[]),
        movie("2", &[]),
        movie("3", &[]),
        episode("10", "100", &[]),
    ];
    let kernel = user_kernel(&["watchlist"], &titles).watchlist(
        ADA,
        [
            watchlisted(TitleKind::TITLE_KIND_MOVIE, "3"),
            watchlisted(TitleKind::TITLE_KIND_MOVIE, "2"),
            watchlisted(TitleKind::TITLE_KIND_SHOW, "100"),
            watchlisted(TitleKind::TITLE_KIND_MOVIE, "1"),
        ],
    );
    let kernel = watching(kernel, &titles[1], ADA, PLAYED, days_ago(2));
    let mut kernel = watching(kernel, &titles[3], ADA, STARTED, days_ago(1));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows),
        [(
            "watchlist",
            "Your Watchlist",
            6 * 60 * 60,
            vec![
                (Some(TitleKind::TITLE_KIND_MOVIE), "3"),
                (Some(TitleKind::TITLE_KIND_MOVIE), "1"),
            ]
        )]
    );
}

#[test]
fn your_watchlist_keeps_a_title_the_server_lacks_for_the_kernel_to_offer_as_requestable() {
    let mut kernel = user_kernel(&["watchlist"], &[])
        .watchlist(ADA, [watchlisted(TitleKind::TITLE_KIND_SHOW, "1396")]);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows)[0].3,
        [(Some(TitleKind::TITLE_KIND_SHOW), "1396")]
    );
}

#[test]
fn your_watchlist_leaves_out_a_started_show_whichever_of_its_ids_names_it() {
    let titles = [
        episode_also_known_as("10", tvdb_id("334824"), tmdb_id("70523")),
        episode_also_known_as("20", tmdb_id("1396"), tvdb_id("81189")),
        movie("1", &[]),
    ];
    let by_tvdb = WatchlistTitle {
        id: tvdb_id("81189").into(),
        ..watchlisted(TitleKind::TITLE_KIND_SHOW, "")
    };
    let kernel = user_kernel(&["watchlist"], &titles).watchlist(
        ADA,
        [
            watchlisted(TitleKind::TITLE_KIND_SHOW, "70523"),
            by_tvdb,
            watchlisted(TitleKind::TITLE_KIND_MOVIE, "1"),
        ],
    );
    let kernel = watching(kernel, &titles[0], ADA, STARTED, days_ago(1));
    let mut kernel = watching(kernel, &titles[1], ADA, PLAYED, days_ago(1));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows)[0].3,
        [(Some(TitleKind::TITLE_KIND_MOVIE), "1")]
    );
}

#[test]
fn watch_it_again_names_a_show_whose_episodes_list_it_under_two_ids_once_by_its_tmdb_id() {
    let titles = [
        episode_also_known_as("10", tvdb_id("334824"), tmdb_id("70523")),
        episode("11", "70523", &[]),
    ];
    let kernel = user_kernel(&["watch-again"], &titles);
    let kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(400));
    let mut kernel = watching(kernel, &titles[1], ADA, PLAYED, days_ago(400));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows)[0].3,
        [(Some(TitleKind::TITLE_KIND_SHOW), "70523")]
    );
}

#[test]
fn watch_it_again_holds_only_titles_played_through_over_a_year_ago_most_played_genre_first() {
    let titles = [
        movie("1", &["Drama"]),
        movie("2", &["Horror"]),
        movie("3", &["Horror"]),
        movie("4", &["Horror"]),
        movie("5", &["Horror"]),
        movie("6", &["Horror"]),
        episode("10", "100", &["Comedy"]),
        episode("11", "100", &["Comedy"]),
        episode("20", "200", &["Comedy"]),
        episode("21", "200", &["Comedy"]),
    ];
    let mut kernel = user_kernel(&["watch-again"], &titles);
    for (title, watched, changed_at) in [
        (&titles[0], PLAYED, days_ago(400)),
        (&titles[1], PLAYED, days_ago(500)),
        (&titles[2], PLAYED, days_ago(30)),
        (&titles[3], STARTED, days_ago(400)),
        (&titles[4], PLAYED, days_ago(400)),
        (&titles[6], PLAYED, days_ago(400)),
        (&titles[7], PLAYED, days_ago(400)),
        (&titles[8], PLAYED, days_ago(400)),
    ] {
        kernel = watching(kernel, title, ADA, watched, changed_at);
    }

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows),
        [(
            "watch-again",
            "Watch it again",
            6 * 60 * 60,
            vec![
                (Some(TitleKind::TITLE_KIND_MOVIE), "2"),
                (Some(TitleKind::TITLE_KIND_MOVIE), "5"),
                (Some(TitleKind::TITLE_KIND_SHOW), "100"),
                (Some(TitleKind::TITLE_KIND_MOVIE), "1"),
            ]
        )]
    );
}

#[test]
fn watch_it_again_leaves_out_a_title_played_over_a_year_ago_and_again_since() {
    let titles = [episode("10", "100", &[]), episode("11", "100", &[])];
    let kernel = user_kernel(&["watch-again"], &titles);
    let kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(400));
    let mut kernel = watching(kernel, &titles[1], ADA, PLAYED, days_ago(3));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows),
        [("watch-again", "Watch it again", 6 * 60 * 60, vec![])]
    );
}

#[test]
fn two_users_with_different_histories_get_different_rows() {
    let titles = [movie("1", &[]), movie("2", &[])];
    let kernel = user_kernel(&["watch-again", "watchlist"], &titles)
        .watchlist(ADA, [watchlisted(TitleKind::TITLE_KIND_MOVIE, "2")])
        .watchlist(BEN, [watchlisted(TitleKind::TITLE_KIND_MOVIE, "1")]);
    let kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(400));
    let mut kernel = watching(kernel, &titles[1], BEN, PLAYED, days_ago(400));

    let adas = user_rows(&mut kernel, ADA).unwrap();
    let bens = user_rows(&mut kernel, BEN).unwrap();

    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    let ttl = 6 * 60 * 60;
    assert_eq!(
        summary(&adas),
        [
            ("watch-again", "Watch it again", ttl, vec![(movie, "1")]),
            ("watchlist", "Your Watchlist", ttl, vec![(movie, "2")]),
        ]
    );
    assert_eq!(
        summary(&bens),
        [
            ("watch-again", "Watch it again", ttl, vec![(movie, "2")]),
            ("watchlist", "Your Watchlist", ttl, vec![(movie, "1")]),
        ]
    );
}

#[test]
fn rows_for_each_user_left_unpicked_are_not_answered() {
    let titles = [movie("1", &[])];
    let kernel = user_kernel(&["watchlist"], &titles)
        .watchlist(ADA, [watchlisted(TitleKind::TITLE_KIND_MOVIE, "2")]);
    let mut kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(400));
    let mut none_picked = user_kernel(&[], &titles);

    let rows = user_rows(&mut kernel, ADA).unwrap();
    let no_rows = user_rows(&mut none_picked, ADA).unwrap();

    let keys: Vec<_> = rows.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["watchlist"]);
    assert_eq!(no_rows.rows, []);
}

#[test]
fn watch_it_again_is_answered_without_your_watchlist_when_read_watchlists_is_not_granted() {
    let titles = [movie("1", &[])];
    let kernel = Kernel::new()
        .setting(USER_ROWS, r#"["watch-again","watchlist"]"#)
        .libraries([Library {
            id: 1,
            ..Library::default()
        }])
        .titles(1, titles.iter().cloned());
    let mut kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(400));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows),
        [(
            "watch-again",
            "Watch it again",
            6 * 60 * 60,
            vec![(Some(TitleKind::TITLE_KIND_MOVIE), "1")]
        )]
    );
}

#[test]
fn because_you_watched_follows_the_three_titles_played_most_recently_newest_first() {
    let titles = [
        movie("1", &[]),
        movie("2", &[]),
        episode("30", "3", &[]),
        movie("4", &[]),
        movie("5", &[]),
    ];
    let mut kernel = user_kernel(&["because"], &titles);
    for (title, changed_at) in [
        (&titles[0], days_ago(4)),
        (&titles[1], days_ago(1)),
        (&titles[2], days_ago(2)),
        (&titles[3], days_ago(3)),
    ] {
        kernel = watching(kernel, title, ADA, PLAYED, changed_at);
    }
    let mut kernel = watching(kernel, &titles[4], ADA, STARTED, days_ago(0));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    let show = Some(TitleKind::TITLE_KIND_SHOW);
    let ttl = 6 * 60 * 60;
    assert_eq!(
        summary(&rows),
        [
            (
                "because-1",
                "Because you watched Movie 2",
                ttl,
                vec![(movie, "21"), (movie, "22")]
            ),
            (
                "because-2",
                "Because you watched Show 3",
                ttl,
                vec![(show, "31"), (show, "32")]
            ),
            (
                "because-3",
                "Because you watched Movie 4",
                ttl,
                vec![(movie, "41"), (movie, "42")]
            ),
        ]
    );
}

#[test]
fn because_you_watched_keeps_its_keys_as_the_titles_it_follows_change() {
    let titles = [movie("1", &[]), movie("2", &[])];
    let kernel = user_kernel(&["because"], &titles);
    let mut kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(2));
    let before = user_rows(&mut kernel, ADA).unwrap();
    let mut kernel = watching(kernel, &titles[1], ADA, PLAYED, days_ago(1));

    let after = user_rows(&mut kernel, ADA).unwrap();

    let keys_and_headings = |rows: &HomeRows| -> Vec<(String, String)> {
        (rows.rows.iter())
            .map(|row| (row.key.clone(), row.heading.clone()))
            .collect()
    };
    assert_eq!(
        keys_and_headings(&before),
        [("because-1".into(), "Because you watched Movie 1".into())]
    );
    assert_eq!(
        keys_and_headings(&after),
        [
            ("because-1".into(), "Because you watched Movie 2".into()),
            ("because-2".into(), "Because you watched Movie 1".into()),
        ]
    );
}

#[test]
fn because_you_watched_cuts_a_long_title_to_40_characters_ending_in_an_ellipsis() {
    let titles = [Title {
        title: "The Lord of the Rings: The Return of the King".to_owned(),
        ..movie("1", &[])
    }];
    let kernel = user_kernel(&["because"], &titles);
    let mut kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(1));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        rows.rows[0].heading,
        "Because you watched The Lord of the Rin…"
    );
}

#[test]
fn because_you_watched_leaves_out_titles_the_user_played_or_started() {
    let titles = [movie("1", &[]), movie("11", &[]), movie("12", &[])];
    let kernel = user_kernel(&["because"], &titles);
    let kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(1));
    let kernel = watching(kernel, &titles[1], ADA, PLAYED, days_ago(5));
    let mut kernel = watching(kernel, &titles[2], ADA, STARTED, days_ago(3));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    let ttl = 6 * 60 * 60;
    assert_eq!(
        summary(&rows),
        [
            ("because-1", "Because you watched Movie 1", ttl, vec![]),
            (
                "because-2",
                "Because you watched Movie 11",
                ttl,
                vec![(movie, "111"), (movie, "112")]
            ),
        ]
    );
}

#[test]
fn because_you_watched_follows_a_title_watched_past_halfway_but_not_one_stopped_before() {
    let titles = [movie("1", &[]), movie("2", &[])];
    let stopped_at = |position_ms, updated_at_ms| WatchState {
        user_id: ADA,
        position_ms,
        duration_ms: 90 * 60_000,
        updated_at_ms,
        ..WatchState::default()
    };
    let mut kernel = user_kernel(&["because"], &titles)
        .watch_states(&titles[0].title_ref, [stopped_at(50 * 60_000, days_ago(2))])
        .watch_states(&titles[1].title_ref, [stopped_at(10 * 60_000, days_ago(1))]);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let headings: Vec<_> = rows.rows.iter().map(|row| row.heading.as_str()).collect();
    assert_eq!(headings, ["Because you watched Movie 1"]);
}

#[test]
fn more_genre_follows_the_two_genres_tmdb_knows_the_user_played_most_leaving_out_played_titles() {
    let titles = [
        movie("1", &["Drama", "Anime"]),
        movie("181", &["Drama", "Anime"]),
        episode("1000", "100", &["Drama", "Comedy", "Anime"]),
        movie("3", &["Comedy", "Anime"]),
        movie("4", &["Horror"]),
        movie("5", &["Comedy"]),
    ];
    let mut kernel = user_kernel(&["more-genre"], &titles);
    for title in &titles[..5] {
        kernel = watching(kernel, title, ADA, PLAYED, days_ago(1));
    }
    let mut kernel = watching(kernel, &titles[5], ADA, STARTED, days_ago(1));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    let show = Some(TitleKind::TITLE_KIND_SHOW);
    let ttl = 6 * 60 * 60;
    assert_eq!(
        summary(&rows),
        [
            (
                "more-genre-1",
                "More Drama",
                ttl,
                vec![(show, "181"), (movie, "182"), (show, "182")]
            ),
            (
                "more-genre-2",
                "More Comedy",
                ttl,
                vec![(movie, "351"), (show, "351"), (movie, "352"), (show, "352")]
            ),
        ]
    );
}

#[test]
fn more_genre_of_a_genre_tmdb_has_only_for_shows_holds_shows() {
    let titles = [episode("10", "100", &["Sci-Fi & Fantasy"])];
    let kernel = user_kernel(&["more-genre"], &titles);
    let mut kernel = watching(kernel, &titles[0], ADA, PLAYED, days_ago(1));

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let show = Some(TitleKind::TITLE_KIND_SHOW);
    assert_eq!(
        summary(&rows),
        [(
            "more-genre-1",
            "More Sci-Fi & Fantasy",
            6 * 60 * 60,
            vec![(show, "107651"), (show, "107652")]
        )]
    );
}

/// A Kernel holding `titles` in Library 1, the fake's API key and an empty Store, with Starring
/// and From picked, whose requests the fake TMDB answers.
fn person_kernel(titles: &[Title]) -> Kernel {
    user_kernel(&["starring", "from"], titles)
        .setting(API_KEY, VALID_KEY)
        .store()
        .http(|request, _body| Ok(tmdb(request)))
}

/// `kernel` with Ada having played each of `titles`.
fn played(mut kernel: Kernel, titles: &[&Title]) -> Kernel {
    for title in titles {
        kernel = watching(kernel, title, ADA, PLAYED, days_ago(1));
    }
    kernel
}

/// The paths of the titles whose credits the Plugin asked TMDB for, in `sent`.
fn credits_asked(sent: &[nama_pdk::testing::Sent]) -> Vec<String> {
    (sent.iter())
        .map(|sent| Url::parse(&sent.request.url).unwrap())
        .filter(|url| {
            url.query_pairs()
                .any(|(name, _)| name == "append_to_response")
        })
        .map(|url| url.path().to_owned())
        .collect()
}

#[test]
fn starring_and_from_follow_the_people_most_in_what_the_user_played_leaving_out_played_titles() {
    let titles = [
        movie("603", &[]),
        movie("604", &[]),
        episode("10", "1396", &[]),
    ];
    let kernel = person_kernel(&titles);
    let mut kernel = played(kernel, &[&titles[0], &titles[1], &titles[2]]);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    let show = Some(TitleKind::TITLE_KIND_SHOW);
    let ttl = 6 * 60 * 60;
    assert_eq!(
        summary(&rows),
        [
            (
                "starring",
                "Starring Keanu Reeves",
                ttl,
                vec![(movie, "624"), (show, "2100"), (show, "1400")]
            ),
            (
                "from",
                "From Lana Wachowski",
                ttl,
                vec![(movie, "2000"), (show, "70523")]
            ),
        ]
    );
}

#[test]
fn an_actor_billed_6th_never_gets_a_starring_row_however_often_they_appear() {
    let titles = [movie("603", &[]), movie("604", &[]), movie("605", &[])];
    let kernel = person_kernel(&titles);
    let mut kernel = played(kernel, &[&titles[0], &titles[1], &titles[2]]);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let headings: Vec<_> = rows.rows.iter().map(|row| row.heading.as_str()).collect();
    assert_eq!(headings, ["Starring Keanu Reeves", "From Lana Wachowski"]);
}

#[test]
fn a_played_title_tmdb_lacks_counts_no_one_and_fails_no_refresh() {
    let titles = [movie("603", &[]), movie("1", &[])];
    let mut kernel = played(person_kernel(&titles), &[&titles[0], &titles[1]]);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let headings: Vec<_> = rows.rows.iter().map(|row| row.heading.as_str()).collect();
    assert_eq!(headings, ["Starring Keanu Reeves", "From Lana Wachowski"]);
}

#[test]
fn a_second_refresh_asks_tmdb_for_credits_only_of_titles_played_since_the_first() {
    let titles = [movie("603", &[]), episode("10", "1396", &[])];
    let mut kernel = played(person_kernel(&titles), &[&titles[0]]);
    user_rows(&mut kernel, ADA).unwrap();
    let mut kernel = played(kernel, &[&titles[1]]);
    let first_refresh = kernel.sent().len();

    user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        credits_asked(&kernel.sent()[..first_refresh]),
        ["/3/movie/603"]
    );
    assert_eq!(
        credits_asked(&kernel.sent()[first_refresh..]),
        ["/3/tv/1396"]
    );
}

#[test]
fn a_person_row_heading_past_40_characters_is_cut_with_an_ellipsis() {
    let titles = [episode("10", "1396", &[])];
    let mut kernel = played(person_kernel(&titles), &[&titles[0]]);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    let headings: Vec<_> = rows.rows.iter().map(|row| row.heading.as_str()).collect();
    assert_eq!(
        headings,
        [
            "Starring Bryan Cranston",
            "From Vince Gilligan, who made Breaking …"
        ]
    );
}

#[test]
fn starring_and_from_are_no_rows_for_a_user_who_played_nothing() {
    let titles = [movie("603", &[])];
    let mut kernel = watching(
        person_kernel(&titles),
        &titles[0],
        ADA,
        STARTED,
        days_ago(1),
    );

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(rows.rows, []);
    assert_eq!(kernel.sent(), []);
}
