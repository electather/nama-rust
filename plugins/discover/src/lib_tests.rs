use nama_pdk::{
    HomeRowSource, HomeRows, Library, Permission, Response, SettingKind, Title, TitleId, TitleKind,
    TitleRef, TitleRefKind, UserRowsAsked, WatchState, WatchlistTitle,
    extism_pdk::{
        Error, HttpRequest,
        json::{self, Value, json},
    },
    nama_plugin_proto::nama::plugin::v1::Texts,
    services, setting_value,
    testing::{Kernel, with},
};
use url::Url;

use super::{
    API_KEY, Discover, PER_USER, REGION, SHARED, SHARED_ROWS, USER_ROWS, check_settings, manifest,
};

/// The API key the fake TMDB takes.
const VALID_KEY: &str = "fake-tmdb-key";

/// TMDB's answer to a GET of `request`'s URL: this week's trending titles, a person among them,
/// or its countries, refusing any key but [`VALID_KEY`] as TMDB does.
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

/// A Kernel holding the fake's API key and `picked` as the Shared rows, whose requests the fake
/// TMDB answers.
fn kernel(picked: &[&str]) -> Kernel {
    Kernel::new()
        .setting(API_KEY, VALID_KEY)
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

#[test]
fn trending_this_week_unpicked_is_no_row_and_asks_tmdb_nothing() {
    let mut kernel = kernel(&["top-rated-movies", "top-rated-shows"]);

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
fn the_settings_check_refuses_21_rows_for_each_user_without_asking_tmdb() {
    let picked: Vec<_> = PER_USER
        .iter()
        .cycle()
        .take(21)
        .map(|(key, _)| *key)
        .collect();
    let mut kernel =
        kernel(&["trending-week"]).setting(USER_ROWS, &json::to_string(&picked).unwrap());

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
    let picked = |values: [&str; 3]| {
        Some(
            Texts {
                values: values.map(str::to_owned).to_vec(),
                ..Texts::default()
            }
            .into(),
        )
    };
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
    let then = nama_pdk::now() - std::time::Duration::from_hours(24 * days);
    let since_epoch = then.duration_since(std::time::UNIX_EPOCH).unwrap();
    i64::try_from(since_epoch.as_millis()).unwrap()
}

fn tmdb_id(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

/// A Movie in Library 1, by its TMDB id.
fn movie(value: &str, genres: &[&str]) -> Title {
    Title {
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

/// An Episode in Library 1 of the Show with the TMDB id `show`.
fn episode(value: &str, show: &str, genres: &[&str]) -> Title {
    Title {
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
        .setting(USER_ROWS, &json::to_string(picked).unwrap())
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
