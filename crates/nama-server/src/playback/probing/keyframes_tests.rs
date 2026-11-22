use std::{fs, path::Path, process::Command, sync::Arc, time::Duration};

use super::tests::{Fixture, system_ffmpeg};
use super::{
    Error, Keyframes, LISTING_WAIT, ListedKeyframes, Probe, ProbeError, ffprobe, plausible,
    testing::{FakeReader, failed},
};
use crate::{
    ffmpeg::{self, Program, RunError},
    playback::made_from_files::WHOLE_FILE_READS_AT_ONCE,
};

/// The file at `path` listing keyframes at 0 and at as many seconds as it has bytes, so a changed
/// file lists differently.
fn listed_by_size(path: &str) -> ListedKeyframes {
    let bytes = i64::try_from(fs::metadata(path).unwrap().len()).unwrap();
    ListedKeyframes {
        file_start_ms: 0,
        keyframes_ms: vec![0, bytes * 1000],
    }
}

/// A reader that probes each file by its size and answers each listing with `listing`.
fn reader_listing(
    listing: impl Fn(&str) -> Result<ListedKeyframes, ProbeError> + Send + Sync + 'static,
) -> Arc<FakeReader> {
    let reader = FakeReader::by_size();
    reader.list(listing);
    reader
}

/// A reader that probes and lists each file by its size.
fn reader_by_size() -> Arc<FakeReader> {
    reader_listing(|path| Ok(listed_by_size(path)))
}

/// Whether `result` failed with a [`ProbeError`] `reason` matches.
fn failed_with<T>(result: &Result<T, Error>, reason: impl Fn(&ProbeError) -> bool) -> bool {
    matches!(result, Err(Error::Probe(err)) if reason(err))
}

/// Whether `err` says the keyframes aren't listed yet.
fn not_listed_yet(err: &ProbeError) -> bool {
    matches!(err, ProbeError::NotListedYet)
}

impl Fixture {
    /// Writes `size` bytes to the file at `relative`, stores it as a Version, and probes it.
    async fn write_probed(&self, relative: &str, size: usize) {
        self.write(relative, size).await;
        self.match_to_alien(relative).await;
        self.current(relative).await.unwrap();
    }

    /// The keyframes of the Version stored at `relative`.
    async fn keyframes(&self, relative: &str) -> Vec<i64> {
        let version = self.file_id(relative).await;
        self.probing.keyframes(version).await.unwrap().keyframes_ms
    }

    /// Lists the keyframes of the Version `version` as a plan would, in the background.
    fn plan(&self, version: i64) -> tokio::task::JoinHandle<Result<ListedKeyframes, Error>> {
        let probing = self.probing.clone();
        tokio::spawn(async move { probing.keyframes(version).await })
    }
}

#[tokio::test]
async fn a_versions_keyframes_are_listed_once_and_kept() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;

    assert_eq!(media.keyframes(alien).await, [0, 10_000]);
    assert_eq!(media.keyframes(alien).await, [0, 10_000]);

    assert_eq!(reader.listings(), 1);
}

#[tokio::test]
async fn a_versions_keyframes_asked_for_at_once_are_listed_once() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    reader.hold();
    let first = media.plan(version);
    reader.until_listed(1).await;

    // Usually reaches the first listing's claim before it stores what it listed; if not, it finds
    // what was stored.
    let (second, ()) = tokio::join!(media.probing.keyframes(version), async {
        reader.release();
    });

    assert_eq!(first.await.unwrap().unwrap(), second.unwrap());
    assert_eq!(reader.listings(), 1);
}

#[tokio::test]
async fn a_versions_keyframes_are_listed_again_once_its_files_size_changes() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    media.keyframes(alien).await;

    // Stored as a scan would, but not probed again, so its Tracks were read from the old file.
    media.write(alien, 20).await;
    media.match_to_alien(alien).await;

    assert_eq!(media.keyframes(alien).await, [0, 20_000]);
    assert_eq!(reader.listings(), 2);
}

#[tokio::test]
async fn a_versions_keyframes_are_listed_again_once_its_file_is_touched() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    media.keyframes(alien).await;

    let touched = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    fs::File::options()
        .write(true)
        .open(media.path(alien))
        .unwrap()
        .set_modified(touched)
        .unwrap();
    media.store(alien).await;
    media.match_to_alien(alien).await;
    media.keyframes(alien).await;

    assert_eq!(reader.listings(), 2);
}

#[tokio::test]
async fn a_versions_keyframes_that_failed_to_list_arent_listed_again() {
    let reader = reader_listing(|_| Err(failed()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;

    media.probing.keyframes(version).await.unwrap_err();
    media.probing.keyframes(version).await.unwrap_err();

    assert_eq!(reader.listings(), 1);
}

#[tokio::test]
async fn a_versions_keyframes_that_failed_to_list_are_listed_again_once_its_file_changes() {
    let reader = reader_listing(|_| Err(failed()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    media.probing.keyframes(version).await.unwrap_err();

    // Stored and probed again, as a scan and the plan after it would.
    media.write_probed(alien, 20).await;
    media.probing.keyframes(version).await.unwrap_err();

    assert_eq!(reader.listings(), 2);
}

#[tokio::test]
async fn a_versions_keyframes_that_failed_to_list_are_listed_again_once_the_failure_version_is_bumped()
 {
    let reader = reader_listing(|_| Err(failed()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    media.probing.keyframes(version).await.unwrap_err();

    media.tracks.failed_by_older_failure_version(version).await;
    media.probing.keyframes(version).await.unwrap_err();

    assert_eq!(reader.listings(), 2);
}

#[tokio::test]
async fn a_versions_keyframes_that_timed_out_arent_listed_again() {
    let reader = reader_listing(|_| Err(ProbeError::Run(RunError::TimedOut(Program::Ffprobe))));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;

    let listed = media.probing.keyframes(version).await;
    let again = media.probing.keyframes(version).await;

    let timed_out = |err: &ProbeError| matches!(err, ProbeError::Run(RunError::TimedOut(_)));
    assert!(failed_with(&listed, timed_out), "{listed:?}");
    let unlisted = |err: &ProbeError| matches!(err, ProbeError::UnlistedBefore);
    assert!(failed_with(&again, unlisted), "{again:?}");
    assert_eq!(reader.listings(), 1);
}

#[tokio::test]
async fn a_listing_carries_on_after_the_plan_waiting_for_it_is_dropped() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    reader.hold();
    let dropped = media.plan(media.file_id(alien).await);
    reader.until_listed(1).await;

    dropped.abort();
    dropped.await.unwrap_err();
    reader.release();
    media.probing.idle().await;

    let kept = media.tracks.keyframes(media.stored(alien).await).await;
    let listed = ListedKeyframes {
        file_start_ms: 0,
        keyframes_ms: vec![0, 10_000],
    };
    assert_eq!(kept.unwrap(), Some(Keyframes::Listed(listed)));
    assert_eq!(media.keyframes(alien).await, [0, 10_000]);
    assert_eq!(reader.listings(), 1);
}

#[tokio::test]
async fn a_listing_not_done_within_the_wait_is_unlisted_for_now_and_carries_on() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    reader.hold();
    let waiting = media.plan(media.file_id(alien).await);
    reader.until_listed(1).await;

    // After opening the database, whose pool times out by the clock.
    tokio::time::pause();
    tokio::time::advance(LISTING_WAIT).await;
    tokio::time::resume();
    let waited = waiting.await.unwrap();
    reader.release();

    assert!(failed_with(&waited, not_listed_yet), "{waited:?}");
    assert_eq!(media.keyframes(alien).await, [0, 10_000]);
    assert_eq!(reader.listings(), 1);
}

/// Writes and probes as many Movies as `count`, and returns their paths in the Library.
async fn write_movies(media: &Fixture, count: usize) -> Vec<String> {
    let mut movies = Vec::new();
    for n in 0..count {
        let movie = format!("Movie {n} (2000)/Movie {n} (2000).mkv");
        media.write_probed(&movie, 10).await;
        movies.push(movie);
    }
    movies
}

#[tokio::test]
async fn a_listing_past_the_cap_waits_for_a_slot() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let movies = write_movies(&media, WHOLE_FILE_READS_AT_ONCE + 1).await;
    let (past_cap, filling) = movies.split_last().unwrap();
    let past_cap = media.file_id(past_cap).await;
    reader.hold();
    for movie in filling {
        media.plan(media.file_id(movie).await);
    }
    reader.until_listed(WHOLE_FILE_READS_AT_ONCE).await;

    // A paused clock skips the wait only once no task, the listing past the cap's included, can
    // go further. Reading the database meanwhile is safe: its readers are all open and never
    // pinged, so taking one waits on no timer.
    tokio::time::pause();
    let waited = media.probing.keyframes(past_cap).await;
    tokio::time::resume();
    let while_full = reader.listings();
    reader.release_one();
    reader.until_listed(WHOLE_FILE_READS_AT_ONCE + 1).await;
    reader.release();
    media.probing.idle().await;

    assert!(failed_with(&waited, not_listed_yet), "{waited:?}");
    assert_eq!(while_full, WHOLE_FILE_READS_AT_ONCE);
    assert_eq!(reader.most_at_once(), WHOLE_FILE_READS_AT_ONCE);
    for movie in &movies {
        assert_eq!(media.keyframes(movie).await, [0, 10_000], "{movie}");
    }
}

#[tokio::test]
async fn another_whole_file_read_waits_while_listings_fill_the_cap() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let movies = write_movies(&media, WHOLE_FILE_READS_AT_ONCE).await;
    reader.hold();
    for movie in &movies {
        media.plan(media.file_id(movie).await);
    }
    reader.until_listed(WHOLE_FILE_READS_AT_ONCE).await;
    let (reading, mut was_read) = tokio::sync::oneshot::channel();
    let read = async move { reading.send(()).unwrap() };

    let spawned = media.reads.spawn(tracing::Span::none(), read);
    // On the test's one thread, the read's task runs until it waits for a slot before this goes on.
    tokio::task::yield_now().await;
    let while_full = was_read.try_recv();
    reader.release();

    assert!(spawned);
    assert!(while_full.is_err(), "it read while listings filled the cap");
    let after = tokio::time::timeout(Duration::from_secs(5), was_read).await;
    assert!(after.is_ok_and(|sent| sent.is_ok()), "it never read");
}

#[tokio::test]
async fn shutting_down_stops_a_listing_and_keeps_nothing() {
    let reader = reader_by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    reader.hold();
    let listing = media.plan(media.file_id(alien).await);
    reader.until_listed(1).await;

    let stopped = tokio::time::timeout(Duration::from_secs(5), media.reads.shutdown()).await;
    let waited = tokio::time::timeout(Duration::from_secs(5), listing).await;

    assert!(stopped.is_ok(), "shutting down waited for the listing");
    assert!(
        waited.is_ok_and(|waited| waited.unwrap().is_err()),
        "the plan waited on"
    );
    let kept = media
        .tracks
        .keyframes(media.stored(alien).await)
        .await
        .unwrap();
    assert!(kept.is_none(), "{kept:?}");
}

#[tokio::test]
async fn a_version_read_by_an_older_probe_gets_its_keyframes_from_its_cues_when_probed_again() {
    let media = Fixture::with_system_ffmpeg().await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    let path = media.path(alien);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    ffmpeg::write_h264_with_5_1(&path, "aac").unwrap();
    media.store(alien).await;
    let file = media.stored(alien).await;
    // As the probe version before stored it: Tracks, but no keyframes.
    let tracks_only = Probe {
        container: "matroska,webm".to_owned(),
        duration_ms: Some(14_000),
        bitrate: None,
        tracks: vec![],
        font_attachments: vec![],
        chapters: vec![],
    };
    media.tracks.store(file, tracks_only).await.unwrap();
    media.tracks.read_by_older_probe(file.id).await;

    media.probe_changed().await;

    let cued = ListedKeyframes {
        file_start_ms: 0,
        // A keyframe every 2 seconds.
        keyframes_ms: (0..7).map(|n| n * 2_000).collect(),
    };
    assert_eq!(
        media.tracks.keyframes(file).await.unwrap(),
        Some(Keyframes::Listed(cued))
    );
}

#[test]
fn index_keyframes_are_plausible_only_from_where_the_first_should_fall_in_order_and_by_the_end() {
    let from_23 = |keyframes_ms: &[i64]| {
        let listed = ListedKeyframes {
            file_start_ms: 0,
            keyframes_ms: keyframes_ms.to_vec(),
        };
        plausible(&listed, 23, Some(5000))
    };

    assert!(from_23(&[23, 2023]));
    assert!(from_23(&[24, 2024]), "within a millisecond's rounding");
    let refused: [(&[i64], _); 5] = [
        (&[], "none"),
        (&[0, 2000], "the first before where it should fall"),
        (&[100, 2023], "the first after"),
        (&[23, 3023, 2023], "out of order"),
        (&[23, 7023], "past the end"),
    ];
    for (keyframes_ms, why) in refused {
        assert!(!from_23(keyframes_ms), "{why}");
    }
}

/// Overwrites the ID of the Cues element in the Matroska file at `path`, so the `SeekHead` points
/// at no Cues, while ffprobe still reads the file's Tracks.
fn corrupt_cues(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    let cues_id = [0x1C, 0x53, 0xBB, 0x6B];
    // The last: the SeekHead names the ID too, and ffmpeg writes the Cues after the Clusters.
    let at = bytes.windows(4).rposition(|id| id == cues_id).unwrap();
    bytes[at..at + 4].fill(0);
    fs::write(path, bytes).unwrap();
}

impl Fixture {
    /// Writes Alien at `relative` as an MP4: [`ffmpeg::write_h264_with_5_1`]'s MKV, copied by
    /// the ffmpeg on `PATH` with the arguments `options` adds, given that MKV, before the output.
    fn write_alien_mp4(&self, relative: &str, options: impl FnOnce(&mut Command, &Path)) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let mkv = scratch.path().join("alien.mkv");
        ffmpeg::write_h264_with_5_1(&mkv, "aac").unwrap();
        let mut copy = Command::new("ffmpeg");
        copy.args(["-v", "error", "-i"]).arg(&mkv);
        options(&mut copy, &mkv);
        let status = copy.args(["-c", "copy"]).arg(&path).status().unwrap();
        assert!(status.success());
    }

    /// Stores the file written at `relative` as a Version, probes it as a scan would, and
    /// returns the keyframes probing kept for it. Fails the test if ffprobe couldn't read it, so
    /// a fixture it can't read never passes as one without an index.
    async fn probe_written(&self, relative: &str) -> Option<Keyframes> {
        self.store(relative).await;
        self.match_to_alien(relative).await;
        self.probe_changed().await;
        let file = self.stored(relative).await;
        let probed = self.tracks.of(&[file]).await.unwrap();
        assert!(probed.contains_key(&file.id), "ffprobe couldn't read it");
        self.tracks.keyframes(file).await.unwrap()
    }

    /// The keyframes the ffprobe on `PATH` lists by reading the whole file at `relative`.
    async fn whole_file_listing(&self, relative: &str) -> ListedKeyframes {
        let path = self.path(relative);
        ffprobe::keyframes(&system_ffmpeg(), path.to_str().unwrap())
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn a_fragmented_mp4_keeps_no_keyframes_when_probed_and_lists_them_from_the_whole_file() {
    let reader = FakeReader::forwarding_to(system_ffmpeg());
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mp4";
    media.write_alien_mp4(alien, |copy, _| {
        copy.args(["-movflags", "frag_keyframe+empty_moov"]);
    });

    let kept = media.probe_written(alien).await;
    let version = media.file_id(alien).await;
    let listed = media.probing.keyframes(version).await.unwrap();

    assert_eq!(kept, None);
    assert_eq!(listed, media.whole_file_listing(alien).await);
    assert_eq!(reader.listings(), 1);
}

#[tokio::test]
async fn an_mp4_whose_video_starts_with_an_empty_edit_keeps_the_keyframes_a_whole_file_listing_gives()
 {
    let reader = FakeReader::forwarding_to(system_ffmpeg());
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mp4";
    media.write_alien_mp4(alien, |copy, mkv| {
        // Video delayed behind the audio, so ffmpeg starts the video's `elst` with an empty edit.
        copy.args(["-itsoffset", "2", "-i"])
            .arg(mkv)
            .args(["-map", "0:a", "-map", "1:v"]);
    });

    let kept = media.probe_written(alien).await;
    let version = media.file_id(alien).await;
    let planned = media.probing.keyframes(version).await.unwrap();

    let whole_file = media.whole_file_listing(alien).await;
    assert!(
        whole_file.keyframes_ms.first() >= Some(&2_000),
        "the video isn't delayed: {whole_file:?}"
    );
    assert_eq!(kept, Some(Keyframes::Listed(whole_file.clone())));
    assert_eq!(planned, whole_file);
    assert_eq!(reader.listings(), 0);
}

#[tokio::test]
async fn an_mkv_whose_cues_are_corrupted_keeps_no_keyframes_when_probed_and_lists_them_once_from_the_whole_file()
 {
    let reader = FakeReader::forwarding_to(system_ffmpeg());
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let (intact, alien) = (
        "Alien (1979)/Alien (1979) - Intact.mkv",
        "Alien (1979)/Alien (1979).mkv",
    );
    let path = media.path(alien);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    ffmpeg::write_h264_with_5_1(&path, "aac").unwrap();
    fs::copy(&path, media.path(intact)).unwrap();
    corrupt_cues(&path);
    // The same file with its Cues whole keeps them, so a Probing that stopped reading Cues at
    // all fails here rather than passing as one that falls back.
    assert!(media.probe_written(intact).await.is_some());

    let kept = media.probe_written(alien).await;
    let version = media.file_id(alien).await;
    let listed = media.probing.keyframes(version).await.unwrap();

    assert_eq!(kept, None);
    assert_eq!(listed, media.whole_file_listing(alien).await);
    assert_eq!(reader.listings(), 1);
}
