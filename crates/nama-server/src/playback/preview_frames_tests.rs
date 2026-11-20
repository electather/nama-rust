use std::{fs, sync::Arc};

use tempfile::TempDir;

use super::*;
use crate::{
    db::Database,
    events::EventBus,
    ffmpeg::{self, Ffmpeg},
    library::{
        Change, Item, Order,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::{SubtitleCache, Track, tracks::Probe},
    plugins::ExtensionPoints,
    server_settings::ServerUrl,
};

/// A Movies Library holding one Version, whose Preview frames an ffmpeg that always fails makes.
struct Fixture {
    frames: PreviewFrames,
    contents: Contents,
    libraries: Libraries,
    tracks: Tracks,
    movie: MatchedMovie,
    bin: TempDir,
    _data: TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let (data, bin) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let movie = MatchedMovie::new(&db, data.path(), tmdb_movie(348, "Alien")).await;
        // There, as ffmpeg failing on a file it can't open says nothing about the file.
        fs::write(data.path().canonicalize().unwrap().join("Alien.mkv"), "").unwrap();
        let runs = bin.path().join("runs");
        let failing = format!("#!/bin/sh\necho run >> '{}'\nexit 1\n", runs.display());
        let ffmpeg = Arc::new(Ffmpeg {
            ffmpeg: Ok(ffmpeg::fake::script(bin.path(), "ffmpeg", &failing)),
            ..Ffmpeg::default()
        });
        let events = EventBus::new(
            Arc::clone(&db),
            ExtensionPoints::default(),
            ServerUrl::default(),
        );
        let images = ImageStore::new(data.path(), Arc::clone(&db));
        let subtitles = SubtitleCache::new(data.path());
        let probing = Probing::new(
            Arc::clone(&db),
            Arc::clone(&ffmpeg),
            subtitles,
            images.clone(),
        );
        let running = RunningPlans::for_tests(&db, events, data.path());
        Self {
            frames: PreviewFrames::new(Arc::clone(&db), images, ffmpeg, running, probing),
            contents: Contents::new(Arc::clone(&db)),
            libraries: Libraries::new(Arc::clone(&db)),
            tracks: Tracks::new(db),
            movie,
            bin,
            _data: data,
        }
    }

    /// Stores that probing found a video Track in the Version's file as it's stored now.
    async fn probe_as_video(&self) {
        let (_, file) = self
            .contents
            .version_file(self.movie.version)
            .await
            .unwrap()
            .unwrap();
        let video = Track {
            codec: "h264".to_owned(),
            language: None,
            title: None,
            default: true,
            forced: false,
            sdh: false,
            kind: TrackKind::Video {
                width: Some(320),
                height: Some(240),
                frame_rate: None,
                bit_depth: None,
                hdr: None,
            },
        };
        let probe = Probe {
            container: "matroska,webm".to_owned(),
            duration_ms: Some(1000),
            bitrate: None,
            tracks: vec![video],
            font_attachments: Vec::new(),
        };
        self.tracks.store(file, probe).await.unwrap();
    }

    /// Stores the Version's file as `size` bytes, as a scan finding it changed would.
    async fn change_file(&self, size: i64) {
        let (path, file) = self
            .contents
            .version_file(self.movie.version)
            .await
            .unwrap()
            .unwrap();
        let changed = Change::Store {
            path,
            size,
            modified_ns: file.modified_ns,
            items: vec![Item::Movie {
                title: "Alien".to_owned(),
                year: None,
                hint: None,
            }],
        };
        self.contents
            .apply(self.movie.library, vec![changed])
            .await
            .unwrap();
    }

    async fn turn_off(&self) {
        let library = self.libraries.get(self.movie.library).await.unwrap();
        let off = Order {
            preview_frames: Some(false),
            ..Order::default()
        };
        (self.libraries)
            .update(library.id, &library.name, library.folders, off)
            .await
            .unwrap();
    }

    fn ffmpeg_runs(&self) -> usize {
        let runs = self.bin.path().join("runs");
        fs::read_to_string(runs).map_or(0, |runs| runs.lines().count())
    }
}

// The fake ffmpeg is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_version_whose_preview_frames_failed_is_tried_again_only_once_its_file_changes() {
    let fx = Fixture::new().await;
    let version = fx.movie.version;
    fx.probe_as_video().await;
    assert_eq!(fx.frames.needed().await.unwrap(), [version]);

    fx.frames.make(version).await.unwrap();
    fx.frames.make(version).await.unwrap();

    assert_eq!(fx.ffmpeg_runs(), 1, "made again for an unchanged file");
    assert_eq!(fx.frames.needed().await.unwrap(), Vec::<i64>::new());
    assert_eq!(fx.frames.of(version).await.unwrap(), None);

    fx.change_file(20).await;
    fx.probe_as_video().await;

    assert_eq!(fx.frames.needed().await.unwrap(), [version]);
    fx.frames.make(version).await.unwrap();
    assert_eq!(fx.ffmpeg_runs(), 2);
}

// The fake ffmpeg is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_version_in_a_library_with_preview_frames_off_is_left_alone() {
    let fx = Fixture::new().await;
    fx.probe_as_video().await;

    fx.turn_off().await;

    assert_eq!(fx.frames.needed().await.unwrap(), Vec::<i64>::new());
    fx.frames.make(fx.movie.version).await.unwrap();
    assert_eq!(fx.ffmpeg_runs(), 0);
}
