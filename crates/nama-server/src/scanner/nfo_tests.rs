use std::{fs, path::Path, sync::Arc};

use nama_plugin_api::metadata::ExternalId;

use super::super::tests::{Fixture, summary};
use crate::{
    library::{
        LibraryKind, Recorded,
        scripted::{ScriptedSource, SourcesAsked},
    },
    log_capture::CapturedLog,
    plugins::Registry,
};

/// A TMDB title source, looking titles up by IMDb ids too, with Alien, Aliens and Alien 3. No
/// search finds anything, so an Item Matches only by an id.
fn movies(asked: &SourcesAsked) -> ScriptedSource {
    ScriptedSource::new("tmdb", asked)
        .looking_up(&["imdb"])
        .with_movie(&["tmdb-348", "imdb-tt0078748"], "Alien")
        .with_movie(&["tmdb-679", "imdb-tt0090605"], "Aliens")
        .with_movie(&["tmdb-8077"], "Alien 3")
}

/// A Library of `kind` asking `source` alone.
async fn library(kind: LibraryKind, source: ScriptedSource) -> Fixture {
    let plugins = Registry::new();
    plugins.enable(Arc::new(source)).unwrap();
    Fixture::with_plugins(kind, plugins).await
}

/// A Movies Library asking [`movies`].
async fn movies_library() -> Fixture {
    library(LibraryKind::Movies, movies(&SourcesAsked::default())).await
}

fn put(media: &Fixture, relative: &str, contents: impl AsRef<[u8]>) {
    let path = media.path(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// What each stored Item Matched, by its file's path: its title's TMDB id, or `unmatched` or
/// `undecided`.
async fn matched(media: &Fixture) -> Vec<(String, String)> {
    let files = media.contents.files(media.library).await.unwrap();
    files
        .into_iter()
        .flat_map(|file| {
            let within = Path::new(&file.path).strip_prefix(&media.root).unwrap();
            let parts: Vec<_> = within.iter().map(|part| part.to_str().unwrap()).collect();
            let relative = parts.join("/");
            let items = file.items.into_iter();
            items.map(move |item| (relative.clone(), outcome(item.recorded.as_ref())))
        })
        .collect()
}

fn outcome(recorded: Option<&Recorded>) -> String {
    match recorded {
        None => "undecided".to_owned(),
        Some(recorded) if !recorded.matched => "unmatched".to_owned(),
        Some(recorded) => {
            let ids = if recorded.show_ids.is_empty() {
                &recorded.ids
            } else {
                &recorded.show_ids
            };
            ids.get("tmdb").unwrap().to_string()
        }
    }
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    let owned = expected
        .iter()
        .map(|&(path, got)| (path.to_owned(), got.to_owned()));
    owned.collect()
}

fn tmdb_nfo(id: &str) -> String {
    format!(r#"<movie><uniqueid type="tmdb" default="true">{id}</uniqueid></movie>"#)
}

#[tokio::test]
async fn a_movies_nfo_names_the_title_it_matches_without_a_search() {
    let asked = SourcesAsked::default();
    let media = library(LibraryKind::Movies, movies(&asked)).await;
    put(&media, "Home Video (2001)/Home Video (2001).mkv", "video");
    put(
        &media,
        "Home Video (2001)/Home Video (2001).nfo",
        tmdb_nfo("679"),
    );

    media.scan().await;

    let got = matched(&media).await;
    assert_eq!(
        got,
        pairs(&[("Home Video (2001)/Home Video (2001).mkv", "tmdb-679")])
    );
    assert!(
        asked.lines().iter().all(|line| !line.contains("search")),
        "{:?}",
        asked.lines()
    );
}

#[tokio::test]
async fn an_nfos_ids_replace_the_names_and_the_names_count_once_the_sources_reject_them() {
    let media = movies_library().await;
    put(&media, "Alien (1979) {tmdb-348}/Alien (1979).mkv", "video");
    put(
        &media,
        "Alien (1979) {tmdb-348}/Alien (1979).nfo",
        tmdb_nfo("679"),
    );
    put(
        &media,
        "Alien 3 (1992) {tmdb-8077}/Alien 3 (1992).mkv",
        "video",
    );
    put(
        &media,
        "Alien 3 (1992) {tmdb-8077}/Alien 3 (1992).nfo",
        tmdb_nfo("999"),
    );

    media.scan().await;

    assert_eq!(
        matched(&media).await,
        pairs(&[
            ("Alien (1979) {tmdb-348}/Alien (1979).mkv", "tmdb-679"),
            ("Alien 3 (1992) {tmdb-8077}/Alien 3 (1992).mkv", "tmdb-8077"),
        ])
    );
}

#[tokio::test]
async fn a_video_takes_its_own_nfo_then_its_stacks_then_movie_nfo_in_a_folder_of_one_movie() {
    let media = movies_library().await;
    // Its own wins over `movie.nfo`, whatever the case of either name.
    put(&media, "Both (2004)/Both (2004).mkv", "video");
    put(&media, "Both (2004)/Both (2004).NFO", tmdb_nfo("679"));
    put(&media, "Both (2004)/Movie.nfo", tmdb_nfo("348"));
    // A stack's parts share the stack's.
    put(&media, "Stack (2005)/Stack (2005)-cd1.mkv", "video");
    put(&media, "Stack (2005)/Stack (2005) - Part 2.mkv", "video");
    put(&media, "Stack (2005)/Stack (2005).nfo", tmdb_nfo("8077"));
    // One Movie in two Versions.
    put(&media, "Solo (2003)/Solo (2003) - 1080p.mkv", "video");
    put(&media, "Solo (2003)/Solo (2003) - 2160p.mkv", "video");
    put(&media, "Solo (2003)/MOVIE.NFO", tmdb_nfo("348"));
    // Two Movies in one folder, which `movie.nfo` names neither of.
    put(&media, "Flat/First (2001).mkv", "video");
    put(&media, "Flat/Second (2002).mkv", "video");
    put(&media, "Flat/movie.nfo", tmdb_nfo("348"));

    media.scan().await;

    assert_eq!(
        matched(&media).await,
        pairs(&[
            ("Both (2004)/Both (2004).mkv", "tmdb-679"),
            ("Flat/First (2001).mkv", "unmatched"),
            ("Flat/Second (2002).mkv", "unmatched"),
            ("Solo (2003)/Solo (2003) - 1080p.mkv", "tmdb-348"),
            ("Solo (2003)/Solo (2003) - 2160p.mkv", "tmdb-348"),
            ("Stack (2005)/Stack (2005) - Part 2.mkv", "tmdb-8077"),
            ("Stack (2005)/Stack (2005)-cd1.mkv", "tmdb-8077"),
        ])
    );
}

#[tokio::test]
async fn tvshow_nfo_names_the_show_and_an_episodes_own_nfo_its_episode() {
    let show = "tmdb-70523";
    let source = ScriptedSource::new("tmdb", &SourcesAsked::default())
        .with_show(&[show], "Dark", &[1, 2])
        .with_season(show, 1, &[1, 2])
        .with_season(show, 2, &[1])
        .episode_id(show, 2, 1, "tmdb-1001");
    let media = library(LibraryKind::Shows, source).await;
    put(
        &media,
        "Kingdom/tvshow.nfo",
        "<tvshow><uniqueid type=\"tmdb\">70523</uniqueid></tvshow>",
    );
    put(&media, "Kingdom/Season 01/Kingdom - S01E01.mkv", "video");
    // Numbered as the Show's Season 1 has no Episode 9, but its `.nfo` names Season 2's first.
    put(&media, "Kingdom/Season 01/Kingdom - S01E09.mkv", "video");
    // Only the first of several Episodes' blocks is read.
    put(
        &media,
        "Kingdom/Season 01/Kingdom - S01E09.nfo",
        "<episodedetails><uniqueid type=\"tmdb\">1001</uniqueid></episodedetails>\n\
         <episodedetails><uniqueid type=\"tmdb\">2002</uniqueid></episodedetails>",
    );
    put(&media, "Elsewhere/Elsewhere - S01E01.mkv", "video");

    media.scan().await;

    let files = media.contents.files(media.library).await.unwrap();
    let recorded = |name: &str| {
        let file = files.iter().find(|file| file.path.ends_with(name)).unwrap();
        file.items[0].recorded.clone()
    };
    let ninth = recorded("Kingdom - S01E09.mkv").unwrap();
    assert_eq!(
        (
            outcome(recorded("Kingdom - S01E01.mkv").as_ref()),
            outcome(Some(&ninth))
        ),
        ("tmdb-70523".to_owned(), "tmdb-70523".to_owned())
    );
    assert_eq!(ninth.ids.get("tmdb"), Some(ExternalId::tmdb(1001)));
    assert_eq!(
        outcome(recorded("Elsewhere - S01E01.mkv").as_ref()),
        "unmatched"
    );
}

#[tokio::test]
async fn adding_editing_or_deleting_an_nfo_matches_an_unchanged_video_again() {
    const VIDEO: &str = "Alien (1979) {tmdb-348}/Alien (1979).mkv";
    const NFO: &str = "Alien (1979) {tmdb-348}/Alien (1979).nfo";
    let media = movies_library().await;
    put(&media, VIDEO, "video");
    media.scan().await;
    let got = async || matched(&media).await.remove(0).1;
    assert_eq!(got().await, "tmdb-348");

    put(&media, NFO, tmdb_nfo("679"));
    assert_eq!(media.scan().await, summary(0, 0, 0, 1));
    assert_eq!(got().await, "tmdb-679");

    put(&media, NFO, tmdb_nfo("8077"));
    media.scan().await;
    assert_eq!(got().await, "tmdb-8077");

    fs::remove_file(media.path(NFO)).unwrap();
    media.scan().await;
    assert_eq!(got().await, "tmdb-348");
}

#[tokio::test]
async fn a_pinned_match_stays_whatever_an_nfo_says() {
    let media = movies_library().await;
    put(&media, "Alien (1979)/Alien (1979).mkv", "video");
    media.scan().await;
    let item = media.contents.files(media.library).await.unwrap()[0].items[0].id;
    let pinned = ExternalId::tmdb(8077);
    media
        .scanner
        .matcher()
        .set_match(item, &pinned)
        .await
        .unwrap();

    put(&media, "Alien (1979)/Alien (1979).nfo", tmdb_nfo("679"));
    media.scan().await;
    assert_eq!(matched(&media).await[0].1, "tmdb-8077");

    // Changed, so its Item is new and undecided, but its name is still pinned.
    put(&media, "Alien (1979)/Alien (1979).mkv", "a remuxed video");
    media.scan().await;
    assert_eq!(matched(&media).await[0].1, "tmdb-8077");
}

#[tokio::test]
async fn an_nfo_is_read_again_only_once_its_size_or_modified_time_changes() {
    const NFO: &str = "Home Video (2001)/Home Video (2001).nfo";
    let media = movies_library().await;
    put(&media, "Home Video (2001)/Home Video (2001).mkv", "video");
    put(&media, NFO, tmdb_nfo("679"));
    media.scan().await;
    let modified = fs::metadata(media.path(NFO)).unwrap().modified().unwrap();

    // As long as before, and given its old modified time back.
    put(&media, NFO, tmdb_nfo("348"));
    let file = fs::File::options().write(true).open(media.path(NFO));
    file.unwrap().set_modified(modified).unwrap();
    media.scan().await;
    assert_eq!(matched(&media).await[0].1, "tmdb-679");

    put(&media, NFO, tmdb_nfo("348"));
    media.scan().await;
    assert_eq!(matched(&media).await[0].1, "tmdb-348");
}

#[tokio::test]
async fn an_nfo_naming_no_id_counts_as_none_and_is_logged() {
    let media = movies_library().await;
    put(&media, "Alien (1979) {tmdb-348}/Alien (1979).mkv", "video");
    put(
        &media,
        "Alien (1979) {tmdb-348}/Alien (1979).nfo",
        "<movie><title>Alien</titl",
    );

    let log = CapturedLog::start();
    media.scan().await;
    let log = log.finish();

    assert_eq!(matched(&media).await[0].1, "tmdb-348");
    assert!(
        log.contains("WARN") && log.contains("Alien (1979).nfo"),
        "{log}"
    );
}

#[tokio::test]
async fn an_nfo_naming_ids_no_source_looks_up_leaves_its_video_undecided() {
    let media = movies_library().await;
    put(&media, "Alien (1979) {tmdb-348}/Alien (1979).mkv", "video");
    put(
        &media,
        "Alien (1979) {tmdb-348}/Alien (1979).nfo",
        "<movie><uniqueid type=\"anidb\">5101</uniqueid></movie>",
    );

    media.scan().await;

    assert_eq!(matched(&media).await[0].1, "undecided");
}

#[tokio::test]
async fn an_nfo_over_4_mib_is_skipped_and_logged() {
    let media = movies_library().await;
    put(&media, "Home Video (2001)/Home Video (2001).mkv", "video");
    let mut huge = tmdb_nfo("679").into_bytes();
    huge.resize(4 * 1024 * 1024 + 1, b' ');
    put(&media, "Home Video (2001)/Home Video (2001).nfo", huge);

    let log = CapturedLog::start();
    media.scan().await;
    let log = log.finish();

    assert_eq!(matched(&media).await[0].1, "unmatched");
    assert!(
        log.contains("WARN") && log.contains("Home Video (2001).nfo"),
        "{log}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_nfo_that_is_a_link_is_skipped_and_logged() {
    let media = movies_library().await;
    put(&media, "Home Video (2001)/Home Video (2001).mkv", "video");
    put(&media, "elsewhere.txt", tmdb_nfo("679"));
    let link = media.path("Home Video (2001)/Home Video (2001).nfo");
    std::os::unix::fs::symlink(media.path("elsewhere.txt"), link).unwrap();

    let log = CapturedLog::start();
    media.scan().await;
    let log = log.finish();

    assert_eq!(matched(&media).await[0].1, "unmatched");
    assert!(
        log.contains("WARN") && log.contains("Home Video (2001).nfo"),
        "{log}"
    );
}

/// `text` in UTF-16, little-endian, after its byte-order mark.
fn utf16_with_bom(text: &str) -> Vec<u8> {
    let units = text.encode_utf16().flat_map(u16::to_le_bytes);
    [0xFF, 0xFE].into_iter().chain(units).collect()
}

/// `.nfo` files as tools write them: what each is, its bytes, and what its video Matches.
fn samples() -> [(&'static str, Vec<u8>, &'static str); 12] {
    [
        (
            "Kodi, the default id first",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>
<movie>
  <title>Aliens</title>
  <uniqueid type="imdb">tt0078748</uniqueid>
  <uniqueid type="tmdb" default="true">679</uniqueid>
</movie>"#
                .to_vec(),
            "tmdb-679",
        ),
        (
            "a type spelled out",
            br#"<movie><uniqueid type="TheMovieDB">8077</uniqueid></movie>"#.to_vec(),
            "tmdb-8077",
        ),
        (
            "an untyped IMDb id",
            b"<movie><uniqueid>tt0090605</uniqueid></movie>".to_vec(),
            "tmdb-679",
        ),
        (
            "legacy id",
            b"<movie><id>tt0078748</id></movie>".to_vec(),
            "tmdb-348",
        ),
        (
            "legacy imdbid over id",
            b"<movie><id>348</id><imdbid>tt0090605</imdbid></movie>".to_vec(),
            "tmdb-679",
        ),
        (
            "a TMDB link alone",
            b"https://www.themoviedb.org/movie/8077-alien-3\n".to_vec(),
            "tmdb-8077",
        ),
        (
            "an IMDb link alone",
            b"http://www.imdb.com/title/tt0078748/".to_vec(),
            "tmdb-348",
        ),
        (
            "UTF-16 with a byte-order mark",
            utf16_with_bom(r#"<movie><uniqueid type="tmdb">679</uniqueid></movie>"#),
            "tmdb-679",
        ),
        (
            "a declared legacy charset",
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n\
              <movie><title>Alien\xB3</title><tmdbid>8077</tmdbid></movie>"
                .to_vec(),
            "tmdb-8077",
        ),
        (
            "placeholders before a real id",
            br#"<movie><uniqueid type="tmdb" default="true">0</uniqueid>
<uniqueid type="imdb">tt0000000</uniqueid><imdbid>N/A</imdbid><tmdbid>348</tmdbid></movie>"#
                .to_vec(),
            "tmdb-348",
        ),
        (
            "entities, and XML left unclosed",
            b"<movie><plot>Ripley & the marines<br></plot><uniqueid type='tmdb'>&#54;79</uniqueid>"
                .to_vec(),
            "tmdb-679",
        ),
        ("garbage", b"\x00\x01\x02 no ids here".to_vec(), "unmatched"),
    ]
}

#[tokio::test]
async fn nfo_files_as_kodi_jellyfin_emby_and_scrapers_write_them_name_their_ids() {
    let samples = samples();
    let media = movies_library().await;
    for (n, (_, nfo, _)) in (1..).zip(&samples) {
        put(
            &media,
            &format!("Sample {n} (2000)/Sample {n} (2000).mkv"),
            "video",
        );
        put(
            &media,
            &format!("Sample {n} (2000)/Sample {n} (2000).nfo"),
            nfo,
        );
    }

    media.scan().await;

    let got = matched(&media).await;
    for (n, (case, _, expected)) in (1..).zip(&samples) {
        let path = format!("Sample {n} (2000)/Sample {n} (2000).mkv");
        let outcome = got
            .iter()
            .find(|(stored, _)| *stored == path)
            .map(|(_, got)| got);
        assert_eq!(outcome.map(String::as_str), Some(*expected), "{case}");
    }
}
