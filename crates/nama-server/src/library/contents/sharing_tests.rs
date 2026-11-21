use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::super::{
    Decided, Item, Metadata,
    fixture::tmdb_movie,
    testing::{Fixture, movie},
};
use crate::library::title_source::{Role, Roles, TitleKind};

const WEAVER: i64 = 10_205;
const SCOTT: i64 = 578;

fn role(person: i64, part: &str) -> Role {
    Role {
        person: ExternalId::tmdb(person),
        name: String::new(),
        part: part.to_owned(),
        headshot: None,
    }
}

fn genres(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

/// Stores the Movie `title` at its own path and Matches it to `metadata`.
async fn matched(fx: &Fixture, title: &str, metadata: Metadata) {
    let path = format!("/media/{title}.mkv");
    fx.store(&path, 10, movie(title, 2000)).await;
    let item = fx.item(&path).await;
    (fx.contents)
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

/// Alien, Aliens, Blade Runner and Heat: Alien shares Sigourney Weaver with Aliens and Ridley
/// Scott with Blade Runner, and Heat shares nothing with it.
async fn movies() -> Fixture {
    let fx = Fixture::new().await;
    let movies = [
        (
            348,
            "Alien",
            vec![role(WEAVER, "Ripley")],
            vec![role(SCOTT, "Director")],
            &["Horror", "Science Fiction"][..],
        ),
        (
            679,
            "Aliens",
            vec![role(WEAVER, "Ripley")],
            vec![],
            &["Action", "Horror", "Science Fiction"],
        ),
        (
            78,
            "Blade Runner",
            vec![],
            vec![role(SCOTT, "Director")],
            &["Science Fiction"],
        ),
        (949, "Heat", vec![], vec![], &["Crime"]),
    ];
    for (tmdb, title, cast, crew, names) in movies {
        let metadata = Metadata {
            genres: genres(names),
            roles: Roles { cast, crew },
            ..tmdb_movie(tmdb, title)
        };
        matched(&fx, title, metadata).await;
    }
    fx
}

/// The TMDB ids of the Movies sharing Alien's Persons and genres, at most `limit`.
async fn sharing_alien(fx: &Fixture, limit: u32) -> Vec<String> {
    let people = [ExternalId::tmdb(WEAVER), ExternalId::tmdb(SCOTT)];
    let alien = genres(&["Horror", "Science Fiction"]);
    let found = (fx.contents).titles_sharing(TitleKind::Movie, &people, &alien, limit);
    (found.await.unwrap().iter())
        .map(|title| title.ids.iter().next().unwrap().value().to_owned())
        .collect()
}

#[tokio::test]
async fn titles_sharing_the_most_persons_come_first_then_the_most_genres() {
    let fx = movies().await;

    assert_eq!(sharing_alien(&fx, 10).await, ["348", "679", "78"]);
}

#[tokio::test]
async fn titles_sharing_stop_at_the_limit() {
    let fx = movies().await;

    assert_eq!(sharing_alien(&fx, 2).await, ["348", "679"]);
}

#[tokio::test]
async fn a_title_sharing_has_its_genres_and_every_role_in_billing_order() {
    let fx = movies().await;

    let found = (fx.contents)
        .titles_sharing(TitleKind::Movie, &[ExternalId::tmdb(SCOTT)], &[], 1)
        .await
        .unwrap();

    let [alien] = found.as_slice() else {
        panic!("{found:?}");
    };
    assert_eq!(alien.ids, ExternalIds::from_iter([ExternalId::tmdb(348)]));
    assert_eq!(alien.genres, ["Horror", "Science Fiction"]);
    assert_eq!(alien.roles.cast, [role(WEAVER, "Ripley")]);
    assert_eq!(alien.roles.crew, [role(SCOTT, "Director")]);
}

#[tokio::test]
async fn a_title_no_longer_matched_is_not_sharing() {
    let fx = movies().await;
    let item = fx.item("/media/Aliens.mkv").await;
    (fx.contents)
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();

    assert_eq!(sharing_alien(&fx, 10).await, ["348", "78"]);
}

#[tokio::test]
async fn a_show_shares_once_not_per_episode_and_only_as_a_show() {
    let fx = Fixture::shows().await;
    for episode in 1..=2 {
        let path = format!("/media/Dark/Season 01/Dark - S01E0{episode}.mkv");
        let dark_episode = Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            hint: None,
            season: 1,
            episode,
        };
        fx.store(&path, 10, dark_episode).await;
        let metadata = Metadata {
            ids: ExternalIds::from_iter([ExternalId::tmdb(1_000 + episode)]),
            show_ids: ExternalIds::from_iter([ExternalId::tmdb(70_523)]),
            show_title: Some("Dark".to_owned()),
            genres: genres(&["Drama", "Mystery"]),
            ..tmdb_movie(0, "Episode")
        };
        let item = fx.item(&path).await;
        (fx.contents)
            .record(vec![(item, Decided::Matched(Box::new(metadata)))])
            .await
            .unwrap();
    }
    let drama = genres(&["Drama"]);

    let shows = fx.contents.titles_sharing(TitleKind::Show, &[], &drama, 10);
    let shows = shows.await.unwrap();
    let movies = fx
        .contents
        .titles_sharing(TitleKind::Movie, &[], &drama, 10);

    let dark: Vec<_> = shows.iter().map(|show| show.ids.clone()).collect();
    assert_eq!(dark, [ExternalIds::from_iter([ExternalId::tmdb(70_523)])]);
    assert_eq!(shows[0].genres, ["Drama", "Mystery"]);
    assert_eq!(movies.await.unwrap(), Vec::new());
}
