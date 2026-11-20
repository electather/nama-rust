//! The Chapters a Playback Plan sends, as Probing read them from the Version's file.

use super::*;

const ALIEN_720P: &str = "Alien (1979)/Alien (1979) - 720p.mkv";

/// An ffprobe, a shell script in `bin`, that reads Alien's 1080p file as one with a titled
/// Chapter and an untitled one, its 2160p file as one with a single Chapter and any other as one
/// with none.
#[cfg(unix)]
fn ffprobe_reading_chapters(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
case "$*" in
  *1080p*) chapters='[{"start_time": "0.000000", "tags": {"title": "Opening"}},
                      {"start_time": "601.750000"}]' ;;
  *2160p*) chapters='[{"start_time": "0.000000", "tags": {"title": "Opening"}}]' ;;
  *) chapters='[]' ;;
esac
cat <<EOF
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "matroska,webm", "duration": "3600.000000"},
 "chapters": $chapters}
EOF
"#;
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        ..Ffmpeg::default()
    }
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_plan_sends_its_versions_chapters_in_order_and_none_for_fewer_than_two() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffprobe_reading_chapters(bin.path());
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    for file in [ALIEN_1080P, ALIEN_2160P, ALIEN_720P] {
        fx.write(file, b"video");
    }
    fx.scan_and_match().await;
    let versions = fx.version_ids(ALIEN).await;
    let [two, one, none] = versions.as_slice() else {
        panic!("expected three Versions: {versions:?}");
    };

    let chapters = async |version| {
        let plan = fx.plan(version).await.unwrap();
        (plan.chapters.into_iter())
            .map(|chapter| (chapter.start_ms, chapter.title))
            .collect::<Vec<_>>()
    };

    assert_eq!(
        chapters(*two).await,
        [(0, "Opening".to_owned()), (601_750, String::new())]
    );
    assert_eq!(chapters(*one).await, [], "one Chapter");
    assert_eq!(chapters(*none).await, [], "no Chapters");
}
