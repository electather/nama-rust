use std::{fs, sync::Arc};

use tempfile::TempDir;

use super::Leftovers;
use crate::{
    db::Database,
    images::{DOWNLOADED_SUBTITLES_DIR, ImageStore, PREVIEW_FRAMES_DIR},
    library::{
        Contents, Stored,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::subtitles,
};

/// A Library holding one Version, and what cleans up after Versions gone, with no ffmpeg.
struct Fixture {
    leftovers: Leftovers,
    images: ImageStore,
    cache: subtitles::Cache,
    /// The stored file of the Library's one Version.
    kept: Stored,
    _data: TempDir,
    _media: TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(348, "Alien")).await;
        let contents = Contents::new(Arc::clone(&db));
        let (_, kept) = contents.version_file(alien.version).await.unwrap().unwrap();
        let images = ImageStore::new(data.path(), Arc::clone(&db));
        let cache = subtitles::Cache::new(data.path());
        Self {
            leftovers: Leftovers::new(db, cache.clone(), images.clone()),
            images,
            cache,
            kept,
            _data: data,
            _media: media,
        }
    }

    /// A Version no Library stores.
    fn gone(&self) -> Stored {
        Stored {
            id: self.kept.id + 1,
            ..self.kept
        }
    }

    /// Keeps a downloaded subtitle, a Preview-frame sheet and a cached subtitle of `file`.
    async fn make_all(&self, file: Stored) {
        let version = file.id.to_string();
        for dir in [DOWNLOADED_SUBTITLES_DIR, PREVIEW_FRAMES_DIR] {
            (self.images.write_on_disk(&[dir, &version, "made"], b"made"))
                .await
                .unwrap();
        }
        let cached = self.cache.folder(file);
        fs::create_dir_all(&cached).unwrap();
        fs::write(cached.join("1.vtt"), "WEBVTT").unwrap();
    }

    /// Whether `file`'s downloaded subtitle, Preview-frame sheet and cached subtitles are there.
    async fn has(&self, file: Stored) -> [bool; 3] {
        let version = file.id.to_string();
        let has = async |dir| {
            self.images
                .contains(&[dir, &version, "made"])
                .await
                .unwrap()
        };
        [
            has(DOWNLOADED_SUBTITLES_DIR).await,
            has(PREVIEW_FRAMES_DIR).await,
            self.cache.folder(file).exists(),
        ]
    }
}

#[tokio::test]
async fn cleaning_deletes_what_was_made_of_a_gone_version() {
    let media = Fixture::new().await;
    media.make_all(media.gone()).await;

    media.leftovers.clean().await;

    assert_eq!(media.has(media.gone()).await, [false; 3]);
}

#[tokio::test]
async fn cleaning_keeps_what_was_made_of_a_stored_version() {
    let media = Fixture::new().await;
    media.make_all(media.kept).await;
    media.make_all(media.gone()).await;

    media.leftovers.clean().await;

    assert_eq!(media.has(media.kept).await, [true; 3]);
}
