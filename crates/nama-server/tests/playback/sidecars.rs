//! Sidecar subtitles beside a Version: listed after its file's own Tracks, served as WebVTT in
//! their own characters whatever charset they're in, burned in and matched by Subtitle
//! Preferences like them.

use encoding_rs::{WINDOWS_1252, WINDOWS_1256};
use ffmpeg::{FRENCH_SRT, PERSIAN_SRT};

use super::*;

/// Says `Sidecar hello` for a second, as an SRT sidecar.
const SIDECAR_HELLO: &str = "1\n00:00:00,000 --> 00:00:01,000\nSidecar hello\n";

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with the ffmpeg on `PATH` and a Movies Library holding Alien in 1080p, with
    /// an untagged SRT Track of its own and the sidecars `sidecars`, each a file name and what it
    /// says, scanned and Matched, and returns it with that Version's id.
    async fn alien_with_sidecars<Says: AsRef<[u8]>>(sidecars: &[(&str, Says)]) -> (Self, i64) {
        let fx = Self::alien_written_by(ALIEN_1080P, |path| {
            write_subtitled(path, HELLO_SRT, "srt");
            for (name, says) in sidecars {
                fs::write(path.with_file_name(name), says).unwrap();
            }
        })
        .await;
        let version = fx.version_ids(ALIEN).await[0];
        (fx, version)
    }

    /// Writes `says` to Alien's sidecar `name` and scans again.
    async fn rewrite_sidecar(&self, name: &str, says: &str) {
        let video = self.folder.join(ALIEN_1080P);
        fs::write(video.with_file_name(name), says).unwrap();
        self.scan_and_match().await;
    }

    /// What the Server serves at `url`, fetched as the Admin.
    async fn served_text(&self, url: &str) -> String {
        String::from_utf8(self.fetch_ok(url).await).unwrap()
    }
}

/// The WebVTT the Server serves for Alien's one sidecar, an `.srt` holding `srt`.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn served_sidecar(srt: &[u8]) -> String {
    let (fx, version) = Fixture::alien_with_sidecars(&[("Alien (1979) - 1080p.en.srt", srt)]).await;
    let url = fx.plan(version).await.unwrap().subtitles[1].url.clone();
    fx.served_text(&url).await
}

/// What the one cue of the `.srt` `srt` says.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn cue_text(srt: &str) -> &str {
    srt.lines().last().unwrap()
}

/// The codec and language of each of `tracks`.
fn codecs_and_languages(tracks: &[Track]) -> Vec<(&str, Option<&str>)> {
    (tracks.iter())
        .map(|track| (track.codec.as_str(), track.language.as_deref()))
        .collect()
}

#[tokio::test]
async fn a_sidecar_srt_follows_the_files_own_subtitle_and_is_served_as_webvtt() {
    let (fx, version) =
        Fixture::alien_with_sidecars(&[("Alien (1979) - 1080p.en.srt", SIDECAR_HELLO)]).await;

    let plan = fx.plan(version).await.unwrap();
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    assert_eq!(
        codecs_and_languages(&movie.versions[0].tracks),
        [("mpeg4", None), ("subrip", None), ("subrip", Some("en"))]
    );
    let sidecar = &plan.subtitles[1];
    assert_eq!(
        (sidecar.track.language.as_deref(), sidecar.format),
        (Some("en"), SubtitleFormat::SUBTITLE_FORMAT_WEBVTT.into())
    );
    let webvtt = fx.served_text(&sidecar.url).await;
    assert!(webvtt.starts_with("WEBVTT"), "{webvtt}");
    assert!(webvtt.contains("Sidecar hello"), "{webvtt}");
}

#[tokio::test]
async fn an_edited_sidecar_scanned_again_serves_its_new_text_from_the_same_url() {
    let name = "Alien (1979) - 1080p.en.srt";
    let (fx, version) = Fixture::alien_with_sidecars(&[(name, SIDECAR_HELLO)]).await;
    let url = fx.plan(version).await.unwrap().subtitles[1].url.clone();
    assert!(fx.served_text(&url).await.contains("Sidecar hello"));

    fx.rewrite_sidecar(name, "1\n00:00:00,000 --> 00:00:01,000\nEdited\n")
        .await;

    let webvtt = fx.served_text(&url).await;
    assert!(webvtt.contains("Edited"), "{webvtt}");
}

#[tokio::test]
async fn a_windows_1256_sidecar_is_served_as_webvtt_with_its_persian_text() {
    let webvtt = served_sidecar(&WINDOWS_1256.encode(PERSIAN_SRT).0).await;

    assert!(webvtt.contains(cue_text(PERSIAN_SRT)), "{webvtt}");
}

#[tokio::test]
async fn a_windows_1252_sidecar_is_served_as_webvtt_with_its_accented_text() {
    let webvtt = served_sidecar(&WINDOWS_1252.encode(FRENCH_SRT).0).await;

    assert!(webvtt.contains(cue_text(FRENCH_SRT)), "{webvtt}");
}

#[tokio::test]
async fn a_utf8_sidecar_with_or_without_a_bom_is_served_with_its_text_unchanged() {
    let plain = served_sidecar(FRENCH_SRT.as_bytes()).await;
    let with_bom = served_sidecar(format!("\u{feff}{FRENCH_SRT}").as_bytes()).await;

    assert!(plain.contains(cue_text(FRENCH_SRT)), "{plain}");
    assert_eq!(with_bom, plain);
}

// `pick` is in the fixture for fake ffprobes, which are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn an_english_preference_starts_a_sidecar_and_a_picked_one_keeps_what_its_name_says() {
    // After the file's own video and subtitle, in path order: forced SDH at 2, then these.
    const SDH_WITH_NOTES: u32 = 3;
    const FULL: u32 = 6;
    let (fx, version) = Fixture::alien_with_sidecars(&[
        ("Alien (1979) - 1080p.en.forced.sdh.srt", SIDECAR_HELLO),
        (
            "Alien (1979) - 1080p.en.forced.sdh.with notes.srt",
            SIDECAR_HELLO,
        ),
        ("Alien (1979) - 1080p.en.forced.srt", SIDECAR_HELLO),
        (
            "Alien (1979) - 1080p.en.forced.with notes.srt",
            SIDECAR_HELLO,
        ),
        ("Alien (1979) - 1080p.en.srt", SIDECAR_HELLO),
    ])
    .await;
    let full = SubtitleKind::SUBTITLE_KIND_FULL;
    set_own_subtitle(fx.server.client_config(), Some("en"), full)
        .await
        .unwrap();
    let preferred = fx.plan(version).await.unwrap().starting_subtitle;

    fx.pick(
        version,
        PickedTrackKind::PICKED_TRACK_KIND_SUBTITLE,
        Some(SDH_WITH_NOTES),
    )
    .await
    .unwrap();

    assert_eq!(preferred, Some(FULL));
    // Kept without its title, it would start the untitled forced SDH one before it; without SDH,
    // the forced one with notes after it.
    let started = fx.plan(version).await.unwrap().starting_subtitle;
    assert_eq!(started, Some(SDH_WITH_NOTES));
}

#[tokio::test]
async fn a_sup_sidecar_follows_the_files_own_tracks_and_is_served_as_pgs_unchanged() {
    let sup = "PG as the sidecar holds it";
    let (fx, version) = Fixture::alien_with_sidecars(&[("Alien (1979) - 1080p.en.sup", sup)]).await;
    let drawing_pgs = DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_PGS.into()],
        ..chrome()
    };

    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            device_decoding: drawing_pgs.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    assert_eq!(
        codecs_and_languages(&movie.versions[0].tracks),
        [
            ("mpeg4", None),
            ("subrip", None),
            ("hdmv_pgs_subtitle", Some("en"))
        ]
    );
    let sidecar = &plan.subtitles[1];
    assert_eq!(
        (sidecar.track.language.as_deref(), sidecar.format),
        (Some("en"), SubtitleFormat::SUBTITLE_FORMAT_PGS.into())
    );
    let served = fetch(&fx.server, &sidecar.url, Some(&fx.server.admin_token)).await;
    assert_eq!(served.header("content-type"), "application/octet-stream");
    assert_eq!(served.body, sup.as_bytes());
}

#[tokio::test]
async fn a_sup_sidecar_is_burned_in_in_step_from_the_start_and_after_a_seek() {
    let (fx, version) =
        Fixture::alien_written(system_ffmpeg(), ffmpeg::write_black_with_pgs_sidecar).await;
    let plan_from = |start_position_ms| PlanPlaybackRequest {
        version_id: version,
        start_position_ms,
        subtitle_track: Some(1),
        device_decoding: chrome().into(),
        ..Default::default()
    };

    let from_start = fx.ask_for_plan(plan_from(0)).await.unwrap();
    let seeking = fx.ask_for_plan(plan_from(7000)).await.unwrap();

    assert_eq!(from_start.burn_in_tracks, vec![1]);
    // The square shows from 5 seconds to 15, and the segments are 6 seconds long. The second
    // segment's last frame catches a square shown 5 seconds early, and the run from it starts
    // after the square began, so it shows only if that run's sidecar input doesn't seek.
    let first = last_frame(&fx, &from_start.url, "0.m4s").await;
    assert!(first[108 * 320 + 108] > 200, "the square is burned in");
    let second = last_frame(&fx, &seeking.url, "1.m4s").await;
    assert!(
        second[108 * 320 + 108] > 200,
        "the square is burned in after a seek"
    );
    assert!(
        second[10 * 320 + 10] < 50,
        "the picture around it stays black"
    );
}
