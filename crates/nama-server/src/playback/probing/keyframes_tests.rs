use std::{fs, time::Duration};

use super::tests::Fixture;
#[cfg(unix)]
use super::tests::{fake_ffprobe, probe_runs, wait_for_probe};
#[cfg(unix)]
use super::{Error, LISTING_WAIT, LISTINGS_AT_ONCE, ProbeError, ffprobe::WHOLE_FILE_TIMEOUT};
#[cfg(unix)]
use crate::ffmpeg::Ffmpeg;

/// A fake ffprobe in `bin` that reads every file as a second-long Matroska file with no
/// Tracks, and lists keyframes at 0 and at as many seconds as the file has bytes, so a changed
/// file lists differently. Each listing logs a line to `bin/runs` first, then takes a second.
#[cfg(unix)]
fn listing_ffprobe(bin: &std::path::Path) -> Ffmpeg {
    ffprobe_listing_by(
        bin,
        r#"sleep 1
    # macOS's wc pads the count with spaces.
    printf '0.000000,K__\n%s.000000,K__\n' "$(wc -c < "$file" | tr -d ' ')""#,
    )
}

/// As [`listing_ffprobe`], but each listing fails, as on a file ffprobe can't read through.
#[cfg(unix)]
fn failing_listing_ffprobe(bin: &std::path::Path) -> Ffmpeg {
    ffprobe_listing_by(bin, "echo 'read failed' >&2; exit 1")
}

/// A fake ffprobe in `bin` that reads every file as a second-long Matroska file with no
/// Tracks, and lists keyframes by running `listing`, after logging a line to `bin/runs`.
#[cfg(unix)]
fn ffprobe_listing_by(bin: &std::path::Path, listing: &str) -> Ffmpeg {
    let runs = bin.join("runs");
    fake_ffprobe(
        bin,
        &format!(
            r#"for file; do :; done
case "$*" in *packet=*)
    echo list >> '{}'
    {listing}
    exit ;;
esac
echo '{{"format": {{"format_name": "matroska,webm", "duration": "1.000000"}}}}'"#,
            runs.display()
        ),
    )
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
        self.probing.keyframes(version).await.unwrap()
    }
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_are_listed_once_and_kept() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;

    assert_eq!(media.keyframes(alien).await, [0, 10_000]);
    assert_eq!(media.keyframes(alien).await, [0, 10_000]);

    assert_eq!(probe_runs(bin.path()), 1);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_asked_for_at_once_are_listed_once() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    let probing = media.probing.clone();
    let first = tokio::spawn(async move { probing.keyframes(version).await });
    wait_for_probe(&bin.path().join("runs")).await;

    let second = media.probing.keyframes(version).await.unwrap();

    assert_eq!(first.await.unwrap().unwrap(), second);
    assert_eq!(probe_runs(bin.path()), 1);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_are_listed_again_once_its_files_size_changes() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    media.keyframes(alien).await;

    // Stored as a scan would, but not probed again, so its Tracks were read from the old file.
    media.write(alien, 20).await;
    media.match_to_alien(alien).await;

    assert_eq!(media.keyframes(alien).await, [0, 20_000]);
    assert_eq!(probe_runs(bin.path()), 2);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_are_listed_again_once_its_file_is_touched() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
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

    assert_eq!(probe_runs(bin.path()), 2);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_that_failed_to_list_arent_listed_again() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(failing_listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;

    media.probing.keyframes(version).await.unwrap_err();
    media.probing.keyframes(version).await.unwrap_err();

    assert_eq!(probe_runs(bin.path()), 1);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_that_failed_to_list_are_listed_again_once_its_file_changes() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(failing_listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    media.probing.keyframes(version).await.unwrap_err();

    // Stored and probed again, as a scan and the plan after it would.
    media.write_probed(alien, 20).await;
    media.probing.keyframes(version).await.unwrap_err();

    assert_eq!(probe_runs(bin.path()), 2);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_that_failed_to_list_are_listed_again_once_the_failure_version_is_bumped()
 {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(failing_listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    media.probing.keyframes(version).await.unwrap_err();

    media.tracks.failed_by_older_failure_version(version).await;
    media.probing.keyframes(version).await.unwrap_err();

    assert_eq!(probe_runs(bin.path()), 2);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_versions_keyframes_that_timed_out_arent_listed_again() {
    let bin = tempfile::tempdir().unwrap();
    // `exec`, so the timeout's kill stops the sleep rather than leaving it orphaned.
    let media = Fixture::with_ffmpeg(ffprobe_listing_by(bin.path(), "exec sleep 3600")).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    let probing = media.probing.clone();
    let listing = tokio::spawn(async move { probing.keyframes(version).await });
    wait_for_probe(&bin.path().join("runs")).await;

    // After opening the database, whose pool times out by the clock.
    tokio::time::pause();
    tokio::time::advance(WHOLE_FILE_TIMEOUT).await;
    // Real time again: storing the timeout writes SQLite, which a paused clock would race.
    tokio::time::resume();
    listing.await.unwrap().unwrap_err();
    // Bounded, so listing it again fails the test rather than sleeping out the fake's hour.
    let again =
        tokio::time::timeout(Duration::from_secs(5), media.probing.keyframes(version)).await;

    assert!(
        again.as_ref().is_ok_and(|again| matches!(
            again,
            Err(Error::Probe(err))
                if matches!(**err, ProbeError::TimedOut(_) | ProbeError::UnlistedBefore)
        )),
        "{again:?}"
    );
    assert_eq!(probe_runs(bin.path()), 1);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_listing_carries_on_after_the_plan_waiting_for_it_is_dropped() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    let probing = media.probing.clone();
    let dropped = tokio::spawn(async move { probing.keyframes(version).await });
    wait_for_probe(&bin.path().join("runs")).await;

    dropped.abort();
    dropped.await.unwrap_err();

    assert_eq!(media.keyframes(alien).await, [0, 10_000]);
    assert_eq!(probe_runs(bin.path()), 1);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_listing_not_done_within_the_wait_is_unlisted_for_now_and_carries_on() {
    let bin = tempfile::tempdir().unwrap();
    let go = bin.path().join("go");
    let listing = format!(
        "while [ ! -e '{}' ]; do sleep 0.1; done\nprintf '0.000000,K__\\n'",
        go.display()
    );
    let media = Fixture::with_ffmpeg(ffprobe_listing_by(bin.path(), &listing)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    let probing = media.probing.clone();
    let waiting = tokio::spawn(async move { probing.keyframes(version).await });
    wait_for_probe(&bin.path().join("runs")).await;

    // After opening the database, whose pool times out by the clock.
    tokio::time::pause();
    tokio::time::advance(LISTING_WAIT).await;
    tokio::time::resume();
    let waited = waiting.await.unwrap();
    fs::write(&go, "").unwrap();

    assert!(
        matches!(&waited, Err(Error::Probe(err)) if matches!(**err, ProbeError::NotListedYet)),
        "{waited:?}"
    );
    assert_eq!(media.keyframes(alien).await, [0]);
    assert_eq!(probe_runs(bin.path()), 1);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn no_more_listings_than_the_cap_run_at_once() {
    let bin = tempfile::tempdir().unwrap();
    let running = bin.path().join("running");
    let most = bin.path().join("most");
    fs::create_dir(&running).unwrap();
    // Counts the listings running alongside it in `running`, keeping the most in `most`.
    let listing = format!(
        r#"me='{running}'/$$
touch "$me"
now=$(ls '{running}' | wc -l)
[ "$now" -gt "$(cat '{most}' 2>/dev/null || echo 0)" ] && echo "$now" > '{most}'
sleep 0.3
rm "$me"
printf '0.000000,K__\n'"#,
        running = running.display(),
        most = most.display()
    );
    let media = Fixture::with_ffmpeg(ffprobe_listing_by(bin.path(), &listing)).await;
    let mut versions = Vec::new();
    for n in 0..=LISTINGS_AT_ONCE {
        let movie = format!("Movie {n} (2000)/Movie {n} (2000).mkv");
        media.write_probed(&movie, 10).await;
        versions.push(media.file_id(&movie).await);
    }

    let listed = futures_util::future::join_all(
        versions
            .into_iter()
            .map(|version| media.probing.keyframes(version)),
    )
    .await;

    assert!(listed.iter().all(Result::is_ok), "{listed:?}");
    let most: usize = fs::read_to_string(&most).unwrap().trim().parse().unwrap();
    assert!((1..=LISTINGS_AT_ONCE).contains(&most), "{most} ran at once");
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn shutting_down_stops_a_listing_and_keeps_nothing() {
    let bin = tempfile::tempdir().unwrap();
    // `exec`, so stopping it kills the sleep rather than leaving it orphaned.
    let media = Fixture::with_ffmpeg(ffprobe_listing_by(bin.path(), "exec sleep 3600")).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write_probed(alien, 10).await;
    let version = media.file_id(alien).await;
    let probing = media.probing.clone();
    let listing = tokio::spawn(async move { probing.keyframes(version).await });
    wait_for_probe(&bin.path().join("runs")).await;

    let stopped = tokio::time::timeout(Duration::from_secs(5), media.probing.shutdown()).await;
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
