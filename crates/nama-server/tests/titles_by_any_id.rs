//! A Library's Movie or Show named by any of its ids, not only the one browsing gives it
//! (ADR 0042), through `BrowseService` and `WatchStateService`, with a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use browsing::{Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    ExternalId, GetLibraryRequest, GetMovieRequest, GetShowRequest, LibraryKind,
    LibraryServiceClient, ListEpisodesRequest, MediaToDelete, PreviewDeleteMediaRequest,
    SeasonToDelete, SetPlayedRequest, ShowToDelete, UpdateLibraryRequest, WatchStateServiceClient,
    media_to_delete, set_played_request::Target,
};
use nama_server::ffmpeg::Ffmpeg;
use server::tmdb::{DARK, DARK_TVDB};

/// Alien's IMDb id, as the fake TMDB gives it.
const ALIEN_IMDB: &str = "tt0078748";

fn id(namespace: &str, value: &str) -> ExternalId {
    ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_movie_opened_and_marked_by_its_imdb_id_is_the_library_movie() {
    let fx = Fixture::alien().await;
    let imdb = id("imdb", ALIEN_IMDB);
    let watch_states =
        WatchStateServiceClient::new(HttpClient::plaintext(), fx.server.client_config());

    watch_states
        .set_played(SetPlayedRequest {
            library_id: fx.library,
            target: Some(Target::MovieId(Box::new(imdb.clone()))),
            played: true,
            ..Default::default()
        })
        .await
        .unwrap();
    let movie = (fx.browse)
        .get_movie(GetMovieRequest {
            library_id: fx.library,
            movie_id: imdb.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movie
        .into_option()
        .unwrap();

    assert_eq!(movie.id.as_option(), Some(&tmdb_id(348)));
    assert!(movie.watch_state.as_option().unwrap().played);
    let by_tmdb = fx.get_movie(fx.library, 348).await.unwrap();
    assert!(by_tmdb.watch_state.as_option().unwrap().played);
}

#[tokio::test]
async fn a_movie_marked_played_stays_played_after_its_librarys_title_source_order_changes() {
    let fx = Fixture::alien().await;
    let config = fx.server.client_config();
    let watch_states = WatchStateServiceClient::new(HttpClient::plaintext(), config.clone());
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), config);
    watch_states
        .set_played(SetPlayedRequest {
            library_id: fx.library,
            target: Some(Target::MovieId(Box::new(tmdb_id(348)))),
            played: true,
            ..Default::default()
        })
        .await
        .unwrap();

    let library = libraries
        .get_library(GetLibraryRequest {
            id: fx.library,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap();
    libraries
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: library.name,
            folders: library.folders,
            title_sources: vec!["imdb".to_owned(), "tmdb".to_owned()],
            ..Default::default()
        })
        .await
        .unwrap();
    let movie = fx.get_movie(fx.library, 348).await.unwrap();

    assert_eq!(movie.id.as_option(), Some(&id("imdb", ALIEN_IMDB)));
    assert!(movie.watch_state.as_option().unwrap().played);
}

#[tokio::test]
async fn a_show_opened_by_its_tvdb_id_is_the_library_show() {
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_SHOWS, Ffmpeg::default()).await;
    fx.write("Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv", b"video");
    fx.scan_and_match().await;
    let tvdb = id("tvdb", &DARK_TVDB.to_string());

    let show = (fx.browse)
        .get_show(GetShowRequest {
            library_id: fx.library,
            show_id: tvdb.clone().into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let episodes = (fx.browse)
        .list_episodes(ListEpisodesRequest {
            library_id: fx.library,
            show_id: tvdb.into(),
            season_number: 1,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .episodes;

    assert_eq!(
        show.show.as_option().unwrap().id.as_option(),
        Some(&tmdb_id(DARK))
    );
    assert_eq!(episodes.len(), 1);
}

#[tokio::test]
async fn a_show_and_season_previewed_for_delete_by_tvdb_id_are_the_library_show() {
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_SHOWS, Ffmpeg::default()).await;
    fx.write("Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv", b"video");
    fx.scan_and_match().await;
    let tvdb = id("tvdb", &DARK_TVDB.to_string());
    let library = LibraryServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let targets = [
        media_to_delete::Target::Show(Box::new(ShowToDelete {
            library_id: fx.library,
            show_id: tvdb.clone().into(),
            ..Default::default()
        })),
        media_to_delete::Target::Season(Box::new(SeasonToDelete {
            library_id: fx.library,
            show_id: tvdb.into(),
            season_number: 1,
            ..Default::default()
        })),
    ];

    for target in targets {
        let preview = library
            .preview_delete_media(PreviewDeleteMediaRequest {
                target: MediaToDelete {
                    target: Some(target),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        assert_eq!(preview.files.len(), 1);
    }
}
