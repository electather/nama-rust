//! Reading a Version's text subtitles and fonts out of its file in one background pass, by a
//! scripted ffmpeg, the first time any of them is fetched (#829).

use super::*;

/// Alien's Tracks as the fake ffprobe reads them: 2 is an SRT, 3 an ASS and 4 a PGS subtitle.
const SRT: usize = 2;
const ASS: usize = 3;
const PGS: usize = 4;

/// A Server whose ffmpeg and ffprobe are shell scripts in `bin`. ffprobe reads every file as an
/// H.264 and AAC MKV with an SRT, an ASS and a PGS subtitle, a cover picture and a font. ffmpeg
/// writes the arguments of each run that reads subtitles or attachments to `bin/runs.log`, then
/// runs `first`, then writes `WEBVTT`, `ASS`, `PGS` or `FONT` wherever it's told to write a
/// subtitle in that format or an attachment.
#[cfg(unix)]
fn ffmpeg_reading_text_subtitles_after(bin: &Path, first: &str) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
cat <<'EOF'
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}},
             {"codec_name": "subrip", "codec_type": "subtitle"},
             {"codec_name": "ass", "codec_type": "subtitle"},
             {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle"},
             {"codec_type": "attachment", "tags": {"filename": "cover.jpg", "mimetype": "image/jpeg"}},
             {"codec_type": "attachment", "tags": {"filename": "vazir.ttf", "mimetype": "font/ttf"}}],
 "format": {"format_name": "matroska,webm", "duration": "1.000000"}}
EOF
"#;
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
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
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

/// Starts a Server with `ffmpeg` and a Movies Library holding Alien, scanned, Matched and
/// planned, as a client does before it fetches a subtitle, and returns it with Alien's Version's
/// id.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_with(ffmpeg: Ffmpeg) -> (Fixture, i64) {
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    fx.plan(version).await.unwrap();
    (fx, version)
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
}

fn subtitle_url(version: i64, track: usize) -> String {
    format!("/media/versions/{version}/subtitles/{track}")
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn the_first_fetch_reads_every_text_track_and_font_out_in_one_run() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, version) = alien_with(ffmpeg_reading_text_subtitles_after(bin.path(), "")).await;

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
    let (fx, version) = alien_with(ffmpeg_reading_text_subtitles_after(bin.path(), "")).await;

    let pgs = fx.fetched(&subtitle_url(version, PGS)).await;

    assert_eq!(pgs, (200, "PGS".to_owned()));
    let runs = ffmpeg_runs(bin.path());
    let [run] = runs.as_slice() else {
        panic!("not one ffmpeg run: {runs:?}");
    };
    assert!(run.contains("-map 0:s:2 -c:s copy -f sup -"), "{run}");
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_fetch_given_up_on_leaves_its_pass_reading_for_the_next() {
    let bin = tempfile::tempdir().unwrap();
    let go = bin.path().join("go");
    let wait_for_go = format!("while [ ! -e '{}' ]; do sleep 0.05; done", go.display());
    let ffmpeg = ffmpeg_reading_text_subtitles_after(bin.path(), &wait_for_go);
    let (fx, version) = alien_with(ffmpeg).await;

    let srt = subtitle_url(version, SRT);
    tokio::select! {
        given_up_on = fx.fetched(&srt) => {
            panic!("answered before ffmpeg finished: {given_up_on:?}");
        }
        () = async {
            while ffmpeg_runs(bin.path()).is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        } => {}
    }
    fs::write(&go, "").unwrap();
    let ass = fx
        .fetched(&format!("{}/ass", subtitle_url(version, ASS)))
        .await;

    assert_eq!(ass, (200, "ASS".to_owned()));
    assert_eq!(ffmpeg_runs(bin.path()).len(), 1);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_pass_is_not_tried_again_until_the_file_changes() {
    let bin = tempfile::tempdir().unwrap();
    let fail = "echo 'read failed' >&2; exit 1";
    let (fx, version) = alien_with(ffmpeg_reading_text_subtitles_after(bin.path(), fail)).await;

    let first = fx.fetched(&subtitle_url(version, SRT)).await;
    let other_track = fx.fetched(&subtitle_url(version, ASS)).await;
    let font = fx
        .fetched(&format!("/media/versions/{version}/fonts/0"))
        .await;
    let runs_before_the_change = ffmpeg_runs(bin.path()).len();
    fx.write(ALIEN_1080P, b"another video");
    fx.scan_and_probe().await;
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
    let (fx, version) = alien_with(ffmpeg_reading_text_subtitles_after(bin.path(), full)).await;

    let first = fx.fetched(&subtitle_url(version, SRT)).await;
    let again = fx.fetched(&subtitle_url(version, SRT)).await;

    assert_eq!([first.0, again.0], [500; 2]);
    assert_eq!(ffmpeg_runs(bin.path()).len(), 2);
}
