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
#[cfg(unix)]
use std::path::Path;

use browsing::{ALIEN_1080P, ALIEN_2160P, Fixture, tmdb_id};
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_radarr::{Changed, FakeRadarr};
use http::fetch;
use log::Log;
use nama_proto::nama::v1::{
    AudioCodec, BrowseServiceClient, Container, DeleteMediaRequest, DeleteMediaResponse,
    DeviceDecoding, Episode, EpisodeToDelete, EventType, GetMovieRequest, GetShowRequest,
    LibraryKind, LibraryServiceClient, ListEpisodesRequest, ListMoviesRequest, ListShowsRequest,
    MediaFile, MediaToDelete, MovieToDelete, Permission, PlanPlaybackRequest, PlaybackMethod,
    PlaybackPlan, PlaybackServiceClient, PreviewDeleteMediaRequest, PreviewDeleteMediaResponse,
    RefusalReason, SeasonToDelete, ShowToDelete, SubscribeRequest, UpdateLibraryRequest,
    VideoCodec, event, media_to_delete,
};
#[cfg(unix)]
use nama_server::ffmpeg::{Ffmpeg, fake::script};
use refused::assert_refused;
use serde_json::json;
use server::{
    Builder, TestServer,
    fulfiller::MOVIE_FULFILLER,
    subscribe, subscribe_with,
    tmdb::{ALIEN, DARK, DUNE},
};

const ALIEN_1080P_SUBTITLES: &str = "Alien (1979)/Alien (1979) - 1080p.en.srt";
const ALIEN_2160P_POSTER: &str = "Alien (1979)/Alien (1979) - 2160p-poster.jpg";
/// Beside Alien's Versions, but named after neither.
const ALIEN_FOLDER_ART: &str = "Alien (1979)/folder.jpg";
/// Straight in the Library's folder, with nothing else of its own.
const DUNE_VIDEO: &str = "Dune (2021).mkv";
const DUNE_NFO: &str = "Dune (2021).nfo";
/// One of the fake Radarr's quality profiles, which a delete never reads.
const ANY_PROFILE: &str = "1";

/// Dark in two folders. The first file holds Secrets and Lies, Season 1's first two Episodes, and
/// the second is Secrets again.
const DARK_S01E01_E02: &str = "Dark (2017)/Season 01/Dark - S01E01-E02.mkv";
const DARK_S01E01_E02_SUBTITLES: &str = "Dark (2017)/Season 01/Dark - S01E01-E02.en.srt";
const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
/// The TMDB ids of Dark's Episodes S01E01 and S01E02.
const SECRETS: i64 = 1_388_538;
const LIES: i64 = 1_388_539;

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

/// Every Version of every Episode of the Show with the TMDB id `tmdb` in `library`.
fn show(library: i64, tmdb: i64) -> MediaToDelete {
    MediaToDelete {
        target: Some(media_to_delete::Target::Show(Box::new(ShowToDelete {
            library_id: library,
            show_id: tmdb_id(tmdb).into(),
            ..Default::default()
        }))),
        ..Default::default()
    }
}

/// Every Version of every Episode of Dark's Season `number` in `library`.
fn dark_season(library: i64, number: u32) -> MediaToDelete {
    MediaToDelete {
        target: Some(media_to_delete::Target::Season(Box::new(SeasonToDelete {
            library_id: library,
            show_id: tmdb_id(DARK).into(),
            season_number: number,
            ..Default::default()
        }))),
        ..Default::default()
    }
}

/// Every Version of the Episode with the TMDB id `tmdb` in `library`, or only `version`.
fn episode(library: i64, tmdb: i64, version: Option<i64>) -> MediaToDelete {
    MediaToDelete {
        target: Some(media_to_delete::Target::Episode(Box::new(
            EpisodeToDelete {
                library_id: library,
                episode_id: tmdb_id(tmdb).into(),
                version_id: version,
                ..Default::default()
            },
        ))),
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

    /// A Shows Library holding Dark's three files and a sidecar, scanned and Matched.
    async fn dark() -> Self {
        let fx = Self::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
        fx.write(DARK_S01E01_E02, b"two episodes");
        fx.write(DARK_S01E01_E02_SUBTITLES, b"subtitles");
        fx.write(DARK_S01E01, b"one episode");
        fx.write(DARK_S02E01, b"video");
        fx.scan_and_match().await;
        fx
    }

    /// Dark's Season `season`, as `ListEpisodes` lists it.
    async fn list_dark(&self, season: u32) -> Result<Vec<Episode>, ConnectError> {
        let request = ListEpisodesRequest {
            library_id: self.library,
            show_id: tmdb_id(DARK).into(),
            season_number: season,
            ..Default::default()
        };
        Ok(self
            .browse
            .list_episodes(request)
            .await?
            .into_owned()
            .episodes)
    }

    /// Dark's Season `season`: each Episode's title and its Versions' file names. Empty once the
    /// Library has no such Season.
    async fn dark_episodes(&self, season: u32) -> Vec<(String, Vec<String>)> {
        let episodes = match self.list_dark(season).await {
            Err(err) if err.code == ErrorCode::NotFound => Vec::new(),
            listed => listed.unwrap(),
        };
        (episodes.into_iter())
            .map(|episode| {
                let files = episode.versions.into_iter().map(|v| v.file_name);
                (episode.title, files.collect())
            })
            .collect()
    }

    /// The id of the Version of Dark's Season 1 with the file name `file_name`.
    async fn dark_version(&self, file_name: &str) -> i64 {
        let episodes = self.list_dark(1).await.unwrap();
        let mut versions = episodes.into_iter().flat_map(|e| e.versions);
        versions.find(|v| v.file_name == file_name).unwrap().id
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

    /// The Admin's Playback Plan for the Version `id`, on a browser that plays H.264 and AAC in
    /// MP4.
    async fn plan(&self, id: i64) -> PlaybackPlan {
        let device = DeviceDecoding {
            containers: vec![Container::CONTAINER_MP4.into()],
            video_codecs: vec![VideoCodec::VIDEO_CODEC_H264.into()],
            audio_codecs: vec![AudioCodec::AUDIO_CODEC_AAC.into()],
            mse: true,
            ..Default::default()
        };
        let playback =
            PlaybackServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let planned = playback
            .plan_playback(PlanPlaybackRequest {
                version_id: id,
                device_decoding: device.into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        planned.plan.into_option().unwrap()
    }

    fn exists(&self, relative: &str) -> bool {
        self.folder.join(relative).exists()
    }

    /// The folder `GetMovie` says refuses deleting the Movie with the TMDB id `tmdb`, as `browse`
    /// is told.
    async fn read_only_folder(
        &self,
        browse: &BrowseServiceClient<HttpClient>,
        tmdb: i64,
    ) -> Option<String> {
        let response = browse
            .get_movie(GetMovieRequest {
                library_id: self.library,
                movie_id: tmdb_id(tmdb).into(),
                ..Default::default()
            })
            .await
            .unwrap();
        response.into_owned().read_only_folder
    }

    /// Sets the mode of the Library's folder to `mode`, then scans it.
    #[cfg(unix)]
    async fn rescan_with_mode(&self, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&self.folder, fs::Permissions::from_mode(mode)).unwrap();
        self.scan_and_match().await;
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
    radarr.enable_on(&fx.server, ANY_PROFILE).await;
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
    radarr.enable_on(&fx.server, ANY_PROFILE).await;

    let deleted = fx.delete(movie(fx.library, ALIEN)).await.unwrap();

    assert_eq!(radarr.changes(), []);
    assert_eq!(deleted.fulfillers_not_told, [""; 0]);
    assert!(!fx.exists(ALIEN_1080P));
}

#[tokio::test]
async fn with_radarr_down_the_delete_goes_ahead_and_names_radarr() {
    let fx = Fixture::alien_and_dune().await;
    let mut radarr = FakeRadarr::start(&[ALIEN], &[]).await;
    radarr.enable_on(&fx.server, ANY_PROFILE).await;
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

// Root writes to any folder whatever its mode, so these need a run as someone else, as CI's is.
#[cfg(unix)]
#[tokio::test]
async fn a_folder_a_scan_found_read_only_refuses_the_delete_naming_it_until_a_scan_finds_it_writable()
 {
    let fx = Fixture::alien_and_dune().await;
    let folder = fx.folder.to_str().unwrap().to_owned();
    fx.rescan_with_mode(0o555).await;

    assert_eq!(
        fx.read_only_folder(&fx.browse, ALIEN).await.as_ref(),
        Some(&folder)
    );
    let preview = fx.preview(movie(fx.library, ALIEN)).await.unwrap();
    assert_eq!(preview.read_only_folder.as_ref(), Some(&folder));
    let refusal = assert_refused(
        fx.delete(movie(fx.library, ALIEN)).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_FOLDER_READ_ONLY,
    );
    assert_eq!(refusal.params.get("folder"), Some(&folder));
    assert!(fx.exists(ALIEN_1080P) && fx.exists(ALIEN_1080P_SUBTITLES));
    assert_eq!(fx.titles().await, ["Alien", "Dune"]);

    fx.rescan_with_mode(0o755).await;

    assert_eq!(fx.read_only_folder(&fx.browse, ALIEN).await, None);
    fx.delete(movie(fx.library, ALIEN)).await.unwrap();
    assert!(!fx.exists(ALIEN_1080P));
}

#[cfg(unix)]
#[tokio::test]
async fn only_someone_who_may_delete_media_is_told_which_folder_refuses_it() {
    let fx = Fixture::alien_and_dune().await;
    let grace = fx.server.user("Grace", &[]).await;
    let as_grace = BrowseServiceClient::new(HttpClient::plaintext(), grace.client_config());

    fx.rescan_with_mode(0o555).await;
    let told = fx.read_only_folder(&as_grace, ALIEN).await;
    // Writable again before asserting, so the temporary folder is cleaned up even if it fails.
    fx.rescan_with_mode(0o755).await;

    assert_eq!(told, None);
}

#[cfg(unix)]
#[tokio::test]
async fn renaming_a_library_keeps_its_folder_read_only_until_a_scan_finds_otherwise() {
    let fx = Fixture::alien_and_dune().await;
    let folder = fx.folder.to_str().unwrap().to_owned();
    fx.rescan_with_mode(0o555).await;

    fx.library_service()
        .update_library(UpdateLibraryRequest {
            id: fx.library,
            name: "Renamed".to_owned(),
            folders: vec![folder.clone()],
            ..Default::default()
        })
        .await
        .unwrap();
    let told = fx.read_only_folder(&fx.browse, ALIEN).await;
    // Writable again before asserting, so the temporary folder is cleaned up even if it fails.
    fx.rescan_with_mode(0o755).await;

    assert_eq!(told, Some(folder));
}

/// The Episodes `preview` takes along, by season number, episode number and title.
fn taken_along(preview: &PreviewDeleteMediaResponse) -> Vec<(u32, u32, &str)> {
    (preview.episodes_taken_along.iter())
        .map(|e| (e.season_number, e.episode_number, e.title.as_str()))
        .collect()
}

/// `(title, files)` of an Episode, as [`Fixture::dark_episodes`] lists it.
fn listed(title: &str, files: &[&str]) -> (String, Vec<String>) {
    let files = files.iter().map(|&file| file.to_owned()).collect();
    (title.to_owned(), files)
}

#[tokio::test]
async fn deleting_an_episode_deletes_every_version_and_the_episodes_only_its_files_hold() {
    let fx = Fixture::dark().await;

    let preview = fx
        .preview(episode(fx.library, SECRETS, None))
        .await
        .unwrap();
    let deleted = fx.delete(episode(fx.library, SECRETS, None)).await.unwrap();

    assert_eq!(taken_along(&preview), [(1, 2, "Lies")]);
    assert_eq!(
        by_name(&deleted.deleted),
        [
            ("Dark - S01E01-E02.en.srt", 9),
            ("Dark - S01E01-E02.mkv", 12),
            ("Dark - S01E01.mkv", 11)
        ]
    );
    assert!(!fx.exists(DARK_S01E01_E02) && !fx.exists(DARK_S01E01));
    assert_eq!(fx.dark_episodes(1).await, []);
    assert_eq!(
        fx.dark_episodes(2).await,
        [listed("Beginnings and Endings", &["Dark - S02E01.mkv"])]
    );
}

#[tokio::test]
async fn deleting_an_episode_in_a_two_episode_file_deletes_the_file_and_the_preview_names_the_other()
 {
    let fx = Fixture::dark().await;

    let preview = fx.preview(episode(fx.library, LIES, None)).await.unwrap();
    fx.delete(episode(fx.library, LIES, None)).await.unwrap();

    assert_eq!(taken_along(&preview), [(1, 1, "Secrets")]);
    assert_eq!(
        by_name(&preview.files),
        [
            ("Dark - S01E01-E02.en.srt", 9),
            ("Dark - S01E01-E02.mkv", 12)
        ]
    );
    assert!(!fx.exists(DARK_S01E01_E02));
    // Secrets keeps the Version it has of its own.
    assert_eq!(
        fx.dark_episodes(1).await,
        [listed("Secrets", &["Dark - S01E01.mkv"])]
    );
}

#[tokio::test]
async fn deleting_one_version_of_an_episode_with_two_leaves_the_other() {
    let fx = Fixture::dark().await;
    let own = fx.dark_version("Dark - S01E01.mkv").await;

    let preview = fx
        .preview(episode(fx.library, SECRETS, Some(own)))
        .await
        .unwrap();
    let deleted = fx
        .delete(episode(fx.library, SECRETS, Some(own)))
        .await
        .unwrap();

    assert_eq!(taken_along(&preview), []);
    assert_eq!(by_name(&deleted.deleted), [("Dark - S01E01.mkv", 11)]);
    assert_eq!(
        fx.dark_episodes(1).await,
        [
            listed("Secrets", &["Dark - S01E01-E02.mkv"]),
            listed("Lies", &["Dark - S01E01-E02.mkv"])
        ]
    );
}

#[tokio::test]
async fn deleting_a_season_deletes_each_of_its_files_once_and_leaves_other_seasons() {
    let fx = Fixture::dark().await;

    let preview = fx.preview(dark_season(fx.library, 1)).await.unwrap();
    fx.delete(dark_season(fx.library, 1)).await.unwrap();

    assert_eq!(preview.bytes_freed, 9 + 12 + 11);
    assert_eq!(taken_along(&preview), []);
    assert!(!fx.exists("Dark (2017)") && !fx.exists("Dark {tmdb-70523}/Season 01"));
    let seasons = (fx.browse)
        .get_show(GetShowRequest {
            library_id: fx.library,
            show_id: tmdb_id(DARK).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .seasons;
    assert_eq!(seasons.iter().map(|s| s.number).collect::<Vec<_>>(), [2]);
}

#[tokio::test]
async fn deleting_a_show_deletes_every_season_and_a_subscriber_is_told_of_each_item() {
    let fx = Fixture::dark().await;
    let about = SubscribeRequest {
        library_ids: vec![fx.library],
        ..Default::default()
    };
    let mut events = subscribe_with(&fx.server.client_config(), about).await;

    let deleted = fx.delete(show(fx.library, DARK)).await.unwrap();

    assert_eq!(by_name(&deleted.deleted).len(), 4, "{deleted:?}");
    // The two-Episode file is two Items.
    for _ in 0..4 {
        let event::Event::LibraryItemRemoved(removed) = events.next().await else {
            panic!("expected an Item removed");
        };
        assert_eq!(removed.library_id, fx.library);
    }
    let shows = (fx.browse)
        .list_shows(ListShowsRequest {
            library_id: fx.library,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .shows;
    assert!(shows.is_empty(), "{shows:?}");
    assert_eq!(fs::read_dir(&fx.folder).unwrap().count(), 0);
}

#[tokio::test]
async fn a_season_or_episode_version_that_isnt_there_or_a_show_in_a_movies_library_is_refused() {
    let fx = Fixture::dark().await;
    let movies = fx
        .server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    let other = fx.dark_version("Dark - S01E01.mkv").await;

    assert_refused(
        fx.preview(dark_season(fx.library, 3)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SEASON_NOT_FOUND,
    );
    let not_lies = assert_refused(
        fx.delete(episode(fx.library, LIES, Some(other))).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    assert_eq!(not_lies.field, "target.episode.version_id");
    let wrong_kind = assert_refused(
        fx.delete(show(movies.id, DARK)).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_eq!(wrong_kind.field, "target.show.library_id");
    assert!(fx.exists(DARK_S01E01) && fx.exists(DARK_S01E01_E02));
}

/// A Server's ffmpeg and ffprobe, shell scripts in `bin`: ffprobe reads every file as an HEVC and
/// AAC MKV, which a browser needs Transcoded, and ffmpeg lists libx264 but writes nothing until
/// it's stopped, keeping its process id in `bin/ffmpeg.pid` while it runs.
#[cfg(unix)]
fn ffmpeg_transcoding_forever(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
cat <<EOF
{"streams": [{"codec_name": "hevc", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "matroska,webm", "duration": "1.000000"}}
EOF
"#;
    let ffmpeg = "#!/bin/sh\ncase \"$*\" in *-encoders*) echo ' V....D libx264  H.264' ;; \
                  *-filters*) ;; *) echo $$ > \"$0.pid\"; exec sleep 600 ;; esac\n";
    Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        gpus: vec![],
    }
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn deleting_a_version_being_transcoded_ends_its_plan_which_the_preview_counted() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffmpeg_transcoding_forever(bin.path());
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_2160P, b"video");
    fx.scan_and_match().await;
    let alien = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan(alien).await;
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    let ended = [EventType::EVENT_TYPE_PLAYBACK_ENDED];
    let mut subscription = subscribe(&fx.server.client_config(), &ended).await;

    let preview = fx.preview(version(alien)).await.unwrap();
    fx.delete(version(alien)).await.unwrap();

    assert_eq!(preview.playing, 1);
    let event::Event::PlaybackEnded(told) = subscription.next().await else {
        panic!("expected its playback ended");
    };
    assert_eq!(told.playback.version_id, alien);
    let token = Some(fx.server.admin_token.as_str());
    assert_eq!(fetch(&fx.server, &plan.url, token).await.status, 404);
    let pid = fs::read_to_string(bin.path().join("ffmpeg.pid")).unwrap();
    let pid = pid.trim();
    assert!(stops(pid).await, "ffmpeg {pid} still runs");
}

/// Whether the process `pid` ends, reaped or not, within 10 seconds.
#[cfg(unix)]
#[expect(clippy::unwrap_used, reason = "ps failing to run fails the test")]
async fn stops(pid: &str) -> bool {
    for _ in 0..100 {
        let ps = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", pid])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&ps.stdout);
        if state.trim().is_empty() || state.trim_start().starts_with('Z') {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}

/// Bigger than the sockets between a Server and its client buffer, so serving it whole waits on
/// the client reading it.
const BIGGER_THAN_SOCKET_BUFFERS: usize = 64 << 20;

#[tokio::test]
async fn a_direct_play_of_a_deleted_version_gets_no_more_of_its_file() {
    use http_body_util::{BodyExt, Empty};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let fx = Fixture::alien_and_dune().await;
    fx.write(DUNE_VIDEO, &vec![0; BIGGER_THAN_SOCKET_BUFFERS]);
    fx.scan_and_match().await;
    let dune = fx.version_ids(DUNE).await[0];
    let plan = fx.plan(dune).await;
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    let stream = tokio::net::TcpStream::connect(fx.server.addr)
        .await
        .unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let request = Request::get(&plan.url)
        .header("host", fx.server.addr.to_string())
        .header("cookie", format!("nama_session={}", fx.server.admin_token))
        .body(Empty::<Bytes>::new())
        .unwrap();
    let mut body = sender.send_request(request).await.unwrap().into_body();
    let mut received = 0;
    if let Some(Ok(frame)) = body.frame().await {
        received += frame.data_ref().map_or(0, Bytes::len);
    }

    fx.delete(movie(fx.library, DUNE)).await.unwrap();

    while let Some(Ok(frame)) = body.frame().await {
        received += frame.data_ref().map_or(0, Bytes::len);
    }
    assert!(received < BIGGER_THAN_SOCKET_BUFFERS, "{received} bytes");
    let token = Some(fx.server.admin_token.as_str());
    assert_eq!(fetch(&fx.server, &plan.url, token).await.status, 404);
}
