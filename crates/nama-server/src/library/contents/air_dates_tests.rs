use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::super::{Decided, Decisions, Item, Metadata, fixture::tmdb_movie, testing::Fixture};
use crate::library::title_source::{Episode, Season};

fn dark_ids() -> ExternalIds {
    ExternalIds::from_iter([ExternalId::tmdb(70_523), ExternalId::tvdb(334_824)])
}

/// Season `number` of Dark, its Episodes numbered from 1 with the air dates `aired`.
fn season(number: u32, aired: &[Option<&str>]) -> Season {
    let episodes = (1..).zip(aired).map(|(episode, air_date)| Episode {
        ids: ExternalIds::default(),
        season: number,
        number: episode,
        title: format!("Episode {episode}"),
        overview: None,
        air_date: air_date.map(str::to_owned),
        runtime_minutes: None,
        still: None,
    });
    Season {
        ids: ExternalIds::default(),
        number,
        title: format!("Season {number}"),
        overview: None,
        air_date: None,
        poster: None,
        episodes: episodes.collect(),
    }
}

/// How many aired Episodes Dark has, once its first Episode is Matched by TMDB with `seasons`
/// fetched.
async fn aired_with(seasons: Vec<Season>) -> u32 {
    let fx = Fixture::shows().await;
    let path = "/media/Dark/Season 01/Dark - S01E01.mkv";
    let item = Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 1,
    };
    fx.store(path, 10, item).await;
    let first = Metadata {
        show_ids: dark_ids(),
        show_title: Some("Dark".to_owned()),
        ..tmdb_movie(1_001, "Episode 1")
    };
    let tmdb = PluginId::from_static("tmdb");
    let decided = Decisions {
        items: vec![(fx.item(path).await, Decided::Matched(Box::new(first)))],
        seasons: (seasons.into_iter())
            .map(|season| (tmdb.clone(), dark_ids(), season))
            .collect(),
    };
    fx.contents.record(decided).await.unwrap();
    let shows = fx.contents.shows(fx.library, None).await.unwrap();
    fx.contents.aired_episodes(&shows[0]).await.unwrap()
}

#[tokio::test]
async fn a_season_counts_only_its_past_dated_episodes() {
    let aired = aired_with(vec![season(
        1,
        &[Some("2017-12-01"), Some("2999-01-01"), None],
    )])
    .await;

    assert_eq!(aired, 1);
}

#[tokio::test]
async fn specials_are_not_counted() {
    let aired = aired_with(vec![
        season(0, &[Some("2017-11-01")]),
        season(1, &[Some("2017-12-01"), Some("2017-12-01")]),
    ])
    .await;

    assert_eq!(aired, 2);
}
