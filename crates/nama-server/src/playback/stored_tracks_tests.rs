use tempfile::TempDir;

use super::*;
use crate::{
    library::{
        Contents,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::{Sidecar, Track, TrackKind, VersionSidecars},
};

/// A database with one Movies Library per title, each in its own folder holding `<title>.mkv`.
struct Media {
    db: Arc<Database>,
    _data: TempDir,
    folders: Vec<TempDir>,
    titles: Vec<String>,
    /// Each title's file, in the order of `titles`.
    files: Vec<Stored>,
}

impl Media {
    async fn new(titles: &[&str]) -> Self {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for (tmdb, title) in (1..).zip(titles) {
            let folder = tempfile::tempdir().unwrap();
            let movie = MatchedMovie::new(&db, folder.path(), tmdb_movie(tmdb, title)).await;
            let stored = Contents::new(Arc::clone(&db)).stored(movie.library).await;
            files.push(*stored.unwrap().values().next().unwrap());
            folders.push(folder);
        }
        Self {
            db,
            _data: data,
            folders,
            titles: titles.iter().map(|&title| title.to_owned()).collect(),
            files,
        }
    }

    /// Stores `languages` as the audio Tracks probing read from `file`.
    async fn probe(&self, file: Stored, languages: &[&str]) {
        let tracks = languages.iter().map(|language| audio(language)).collect();
        let probe = Probe {
            container: "matroska,webm".to_owned(),
            duration_ms: None,
            bitrate: None,
            tracks,
            font_attachments: vec![],
            chapters: vec![],
        };
        Tracks::new(Arc::clone(&self.db))
            .store(file, probe)
            .await
            .unwrap();
    }

    /// The path of the file `name` beside the `index`th title's.
    fn beside(&self, index: usize, name: &str) -> String {
        let folder = self.folders[index].path().canonicalize().unwrap();
        folder.join(name).to_str().unwrap().to_owned()
    }

    /// Stores `names` as the sidecars beside the `index`th title's file.
    async fn sidecars(&self, index: usize, names: &[&str]) {
        let sidecars = (names.iter())
            .map(|name| Sidecar {
                path: self.beside(index, name),
                size: 1,
                modified_ns: 0,
            })
            .collect();
        let beside = VersionSidecars {
            version: self.files[index].id,
            stem: self.titles[index].clone(),
            sidecars,
        };
        (Sidecars::new(Arc::clone(&self.db)).replace(vec![beside]))
            .await
            .unwrap();
    }

    /// What the one read gives for every title's file.
    async fn read(&self) -> HashMap<i64, Probe> {
        (StoredTracks::new(Arc::clone(&self.db)).of(&self.files))
            .await
            .unwrap()
    }
}

fn audio(language: &str) -> Track {
    Track {
        codec: "ac3".to_owned(),
        language: Some(language.to_owned()),
        title: None,
        default: false,
        forced: false,
        sdh: false,
        kind: TrackKind::Audio { channels: Some(2) },
    }
}

/// Each of `probe`'s Tracks as its kind and language.
fn listed(probe: &Probe) -> Vec<(&'static str, &str)> {
    (probe.tracks.iter())
        .map(|track| {
            let kind = match &track.kind {
                TrackKind::Audio { .. } => "audio",
                TrackKind::Subtitle {
                    sidecar: Some(_), ..
                } => "sidecar",
                TrackKind::Video { .. } | TrackKind::Subtitle { .. } => "other",
            };
            (kind, track.language.as_deref().unwrap_or_default())
        })
        .collect()
}

#[tokio::test]
async fn a_files_own_tracks_keep_their_places_before_its_sidecars() {
    let media = Media::new(&["Alien"]).await;
    media.probe(media.files[0], &["en", "ja"]).await;
    media.sidecars(0, &["Alien.de.srt"]).await;

    let read = media.read().await;

    assert_eq!(
        listed(&read[&media.files[0].id]),
        [("audio", "en"), ("audio", "ja"), ("sidecar", "de")]
    );
}

#[tokio::test]
async fn sidecars_follow_in_path_order() {
    let media = Media::new(&["Alien"]).await;
    media.probe(media.files[0], &["en"]).await;
    (media.sidecars(0, &["Alien.fr.srt", "Alien.de.srt", "Alien.en.srt"])).await;

    let read = media.read().await;

    let paths: Vec<_> = (read[&media.files[0].id].tracks.iter())
        .filter_map(|track| track.sidecar().map(|sidecar| sidecar.path.clone()))
        .collect();
    let in_path_order =
        ["Alien.de.srt", "Alien.en.srt", "Alien.fr.srt"].map(|name| media.beside(0, name));
    assert_eq!(paths, in_path_order);
}

#[tokio::test]
async fn a_version_without_sidecars_has_only_its_files_own_tracks() {
    let media = Media::new(&["Alien"]).await;
    media.probe(media.files[0], &["en"]).await;

    let read = media.read().await;

    assert_eq!(listed(&read[&media.files[0].id]), [("audio", "en")]);
}

#[tokio::test]
async fn a_version_probed_before_its_file_changed_has_no_tracks_not_even_its_sidecars() {
    let media = Media::new(&["Alien"]).await;
    let before_it_changed = Stored {
        modified_ns: media.files[0].modified_ns - 1,
        ..media.files[0]
    };
    media.probe(before_it_changed, &["en"]).await;
    media.sidecars(0, &["Alien.de.srt"]).await;

    let read = media.read().await;

    assert!(read.is_empty(), "{read:?}");
}

#[tokio::test]
async fn several_versions_are_read_at_once_each_with_its_own_sidecars() {
    let media = Media::new(&["Alien", "Aliens"]).await;
    media.probe(media.files[0], &["en"]).await;
    media.probe(media.files[1], &["ja"]).await;
    media.sidecars(0, &["Alien.de.srt"]).await;
    media.sidecars(1, &["Aliens.fr.srt"]).await;

    let read = media.read().await;

    assert_eq!(read.len(), 2);
    assert_eq!(
        listed(&read[&media.files[0].id]),
        [("audio", "en"), ("sidecar", "de")]
    );
    assert_eq!(
        listed(&read[&media.files[1].id]),
        [("audio", "ja"), ("sidecar", "fr")]
    );
}
