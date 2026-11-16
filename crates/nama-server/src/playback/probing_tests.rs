use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use nama_proto::nama::v1::ProbingFinished;
use tempfile::TempDir;

use super::{Error, PROBES_AT_ONCE, ProbeError, Probing, ffprobe};
use crate::{
    config::Config,
    db::Database,
    ffmpeg::{self, Ffmpeg},
    library::{
        Change, Contents, Decided, Item, Libraries, LibraryKind as Kind, Stored,
        fixture::{decide, tmdb_movie},
    },
    playback::{
        subtitles,
        tracks::{BaseLayer, Hdr, PROBE_VERSION, Probe, Track, TrackKind, Tracks},
    },
};

/// A database, a Movies Library backed by one scratch folder, and Probing with `ffmpeg`.
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
    /// With no ffprobe, so nothing is probed.
    async fn new() -> Self {
        Self::with_ffmpeg(Ffmpeg::default()).await
    }

    pub(super) async fn with_ffmpeg(ffmpeg: Ffmpeg) -> Self {
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
            probing: Probing::new(
                Arc::clone(&db),
                Arc::new(ffmpeg),
                subtitles::Cache::new(data.path()),
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
            items: vec![Item::Movie {
                title: "Alien".to_owned(),
                year: Some(1979),
                hint: None,
            }],
        };
        self.contents.apply(self.library, vec![file]).await.unwrap();
    }

    /// Tells Probing the Library's files changed, then waits for the pass it starts.
    pub(super) async fn probe_changed(&self) {
        self.probing.files_changed(self.library, |_| {});
        self.probing.idle().await;
    }

    /// The channels of the one audio Track stored for the file at `relative`.
    #[cfg(unix)]
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
    async fn probe(&self, relative: &str) -> Option<Probe> {
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
fn system_ffmpeg() -> Ffmpeg {
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
    let media = Fixture::with_ffmpeg(system_ffmpeg()).await;
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
    let media = Fixture::with_ffmpeg(system_ffmpeg()).await;
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
        matches!(&probed, Err(Error::Probe(err)) if matches!(**err, ProbeError::Unavailable)),
        "{probed:?}"
    );
}

#[tokio::test]
async fn probing_a_file_that_isnt_a_version_reports_no_such_version() {
    let media = Fixture::new().await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;

    let probed = media.current(alien).await;

    assert!(matches!(probed, Err(Error::NoSuchVersion(_))), "{probed:?}");
}

/// An ffprobe in `bin` that runs `script`, a shell script.
#[cfg(unix)]
pub(super) fn fake_ffprobe(bin: &std::path::Path, script: &str) -> Ffmpeg {
    Ffmpeg {
        ffprobe: Ok(ffmpeg::fake::script(
            bin,
            "ffprobe",
            &format!("#!/bin/sh\n{script}\n"),
        )),
        ..Ffmpeg::default()
    }
}

// ffmpeg can't write Dolby Vision, so a fake ffprobe prints what a real one does for it, cut
// down to what probing reads. The fake is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_dolby_vision_files_profile_and_whether_its_base_layer_is_hdr10_are_stored() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffprobe(
        bin.path(),
        r#"cat <<'EOF'
{"streams": [{"codec_name": "hevc", "codec_type": "video", "pix_fmt": "yuv420p10le",
  "color_transfer": "smpte2084",
  "side_data_list": [{"side_data_type": "DOVI configuration record", "dv_profile": 8,
                      "dv_bl_signal_compatibility_id": 1}]}],
 "format": {"format_name": "matroska,webm", "duration": "1.000000"}}
EOF"#,
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;

    let probe = media.current(alien).await.unwrap();

    let kind = probe.tracks.first().map(|track| track.kind.clone());
    assert_eq!(
        kind,
        Some(TrackKind::Video {
            width: None,
            height: None,
            frame_rate: None,
            bit_depth: Some(10),
            hdr: Some(Hdr::DolbyVision {
                profile: 8,
                base_layer: Some(BaseLayer::Hdr10),
            }),
        })
    );
    assert_eq!(media.probe(alien).await, Some(probe));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_whose_packets_cant_be_read_keeps_its_tracks_without_a_length() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffprobe(
        bin.path(),
        r#"case "$*" in *packet=*) echo 'Invalid data found when processing input' >&2; exit 1 ;; esac
echo '{"format": {"format_name": "matroska,webm"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'"#,
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;

    let probe = media.current(alien).await.unwrap();

    assert_eq!((probe.tracks.len(), probe.duration_ms), (1, None));
    assert_eq!(media.probe(alien).await, Some(probe));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn shutting_down_stops_the_probes() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(fake_ffprobe(bin.path(), "exec sleep 60")).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10).await;
    media.probing.files_changed(media.library, |_| {});

    let stopped = tokio::time::timeout(Duration::from_secs(5), media.probing.shutdown()).await;

    assert!(stopped.is_ok(), "shutting down waited for ffprobe");
}

/// An ffprobe in `bin` that adds a line to `runs` each time it runs, then takes a second to
/// find a second-long Matroska file with no Tracks.
#[cfg(unix)]
fn slow_ffprobe(bin: &std::path::Path, runs: &std::path::Path) -> Ffmpeg {
    fake_ffprobe(
        bin,
        &format!(
            "echo run >> '{}'\nsleep 1\necho '{{\"format\": {{\"format_name\": \"matroska,webm\", \"duration\": \"1.000000\"}}}}'",
            runs.display()
        ),
    )
}

/// Waits until a [`slow_ffprobe`] writing `runs` has started.
#[cfg(unix)]
pub(super) async fn wait_for_probe(runs: &std::path::Path) {
    let started = tokio::time::Instant::now();
    while !runs.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the background probe never started"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn files_changed_during_a_pass_leaves_the_files_being_probed_to_it() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;

    media.probing.files_changed(media.library, |_| {});
    wait_for_probe(&runs).await;
    media.probe_changed().await;

    assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
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

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_pass_waits_for_the_file_another_prober_holds_and_counts_it_as_probed() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    let mut first = files_changed_reporting(&media);
    wait_for_probe(&runs).await;

    let mut second = files_changed_reporting(&media);

    let caught_up = ProbingFinished {
        library_id: media.library,
        probed: 1,
        ..Default::default()
    };
    assert_eq!(first.recv().await, Some(caught_up.clone()));
    assert_eq!(second.recv().await, Some(caught_up));
    assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_pass_probes_a_file_itself_once_the_prober_it_waited_for_gives_up() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;
    // Holds the background pass back until `current` has claimed the file.
    let busy = media
        .probing
        .inner
        .running
        .acquire_many(u32::try_from(PROBES_AT_ONCE).unwrap())
        .await
        .unwrap();
    let mut reported = files_changed_reporting(&media);
    let probing = media.probing.clone();
    let version = media.file_id(alien).await;
    let gives_up = tokio::spawn(async move { probing.current(version).await });
    wait_for_probe(&runs).await;
    drop(busy);
    // ponytail: no hook says when the pass is waiting; this gives it time to find the claim.
    tokio::time::sleep(Duration::from_millis(100)).await;

    gives_up.abort();

    let caught_up = ProbingFinished {
        library_id: media.library,
        probed: 1,
        ..Default::default()
    };
    assert_eq!(reported.recv().await, Some(caught_up));
    assert_eq!(fs::read_to_string(&runs).unwrap(), "run\nrun\n");
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_pass_counts_the_files_probed_and_those_ffprobe_couldnt_read() {
    let bin = tempfile::tempdir().unwrap();
    // Reads a file of up to 10 bytes, failing on a bigger one.
    let ffmpeg = fake_ffprobe(
        bin.path(),
        r#"for file; do :; done
[ "$(wc -c < "$file")" -le 10 ] || exit 1
echo '{"format": {"format_name": "matroska,webm", "duration": "1.000000"}}'"#,
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10).await;
    media.write("Heat (1995)/Heat (1995).mkv", 20).await;

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

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn probing_a_version_right_away_while_the_background_probes_it_runs_ffprobe_once() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;
    media.probing.files_changed(media.library, |_| {});
    wait_for_probe(&runs).await;

    let probe = media.current(alien).await.unwrap();

    assert_eq!(probe.container, "matroska,webm");
    media.probing.idle().await;
    assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn probing_a_version_right_away_while_the_background_probe_fails_returns_that_failure() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let ffmpeg = fake_ffprobe(
        bin.path(),
        &format!("echo run >> '{}'\nsleep 1\nexit 1", runs.display()),
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;
    media.probing.files_changed(media.library, |_| {});
    wait_for_probe(&runs).await;

    let probed = media.current(alien).await;

    assert!(
        matches!(&probed, Err(Error::Probe(err)) if matches!(**err, ProbeError::Failed(_))),
        "{probed:?}"
    );
    media.probing.idle().await;
    assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn probing_a_version_right_away_doesnt_wait_for_the_background_pass_to_reach_it() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.match_to_alien(alien).await;
    // The background pass is busy with other files.
    let busy = (media.probing.inner.running)
        .acquire_many(u32::try_from(PROBES_AT_ONCE).unwrap())
        .await
        .unwrap();
    media.probing.files_changed(media.library, |_| {});

    let probed = tokio::time::timeout(Duration::from_secs(5), media.current(alien)).await;

    assert!(matches!(probed, Ok(Ok(_))), "{probed:?}");
    drop(busy);
    media.probing.idle().await;
    assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
}

/// A fake ffprobe in `bin` that logs each run to `bin/runs` and reads every file as one audio
/// Track with as many channels as the file has bytes, so a changed file reads differently.
#[cfg(unix)]
pub(super) fn counting_ffprobe(bin: &std::path::Path) -> Ffmpeg {
    let runs = bin.join("runs");
    fake_ffprobe(
        bin,
        &format!(
            r#"echo run >> '{}'
for file; do :; done
echo "{{\"format\": {{\"format_name\": \"matroska,webm\", \"duration\": \"1.000000\"}}, \"streams\": [{{\"codec_type\": \"audio\", \"codec_name\": \"ac3\", \"channels\": $(wc -c < "$file")}}]}}""#,
            runs.display()
        ),
    )
}

/// How many times [`counting_ffprobe`] in `bin` has run.
#[cfg(unix)]
pub(super) fn probe_runs(bin: &std::path::Path) -> usize {
    fs::read_to_string(bin.join("runs")).map_or(0, |runs| runs.lines().count())
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn no_more_files_than_the_limit_are_probed_at_once() {
    let bin = tempfile::tempdir().unwrap();
    let running = bin.path().join("running");
    let most = bin.path().join("most");
    fs::create_dir(&running).unwrap();
    // Counts the ffprobes running alongside it in `running`, keeping the most in `most`.
    let ffmpeg = fake_ffprobe(
        bin.path(),
        &format!(
            r#"me='{running}'/$$
touch "$me"
now=$(ls '{running}' | wc -l)
[ "$now" -gt "$(cat '{most}' 2>/dev/null || echo 0)" ] && echo "$now" > '{most}'
sleep 0.3
rm "$me"
echo '{{"format": {{"format_name": "matroska,webm", "duration": "1.000000"}}}}'"#,
            running = running.display(),
            most = most.display()
        ),
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    for n in 0..PROBES_AT_ONCE * 2 {
        media
            .write(&format!("Movie {n} (2000)/Movie {n} (2000).mkv"), 10)
            .await;
    }

    media.probe_changed().await;

    let most: usize = fs::read_to_string(&most).unwrap().trim().parse().unwrap();
    assert!((1..=PROBES_AT_ONCE).contains(&most), "{most} ran at once");
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

    let probe = ffprobe::probe(&ffmpeg, file.to_str().unwrap())
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

    let probe = ffprobe::probe(&ffmpeg, file.to_str().unwrap())
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
