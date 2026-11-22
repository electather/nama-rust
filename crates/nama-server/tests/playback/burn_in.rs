//! A Text Burn-in reads the file's subtitle and fonts from the subtitle cache once they're in it.

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

/// The folder the Server caches `fx`'s one Version's subtitles in, once a text pass has cached
/// its ASS Track 2 and its font there.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn cached_ass_and_font(fx: &Fixture) -> PathBuf {
    tokio::time::timeout(WAIT, async {
        loop {
            if let [cached] = &fx.cached_subtitles()[..]
                && cached.join("2.ass").is_file()
                && cached.join("fonts/0").is_file()
            {
                return cached.clone();
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
async fn a_burn_in_after_the_ass_and_fonts_are_cached_reads_them_from_the_cache() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let ffmpeg = ffmpeg_reading_ass_and_a_font(bin.path(), &log);
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    // By the text pass after probing.
    let cached = cached_ass_and_font(&fx).await;

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
    let [burn_in] = &burn_ins(&log, 1).await[..] else {
        panic!("not one burn-in");
    };
    let (ass, fonts) = (cached.join("2.ass"), cached.join("fonts"));
    let from_cache = format!(
        r"subtitles=filename=\'{}\':fontsdir=\'{}\'",
        ass.display(),
        fonts.display()
    );
    assert!(burn_in.contains(&from_cache), "{burn_in}");
}

#[tokio::test]
async fn a_pgs_track_burned_into_hdr10_on_an_hdr_screen_keeps_hdr10_its_square_at_reference_white()
{
    let write = |file: &Path| ffmpeg::write_hdr_ramp_with_pgs(file, "smpte2084");
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), write).await;

    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            subtitle_track: Some(1),
            device_decoding: DeviceDecoding {
                hdr: true,
                ..chrome()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(plan.burn_in_tracks, vec![1]);
    let transfer = played_streams(&fx, &plan.url, "v", "0.m4s", "color_transfer").await;
    assert_eq!(transfer, "smpte2084");
    let picture = sidecars::last_frame(&fx, &plan.url, "0.m4s").await;
    // As a code of 10-bit limited range: `last_frame` reads full-range 8-bit grey.
    let square = 64 + u32::from(picture[108 * 320 + 108]) * 876 / 255;
    assert!(
        (563..=583).contains(&square),
        "the square's luma is {square}, not PQ's 203 nits"
    );
}
