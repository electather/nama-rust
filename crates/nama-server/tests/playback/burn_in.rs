//! A Text Burn-in reads the file's subtitle and fonts from the subtitle cache once they're in it.

use std::time::Instant;

use super::*;

/// The runs of ffmpeg so far that burned a subtitle in, as `log` has them, once there are `n`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn burn_ins(log: &Path, n: usize) -> Vec<String> {
    tokio::time::timeout(WAIT, async {
        loop {
            let runs = fs::read_to_string(log).unwrap_or_default();
            let burn_ins: Vec<_> = (runs.lines())
                .filter(|run| run.contains("subtitles="))
                .map(str::to_owned)
                .collect();
            if burn_ins.len() >= n {
                return burn_ins;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_seek_after_the_ass_and_fonts_are_cached_burns_them_in_from_the_cache() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let ffmpeg = ffmpeg_reading_ass_and_a_font(bin.path(), &log);
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            subtitle_track: Some(2),
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    let [first] = &burn_ins(&log, 1).await[..] else {
        panic!("not one burn-in");
    };

    fx.fetch_ok(&format!("/media/versions/{version}/subtitles/2/ass"))
        .await;
    fx.fetch_ok(&format!("/media/versions/{version}/fonts/0"))
        .await;
    // A segment far ahead, where ffmpeg starts again.
    let token = Some(fx.server.admin_token.as_str());
    let seeking = Instant::now();
    fetch(&fx.server, &beside(&plan.url, "5.m4s"), token).await;
    let seek_took = seeking.elapsed();

    let seek = &burn_ins(&log, 2).await[1];
    let media = fx.folder.join(ALIEN_1080P);
    let media = media.to_str().unwrap();
    assert!(
        first.contains(&format!(r"subtitles=filename=\'{media}\':si=0")),
        "{first}"
    );
    let [cached] = &fx.cached_subtitles()[..] else {
        panic!("not one Version's subtitles cached");
    };
    let (ass, fonts) = (cached.join("2.ass"), cached.join("fonts"));
    let from_cache = format!(
        r"subtitles=filename=\'{}\':fontsdir=\'{}\'",
        ass.display(),
        fonts.display()
    );
    assert!(seek.contains(&from_cache), "{seek}");
    // Without subtitles the fake answers at once; reading them from the file takes it 30 s.
    assert!(seek_took < Duration::from_secs(10), "{seek_took:?}");
}
