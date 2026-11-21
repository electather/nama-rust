use std::time::Duration;

use nama_plugin_api::contract::{HomeRow, TitleKind};

use super::kept;
use crate::home_rows::{
    Kind, Title,
    tests::{given, movie, row, titled},
};

/// An answer of `count` rows keyed `row-1` to `row-{count}`, each with no titles.
fn numbered(count: usize) -> Vec<HomeRow> {
    (1..=count)
        .map(|n| row(&format!("row-{n}"), 3600, vec![]))
        .collect()
}

#[test]
fn an_answer_of_21_rows_keeps_the_first_20_in_order() {
    let (kept, _) = kept(given(numbered(21)));

    let keys: Vec<&str> = kept.iter().map(|row| row.key.as_str()).collect();
    let first_20: Vec<String> = (1..=20).map(|n| format!("row-{n}")).collect();
    assert_eq!(keys, first_20);
}

#[test]
fn an_answer_of_20_rows_keeps_them_all() {
    let (kept, _) = kept(given(numbered(20)));

    assert_eq!(kept.len(), 20);
}

#[test]
fn titles_past_200_and_headings_past_40_characters_are_cut() {
    let many = (1..=250).map(|n| movie(&n.to_string())).collect();
    let long = HomeRow {
        heading: "Ü".repeat(45),
        ..row("long", 3600, many)
    };

    let (kept, _) = kept(given(vec![long]));

    assert_eq!(kept[0].heading, "Ü".repeat(40));
    assert_eq!(kept[0].titles.len(), 200);
    assert_eq!(
        kept[0].titles[199],
        Title(Kind::Movie, "tmdb-200".to_owned())
    );
}

#[test]
fn the_rows_are_fresh_for_the_shortest_time_to_live() {
    let (_, fresh_for) = kept(given(vec![row("a", 7200, vec![]), row("b", 60, vec![])]));

    assert_eq!(fresh_for, Duration::from_mins(1));
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
