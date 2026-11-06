//! Managing Libraries through the generated `LibraryService` client, over a real socket.

#[path = "support/server.rs"]
mod server;

use std::path::{Path, PathBuf};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    CreateLibraryRequest, DeleteLibraryRequest, GetLibraryRequest, Library, LibraryKind,
    LibraryServiceClient, ListLibrariesRequest, ScanLibraryRequest, UpdateLibraryRequest,
};
use nama_server::config::Config;
use server::TestServer;
use tempfile::TempDir;

/// A Server, a client for its `LibraryService`, and a scratch directory for media folders.
struct Fixture {
    client: LibraryServiceClient<HttpClient>,
    media: TempDir,
    _server: TestServer,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let server = TestServer::start(Config::default()).await;
        let client = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
        Self {
            client,
            media: tempfile::tempdir().unwrap(),
            _server: server,
        }
    }

    /// Creates a directory named `name` in the scratch directory and returns its path as a
    /// client would send it.
    fn folder(&self, name: &str) -> String {
        let path = self.media.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        canonical(&path)
    }

    async fn create(
        &self,
        name: &str,
        kind: LibraryKind,
        folders: &[&str],
    ) -> Result<Library, ConnectError> {
        let response = self
            .client
            .create_library(CreateLibraryRequest {
                name: name.to_owned(),
                kind: kind.into(),
                folders: folders.iter().map(|f| (*f).to_owned()).collect(),
                ..Default::default()
            })
            .await?
            .into_owned();
        Ok(response.library.into_option().unwrap())
    }

    async fn get(&self, id: i64) -> Result<Library, ConnectError> {
        let response = self
            .client
            .get_library(GetLibraryRequest {
                id,
                ..Default::default()
            })
            .await?
            .into_owned();
        Ok(response.library.into_option().unwrap())
    }

    async fn update(&self, id: i64, name: &str, folders: &[&str]) -> Result<Library, ConnectError> {
        let response = self
            .client
            .update_library(UpdateLibraryRequest {
                id,
                name: name.to_owned(),
                folders: folders.iter().map(|f| (*f).to_owned()).collect(),
                ..Default::default()
            })
            .await?
            .into_owned();
        Ok(response.library.into_option().unwrap())
    }

    async fn delete(&self, id: i64) -> Result<(), ConnectError> {
        self.client
            .delete_library(DeleteLibraryRequest {
                id,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    async fn scan(&self, id: i64) -> Result<(), ConnectError> {
        self.client
            .scan_library(ScanLibraryRequest {
                id,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    async fn list(&self) -> Vec<Library> {
        self.client
            .list_libraries(ListLibrariesRequest::default())
            .await
            .unwrap()
            .into_owned()
            .libraries
    }
}

/// `path` with symlinks resolved, the form the Server stores folders in. (On macOS the temp
/// directory sits behind a symlink.)
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn canonical(path: &Path) -> String {
    PathBuf::from(path)
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn a_created_library_can_be_read_back_and_is_listed() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");
    let more_films = fx.folder("more-films");

    let created = fx
        .create(
            "Movies",
            LibraryKind::LIBRARY_KIND_MOVIES,
            &[&films, &more_films],
        )
        .await
        .unwrap();

    assert_eq!(created.name, "Movies");
    assert_eq!(created.kind, LibraryKind::LIBRARY_KIND_MOVIES);
    assert_eq!(created.folders, [films, more_films]);
    assert_eq!(fx.get(created.id).await.unwrap(), created);
    assert_eq!(fx.list().await, [created]);
}

#[tokio::test]
async fn updating_a_library_renames_it_and_replaces_its_folders_but_keeps_its_kind() {
    let fx = Fixture::start().await;
    let tv = fx.folder("tv");
    let more_tv = fx.folder("more-tv");
    let created = fx
        .create("TV", LibraryKind::LIBRARY_KIND_SHOWS, &[&tv])
        .await
        .unwrap();

    let updated = fx
        .update(created.id, "Television", &[&more_tv, &tv])
        .await
        .unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "Television");
    assert_eq!(updated.kind, LibraryKind::LIBRARY_KIND_SHOWS);
    assert_eq!(updated.folders, [more_tv, tv]);
    assert_eq!(fx.get(created.id).await.unwrap(), updated);
}

#[tokio::test]
async fn an_update_with_bad_input_is_refused_and_leaves_the_library_as_it_was() {
    let fx = Fixture::start().await;
    let tv = fx.folder("tv");
    let films = fx.folder("films");
    let shows = fx
        .create("TV", LibraryKind::LIBRARY_KIND_SHOWS, &[&tv])
        .await
        .unwrap();
    fx.create("Movies", LibraryKind::LIBRARY_KIND_MOVIES, &[&films])
        .await
        .unwrap();

    let cases: [(&str, &str, &[&str], ErrorCode); 3] = [
        ("a blank name", " ", &[&tv], ErrorCode::InvalidArgument),
        (
            "the same folder twice",
            "TV",
            &[&tv, &tv],
            ErrorCode::InvalidArgument,
        ),
        (
            "another Library's folder",
            "TV",
            &[&tv, &films],
            ErrorCode::AlreadyExists,
        ),
    ];
    for (case, name, folders, expected) in cases {
        assert_eq!(
            code(fx.update(shows.id, name, folders).await),
            expected,
            "{case}"
        );
    }
    assert_eq!(fx.get(shows.id).await.unwrap(), shows);
}

#[tokio::test]
async fn a_deleted_library_is_gone_and_its_folders_can_back_another() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");
    let first = fx
        .create("Movies", LibraryKind::LIBRARY_KIND_MOVIES, &[&films])
        .await
        .unwrap();

    fx.delete(first.id).await.unwrap();

    assert_eq!(code(fx.get(first.id).await), ErrorCode::NotFound);
    assert_eq!(fx.list().await, []);
    assert!(
        Path::new(&films).is_dir(),
        "deleting a Library touched its folder"
    );
    let second = fx
        .create("Films", LibraryKind::LIBRARY_KIND_MOVIES, &[&films])
        .await
        .unwrap();
    assert_ne!(second.id, first.id, "a deleted Library's id was reused");
}

#[tokio::test]
async fn reading_changing_or_deleting_a_library_that_does_not_exist_is_not_found() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");

    assert_eq!(code(fx.get(42).await), ErrorCode::NotFound);
    assert_eq!(
        code(fx.update(42, "Movies", &[&films]).await),
        ErrorCode::NotFound
    );
    assert_eq!(code(fx.delete(42).await), ErrorCode::NotFound);
}

/// The Connect code `result` failed with.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn code<T: std::fmt::Debug>(result: Result<T, ConnectError>) -> ErrorCode {
    result.unwrap_err().code
}

#[tokio::test]
async fn a_library_with_a_bad_name_kind_or_folder_is_refused_as_an_invalid_argument() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");
    let missing = fx.media.path().join("missing");
    let missing = missing.to_str().unwrap();
    let file = fx.media.path().join("film.mkv");
    std::fs::write(&file, b"").unwrap();
    let file = canonical(&file);
    let movies = LibraryKind::LIBRARY_KIND_MOVIES;

    let cases: [(&str, &str, LibraryKind, &[&str]); 7] = [
        ("blank name", "  ", movies, &[&films]),
        (
            "unspecified kind",
            "Movies",
            LibraryKind::LIBRARY_KIND_UNSPECIFIED,
            &[&films],
        ),
        ("no folders", "Movies", movies, &[]),
        ("relative folder", "Movies", movies, &["films"]),
        ("missing folder", "Movies", movies, &[missing]),
        ("a file, not a folder", "Movies", movies, &[&file]),
        ("the same folder twice", "Movies", movies, &[&films, &films]),
    ];
    for (case, name, kind, folders) in cases {
        let err = fx.create(name, kind, folders).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument, "{case}: {err:?}");
    }
    assert_eq!(fx.list().await, [], "a refused Library was created");

    let created = fx.create("Movies", movies, &[&films]).await.unwrap();
    assert_eq!(
        code(fx.update(created.id, "", &[&films]).await),
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        code(fx.update(created.id, "Movies", &[missing]).await),
        ErrorCode::InvalidArgument
    );
    assert_eq!(fx.get(created.id).await.unwrap(), created);
}

#[tokio::test]
async fn a_folder_that_already_backs_another_library_is_refused_as_already_existing() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");
    let tv = fx.folder("tv");
    let movies = fx
        .create("Movies", LibraryKind::LIBRARY_KIND_MOVIES, &[&films])
        .await
        .unwrap();
    let shows = fx
        .create("Shows", LibraryKind::LIBRARY_KIND_SHOWS, &[&tv])
        .await
        .unwrap();

    // The same folder, also when spelled differently, whatever the kind.
    let films_with_slash = format!("{films}{}", std::path::MAIN_SEPARATOR);
    for folder in [&films, &films_with_slash] {
        let err = fx
            .create("Anime", LibraryKind::LIBRARY_KIND_SHOWS, &[&tv, folder])
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::AlreadyExists, "{folder}: {err:?}");
    }
    assert_eq!(
        code(fx.update(shows.id, "Shows", &[&tv, &films]).await),
        ErrorCode::AlreadyExists
    );

    assert_eq!(
        fx.list().await,
        [movies, shows],
        "a refused change was kept"
    );
}

/// Directories the Server can't fully read: one it can't list, and one it can list but not
/// open anything in. Unix only: Windows has no simple equivalent of taking away these permissions.
#[cfg(unix)]
#[tokio::test]
async fn a_folder_the_server_cannot_read_is_refused_as_an_invalid_argument() {
    use std::os::unix::fs::PermissionsExt;

    let fx = Fixture::start().await;
    for (case, mode) in [("can't list", 0o300), ("can't open files in", 0o600)] {
        let locked = fx.folder(case);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(mode)).unwrap();
        if std::fs::metadata(Path::new(&locked).join(".")).is_ok()
            && std::fs::read_dir(&locked).is_ok()
        {
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
            // Running as root, which reads anything; there's nothing to test.
            return;
        }

        let err = fx
            .create("Movies", LibraryKind::LIBRARY_KIND_MOVIES, &[&locked])
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::InvalidArgument, "{case}: {err:?}");
        // So the scratch directory can be cleaned up.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}

/// A symlink is another name for the folder it points to. Unix only: creating one on Windows
/// needs a privilege CI doesn't have.
#[cfg(unix)]
#[tokio::test]
async fn a_symlink_to_a_folder_that_backs_a_library_is_refused_as_already_existing() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");
    fx.create("Movies", LibraryKind::LIBRARY_KIND_MOVIES, &[&films])
        .await
        .unwrap();
    let link = fx.media.path().join("films-link");
    std::os::unix::fs::symlink(&films, &link).unwrap();

    let err = fx
        .create(
            "Also Movies",
            LibraryKind::LIBRARY_KIND_MOVIES,
            &[link.to_str().unwrap()],
        )
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::AlreadyExists, "{err:?}");
}

#[tokio::test]
async fn a_scan_can_be_started_for_a_library_but_not_for_one_that_doesnt_exist() {
    let fx = Fixture::start().await;
    let films = fx.folder("films");
    let movies = fx
        .create("Movies", LibraryKind::LIBRARY_KIND_MOVIES, &[&films])
        .await
        .unwrap();

    fx.scan(movies.id).await.unwrap();
    // Asking again while that scan may still be running is fine too.
    fx.scan(movies.id).await.unwrap();

    let missing = fx.scan(movies.id + 1).await.unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound, "{missing:?}");
}
