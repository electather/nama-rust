use std::{path::Path, sync::Arc};

use super::tests::{Fixture, summary};
use crate::{
    library::{
        LibraryKind,
        scripted::{ScriptedSource, SourcesAsked},
    },
    plugins::Registry,
};

const THEATRICAL: &str = "Alien (1979) {tmdb-348} {edition-Theatrical}/Alien (1979).mkv";
const DIRECTORS_1080P: &str = "Alien (1979) {tmdb-348}/Alien (1979) {edition-Director's Cut}.mkv";
const DIRECTORS_2160P: &str =
    "Alien (1979) {tmdb-348} {edition-Theatrical}/Alien (1979) {edition-Director's Cut} 2160p.mkv";
const UNCUT: &str = "Alien (1979) {tmdb-348}/Alien (1979) - 720p.mkv";

/// A Movies Library holding Alien in four Versions, three of them with an Edition, whose title
/// source has Alien.
async fn alien_in_four_versions() -> Fixture {
    let tmdb =
        ScriptedSource::new("tmdb", &SourcesAsked::default()).with_movie(&["tmdb-348"], "Alien");
    let plugins = Registry::new();
    plugins.enable(Arc::new(tmdb)).unwrap();
    let media = Fixture::with_plugins(LibraryKind::Movies, plugins).await;
    for relative in [THEATRICAL, DIRECTORS_1080P, DIRECTORS_2160P, UNCUT] {
        media.write(relative, 10);
    }
    media
}

/// The Edition of each of the Movie's Versions, by file name.
async fn editions(media: &Fixture) -> Vec<(String, Option<String>)> {
    let movies = media.contents.movies(media.library, None).await.unwrap();
    let [alien] = movies.as_slice() else {
        panic!("one Movie, not {movies:?}");
    };
    let mut editions: Vec<_> = (alien.versions.iter())
        .map(|version| {
            let name = Path::new(&version.path).file_name().unwrap();
            (name.to_str().unwrap().to_owned(), version.edition.clone())
        })
        .collect();
    editions.sort();
    editions
}

#[tokio::test]
async fn each_version_holds_the_edition_its_file_or_else_its_folder_names() {
    let media = alien_in_four_versions().await;

    media.scan().await;

    let edition = |name: &str| Some(name.to_owned());
    let named = |name: &str, held| (name.to_owned(), held);
    assert_eq!(
        editions(&media).await,
        [
            named("Alien (1979) - 720p.mkv", None),
            named(
                "Alien (1979) {edition-Director's Cut} 2160p.mkv",
                edition("Director's Cut")
            ),
            named(
                "Alien (1979) {edition-Director's Cut}.mkv",
                edition("Director's Cut")
            ),
            named("Alien (1979).mkv", edition("Theatrical")),
        ]
    );
}

#[tokio::test]
async fn files_stored_before_editions_get_theirs_on_the_next_scan() {
    let media = alien_in_four_versions().await;
    media.scan().await;
    let before = editions(&media).await;

    media.contents.read_by_older_parser(media.library).await;

    assert_eq!(media.scan().await, summary(0, 4, 0, 0));
    assert_eq!(editions(&media).await, before);
}
