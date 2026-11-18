use std::time::Duration;

use nama_plugin_api::contract::{HomeRow, HomeRowTitle, HomeRows, TitleId, TitleKind};

use super::{Kind, Title, kept};

fn movie(value: &str) -> HomeRowTitle {
    titled(TitleKind::TITLE_KIND_MOVIE, "tmdb", value)
}

fn titled(kind: TitleKind, namespace: &str, value: &str) -> HomeRowTitle {
    HomeRowTitle {
        kind: kind.into(),
        id: TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..TitleId::default()
        }
        .into(),
        ..HomeRowTitle::default()
    }
}

fn row(key: &str, ttl_seconds: u32, titles: Vec<HomeRowTitle>) -> HomeRow {
    HomeRow {
        key: key.to_owned(),
        heading: key.to_owned(),
        ttl_seconds,
        titles,
        ..HomeRow::default()
    }
}

fn given(rows: Vec<HomeRow>) -> HomeRows {
    HomeRows {
        rows,
        ..HomeRows::default()
    }
}

#[test]
fn rows_past_three_titles_past_200_and_headings_past_40_characters_are_cut() {
    let many = (1..=250).map(|n| movie(&n.to_string())).collect();
    let long = HomeRow {
        heading: "Ü".repeat(45),
        ..row("long", 3600, many)
    };
    let rows = vec![
        long,
        row("b", 3600, vec![]),
        row("c", 3600, vec![]),
        row("d", 3600, vec![]),
    ];

    let (kept, _) = kept(given(rows));

    let keys: Vec<&str> = kept.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["long", "b", "c"]);
    assert_eq!(kept[0].heading, "Ü".repeat(40));
    assert_eq!(kept[0].titles.len(), 200);
    assert_eq!(
        kept[0].titles[199],
        Title(Kind::Movie, "tmdb-200".to_owned())
    );
}

#[test]
fn a_time_to_live_is_clamped_and_the_rows_are_fresh_for_the_shortest() {
    let (_, too_short) = kept(given(vec![row("a", 60, vec![]), row("b", 3600, vec![])]));
    let (_, too_long) = kept(given(vec![row("a", 30 * 24 * 3600, vec![])]));
    let (_, within) = kept(given(vec![row("a", 7200, vec![]), row("b", 3600, vec![])]));

    assert_eq!(too_short, Duration::from_mins(15));
    assert_eq!(too_long, Duration::from_hours(7 * 24));
    assert_eq!(within, Duration::from_hours(1));
}

#[test]
fn a_plugin_giving_no_rows_is_asked_again_in_15_minutes() {
    let (rows, fresh_for) = kept(given(vec![]));

    assert_eq!(rows, []);
    assert_eq!(fresh_for, Duration::from_mins(15));
}

#[test]
fn a_repeated_key_and_titles_without_a_kind_or_a_well_formed_id_are_dropped() {
    let titles = vec![
        titled(TitleKind::TITLE_KIND_SHOW, "tvdb", "1"),
        titled(TitleKind::TITLE_KIND_UNSPECIFIED, "tmdb", "2"),
        titled(TitleKind::TITLE_KIND_MOVIE, "TMDB", "3"),
        titled(TitleKind::TITLE_KIND_MOVIE, "tmdb", ""),
    ];
    let rows = vec![row("a", 3600, titles), row("a", 3600, vec![movie("4")])];

    let (kept, _) = kept(given(rows));

    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].titles, [Title(Kind::Show, "tvdb-1".to_owned())]);
}
