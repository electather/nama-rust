use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::super::{
    Decided, Item, Metadata,
    fixture::tmdb_movie,
    testing::{ALIEN, Fixture, movie},
};
use crate::library::title_source::{Role, Roles};

const ALIENS: &str = "/media/Aliens (1986)/Aliens (1986).mkv";

fn weaver(character: &str) -> Role {
    Role {
        person: ExternalId::tmdb(10_205),
        name: "Sigourney Weaver".to_owned(),
        part: character.to_owned(),
        headshot: Some("/weaver.jpg".to_owned()),
    }
}

fn ridley_scott() -> Role {
    Role {
        person: ExternalId::tmdb(578),
        name: "Ridley Scott".to_owned(),
        part: "Director".to_owned(),
        headshot: None,
    }
}

/// Matches the one Item stored at `path` to `metadata`.
async fn record(fx: &Fixture, path: &str, metadata: Metadata) {
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

#[tokio::test]
async fn a_movie_has_the_roles_its_source_gave_in_billing_order() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let roles = Roles {
        cast: vec![weaver("Ellen Ripley")],
        crew: vec![ridley_scott()],
    };
    let alien = Metadata {
        roles: roles.clone(),
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, alien).await;

    let movies = fx.contents.movies(fx.library, None).await.unwrap();

    assert_eq!(fx.contents.movie_roles(&movies[0]).await.unwrap(), roles);
}

#[tokio::test]
async fn a_shows_roles_are_stored_once_not_on_each_episode() {
    let fx = Fixture::shows().await;
    let roles = Roles {
        cast: vec![weaver("Jonas Kahnwald")],
        crew: vec![],
    };
    for episode in 1..=2 {
        let path = format!("/media/Dark/Season 01/Dark - S01E0{episode}.mkv");
        let item = Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            hint: None,
            season: 1,
            episode,
        };
        fx.store(&path, 10, item).await;
        let matched = Metadata {
            ids: ExternalIds::from_iter([ExternalId::tmdb(1_000 + episode)]),
            show_ids: ExternalIds::from_iter([ExternalId::tmdb(70_523)]),
            show_title: Some("Dark".to_owned()),
            roles: roles.clone(),
            ..tmdb_movie(0, "Episode")
        };
        record(&fx, &path, matched).await;
    }

    let shows = fx.contents.shows(fx.library, None).await.unwrap();
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM title_roles")
        .fetch_one(fx.contents.db.reader())
        .await
        .unwrap();

    assert_eq!(fx.contents.show_roles(&shows[0]).await.unwrap(), roles);
    assert_eq!(stored, 1);
}

#[tokio::test]
async fn a_person_in_two_matched_titles_has_one_stored_headshot() {
    let fx = Fixture::new().await;
    for (path, title, year, tmdb, character) in [
        (ALIEN, "Alien", 1979, 348, "Ellen Ripley"),
        (ALIENS, "Aliens", 1986, 679, "Ripley"),
    ] {
        fx.store(path, 10, movie(title, year)).await;
        let matched = Metadata {
            roles: Roles {
                cast: vec![weaver(character)],
                crew: vec![],
            },
            ..tmdb_movie(tmdb, title)
        };
        record(&fx, path, matched).await;
    }

    let images = fx.contents.images(None).await.unwrap();

    let headshots = images.iter().filter(|(_, path)| path == "/weaver.jpg");
    assert_eq!(headshots.count(), 1);
    assert!(
        fx.contents
            .stores_image("tmdb", "/weaver.jpg")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_headshot_no_matched_title_has_is_not_stored() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let alien = Metadata {
        roles: Roles {
            cast: vec![weaver("Ellen Ripley")],
            crew: vec![],
        },
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, alien).await;
    let item = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();

    let images = fx.contents.images(None).await.unwrap();

    assert!(!images.iter().any(|(_, path)| path == "/weaver.jpg"));
    assert!(
        !fx.contents
            .stores_image("tmdb", "/weaver.jpg")
            .await
            .unwrap()
    );
}
