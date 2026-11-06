//! Fixing Matches by hand through the generated `MatchService` client, over a real socket, with
//! the TMDB Plugin enabled against a fake TMDB.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    ClearMatchRequest, CreateLibraryRequest, ExternalId, LibraryKind, LibraryServiceClient,
    MatchServiceClient, ScanLibraryRequest, SearchMetadataRequest, SetMatchRequest,
    SetMatchResponse, TitleKind, external_id,
};
use nama_server::{config::Config, contents::Contents, db::Database, plugins::Registry};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

/// TMDB finds nothing for this file, so it is Unmatched until Matched by hand.
const HOME_MOVIES: &str = "Home Movies (2019)/Home Movies (2019).mkv";

/// What a file holds when the test first writes it.
const VIDEO: &[u8] = b"video";

/// What Matching stored for a file's item, as the test reads it back.
#[derive(Debug, PartialEq, Eq)]
struct Decision {
    matched: bool,
    tmdb_id: Option<i64>,
}

/// A Server with one Library, the TMDB Plugin enabled against a fake TMDB, and clients for its
/// `LibraryService` and `MatchService`.
struct Fixture {
    library: i64,
    folder: PathBuf,
    libraries: LibraryServiceClient<HttpClient>,
    matches: MatchServiceClient<HttpClient>,
    contents: Contents,
    _tmdb: FakeTmdb,
    _stop: oneshot::Sender<()>,
    _data: TempDir,
    _media: TempDir,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Library of `kind`.
    async fn start(kind: LibraryKind) -> Self {
        let tmdb = FakeTmdb::start().await;
        let plugins = Registry::new();
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = ClientConfig::new(
            format!("http://{}", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
        );
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(nama_server::serve(
            listener,
            &Config::default(),
            Arc::clone(&db),
            plugins,
            async {
                let _: Result<(), oneshot::error::RecvError> = stopped.await;
            },
        ));
        let libraries = LibraryServiceClient::new(HttpClient::plaintext(), config.clone());
        let media = tempfile::tempdir().unwrap();
        let folder = media.path().canonicalize().unwrap();
        let library = libraries
            .create_library(CreateLibraryRequest {
                name: "Media".to_owned(),
                kind: kind.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap();
        Self {
            library: library.id,
            folder,
            libraries,
            matches: MatchServiceClient::new(HttpClient::plaintext(), config),
            contents: Contents::new(db),
            _tmdb: tmdb,
            _stop: stop,
            _data: data,
            _media: media,
        }
    }

    /// Writes `contents` to the file at `relative` in the Library's folder.
    fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.folder.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Scans the Library, and waits until the file at `relative` is stored with `contents`'s
    /// size and Matching has decided about its item.
    async fn scan_until_decided(&self, relative: &str, contents: &[u8]) -> (i64, Decision) {
        self.libraries
            .scan_library(ScanLibraryRequest {
                id: self.library,
                ..Default::default()
            })
            .await
            .unwrap();
        let path = self.folder.join(relative);
        let size = i64::try_from(contents.len()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.wait_for_decision(&path, size))
            .await
            .unwrap()
    }

    /// Waits until [`Self::decision`] has one.
    async fn wait_for_decision(&self, path: &Path, size: i64) -> (i64, Decision) {
        loop {
            if let Some(decided) = self.decision(path, size).await {
                return decided;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The item stored for the file at `path` with size `size`, and what Matching decided about
    /// it, once it has.
    async fn decision(&self, path: &Path, size: i64) -> Option<(i64, Decision)> {
        let path = path.to_str().unwrap();
        let file = self
            .contents
            .files(self.library)
            .await
            .unwrap()
            .into_iter()
            .find(|file| file.path == path && file.size == size)?;
        let item = file.items.into_iter().next()?;
        let recorded = item.recorded?;
        Some((
            item.id,
            Decision {
                matched: recorded.matched,
                tmdb_id: recorded.ids.tmdb,
            },
        ))
    }

    async fn set_match(&self, item: i64, tmdb: i64) -> Result<SetMatchResponse, ConnectError> {
        Ok(self
            .matches
            .set_match(SetMatchRequest {
                item_id: item,
                title_id: tmdb_id(tmdb).into(),
                ..Default::default()
            })
            .await?
            .into_owned())
    }
}

fn tmdb_id(id: i64) -> ExternalId {
    ExternalId {
        id: Some(external_id::Id::Tmdb(id)),
        ..Default::default()
    }
}

fn unmatched() -> Decision {
    Decision {
        matched: false,
        tmdb_id: None,
    }
}

fn matched(tmdb_id: i64) -> Decision {
    Decision {
        matched: true,
        tmdb_id: Some(tmdb_id),
    }
}

#[tokio::test]
async fn an_unmatched_file_matched_by_hand_stays_matched_when_it_changes_and_is_rescanned() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let (item, before) = fx.scan_until_decided(HOME_MOVIES, VIDEO).await;

    let response = fx.set_match(item, 949).await.unwrap();
    let upgraded = b"a better copy of the video";
    fx.write(HOME_MOVIES, upgraded);
    let (_, after) = fx.scan_until_decided(HOME_MOVIES, upgraded).await;

    assert_eq!(before, unmatched());
    assert_eq!((response.matched, response.unmatched), (1, 0));
    assert_eq!(after, matched(949));
}

#[tokio::test]
async fn a_cleared_match_is_unmatched() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write("Alien (1979)/Alien (1979).mkv", VIDEO);
    let (item, before) = fx
        .scan_until_decided("Alien (1979)/Alien (1979).mkv", VIDEO)
        .await;

    let response = fx
        .matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    assert_eq!(before, matched(348));
    assert_eq!(response.unmatched, 1);
    let path = fx.folder.join("Alien (1979)/Alien (1979).mkv");
    let size = i64::try_from(VIDEO.len()).unwrap();
    assert_eq!(fx.decision(&path, size).await.unwrap().1, unmatched());
}

#[tokio::test]
async fn search_metadata_passes_the_search_to_the_metadata_provider() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let response = fx
        .matches
        .search_metadata(SearchMetadataRequest {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            title: "Alien".to_owned(),
            year: Some(1979),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    let best = &response.candidates[0];
    assert_eq!(
        (best.id.id.clone(), best.title.as_str(), best.year),
        (Some(external_id::Id::Tmdb(348)), "Alien", Some(1979))
    );
}

#[tokio::test]
async fn search_metadata_without_a_kind_is_invalid() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let err = fx
        .matches
        .search_metadata(SearchMetadataRequest {
            title: "Alien".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgument, "{err:?}");
}

#[tokio::test]
async fn set_match_for_an_item_that_does_not_exist_is_not_found() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let err = fx.set_match(42, 949).await.unwrap_err();

    assert_eq!(err.code, ErrorCode::NotFound, "{err:?}");
}

#[tokio::test]
async fn set_match_to_a_title_tmdb_does_not_have_is_invalid() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let (item, _) = fx.scan_until_decided(HOME_MOVIES, VIDEO).await;

    let err = fx.set_match(item, 1).await.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgument, "{err:?}");
}

#[tokio::test]
async fn set_match_without_a_title_is_invalid() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let (item, _) = fx.scan_until_decided(HOME_MOVIES, VIDEO).await;

    let err = fx
        .matches
        .set_match(SetMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgument, "{err:?}");
}

#[tokio::test]
async fn re_matching_a_show_by_hand_re_matches_its_episodes() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
    let pilot = "The Office/Season 01/The Office - S01E01.mkv";
    let second = "The Office/Season 01/The Office - S01E02.mkv";
    fx.write(pilot, VIDEO);
    fx.write(second, VIDEO);
    // Both the US and the UK Office fit, so the scan leaves them Unmatched.
    let (item, before) = fx.scan_until_decided(pilot, VIDEO).await;
    let size = i64::try_from(VIDEO.len()).unwrap();
    let episodes = || async {
        let mut decisions = Vec::new();
        for relative in [pilot, second] {
            let (_, decision) = fx.decision(&fx.folder.join(relative), size).await.unwrap();
            decisions.push(decision);
        }
        decisions
    };

    let to_american = fx.set_match(item, 2316).await.unwrap();
    let us = episodes().await;
    let to_british = fx.set_match(item, 2996).await.unwrap();
    let uk = episodes().await;

    assert_eq!(before, unmatched());
    assert_eq!((to_american.matched, to_british.matched), (2, 2));
    assert_eq!(us, [matched(2_316_001), matched(2_316_002)]);
    assert_eq!(uk, [matched(2_996_001), matched(2_996_002)]);
}
