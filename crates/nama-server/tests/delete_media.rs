//! Deleting media through `LibraryService.PreviewDeleteMedia` and `DeleteMedia`, over a real
//! socket, against Libraries over temporary folders with real files and TMDB on against a fake,
//! and the Radarr Plugin, bundled in the Server, against a fake Radarr.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary only deletes")]
#[path = "support/fake_radarr.rs"]
mod fake_radarr;
#[expect(dead_code, reason = "this binary reads only the status")]
#[path = "support/http.rs"]
mod http;
#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::fs;

use browsing::{ALIEN_1080P, ALIEN_2160P, Fixture, tmdb_id};
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_radarr::{API_KEY as RADARR_KEY, Changed, FakeRadarr};
use http::fetch;
use log::Log;
use nama_proto::nama::v1::{
    DeleteMediaRequest, DeleteMediaResponse, EnablePluginRequest, LibraryKind,
    LibraryServiceClient, ListMoviesRequest, MediaFile, MediaToDelete, MovieToDelete, Permission,
    PluginServiceClient, PreviewDeleteMediaRequest, PreviewDeleteMediaResponse, RefusalReason,
    SettingValue, SubscribeRequest, UpdatePluginSettingsRequest, event, media_to_delete,
    setting_value,
};
use refused::assert_refused;
use serde_json::json;
use server::{
    Builder, TestServer,
    fulfiller::MOVIE_FULFILLER,
    subscribe_with,
    tmdb::{ALIEN, DUNE},
};

const ALIEN_1080P_SUBTITLES: &str = "Alien (1979)/Alien (1979) - 1080p.en.srt";
const ALIEN_2160P_POSTER: &str = "Alien (1979)/Alien (1979) - 2160p-poster.jpg";
/// Beside Alien's Versions, but named after neither.
const ALIEN_FOLDER_ART: &str = "Alien (1979)/folder.jpg";
/// Straight in the Library's folder, with nothing else of its own.
const DUNE_VIDEO: &str = "Dune (2021).mkv";
const DUNE_NFO: &str = "Dune (2021).nfo";

/// Every Version of the Movie with the TMDB id `tmdb` in `library`.
fn movie(library: i64, tmdb: i64) -> MediaToDelete {
    MediaToDelete {
        target: Some(media_to_delete::Target::Movie(Box::new(MovieToDelete {
            library_id: library,
            movie_id: tmdb_id(tmdb).into(),
            ..Default::default()
        }))),
        ..Default::default()
    }
}

/// The one Version `id`.
fn version(id: i64) -> MediaToDelete {
    MediaToDelete {
        target: Some(media_to_delete::Target::VersionId(id)),
        ..Default::default()
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// A Movies Library holding Alien in two Versions and Dune, with their sidecars, scanned and
    /// Matched.
    async fn alien_and_dune() -> Self {
        Self::alien_and_dune_on(TestServer::builder()).await
    }

    /// [`Self::alien_and_dune`] on a Server `builder` starts.
    async fn alien_and_dune_on(builder: Builder) -> Self {
        let fx = Self::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
        fx.write(ALIEN_1080P, b"1080p video");
        fx.write(ALIEN_2160P, b"2160p video, which is larger");
        fx.write(ALIEN_1080P_SUBTITLES, b"subtitles");
        fx.write(ALIEN_2160P_POSTER, b"poster");
        fx.write(DUNE_VIDEO, b"dune video");
        fx.write(DUNE_NFO, b"nfo");
        fx.scan_and_match().await;
        fx
    }

    /// Enables the Radarr Plugin against `radarr`, as an Admin sets it up.
    async fn enable_radarr(&self, radarr: &FakeRadarr) {
        let plugins =
            PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let settings = [
            ("url", radarr.url()),
            ("api_key", RADARR_KEY.to_owned()),
            ("quality_profile", "4".to_owned()),
            ("root_folder", "/movies".to_owned()),
        ]
        .map(|(key, value)| {
            let value = SettingValue {
                value: Some(setting_value::Value::TextValue(value)),
                ..Default::default()
            };
            (key.to_owned(), value)
        });
        plugins
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "radarr".to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        plugins
            .enable_plugin(EnablePluginRequest {
                id: "radarr".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    fn library_service(&self) -> LibraryServiceClient<HttpClient> {
        LibraryServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    async fn preview(
        &self,
        target: MediaToDelete,
    ) -> Result<PreviewDeleteMediaResponse, ConnectError> {
        let response = self
            .library_service()
            .preview_delete_media(PreviewDeleteMediaRequest {
                target: target.into(),
                ..Default::default()
            })
            .await?;
        Ok(response.into_owned())
    }

    async fn delete(&self, target: MediaToDelete) -> Result<DeleteMediaResponse, ConnectError> {
        self.delete_as(&self.library_service(), target).await
    }

    async fn delete_as(
        &self,
        client: &LibraryServiceClient<HttpClient>,
        target: MediaToDelete,
    ) -> Result<DeleteMediaResponse, ConnectError> {
        let response = client
            .delete_media(DeleteMediaRequest {
                target: target.into(),
                ..Default::default()
            })
            .await?;
        Ok(response.into_owned())
    }

    fn exists(&self, relative: &str) -> bool {
        self.folder.join(relative).exists()
    }

    /// The titles of the Library's Movies, as `ListMovies` gives them.
    async fn titles(&self) -> Vec<String> {
        let listed = self
            .browse
            .list_movies(ListMoviesRequest {
                library_id: self.library,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        listed.movies.into_iter().map(|movie| movie.title).collect()
    }
}

/// `files` by name and size, sorted by name.
fn by_name(files: &[MediaFile]) -> Vec<(&str, u64)> {
    let mut named: Vec<_> = files
        .iter()
        .map(|file| (file.file_name.as_str(), file.size))
        .collect();
    named.sort_unstable();
    named
}

#[tokio::test]
async fn the_preview_lists_the_files_that_would_go_and_the_bytes_they_free_deleting_nothing() {
    let fx = Fixture::alien_and_dune().await;

    let preview = fx.preview(movie(fx.library, ALIEN)).await.unwrap();

    assert_eq!(
        by_name(&preview.files),
        [
            ("Alien (1979) - 1080p.en.srt", 9),
            ("Alien (1979) - 1080p.mkv", 11),
            ("Alien (1979) - 2160p-poster.jpg", 6),
            ("Alien (1979) - 2160p.mkv", 28),
        ]
    );
    assert_eq!(preview.bytes_freed, 9 + 11 + 6 + 28);
    assert!(fx.exists(ALIEN_1080P) && fx.exists(ALIEN_1080P_SUBTITLES));
}

#[tokio::test]
async fn deleting_a_movie_deletes_its_versions_and_their_sidecars_and_leaves_other_files() {
    let fx = Fixture::alien_and_dune().await;

    let deleted = fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    assert_eq!(by_name(&deleted.deleted).len(), 4, "{deleted:?}");
    assert_eq!(deleted.bytes_freed, 9 + 11 + 6 + 28);
    assert!(deleted.not_deleted.is_empty(), "{deleted:?}");
    for gone in [
        ALIEN_1080P,
        ALIEN_2160P,
        ALIEN_1080P_SUBTITLES,
        ALIEN_2160P_POSTER,
    ] {
        assert!(!fx.exists(gone), "{gone}");
    }
    for kept in [DUNE_VIDEO, DUNE_NFO] {
        assert!(fx.exists(kept), "{kept}");
    }
}

#[tokio::test]
async fn deleting_removes_folders_left_empty_but_never_the_librarys_own_folder() {
    let fx = Fixture::alien_and_dune().await;

    fx.delete(movie(fx.library, ALIEN)).await.unwrap();
    fx.delete(movie(fx.library, DUNE)).await.unwrap();

    assert!(!fx.exists("Alien (1979)"));
    assert!(fx.folder.is_dir());
    assert_eq!(fs::read_dir(&fx.folder).unwrap().count(), 0);
}

#[tokio::test]
async fn a_folder_holding_a_file_named_after_nothing_deleted_stays() {
    let fx = Fixture::alien_and_dune().await;
    fx.write(ALIEN_FOLDER_ART, b"art");

    fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    assert!(fx.exists(ALIEN_FOLDER_ART));
}

#[tokio::test]
async fn deleting_one_version_of_two_leaves_the_other_playable() {
    let fx = Fixture::alien_and_dune().await;
    let [first, second] = <[i64; 2]>::try_from(fx.version_ids(ALIEN).await).unwrap();

    let deleted = fx.delete(version(first)).await.unwrap();

    assert_eq!(
        by_name(&deleted.deleted),
        [
            ("Alien (1979) - 1080p.en.srt", 9),
            ("Alien (1979) - 1080p.mkv", 11)
        ]
    );
    assert_eq!(fx.version_ids(ALIEN).await, [second]);
    assert!(fx.exists(ALIEN_2160P_POSTER));
    let path = format!("/media/versions/{second}");
    let reply = fetch(&fx.server, &path, Some(&fx.server.admin_token)).await;
    assert_eq!(reply.status, 200);
}

#[tokio::test]
async fn browse_drops_what_was_deleted_and_a_subscriber_is_told_it_was_removed() {
    let fx = Fixture::alien_and_dune().await;
    let about = SubscribeRequest {
        library_ids: vec![fx.library],
        ..Default::default()
    };
    let mut events = subscribe_with(&fx.server.client_config(), about).await;

    fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    for _ in 0..2 {
        let event::Event::LibraryItemRemoved(removed) = events.next().await else {
            panic!("expected an Item removed");
        };
        assert_eq!(removed.library_id, fx.library);
    }
    assert_eq!(fx.titles().await, ["Dune"]);
}

#[tokio::test]
async fn a_file_that_cant_be_deleted_is_reported_with_why_while_the_rest_go() {
    let fx = Fixture::alien_and_dune().await;
    // A folder where the 2160p file was can't be deleted as a file, whoever the Server runs as.
    fs::remove_file(fx.folder.join(ALIEN_2160P)).unwrap();
    fx.write(&format!("{ALIEN_2160P}/inside"), b"");

    let deleted = fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    let [not_deleted] = deleted.not_deleted.as_slice() else {
        panic!("expected one file not deleted: {deleted:?}");
    };
    assert_eq!(not_deleted.file_name, "Alien (1979) - 2160p.mkv");
    assert_ne!(not_deleted.reason, "");
    assert_eq!(
        by_name(&deleted.deleted),
        [
            ("Alien (1979) - 1080p.en.srt", 9),
            ("Alien (1979) - 1080p.mkv", 11)
        ]
    );
    // Its sidecar stays with it.
    assert!(fx.exists(ALIEN_2160P_POSTER));
    assert_eq!(fx.version_ids(ALIEN).await.len(), 1);
}

#[tokio::test]
async fn deleting_a_movie_or_one_of_its_versions_has_the_movie_fulfiller_stop_monitoring_it() {
    let fx = Fixture::alien_and_dune_on(TestServer::builder().fulfiller(&[])).await;
    let first = fx.version_ids(ALIEN).await[0];

    let one_version = fx.delete(version(first)).await.unwrap();
    let every_version = fx.delete(movie(fx.library, DUNE)).await.unwrap();

    assert_eq!(fx.server.fulfiller().stopped_monitoring(), [ALIEN, DUNE]);
    assert_eq!(one_version.fulfillers_not_told, [""; 0]);
    assert_eq!(every_version.fulfillers_not_told, [""; 0]);
}

#[tokio::test]
async fn a_fulfiller_that_cant_be_told_is_named_and_the_delete_still_happens() {
    let fx = Fixture::alien_and_dune_on(TestServer::builder().fulfiller(&[])).await;
    fx.server.fulfiller().refuse();

    let deleted = fx.delete(movie(fx.library, DUNE)).await.unwrap();

    assert_eq!(deleted.fulfillers_not_told, [MOVIE_FULFILLER]);
    assert!(!fx.exists(DUNE_VIDEO));
    assert_eq!(fx.titles().await, ["Alien"]);
}

/// What the fake Radarr is asked when told to unmonitor Alien, which it has as its Movie 2, while
/// the file it watches is `still_there` or not.
fn unmonitored_alien(still_there: bool) -> Changed {
    Changed {
        request: "PUT /api/v3/movie/editor".to_owned(),
        body: json!({ "movieIds": [2], "monitored": false }),
        watched_was_there: still_there,
    }
}

#[tokio::test]
async fn a_fulfiller_that_panics_when_asked_is_named_and_the_delete_still_happens() {
    let fx = Fixture::alien_and_dune_on(TestServer::builder().fulfiller(&[])).await;
    fx.server.fulfiller().panic_stopping_monitoring();

    let deleted = fx.delete(movie(fx.library, DUNE)).await.unwrap();

    assert_eq!(deleted.fulfillers_not_told, [MOVIE_FULFILLER]);
    assert!(!fx.exists(DUNE_VIDEO));
}

#[tokio::test]
async fn deleting_a_movie_or_one_of_its_versions_has_radarr_unmonitor_it_first_and_keep_it() {
    let fx = Fixture::alien_and_dune().await;
    let radarr = FakeRadarr::start(&[ALIEN], &[]).await;
    fx.enable_radarr(&radarr).await;
    let first = fx.version_ids(ALIEN).await[0];

    radarr.watch(fx.folder.join(ALIEN_1080P));
    let one_version = fx.delete(version(first)).await.unwrap();
    radarr.watch(fx.folder.join(ALIEN_2160P));
    let every_version = fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    assert_eq!(
        radarr.changes(),
        [unmonitored_alien(true), unmonitored_alien(true)]
    );
    assert_eq!(one_version.fulfillers_not_told, [""; 0]);
    assert_eq!(every_version.fulfillers_not_told, [""; 0]);
    assert!(!fx.exists(ALIEN_1080P) && !fx.exists(ALIEN_2160P));
}

#[tokio::test]
async fn a_movie_radarr_doesnt_have_deletes_with_nothing_reported() {
    let fx = Fixture::alien_and_dune().await;
    let radarr = FakeRadarr::start(&[], &[]).await;
    fx.enable_radarr(&radarr).await;

    let deleted = fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    assert_eq!(radarr.changes(), []);
    assert_eq!(deleted.fulfillers_not_told, [""; 0]);
    assert!(!fx.exists(ALIEN_1080P));
}

#[tokio::test]
async fn with_radarr_down_the_delete_goes_ahead_and_names_radarr() {
    let fx = Fixture::alien_and_dune().await;
    let mut radarr = FakeRadarr::start(&[ALIEN], &[]).await;
    fx.enable_radarr(&radarr).await;
    radarr.go_down();

    let deleted = fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    assert_eq!(deleted.fulfillers_not_told, ["Radarr"]);
    assert!(!fx.exists(ALIEN_1080P) && !fx.exists(ALIEN_2160P));
    assert_eq!(fx.titles().await, ["Dune"]);
}

#[tokio::test]
async fn a_scan_racing_the_delete_fails_nothing_and_leaves_nothing_behind() {
    let fx = Fixture::alien_and_dune().await;

    let (scanned, deleted) = tokio::join!(fx.scan_and_match(), fx.delete(movie(fx.library, ALIEN)));
    drop(scanned);
    deleted.unwrap();
    fx.scan_and_match().await;

    assert_eq!(fx.titles().await, ["Dune"]);
    assert!(!fx.exists("Alien (1979)"));
    assert_eq!(Log::global().count(&["scanning a Library failed"]), 0);
}

#[tokio::test]
async fn each_delete_is_logged_with_who_what_which_files_and_the_bytes_freed() {
    let log = Log::global();
    let fx = Fixture::alien_and_dune().await;
    let grace = fx.server.user("Grace", &[Permission::DeleteMedia]).await;
    let as_grace = LibraryServiceClient::new(HttpClient::plaintext(), grace.client_config());

    fx.delete_as(&as_grace, movie(fx.library, DUNE))
        .await
        .unwrap();

    log.wait_for(
        &[
            "deleted media",
            "Grace",
            "Dune",
            "Dune (2021).mkv",
            "Dune (2021).nfo",
            "bytes_freed",
            "13",
        ],
        1,
    )
    .await;
}

#[tokio::test]
async fn a_library_movie_or_version_that_isnt_there_is_refused_naming_its_field() {
    let fx = Fixture::alien_and_dune().await;

    let no_library = assert_refused(
        fx.preview(movie(fx.library + 1, 1)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    assert_eq!(no_library.field, "target.movie.library_id");
    assert_refused(
        fx.delete(movie(fx.library, 404)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
    let no_version = assert_refused(
        fx.preview(version(404)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    assert_eq!(no_version.field, "target.version_id");
    assert_refused(
        fx.delete(MediaToDelete::default()).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert!(fx.exists(DUNE_VIDEO) && fx.exists(ALIEN_1080P));
}
