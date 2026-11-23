use std::sync::Arc;

use nama_plugin_api::metadata::ExternalId;
use tempfile::TempDir;

use super::{Browse, Error, PersonTitle};
use crate::{
    db::Database,
    library::{
        Contents, Decided, Metadata, NotFound, OnServer, Roles, SourceRole as Role,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
    plugins::ExtensionPoints,
};

/// A Movies Library and the Browse of its database.
struct Fixture {
    _data: TempDir,
    folder: TempDir,
    library: i64,
    contents: Contents,
    browse: Browse,
}

impl Fixture {
    async fn new() -> Self {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = movies_library(&db, folder.path()).await;
        Self {
            _data: data,
            folder,
            library,
            contents: Contents::new(Arc::clone(&db)),
            browse: Browse::new(db, ExtensionPoints::default()),
        }
    }

    /// Stores the Movie `tmdb`, released on `released`, Matched with `cast`.
    async fn movie(&self, tmdb: i64, released: &str, cast: Vec<Role>) -> MatchedMovie {
        let path = self.folder.path().canonicalize().unwrap();
        let path = path.join(format!("{tmdb}.mkv"));
        let matched = Metadata {
            release_date: Some(released.to_owned()),
            roles: Roles { cast, crew: vec![] },
            ..tmdb_movie(tmdb, &format!("Movie {tmdb}"))
        };
        MatchedMovie::store(
            &self.contents,
            self.library,
            path.to_str().unwrap(),
            matched,
        )
        .await
    }
}

fn role(tmdb: i64, name: &str, part: &str) -> Role {
    Role {
        person: ExternalId::tmdb(tmdb),
        name: name.to_owned(),
        part: part.to_owned(),
        headshot: None,
    }
}

fn titles(titles: &[PersonTitle]) -> Vec<(&str, Vec<&str>)> {
    (titles.iter())
        .map(|listed| {
            let OnServer::Movie(movie) = &listed.title else {
                panic!("only Movies here");
            };
            let parts = listed.parts.iter().map(String::as_str).collect();
            (movie.title.as_str(), parts)
        })
        .collect()
}

#[tokio::test]
async fn a_persons_titles_are_newest_first() {
    let fx = Fixture::new().await;
    fx.movie(
        348,
        "1979-05-25",
        vec![role(10_205, "Sigourney Weaver", "Ripley")],
    )
    .await;
    fx.movie(
        679,
        "1986-07-18",
        vec![role(10_205, "Sigourney Weaver", "")],
    )
    .await;

    let page = fx.browse.person(&ExternalId::tmdb(10_205)).await.unwrap();

    assert_eq!(
        titles(&page.cast),
        [("Movie 679", vec![]), ("Movie 348", vec!["Ripley"])]
    );
}

#[tokio::test]
async fn a_person_whose_only_title_left_the_libraries_is_not_found() {
    let fx = Fixture::new().await;
    let alien = fx
        .movie(
            348,
            "1979-05-25",
            vec![role(10_205, "Sigourney Weaver", "Ripley")],
        )
        .await;
    let unmatched = Decided::Unmatched(None);
    fx.contents
        .record(vec![(alien.item, unmatched)])
        .await
        .unwrap();

    let page = fx.browse.person(&ExternalId::tmdb(10_205)).await;

    assert!(matches!(page, Err(Error::NotFound(NotFound::NoSuchPerson))));
}

#[tokio::test]
async fn a_search_finds_people_by_word_starts_ignoring_accents_exact_names_first() {
    let fx = Fixture::new().await;
    let cruz = role(955, "Penélope Cruz", "");
    let cruzado = role(956, "Penelope Cruzado", "");
    fx.movie(1, "2001-01-01", vec![cruzado.clone()]).await;
    fx.movie(2, "2002-01-01", vec![cruzado, cruz]).await;

    let found = fx.browse.people_named("penelope cruz").await.unwrap();
    let missed = fx.browse.people_named("lope").await.unwrap();

    let names: Vec<&str> = found.iter().map(|page| page.name.as_str()).collect();
    assert_eq!(names, ["Penélope Cruz", "Penelope Cruzado"]);
    assert!(missed.is_empty(), "a word's middle isn't its start");
}
