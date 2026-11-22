use nama_pdk::{
    FeaturedSuggestions, HomeRowSource, HomeRows, Library, Permission, SettingKind, Title, TitleId,
    TitleKind, TitleRef, TitleRefKind, TitleWatchStates, UserFeaturedAsked, UserRowsAsked,
    WatchState, WatchlistTitle,
    extism_pdk::Error,
    services,
    testing::{Kernel, with},
};

use super::{DEFAULT_FAMILY_RATINGS, FAMILY_RATINGS, TopPicks, featured, manifest, ranked, rows};

const ADA: i64 = 1;
const BEN: i64 = 2;

fn id(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

fn tvdb(value: &str) -> TitleId {
    TitleId {
        namespace: "tvdb".to_owned(),
        ..id(value)
    }
}

fn title(kind: TitleRefKind, value: &str, genres: &[&str], rating: Option<&str>) -> Title {
    Title {
        title_ref: TitleRef {
            library_id: 1,
            kind: kind.into(),
            id: id(value).into(),
            ..TitleRef::default()
        }
        .into(),
        genres: genres.iter().map(|&genre| genre.to_owned()).collect(),
        content_rating: rating.map(str::to_owned),
        ..Title::default()
    }
}

fn movie(value: &str, genres: &[&str]) -> Title {
    title(
        TitleRefKind::TITLE_REF_KIND_MOVIE,
        value,
        genres,
        Some("PG"),
    )
}

fn rated(value: &str, rating: Option<&str>) -> Title {
    title(TitleRefKind::TITLE_REF_KIND_MOVIE, value, &[], rating)
}

fn episode(value: &str, show: &str) -> Title {
    Title {
        show_id: id(show).into(),
        ..title(
            TitleRefKind::TITLE_REF_KIND_EPISODE,
            value,
            &[],
            Some("TV-G"),
        )
    }
}

/// A Movie a Library preferring TVDB lists by the TVDB id `value`, with the TMDB id `tmdb` among
/// its ids (ADR 0042).
fn tvdb_movie(value: &str, tmdb: &str) -> Title {
    let mut title = movie(value, &[]);
    title.title_ref.get_or_insert_default().id = tvdb(value).into();
    title.ids = vec![tvdb(value), id(tmdb)];
    title
}

/// An Episode of the Show a Library preferring TVDB lists by the TVDB id `show`, with `others`
/// among its Show's ids.
fn tvdb_episode(value: &str, show: &str, others: &[TitleId]) -> Title {
    Title {
        show_id: tvdb(show).into(),
        show_ids: [&[tvdb(show)], others].concat(),
        ..episode(value, show)
    }
}

/// The User's Watch State of `title`: `played`, or stopped `position_ms` in.
fn watched(title: &Title, played: bool, position_ms: u64) -> TitleWatchStates {
    TitleWatchStates {
        title_ref: title.title_ref.clone(),
        states: vec![state(ADA, played, position_ms)],
        ..TitleWatchStates::default()
    }
}

fn unwatched(title: &Title) -> TitleWatchStates {
    TitleWatchStates {
        title_ref: title.title_ref.clone(),
        ..TitleWatchStates::default()
    }
}

/// Each row's key and its titles' ids.
fn summary(rows: &HomeRows) -> Vec<(&str, Vec<&str>)> {
    (rows.rows.iter())
        .map(|row| {
            let ids = (row.titles.iter())
                .map(|title| title.id.value.as_str())
                .collect();
            (row.key.as_str(), ids)
        })
        .collect()
}

/// The Top picks row's ids, of `titles` watched as `states`.
fn top_picks(
    titles: &[Title],
    states: &[TitleWatchStates],
    watchlist: &[WatchlistTitle],
) -> Vec<String> {
    let rows = rows(&ranked(titles, states, watchlist), &DEFAULT_FAMILY_RATINGS);
    summary(&rows)[0]
        .1
        .iter()
        .map(|&id| id.to_owned())
        .collect()
}

fn watchlisted(kind: TitleKind, value: &str) -> WatchlistTitle {
    WatchlistTitle {
        kind: kind.into(),
        id: id(value).into(),
        ..WatchlistTitle::default()
    }
}

#[test]
fn two_users_with_different_histories_get_different_top_picks() {
    let titles = [
        movie("1", &["Horror"]),
        movie("2", &["Comedy"]),
        movie("3", &["Horror", "Thriller"]),
        movie("4", &["Comedy"]),
    ];
    let adas = [
        watched(&titles[0], true, 0),
        unwatched(&titles[1]),
        unwatched(&titles[2]),
        unwatched(&titles[3]),
    ];
    let bens = [
        unwatched(&titles[0]),
        watched(&titles[1], true, 0),
        unwatched(&titles[2]),
        unwatched(&titles[3]),
    ];

    assert_eq!(top_picks(&titles, &adas, &[]), ["3", "2", "4"]);
    assert_eq!(top_picks(&titles, &bens, &[]), ["4", "1", "3"]);
}

#[test]
fn a_watchlisted_title_outranks_the_users_genres() {
    let titles = [
        movie("1", &["Horror"]),
        movie("2", &["Comedy"]),
        movie("3", &["Horror"]),
    ];
    let states = [
        watched(&titles[0], true, 0),
        unwatched(&titles[1]),
        unwatched(&titles[2]),
    ];
    assert_eq!(
        top_picks(
            &titles,
            &states,
            &[watchlisted(TitleKind::TITLE_KIND_MOVIE, "2")]
        ),
        ["2", "3"]
    );
}

/// A played Horror Movie, then `listed`, then an unplayed Horror Movie `3`.
fn horror_fan_and(listed: &[Title]) -> (Vec<Title>, Vec<TitleWatchStates>) {
    let mut titles = vec![movie("1", &["Horror"])];
    titles.extend_from_slice(listed);
    titles.push(movie("3", &["Horror"]));
    let mut states: Vec<_> = titles.iter().map(unwatched).collect();
    states[0] = watched(&titles[0], true, 0);
    (titles, states)
}

#[test]
fn a_movie_listed_by_another_id_is_watchlisted_by_its_tmdb_id_and_keeps_the_listed_id() {
    let (titles, states) = horror_fan_and(&[tvdb_movie("t2", "2")]);

    let picks = top_picks(
        &titles,
        &states,
        &[watchlisted(TitleKind::TITLE_KIND_MOVIE, "2")],
    );

    assert_eq!(picks, ["t2", "3"]);
}

#[test]
fn a_show_listed_by_another_id_is_watchlisted_by_the_tmdb_id_any_episode_names() {
    let (titles, states) = horror_fan_and(&[
        tvdb_episode("10", "ts1", &[]),
        tvdb_episode("11", "ts1", &[id("s1")]),
    ]);

    let picks = top_picks(
        &titles,
        &states,
        &[watchlisted(TitleKind::TITLE_KIND_SHOW, "s1")],
    );

    assert_eq!(picks, ["ts1", "3"]);
}

#[test]
fn a_watchlist_entry_of_another_kind_or_id_boosts_nothing() {
    let (titles, states) = horror_fan_and(&[tvdb_movie("t2", "2")]);

    let picks = top_picks(
        &titles,
        &states,
        &[
            watchlisted(TitleKind::TITLE_KIND_SHOW, "2"),
            watchlisted(TitleKind::TITLE_KIND_MOVIE, "9"),
        ],
    );

    assert_eq!(picks, ["3", "t2"]);
}

/// The Movie `tvdb_movie(tvdb_value, value)` as a Library preferring TMDB lists it, by `value`.
fn tmdb_movie(value: &str, tvdb_value: &str, genres: &[&str]) -> Title {
    Title {
        ids: vec![id(value), tvdb(tvdb_value)],
        ..movie(value, genres)
    }
}

/// A Movie and a Show each listed by TVDB in one Library and TMDB in another, and the Movie `5`.
fn listed_twice() -> [Title; 5] {
    [
        tvdb_movie("t2", "2"),
        tvdb_episode("10", "ts1", &[id("s1")]),
        tmdb_movie("2", "t2", &[]),
        Title {
            show_ids: vec![id("s1"), tvdb("ts1")],
            ..episode("11", "s1")
        },
        movie("5", &[]),
    ]
}

#[test]
fn a_title_libraries_list_by_different_ids_is_picked_once_by_the_first_listed() {
    let titles = listed_twice();
    let states: Vec<_> = titles.iter().map(unwatched).collect();

    let rows = rows(&ranked(&titles, &states, &[]), &DEFAULT_FAMILY_RATINGS);

    assert_eq!(
        summary(&rows),
        [
            ("top-picks", vec!["t2", "ts1", "5"]),
            ("family-night", vec!["t2", "ts1", "5"])
        ]
    );
}

#[test]
fn a_title_started_or_played_in_one_library_is_in_neither_row() {
    let titles = listed_twice();
    let mut states: Vec<_> = titles.iter().map(unwatched).collect();
    states[2] = watched(&titles[2], false, 60_000);
    states[3] = watched(&titles[3], true, 0);

    let rows = rows(&ranked(&titles, &states, &[]), &DEFAULT_FAMILY_RATINGS);

    assert_eq!(
        summary(&rows),
        [("top-picks", vec!["5"]), ("family-night", vec!["5"])]
    );
}

#[test]
fn a_movie_and_a_show_sharing_an_id_value_are_two_picks() {
    let titles = [movie("5", &[]), episode("10", "5")];
    let states: Vec<_> = titles.iter().map(unwatched).collect();

    assert_eq!(top_picks(&titles, &states, &[]), ["5", "5"]);
}

#[test]
fn a_title_played_in_two_libraries_counts_its_genres_once() {
    let titles = [
        Title {
            genres: vec!["Horror".to_owned()],
            ..tvdb_movie("t1", "1")
        },
        tmdb_movie("1", "t1", &["Horror"]),
        movie("2", &["Comedy"]),
        movie("3", &["Comedy"]),
        movie("4", &["Horror"]),
        movie("6", &["Comedy"]),
    ];
    let mut states: Vec<_> = titles.iter().map(unwatched).collect();
    for (state, title) in states.iter_mut().zip(&titles).take(4) {
        *state = watched(title, true, 0);
    }

    assert_eq!(top_picks(&titles, &states, &[]), ["6", "4"]);
}

#[test]
fn a_title_the_user_started_or_played_is_in_neither_row() {
    let titles = [
        rated("1", Some("G")),
        rated("2", Some("G")),
        rated("3", Some("G")),
        episode("10", "s1"),
        episode("11", "s1"),
    ];
    let states = [
        watched(&titles[0], true, 0),
        watched(&titles[1], false, 60_000),
        unwatched(&titles[2]),
        unwatched(&titles[3]),
        watched(&titles[4], false, 1_000),
    ];

    let rows = rows(&ranked(&titles, &states, &[]), &DEFAULT_FAMILY_RATINGS);

    assert_eq!(
        summary(&rows),
        [("top-picks", vec!["3"]), ("family-night", vec!["3"])]
    );
}

#[test]
fn a_shows_episodes_pick_the_show_once_by_its_id() {
    let titles = [episode("10", "s1"), episode("11", "s1")];
    let states = [unwatched(&titles[0]), unwatched(&titles[1])];

    let rows = rows(&ranked(&titles, &states, &[]), &DEFAULT_FAMILY_RATINGS);

    let show = &rows.rows[0].titles[0];
    assert_eq!(rows.rows[0].titles.len(), 1);
    assert_eq!(show.kind.as_known(), Some(TitleKind::TITLE_KIND_SHOW));
    assert_eq!(show.id.value, "s1");
}

#[test]
fn family_night_holds_only_titles_rated_in_the_setting() {
    let titles = [
        rated("1", Some("G")),
        rated("2", Some("R")),
        rated("3", None),
        rated("4", Some("tv-pg")),
        rated("5", Some("TV-MA")),
    ];
    let states: Vec<_> = titles.iter().map(unwatched).collect();
    let picks = ranked(&titles, &states, &[]);

    let by_default = rows(&picks, &DEFAULT_FAMILY_RATINGS);
    let by_admin = rows(&picks, &["R", "TV-MA"]);

    assert_eq!(summary(&by_default)[1], ("family-night", vec!["1", "4"]));
    assert_eq!(summary(&by_admin)[1], ("family-night", vec!["2", "5"]));
}

#[test]
fn featured_suggests_only_the_best_few_scored_picks_heaviest_first() {
    let mut titles = vec![movie("1", &["Horror"]), movie("2", &["Comedy"])];
    titles.extend((10..20).map(|n| movie(&n.to_string(), &["Horror"])));
    let mut states: Vec<_> = titles.iter().map(unwatched).collect();
    states[0] = watched(&titles[0], true, 0);

    let suggested = featured(&ranked(&titles, &states, &[]));

    let ids: Vec<_> = (suggested.suggestions.iter())
        .map(|suggestion| suggestion.id.value.as_str())
        .collect();
    assert_eq!(ids, ["10", "11", "12", "13", "14"]);
    let weights: Vec<_> = (suggested.suggestions.iter())
        .map(|suggestion| suggestion.weight)
        .collect();
    assert!(weights.is_sorted_by(|a, b| a > b), "{weights:?}");
    assert!(
        weights.iter().all(|weight| (0.0..=0.8).contains(weight)),
        "{weights:?}"
    );
}

#[test]
fn a_user_with_no_history_is_suggested_nothing_for_featured() {
    let titles = [movie("1", &["Horror"])];

    let suggested = featured(&ranked(&titles, &[unwatched(&titles[0])], &[]));

    assert_eq!(suggested.suggestions, []);
}

#[test]
fn manifest_is_a_home_row_source_with_the_permissions_and_family_content_ratings_setting() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::HOME_ROW]);
    assert_eq!(
        manifest.permissions,
        [
            Permission::PERMISSION_READ_LIBRARIES,
            Permission::PERMISSION_READ_WATCH_STATES,
            Permission::PERMISSION_READ_WATCHLISTS,
        ]
    );
    let [field] = manifest.settings.as_slice() else {
        panic!("{:?}", manifest.settings);
    };
    assert_eq!(field.key, "family_content_ratings");
    assert_eq!(field.kind, SettingKind::SETTING_KIND_CHOICES);
    let choices: Vec<_> = (field.choices.iter())
        .map(|choice| (choice.value.as_str(), choice.label.as_str()))
        .collect();
    assert_eq!(
        choices,
        [
            "G", "PG", "PG-13", "R", "NC-17", "TV-Y", "TV-Y7", "TV-G", "TV-PG", "TV-14", "TV-MA"
        ]
        .map(|rating| (rating, rating))
    );
    assert_eq!(
        field.default_value,
        nama_pdk::picked_default(["G", "PG", "PG-13", "TV-Y", "TV-G", "TV-PG"])
    );
}

/// A Kernel listing `titles` in Library 1, granting Read Libraries and Read Watch States, with
/// no Watch State yet and the default Family Content Ratings.
fn refusing_watchlists(titles: &[Title]) -> Kernel {
    let library = Library {
        id: 1,
        ..Library::default()
    };
    Kernel::new()
        .setting_picked(FAMILY_RATINGS, DEFAULT_FAMILY_RATINGS)
        .libraries([library])
        .titles(1, titles.iter().cloned())
        .watch_states(&TitleRef::default(), [])
}

/// [`refusing_watchlists`], granting Read Watchlists too, every Watchlist empty.
fn granting_watchlists(titles: &[Title]) -> Kernel {
    refusing_watchlists(titles).watchlist(ADA, [])
}

/// `kernel` with the User `user_id` having played `title`, or stopped `position_ms` in.
fn watching(kernel: Kernel, title: &Title, user_id: i64, played: bool, position_ms: u64) -> Kernel {
    kernel.watch_states(&title.title_ref, [state(user_id, played, position_ms)])
}

/// The User `user_id`'s Watch State: `played`, or stopped `position_ms` in.
fn state(user_id: i64, played: bool, position_ms: u64) -> WatchState {
    WatchState {
        user_id,
        played,
        position_ms,
        ..WatchState::default()
    }
}

fn user_rows(kernel: &mut Kernel, user_id: i64) -> Result<HomeRows, Error> {
    let asked = UserRowsAsked {
        user_id,
        ..UserRowsAsked::default()
    };
    with(kernel, || TopPicks::user_rows(asked))
}

fn user_featured(kernel: &mut Kernel, user_id: i64) -> Result<FeaturedSuggestions, Error> {
    let asked = UserFeaturedAsked {
        user_id,
        ..UserFeaturedAsked::default()
    };
    with(kernel, || TopPicks::user_featured(asked))
}

#[test]
fn user_rows_differ_for_two_users_with_different_histories() {
    let titles = [
        movie("1", &["Horror"]),
        movie("2", &["Comedy"]),
        movie("3", &["Horror", "Thriller"]),
        movie("4", &["Comedy"]),
    ];
    let kernel = watching(granting_watchlists(&titles), &titles[0], ADA, true, 0);
    let mut kernel = watching(kernel, &titles[1], BEN, true, 0);

    let adas = user_rows(&mut kernel, ADA).unwrap();
    let bens = user_rows(&mut kernel, BEN).unwrap();

    assert_eq!(
        summary(&adas),
        [
            ("top-picks", vec!["3", "2", "4"]),
            ("family-night", vec!["3", "2", "4"])
        ]
    );
    assert_eq!(
        summary(&bens),
        [
            ("top-picks", vec!["4", "1", "3"]),
            ("family-night", vec!["4", "1", "3"])
        ]
    );
}

#[test]
fn user_rows_and_featured_put_a_watchlisted_title_above_the_users_genres() {
    let titles = [
        movie("1", &["Horror"]),
        movie("2", &["Comedy"]),
        movie("3", &["Horror"]),
    ];
    let kernel = granting_watchlists(&titles)
        .watchlist(ADA, [watchlisted(TitleKind::TITLE_KIND_MOVIE, "2")]);
    let mut kernel = watching(kernel, &titles[0], ADA, true, 0);

    let rows = user_rows(&mut kernel, ADA).unwrap();
    let suggested = user_featured(&mut kernel, ADA).unwrap();

    assert_eq!(summary(&rows)[0], ("top-picks", vec!["2", "3"]));
    let ids: Vec<_> = (suggested.suggestions.iter())
        .map(|suggestion| suggestion.id.value.as_str())
        .collect();
    assert_eq!(ids, ["2", "3"]);
}

#[test]
fn user_rows_leave_out_a_title_the_user_started_or_played() {
    let titles = [
        rated("1", Some("G")),
        rated("2", Some("G")),
        rated("3", Some("G")),
        episode("10", "s1"),
        episode("11", "s1"),
    ];
    let kernel = watching(granting_watchlists(&titles), &titles[0], ADA, true, 0);
    let kernel = watching(kernel, &titles[1], ADA, false, 60_000);
    let mut kernel = watching(kernel, &titles[4], ADA, false, 1_000);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(
        summary(&rows),
        [("top-picks", vec!["3"]), ("family-night", vec!["3"])]
    );
}

#[test]
fn user_rows_family_night_holds_only_titles_rated_in_the_setting() {
    let titles = [
        rated("1", Some("G")),
        rated("2", Some("R")),
        rated("3", None),
        rated("4", Some("tv-pg")),
        rated("5", Some("TV-MA")),
    ];
    let mut by_admin = granting_watchlists(&titles).setting_picked(FAMILY_RATINGS, ["R", "TV-MA"]);

    let by_default = user_rows(&mut granting_watchlists(&titles), ADA).unwrap();
    let by_admin = user_rows(&mut by_admin, ADA).unwrap();

    assert_eq!(summary(&by_default)[1], ("family-night", vec!["1", "4"]));
    assert_eq!(summary(&by_admin)[1], ("family-night", vec!["2", "5"]));
}

#[test]
fn user_rows_rank_by_genres_when_the_kernel_refuses_the_watchlist() {
    let titles = [
        movie("1", &["Horror"]),
        movie("2", &["Comedy"]),
        movie("3", &["Horror"]),
    ];
    let mut kernel = watching(refusing_watchlists(&titles), &titles[0], ADA, true, 0);

    let rows = user_rows(&mut kernel, ADA).unwrap();

    assert_eq!(summary(&rows)[0], ("top-picks", vec!["3", "2"]));
}

#[test]
fn user_rows_and_featured_fail_naming_a_permission_not_granted() {
    let mut kernel = Kernel::new()
        .libraries([Library::default()])
        .watchlist(ADA, []);

    let rows = user_rows(&mut kernel, ADA).unwrap_err();
    let suggested = user_featured(&mut kernel, ADA).unwrap_err();

    let refused = "the Admin hasn't granted the Plugin the Read Watch States Permission";
    assert_eq!(rows.to_string(), refused);
    assert_eq!(suggested.to_string(), refused);
}
