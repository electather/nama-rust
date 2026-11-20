use nama_pdk::{
    HomeRows, Permission, Title, TitleId, TitleKind, TitleRef, TitleRefKind, TitleWatchStates,
    WatchState, WatchlistTitle,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
};

use super::{DEFAULT_FAMILY_RATINGS, FAMILY_RATINGS, featured, manifest, ranked, rows};

fn id(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
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

/// The User's Watch State of `title`: `played`, or stopped `position_ms` in.
fn watched(title: &Title, played: bool, position_ms: u64) -> TitleWatchStates {
    TitleWatchStates {
        title_ref: title.title_ref.clone(),
        states: vec![WatchState {
            user_id: 1,
            played,
            position_ms,
            ..WatchState::default()
        }],
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
    let rows = rows(&ranked(titles, states, watchlist), DEFAULT_FAMILY_RATINGS);
    summary(&rows)[0]
        .1
        .iter()
        .map(|&id| id.to_owned())
        .collect()
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
    let watchlist = WatchlistTitle {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: id("2").into(),
        ..WatchlistTitle::default()
    };

    assert_eq!(top_picks(&titles, &states, &[watchlist]), ["2", "3"]);
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

    let rows = rows(&ranked(&titles, &states, &[]), DEFAULT_FAMILY_RATINGS);

    assert_eq!(
        summary(&rows),
        [("top-picks", vec!["3"]), ("family-night", vec!["3"])]
    );
}

#[test]
fn a_shows_episodes_pick_the_show_once_by_its_id() {
    let titles = [episode("10", "s1"), episode("11", "s1")];
    let states = [unwatched(&titles[0]), unwatched(&titles[1])];

    let rows = rows(&ranked(&titles, &states, &[]), DEFAULT_FAMILY_RATINGS);

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

    let by_default = rows(&picks, DEFAULT_FAMILY_RATINGS);
    let by_admin = rows(&picks, " R ,TV-MA");

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
fn manifest_serves_users_rows_with_the_permissions_and_family_ratings_setting() {
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
    let settings: Vec<_> = (manifest.settings.iter())
        .map(|field| {
            let default = field.default_value.as_option();
            (
                field.key.as_str(),
                field.kind.as_known(),
                default.and_then(|default| default.value.clone()),
            )
        })
        .collect();
    assert_eq!(
        settings,
        [(
            FAMILY_RATINGS,
            Some(SettingKind::SETTING_KIND_TEXT),
            Some(Value::TextValue(
                "G, PG, PG-13, TV-Y, TV-G, TV-PG".to_owned()
            )),
        )]
    );
}
