use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use nama_proto::nama::v1::ProbingFinished;
use tempfile::TempDir;

use super::{
    Error, PROBES_AT_ONCE, ProbeError, Probing,
    ffprobe::{self, Ffprobe, Reader},
    testing::{FakeReader, failed, one_audio_track},
};
use crate::{
    config::Config,
    db::Database,
    ffmpeg::{self, Ffmpeg, RunError},
    library::{
        Change, Contents, Decided, Item, Libraries, LibraryKind as Kind, Stored,
        fixture::{decide, tmdb_movie},
    },
    playback::{
        subtitles,
        tracks::{BaseLayer, Chapter, Hdr, PROBE_VERSION, Probe, Track, TrackKind, Tracks},
    },
};

/// A database, a Movies Library backed by one scratch folder, and Probing reading files with
/// ffprobe or a stand-in.
/// Files are stored as a scan would, without scanning.
pub(super) struct Fixture {
    pub(super) probing: Probing,
    contents: Contents,
    pub(super) tracks: Tracks,
    library: i64,
    root: PathBuf,
    _media: TempDir,
    _data: TempDir,
}

impl Fixture {
    /// With a reader that can't read, as on a Server without ffprobe, so nothing is probed.
    async fn new() -> Self {
        Self::with_reader(FakeReader::unable()).await
    }

    /// Reading files with the ffprobe on `PATH`.
    pub(super) async fn with_system_ffmpeg() -> Self {
        Self::with_reader(Arc::new(Ffprobe(Arc::new(system_ffmpeg())))).await
    }

    /// Reading files with `reader`.
    pub(super) async fn with_reader(reader: Arc<impl Reader + 'static>) -> Self {
        let data = tempfile::tempdir().unwrap();
        let media = tempfile::tempdir().unwrap();
        let root = media.path().canonicalize().unwrap().join("library");
        fs::create_dir(&root).unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = Libraries::new(Arc::clone(&db))
            .create(
                "Media",
                Kind::Movies,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap();
        Self {
            probing: Probing::with_reader(
                Arc::clone(&db),
                reader,
                subtitles::Cache::new(data.path()),
                crate::images::ImageStore::new(data.path(), Arc::clone(&db)),
            ),
            tracks: Tracks::new(Arc::clone(&db)),
            contents: Contents::new(db),
            library: library.id,
            root,
            _media: media,
            _data: data,
        }
    }

    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Writes `size` bytes to the file at `relative` and stores it as a scan would.
    pub(super) async fn write(&self, relative: &str, size: usize) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![0_u8; size]).unwrap();
        self.store(relative).await;
    }

    /// Stores the file at `relative` as holding Alien, at its size and modified time now, as a
    /// scan would.
    pub(super) async fn store(&self, relative: &str) {
        let alien = Item::Movie {
            title: "Alien".to_owned(),
            year: Some(1979),
            hint: None,
        };
        self.store_holding(relative, vec![alien]).await;
    }

    /// Stores the file at `relative` as holding `items`, as [`Self::store`] does.
    async fn store_holding(&self, relative: &str, items: Vec<Item>) {
        let path = self.path(relative);
        let metadata = fs::metadata(&path).unwrap();
        let modified = metadata.modified().unwrap();
        let file = Change::Store {
            path: path.to_str().unwrap().to_owned(),
            size: i64::try_from(metadata.len()).unwrap(),
            modified_ns: i64::try_from(
                modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            )
            .unwrap(),
            items,
        };
        self.contents.apply(self.library, vec![file]).await.unwrap();
    }

    /// Tells Probing the Library's files changed, then waits for the pass it starts.
    pub(super) async fn probe_changed(&self) {
        self.probing.files_changed(self.library, |_| {});
        self.probing.idle().await;
    }

    /// The channels of the one audio Track stored for the file at `relative`.
    pub(super) async fn channels(&self, relative: &str) -> Option<i64> {
        let probe = self.probe(relative).await?;
        match probe.tracks.as_slice() {
            [
                Track {
                    kind: TrackKind::Audio { channels },
                    ..
                },
            ] => *channels,
            tracks => panic!("expected one audio Track: {tracks:?}"),
        }
    }

    /// The file stored at `relative`.
    pub(super) async fn stored(&self, relative: &str) -> Stored {
        let path = self.path(relative);
        self.contents.stored(self.library).await.unwrap()[path.to_str().unwrap()]
    }

    /// The id of the file stored at `relative`.
    pub(super) async fn file_id(&self, relative: &str) -> i64 {
        self.stored(relative).await.id
    }

    /// What probing found in the file at `relative`, if it read the file as it is stored now.
    pub(super) async fn probe(&self, relative: &str) -> Option<Probe> {
        let file = self.stored(relative).await;
        self.tracks.of(&[file]).await.unwrap().remove(&file.id)
    }

    /// Matches the Item of the file stored at `relative` to Alien, which makes it a Version.
    pub(super) async fn match_to_alien(&self, relative: &str) {
        let path = self.path(relative);
        let alien = vec![Decided::Matched(Box::new(tmdb_movie(348, "Alien")))];
        decide(&self.contents, self.library, path.to_str().unwrap(), alien).await;
    }

    /// Probes the Version stored at `relative` right away.
    pub(super) async fn current(&self, relative: &str) -> Result<Probe, Error> {
        self.probing.current(self.file_id(relative).await).await
    }
}

/// The ffmpeg and ffprobe on `PATH`, which tests that probe need.
pub(super) fn system_ffmpeg() -> Ffmpeg {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let ffmpeg = Ffmpeg::find(&Config::default(), &path);
    assert!(
        ffmpeg.direct_play_only_because().is_none(),
        "this test needs ffmpeg and ffprobe on PATH: {ffmpeg:?}"
    );
    ffmpeg
}

/// Writes a fifth of a second of 4K video with `transfer` and 5.1 audio at `relative`, and
/// stores it as a Version.
async fn write_4k_with_5_1(media: &Fixture, relative: &str, transfer: &str) {
    let path = media.path(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    ffmpeg::write_4k_with_5_1(&path, transfer).unwrap();
    media.store(relative).await;
    media.match_to_alien(relative).await;
}

#[tokio::test]
async fn a_4k_hdr10_file_with_5_1_audio_is_probed_and_its_tracks_stored() {
    let media = Fixture::with_system_ffmpeg().await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    write_4k_with_5_1(&media, alien, "smpte2084").await;

    let probe = media.current(alien).await.unwrap();

    assert_eq!(probe.container, "matroska,webm");
    // A fifth of a second, give or take a frame and the audio's padding.
    assert!(
        probe.duration_ms.is_some_and(|ms| (150..300).contains(&ms)),
        "{probe:?}"
    );
    assert!(probe.bitrate.is_some_and(|bps| bps > 0), "{probe:?}");
    assert_eq!(
        probe.tracks,
        [
            Track {
                codec: "ffv1".to_owned(),
                language: None,
                title: None,
                default: false,
                forced: false,
                sdh: false,
                kind: TrackKind::Video {
                    width: Some(3840),
                    height: Some(2160),
                    frame_rate: Some(24.0),
                    bit_depth: Some(10),
                    hdr: Some(Hdr::Hdr10),
                },
            },
            Track {
                codec: "ac3".to_owned(),
                language: Some("eng".to_owned()),
                title: None,
                default: false,
                forced: false,
                sdh: false,
                kind: TrackKind::Audio { channels: Some(6) },
            },
        ]
    );
    assert_eq!(media.probe(alien).await, Some(probe));
    assert_eq!(
        media
            .tracks
            .probe_version_of(media.file_id(alien).await)
            .await,
        Some(PROBE_VERSION)
    );
}

#[tokio::test]
async fn an_hlg_file_is_recorded_as_hlg_and_an_sdr_file_as_no_hdr() {
    let media = Fixture::with_system_ffmpeg().await;
    let hlg = "Alien (1979)/Alien (1979).mkv";
    let sdr = "Heat (1995)/Heat (1995).mkv";
    write_4k_with_5_1(&media, hlg, "arib-std-b67").await;
    write_4k_with_5_1(&media, sdr, "bt709").await;

    let hdr_of = async |file| {
        let tracks = media.current(file).await.unwrap().tracks;
        match tracks.first().map(|track| &track.kind) {
            Some(TrackKind::Video { hdr, .. }) => *hdr,
            _ => panic!("no video Track first: {tracks:?}"),
        }
    };
    assert_eq!(hdr_of(hlg).await, Some(Hdr::Hlg));
    assert_eq!(hdr_of(sdr).await, None);
}

#[tokio::test]
async fn without_ffprobe_a_pass_stores_no_tracks() {
    let media = Fixture::new().await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;

    media.probe_changed().await;

    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.probe_version_of(version).await, None);
}

#[tokio::test]
async fn without_ffprobe_probing_a_version_right_away_reports_that_probing_is_unavailable() {
    let media = Fixture::new().await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;

    let probed = media.current(alien).await;

    assert!(
        matches!(&probed, Err(Error::Probe(err)) if matches!(**err, ProbeError::Run(RunError::Unavailable(_)))),
        "{probed:?}"
    );
}

#[tokio::test]
async fn probing_a_file_holding_no_item_reports_no_such_version() {
    let media = Fixture::new().await;
    let home_video = "home video.mkv";
    fs::write(media.path(home_video), [0_u8; 10]).unwrap();
    media.store_holding(home_video, Vec::new()).await;

    let probed = media.current(home_video).await;

    assert!(matches!(probed, Err(Error::NoSuchVersion(_))), "{probed:?}");
}

#[tokio::test]
async fn a_dolby_vision_files_profile_and_whether_its_base_layer_is_hdr10_are_stored() {
    let dolby_vision = TrackKind::Video {
        width: None,
        height: None,
        frame_rate: None,
        bit_depth: Some(10),
        hdr: Some(Hdr::DolbyVision {
            profile: 8,
            base_layer: Some(BaseLayer::Hdr10),
        }),
    };
    let read = Probe {
        tracks: vec![Track {
            codec: "hevc".to_owned(),
            language: None,
            title: None,
            default: false,
            forced: false,
            sdh: false,
            kind: dolby_vision.clone(),
        }],
        ..one_audio_track(2)
    };
    let media = Fixture::with_reader(FakeReader::answering(move |_| Ok(read.clone()))).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;

    let probe = media.current(alien).await.unwrap();

    let kind = probe.tracks.first().map(|track| track.kind.clone());
    assert_eq!(kind, Some(dolby_vision));
    assert_eq!(media.probe(alien).await, Some(probe));
}

/// Chapters, one of them untitled, as a probe of a file reads them.
pub(super) fn chapters_read() -> Vec<Chapter> {
    let chapter = |start_ms, title: &str| Chapter {
        start_ms,
        title: title.to_owned(),
    };
    vec![
        chapter(0, "Opening"),
        chapter(600_250, ""),
        chapter(3_000_000, "Credits"),
    ]
}

#[tokio::test]
async fn a_files_chapters_are_stored_in_order_with_an_untitled_one_titled_empty() {
    let reader = FakeReader::answering(|_| {
        Ok(Probe {
            chapters: chapters_read(),
            ..one_audio_track(2)
        })
    });
    let media = Fixture::with_reader(reader).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;

    media.probe_changed().await;

    let stored = media.probe(alien).await.map(|probe| probe.chapters);
    assert_eq!(stored, Some(chapters_read()));
}

/// A reader that reads each file as one audio Track, holding each read until released.
fn held_reader() -> Arc<FakeReader> {
    let reader = FakeReader::answering(|_| Ok(one_audio_track(2)));
    reader.hold();
    reader
}

#[tokio::test]
async fn shutting_down_stops_the_probes() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10).await;
    media.probing.files_changed(media.library, |_| {});
    reader.until_started(1).await;

    let stopped = tokio::time::timeout(Duration::from_secs(5), media.probing.shutdown()).await;

    assert!(stopped.is_ok(), "shutting down waited for the read");
}

#[tokio::test]
async fn files_changed_during_a_pass_leaves_the_files_being_probed_to_it() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probing.files_changed(media.library, |_| {});
    reader.until_started(1).await;
    // The second pass reading Heat shows it found Alien stale, and it tried to claim Alien in the
    // same poll.
    media.write("Heat (1995)/Heat (1995).mkv", 10).await;

    media.probing.files_changed(media.library, |_| {});
    reader.until_started(2).await;
    reader.release();
    media.probing.idle().await;

    assert_eq!(
        reader.probes(),
        2,
        "Alien read by the first pass alone, and Heat"
    );
    assert!(media.probe(alien).await.is_some());
}

/// Tells Probing the Library's files changed, and returns what its pass reports once done.
fn files_changed_reporting(
    media: &Fixture,
) -> tokio::sync::mpsc::UnboundedReceiver<ProbingFinished> {
    let (told, reported) = tokio::sync::mpsc::unbounded_channel();
    media.probing.files_changed(media.library, move |probed| {
        drop(told.send(probed));
    });
    reported
}

#[tokio::test]
async fn a_pass_waits_for_the_file_another_prober_holds_and_counts_it_as_probed() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10).await;
    let mut first = files_changed_reporting(&media);
    reader.until_started(1).await;
    // The second pass reading Heat shows it found Alien stale, and it tried to claim Alien in the
    // same poll.
    media.write("Heat (1995)/Heat (1995).mkv", 10).await;

    let mut second = files_changed_reporting(&media);
    reader.until_started(2).await;
    reader.release();

    let probed = |probed| ProbingFinished {
        library_id: media.library,
        probed,
        ..Default::default()
    };
    assert_eq!(first.recv().await, Some(probed(1)));
    assert_eq!(second.recv().await, Some(probed(2)));
    assert_eq!(
        reader.probes(),
        2,
        "Alien read by the first pass alone, and Heat"
    );
}

#[tokio::test]
async fn a_pass_probes_a_file_itself_once_the_prober_it_waited_for_gives_up() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;
    let probing = media.probing.clone();
    let version = media.file_id(alien).await;
    let gives_up = tokio::spawn(async move { probing.current(version).await });
    reader.until_started(1).await;
    // The pass reading Heat shows it tried to claim Alien, as in
    // `files_changed_during_a_pass_leaves_the_files_being_probed_to_it`.
    media.write("Heat (1995)/Heat (1995).mkv", 10).await;
    let mut reported = files_changed_reporting(&media);
    reader.until_started(2).await;

    gives_up.abort();

    reader.until_started(3).await;
    reader.release();
    let caught_up = ProbingFinished {
        library_id: media.library,
        probed: 2,
        ..Default::default()
    };
    assert_eq!(reported.recv().await, Some(caught_up));
    assert_eq!(
        reader.probes(),
        3,
        "Alien read by `current`, then by the pass, and Heat"
    );
    assert!(media.probe(alien).await.is_some());
}

#[tokio::test]
async fn a_pass_counts_the_files_probed_and_those_ffprobe_couldnt_read() {
    let reader = FakeReader::answering(|path| {
        if path.contains("Heat") {
            Err(failed())
        } else {
            Ok(one_audio_track(2))
        }
    });
    let media = Fixture::with_reader(reader).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10).await;
    media.write("Heat (1995)/Heat (1995).mkv", 10).await;

    let mut first = files_changed_reporting(&media);

    let counted = ProbingFinished {
        library_id: media.library,
        probed: 1,
        failed: 1,
        ..Default::default()
    };
    assert_eq!(first.recv().await, Some(counted));

    // Nothing is left to probe: Alien's Tracks are current, and Heat failed for good.
    let mut second = files_changed_reporting(&media);

    let nothing = ProbingFinished {
        library_id: media.library,
        ..Default::default()
    };
    assert_eq!(second.recv().await, Some(nothing));
}

/// Probes the Version stored at `relative` right away while a background pass's held read of it
/// runs, then lets that read finish, and returns what probing it right away returned. `current`
/// usually reaches the background probe's claim before that probe stores what it read; if not,
/// it finds what was stored.
async fn current_while_the_background_probes_it(
    media: &Fixture,
    reader: &FakeReader,
    relative: &str,
) -> Result<Probe, Error> {
    media.write(relative, 10).await;
    media.match_to_alien(relative).await;
    let version = media.file_id(relative).await;
    media.probing.files_changed(media.library, |_| {});
    reader.until_started(1).await;
    let (probed, ()) = tokio::join!(media.probing.current(version), async { reader.release() });
    media.probing.idle().await;
    probed
}

#[tokio::test]
async fn probing_a_version_right_away_while_the_background_probes_it_runs_ffprobe_once() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;

    let probed =
        current_while_the_background_probes_it(&media, &reader, "Alien (1979)/Alien (1979).mkv")
            .await;

    assert_eq!(probed.unwrap(), one_audio_track(2));
    assert_eq!(reader.probes(), 1);
}

#[tokio::test]
async fn probing_a_version_right_away_while_the_background_probe_fails_returns_that_failure() {
    let reader = FakeReader::answering(|_| Err(failed()));
    reader.hold();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;

    let probed =
        current_while_the_background_probes_it(&media, &reader, "Alien (1979)/Alien (1979).mkv")
            .await;

    assert!(
        matches!(&probed, Err(Error::Probe(err)) if matches!(**err, ProbeError::Run(RunError::Failed(..)))),
        "{probed:?}"
    );
    // ponytail: doesn't count reads, as `current` may reach the file only once the background
    // probe stored its failure, and then rightly reads it again; a hook saying `current` waits
    // would let it.
}

#[tokio::test]
async fn probing_a_version_right_away_doesnt_wait_for_the_background_pass_to_reach_it() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    for n in 0..PROBES_AT_ONCE {
        media
            .write(&format!("Movie {n} (2000)/Movie {n} (2000).mkv"), 10)
            .await;
    }
    media.probing.files_changed(media.library, |_| {});
    reader.until_started(PROBES_AT_ONCE).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;
    // Queued behind the busy pass's reads.
    media.probing.files_changed(media.library, |_| {});

    let current = tokio::spawn({
        let probing = media.probing.clone();
        let version = media.file_id(alien).await;
        async move { probing.current(version).await }
    });

    reader.until_started(PROBES_AT_ONCE + 1).await;
    reader.release();
    current.await.unwrap().unwrap();
    media.probing.idle().await;
    assert_eq!(reader.probes(), PROBES_AT_ONCE + 1);
}

#[tokio::test]
async fn no_more_files_than_the_limit_are_probed_at_once() {
    let reader = held_reader();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    for n in 0..PROBES_AT_ONCE * 2 {
        media
            .write(&format!("Movie {n} (2000)/Movie {n} (2000).mkv"), 10)
            .await;
    }
    media.probing.files_changed(media.library, |_| {});
    reader.until_started(PROBES_AT_ONCE).await;

    reader.release();
    media.probing.idle().await;

    assert_eq!(reader.most_at_once(), PROBES_AT_ONCE);
    assert_eq!(reader.probes(), PROBES_AT_ONCE * 2);
}

#[tokio::test]
async fn an_mkv_written_to_a_pipe_probes_to_its_real_length() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("captured.mkv");
    // A source that just ends, unlike `-t`, leaves the muxer no length to write up front, and
    // on a pipe it can't seek back to write it at the end, as with a stream capture.
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=size=320x240:rate=24:duration=2"])
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-f",
            "matroska",
            "pipe:1",
        ])
        .stdout(fs::File::create(&file).unwrap())
        .status()
        .unwrap();
    assert!(status.success());

    let (probe, _) = ffprobe::probe(&ffmpeg, file.to_str().unwrap())
        .await
        .unwrap();

    // Within a frame, a 24th of a second, of 2 seconds.
    assert!(
        probe
            .duration_ms
            .is_some_and(|ms| (1958..=2042).contains(&ms)),
        "{probe:?}"
    );
}

#[tokio::test]
async fn an_avi_written_to_a_pipe_probes_to_its_real_length() {
    let ffmpeg = system_ffmpeg();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("piped.avi");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=duration=20:size=320x240:rate=24"])
        .args(["-f", "lavfi", "-i", "sine=duration=20"])
        // Not `ultrafast`, whose AVI skips a frame's time after the first packet.
        .args(["-c:v", "libx264", "-c:a", "aac", "-f", "avi", "pipe:1"])
        .stdout(fs::File::create(&file).unwrap())
        .status()
        .unwrap();
    assert!(status.success());

    let (probe, _) = ffprobe::probe(&ffmpeg, file.to_str().unwrap())
        .await
        .unwrap();

    // Within a frame, a 24th of a second, of 20 seconds.
    assert!(
        probe
            .duration_ms
            .is_some_and(|ms| (19_958..=20_042).contains(&ms)),
        "{probe:?}"
    );
}
