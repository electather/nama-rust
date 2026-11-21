//! Reading a Version's text subtitles and fonts out of its file in one background pass, by a
//! scripted ffmpeg, after Probing while nothing plays (#1649), or when one is fetched before then
//! (#829).

use super::*;

/// Alien's Tracks as the fake ffprobe reads them: 2 is an SRT, 3 an ASS and 4 a PGS subtitle.
const SRT: usize = 2;
const ASS: usize = 3;
const PGS: usize = 4;

/// What the fake ffprobe reads every file's streams as: an H.264 and AAC MKV with an SRT, an ASS
/// and a PGS subtitle, a cover picture and a font.
const SUBTITLED: &str = r#"[{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}},
             {"codec_name": "subrip", "codec_type": "subtitle"},
             {"codec_name": "ass", "codec_type": "subtitle"},
             {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle"},
             {"codec_type": "attachment", "tags": {"filename": "cover.jpg", "mimetype": "image/jpeg"}},
             {"codec_type": "attachment", "tags": {"filename": "vazir.ttf", "mimetype": "font/ttf"}}]"#;

/// As [`SUBTITLED`], with only the PGS subtitle, Track 2.
const IMAGE_SUBTITLED: &str = r#"[{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}},
             {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle"}]"#;

/// A Server whose ffmpeg and ffprobe are shell scripts in `bin`. ffprobe reads every file as
/// [`SUBTITLED`]. ffmpeg writes the arguments of each run that reads subtitles or attachments to
/// `bin/runs.log`, then runs `first`, then writes `WEBVTT`, `ASS`, `PGS` or `FONT` wherever it's
/// told to write a subtitle in that format or an attachment.
#[cfg(unix)]
fn ffmpeg_reading_text_subtitles_after(bin: &Path, first: &str) -> Ffmpeg {
    ffmpeg_reading_subtitles(bin, SUBTITLED, first)
}

/// As [`ffmpeg_reading_text_subtitles_after`], with ffprobe reading every file's streams as
/// `streams`.
#[cfg(unix)]
fn ffmpeg_reading_subtitles(bin: &Path, streams: &str, first: &str) -> Ffmpeg {
    let ffprobe = format!(
        r#"#!/bin/sh
cat <<'EOF'
{{"streams": {streams},
 "format": {{"format_name": "matroska,webm", "duration": "1.000000"}}}}
EOF
"#
    );
    let ffmpeg = format!(
        r#"#!/bin/sh
case "$*" in *0:s:*|*-dump_attachment*)
    echo "$*" >> '{}'
    {first} ;;
esac
out=
for arg; do
    if [ -n "$out" ]; then
        if [ "$arg" = - ]; then printf %s "$out"; else printf %s "$out" > "$arg"; fi
        out=
    fi
    case "$prev,$arg" in
        -f,webvtt) out=WEBVTT ;;
        -f,ass) out=ASS ;;
        -f,sup) out=PGS ;;
        *,-dump_attachment*) out=FONT ;;
    esac
    prev=$arg
done
"#,
        bin.join("runs.log").display()
    );
    Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", &ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", &ffprobe)),
        gpus: vec![],
    }
}

/// Each run of the fake ffmpeg in `bin` that read subtitles or attachments, by its arguments.
#[cfg(unix)]
fn ffmpeg_runs(bin: &Path) -> Vec<String> {
    // No log until the first run.
    let runs = fs::read_to_string(bin.join("runs.log")).unwrap_or_default();
    runs.lines().map(str::to_owned).collect()
}

/// Waits until the fake ffmpeg in `bin` has read subtitles or attachments `n` times, failing
/// after [`WAIT`].
#[cfg(unix)]
async fn until_runs(bin: &Path, n: usize) {
    let ran = async {
        while ffmpeg_runs(bin).len() < n {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    let waited = tokio::time::timeout(WAIT, ran).await;
    assert!(
        waited.is_ok(),
        "fewer than {n} runs: {:?}",
        ffmpeg_runs(bin)
    );
}

/// Waits until `fx`'s one Version has `name` in its subtitle cache, failing after [`WAIT`].
async fn until_cached(fx: &Fixture, name: &str) {
    let cached = async {
        while !matches!(&fx.cached_subtitles()[..], [cached] if cached.join(name).is_file()) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    let waited = tokio::time::timeout(WAIT, cached).await;
    assert!(
        waited.is_ok(),
        "{name} isn't cached: {:?}",
        fx.cached_subtitles()
    );
}

/// Starts a Server with `ffmpeg` and a Movies Library holding Alien, scanned and Matched, and
/// returns it with Alien's Version's id.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_with(ffmpeg: Ffmpeg) -> (Fixture, i64) {
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    (fx, version)
}

/// As [`alien_with`], once the background text pass has started, which the fake ffmpeg in `bin`
/// logs as its first run.
#[cfg(unix)]
async fn alien_read_out_with(bin: &Path, ffmpeg: Ffmpeg) -> (Fixture, i64) {
    let alien = alien_with(ffmpeg).await;
    until_runs(bin, 1).await;
    alien
}

impl Fixture {
    /// What the Server answers the Admin for `url`, as its status and body.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn fetched(&self, url: &str) -> (u16, String) {
        let reply = fetch(&self.server, url, Some(&self.server.admin_token)).await;
        (reply.status, String::from_utf8(reply.body).unwrap())
    }

    /// The Admin's Direct Play of the Version `version`, on [`chrome`] decoding Matroska too, as
    /// the fake ffmpeg can't Remux, and a plan that doesn't play wouldn't stop a background pass.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn play(&self, version: i64) -> PlaybackPlan {
        let mut device = chrome();
        device.containers.push(Container::CONTAINER_MATROSKA.into());
        let plan = self
            .ask_for_plan(PlanPlaybackRequest {
                version_id: version,
                device_decoding: device.into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_ne!(plan.id, 0, "the plan doesn't play: {plan:?}");
        plan
    }
}

fn subtitle_url(version: i64, track: usize) -> String {
    format!("/media/versions/{version}/subtitles/{track}")
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_pass_after_probing_reads_every_text_track_and_font_in_one_run_fetches_are_served_from() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), "");
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;
    // Its last step caches the font.
    until_cached(&fx, "fonts/0").await;

    let srt = fx.fetched(&subtitle_url(version, SRT)).await;
    let ass = fx
        .fetched(&format!("{}/ass", subtitle_url(version, ASS)))
        .await;
    let ass_as_webvtt = fx.fetched(&subtitle_url(version, ASS)).await;
    let font = fx
        .fetched(&format!("/media/versions/{version}/fonts/0"))
        .await;

    assert_eq!(srt, (200, "WEBVTT".to_owned()));
    assert_eq!(ass, (200, "ASS".to_owned()));
    assert_eq!(ass_as_webvtt.0, 200);
    assert!(ass_as_webvtt.1.starts_with("WEBVTT"), "{ass_as_webvtt:?}");
    assert_eq!(font, (200, "FONT".to_owned()));
    let runs = ffmpeg_runs(bin.path());
    let [run] = runs.as_slice() else {
        panic!("not one ffmpeg run: {runs:?}");
    };
    // The font is the file's second attachment, after the cover.
    for read in ["-dump_attachment:t:1", "0:s:0", "0:s:1"] {
        assert!(run.contains(read), "{run}");
    }
    assert!(!run.contains("0:s:2"), "an image subtitle was read: {run}");
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn an_image_track_is_still_read_out_on_its_own_when_fetched() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), "");
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;

    let pgs = fx.fetched(&subtitle_url(version, PGS)).await;

    assert_eq!(pgs, (200, "PGS".to_owned()));
    let runs = ffmpeg_runs(bin.path());
    let [_text_pass, run] = runs.as_slice() else {
        panic!("not the text pass and one more ffmpeg run: {runs:?}");
    };
    assert!(run.contains("-map 0:s:2 -c:s copy -f sup -"), "{run}");
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_version_with_only_image_tracks_gets_no_background_pass() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffmpeg_reading_subtitles(bin.path(), IMAGE_SUBTITLED, "");
    let (fx, version) = alien_with(ffmpeg).await;
    // Probed, which the fetch needs, and the background work told.
    fx.scan_and_probe().await;

    let pgs = fx.fetched(&subtitle_url(version, 2)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert_eq!(pgs, (200, "PGS".to_owned()));
    assert_eq!(ffmpeg_runs(bin.path()).len(), 1);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_background_pass_stops_while_a_plan_plays_and_runs_again_after() {
    let bin = tempfile::tempdir().unwrap();
    let go = bin.path().join("go");
    let wait_for_go = format!("while [ ! -e '{}' ]; do sleep 0.05; done", go.display());
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), &wait_for_go);
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;

    let plan = fx.play(version).await;
    // Longer than the background work takes to see the plan.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let runs_while_playing = ffmpeg_runs(bin.path()).len();
    fx.stop(plan.id).await;
    until_runs(bin.path(), 2).await;
    fs::write(&go, "").unwrap();
    let srt = fx.fetched(&subtitle_url(version, SRT)).await;

    assert_eq!(runs_while_playing, 1);
    assert_eq!(srt, (200, "WEBVTT".to_owned()));
    assert_eq!(ffmpeg_runs(bin.path()).len(), 2);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_fetch_waiting_on_a_pass_playback_stopped_runs_its_own() {
    let bin = tempfile::tempdir().unwrap();
    let go = bin.path().join("go");
    let wait_for_go = format!("while [ ! -e '{}' ]; do sleep 0.05; done", go.display());
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), &wait_for_go);
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;

    let srt = subtitle_url(version, SRT);
    let (srt, ()) = tokio::join!(fx.fetched(&srt), async {
        // Long enough for the fetch to be waiting on the background pass.
        tokio::time::sleep(Duration::from_millis(200)).await;
        fx.play(version).await;
        until_runs(bin.path(), 2).await;
        fs::write(&go, "").unwrap();
    });

    assert_eq!(srt, (200, "WEBVTT".to_owned()));
    assert_eq!(ffmpeg_runs(bin.path()).len(), 2);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_fetch_given_up_on_leaves_its_pass_reading_for_the_next() {
    let bin = tempfile::tempdir().unwrap();
    let go = bin.path().join("go");
    let wait_for_go = format!("while [ ! -e '{}' ]; do sleep 0.05; done", go.display());
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), &wait_for_go);
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;
    // Stops the background pass, so the fetch runs its own.
    fx.play(version).await;

    let srt = subtitle_url(version, SRT);
    tokio::select! {
        given_up_on = fx.fetched(&srt) => {
            panic!("answered before ffmpeg finished: {given_up_on:?}");
        }
        () = until_runs(bin.path(), 2) => {}
    }
    fs::write(&go, "").unwrap();
    let ass = fx
        .fetched(&format!("{}/ass", subtitle_url(version, ASS)))
        .await;

    assert_eq!(ass, (200, "ASS".to_owned()));
    assert_eq!(ffmpeg_runs(bin.path()).len(), 2);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_pass_is_not_tried_again_until_the_file_changes() {
    let bin = tempfile::tempdir().unwrap();
    let fail = "echo 'read failed' >&2; exit 1";
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), fail);
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;

    let first = fx.fetched(&subtitle_url(version, SRT)).await;
    let other_track = fx.fetched(&subtitle_url(version, ASS)).await;
    let font = fx
        .fetched(&format!("/media/versions/{version}/fonts/0"))
        .await;
    fx.scan_and_probe().await;
    let runs_before_the_change = ffmpeg_runs(bin.path()).len();
    fx.write(ALIEN_1080P, b"another video");
    fx.scan_and_probe().await;
    until_runs(bin.path(), 2).await;
    let after_the_change = fx.fetched(&subtitle_url(version, SRT)).await;

    assert_eq!(
        [first.0, other_track.0, font.0, after_the_change.0],
        [500; 4]
    );
    assert_eq!(runs_before_the_change, 1);
    assert_eq!(ffmpeg_runs(bin.path()).len(), 2);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_pass_that_couldnt_write_its_cache_is_tried_again() {
    let bin = tempfile::tempdir().unwrap();
    let full = "echo 'Error writing trailer: No space left on device' >&2; exit 1";
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), full);
    let (fx, version) = alien_read_out_with(bin.path(), ffmpeg).await;

    let first = fx.fetched(&subtitle_url(version, SRT)).await;
    let again = fx.fetched(&subtitle_url(version, SRT)).await;

    // The background pass, then one for each fetch.
    assert_eq!([first.0, again.0], [500; 2]);
    assert_eq!(ffmpeg_runs(bin.path()).len(), 3);
}
