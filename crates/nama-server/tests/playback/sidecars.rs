//! Text sidecar subtitles beside a Version: listed after its file's own Tracks, served as WebVTT
//! and matched by Subtitle Preferences like them.

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
    async fn alien_with_sidecars(sidecars: &[(&str, &str)]) -> (Self, i64) {
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
