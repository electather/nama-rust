//! Burning a subtitle into a Transcode that keeps HDR, at reference white (ADR 0103).

use std::{collections::BTreeSet, time::Duration};

use super::{SOFTWARE, args, ffmpeg, on_default_gpu, plan, transcode_args, value_of};
use crate::ffmpeg::{
    Accel, BurnIn, Ffmpeg, FromCut, ImageSubtitle, Priority, Program, ReferenceWhite, TextLayer,
    TextSubtitle, Transcode, VideoConversion, hls_tests::system_ffmpeg, video_args,
    write_hdr_ramp_with_pgs,
};

/// What ffmpeg and ffprobe may take on the 2-second ramp.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The libass Track 2 of `alien.mkv`, drawn on a 1080p layer at 23.976 frames per second.
fn text_on_a_layer() -> BurnIn {
    BurnIn::Text {
        subtitle: TextSubtitle::Stream {
            subtitle: 2,
            track: 4,
            fonts: 0,
        },
        layer: Some(TextLayer {
            width: 1920,
            height: 1080,
            frame_rate: "23.976".to_owned(),
        }),
    }
}

#[test]
fn an_image_subtitle_burned_into_hdr_is_mapped_fitted_and_made_10_bit_in_one_scale() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Image {
            subtitle: ImageSubtitle::Stream(1),
            picture: Some((3840, 2160)),
        }),
        subtitle_white: Some(ReferenceWhite::Pq),
        ..SOFTWARE
    };

    let args = transcode_args(burned);

    let graph = value_of(&args, "-filter_complex").unwrap();
    let [picture, subtitle, overlay] = graph.split(';').collect::<Vec<_>>()[..] else {
        panic!("{graph}");
    };
    assert_eq!(picture, "[0:V:0]null[picture]");
    assert!(
        subtitle.starts_with("[0:s:1]format=rgba64le,lutrgb=r='maxval*pow("),
        "{subtitle}"
    );
    assert_eq!(subtitle.matches("scale=").count(), 1, "{subtitle}");
    assert!(
        subtitle.ends_with(
            ",scale=3840:2160:force_original_aspect_ratio=decrease:out_color_matrix=bt2020:\
             out_range=tv,format=yuva420p10le[subtitle]"
        ),
        "{subtitle}"
    );
    assert_eq!(
        overlay,
        "[picture][subtitle]overlay=(W-w)/2:(H-h)/2:format=yuv420p10[video]"
    );
}

#[test]
fn a_text_subtitle_burned_into_hdr_is_drawn_on_its_own_layer_from_where_the_run_starts() {
    let burned = VideoConversion {
        burn_in: Some(text_on_a_layer()),
        subtitle_white: Some(ReferenceWhite::Pq),
        ..SOFTWARE
    };
    let transcode = Transcode {
        audio: None,
        video: burned,
    };
    let from = FromCut {
        n: 1,
        cuts: &[6000, 12_000],
    };

    let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, &plan(), from);
    let args = args(&run.unwrap());

    let graph = value_of(&args, "-filter_complex").unwrap();
    assert!(
        graph.starts_with(
            r"[0:V:0]null[picture];alphasrc=s=1920x1080:r=23.976:start=6.000,format=yuva420p,subtitles=filename=\'alien.mkv\':si=2:alpha=1:sub2video=1,format=rgba64le,lutrgb="
        ),
        "{graph}"
    );
    assert!(
        graph.ends_with(
            ",scale=out_color_matrix=bt2020:out_range=tv,format=yuva420p10le[subtitle];\
             [picture][subtitle]overlay=eof_action=pass:repeatlast=0:format=yuv420p10[video]"
        ),
        "{graph}"
    );
    assert_eq!(value_of(&args, "-vf"), None);
}

#[test]
fn without_a_layer_a_text_subtitle_is_drawn_straight_onto_hdr_as_before() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Text {
            subtitle: TextSubtitle::Stream {
                subtitle: 2,
                track: 4,
                fonts: 0,
            },
            layer: None,
        }),
        subtitle_white: Some(ReferenceWhite::Pq),
        ..SOFTWARE
    };

    let args = transcode_args(burned);

    assert_eq!(
        value_of(&args, "-vf"),
        Some(r"subtitles=filename=\'alien.mkv\':si=2")
    );
}

/// What a viewer gets of a ramp tagged with `transfer` with `burn_in` burned in, mapped to
/// `white`: the stream's pixel format and transfer as ffprobe names them, and the 10-bit luma of
/// its last frame, half a second in, row by row.
async fn burned_into(
    ffmpeg: &Ffmpeg,
    transfer: &str,
    burn_in: BurnIn,
    white: ReferenceWhite,
) -> (String, Vec<u16>) {
    let dir = tempfile::tempdir().unwrap();
    let ramp = dir.path().join("ramp.mkv");
    write_hdr_ramp_with_pgs(&ramp, transfer).unwrap();
    let out = dir.path().join("burned.mp4");
    let video = VideoConversion {
        hardware: Some(on_default_gpu(Accel::VideoToolbox)),
        hevc: true,
        burn_in: Some(burn_in),
        subtitle_white: Some(white),
        ..SOFTWARE
    };
    // VideoToolbox's frames stay in memory, so its filters run anywhere; lossless libx265 stands
    // in for the GPU's HEVC encoder, which CI lacks, so the filters' codes reach the file.
    let video = video_args(ramp.to_str().unwrap(), &video, 0).video;
    let mut args: Vec<&str> = vec!["-v", "error", "-i", ramp.to_str().unwrap()];
    args.extend(video.iter().map(String::as_str));
    // Not the first frame, which goes out before the subtitle's.
    args.extend(["-frames:v", "12", "-c:v", "libx265"]);
    args.extend(["-x265-params", "lossless=1:log-level=error", "-f", "mp4"]);
    args.push(out.to_str().unwrap());
    ffmpeg
        .run(Program::Ffmpeg, &args, TIMEOUT, Priority::Normal)
        .await
        .unwrap();

    let entries = [
        "-show_entries",
        "stream=pix_fmt,color_transfer",
        "-of",
        "csv=p=0",
    ];
    let mut probe = vec!["-v", "error", "-select_streams", "v"];
    probe.extend(entries);
    probe.push(out.to_str().unwrap());
    let stream = ffmpeg.run(Program::Ffprobe, &probe, TIMEOUT, Priority::Normal);
    let stream = String::from_utf8(stream.await.unwrap()).unwrap();
    let raw = ["-pix_fmt", "yuv420p10le", "-f", "rawvideo", "-"];
    let mut decode = vec!["-v", "error", "-i", out.to_str().unwrap()];
    decode.extend(raw);
    let frame = ffmpeg.run(Program::Ffmpeg, &decode, TIMEOUT, Priority::Normal);
    let frames = frame.await.unwrap();
    // Luma, then each chroma plane a quarter its size, 2 bytes a code.
    let last = frames.rchunks_exact(320 * 240 * 3).next().unwrap();
    let luma = (last.as_chunks::<2>().0.iter())
        .take(320 * 240)
        .map(|&code| u16::from_le_bytes(code))
        .collect();
    (stream.trim().to_owned(), luma)
}

/// The PGS Track of [`write_hdr_ramp_with_pgs`], fitted to its picture.
const PGS: BurnIn = BurnIn::Image {
    subtitle: ImageSubtitle::Stream(0),
    picture: Some((320, 240)),
};

/// The luma of the square every subtitle here draws at 100, 100.
fn square(luma: &[u16]) -> u16 {
    luma[108 * 320 + 108]
}

#[tokio::test]
async fn a_pgs_burned_into_hdr10_is_at_203_nits_and_the_picture_stays_10_bit_pq_unbanded() {
    let ffmpeg = system_ffmpeg();

    let (stream, luma) = burned_into(&ffmpeg, "smpte2084", PGS, ReferenceWhite::Pq).await;

    assert_eq!(stream, "yuv420p10le,smpte2084");
    assert!(
        square(&luma).abs_diff(573) <= 10,
        "PQ's 203 nits is 573, not {}",
        square(&luma)
    );
    let row: BTreeSet<u16> = luma[10 * 320..11 * 320].iter().copied().collect();
    assert!(row.len() > 256, "{} codes along the ramp", row.len());
}

#[tokio::test]
async fn a_pgs_burned_into_hlg_is_at_75_percent_signal() {
    let ffmpeg = system_ffmpeg();

    let (stream, luma) = burned_into(&ffmpeg, "arib-std-b67", PGS, ReferenceWhite::Hlg).await;

    assert_eq!(stream, "yuv420p10le,arib-std-b67");
    assert!(
        square(&luma).abs_diff(721) <= 10,
        "HLG's 75% is 721, not {}",
        square(&luma)
    );
}

#[tokio::test]
async fn a_text_subtitle_burned_into_hdr10_on_its_own_layer_is_at_203_nits() {
    let ffmpeg = system_ffmpeg();
    if !ffmpeg.has_subtitle_layer().await.unwrap() {
        // CI's jellyfin-ffmpeg has them, and its job fails without them.
        eprintln!("skipped: the ffmpeg on PATH lacks jellyfin-ffmpeg's alphasrc");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ass = dir.path().join("square.ass");
    std::fs::write(&ass, WHITE_SQUARE_ASS).unwrap();
    let text = BurnIn::Text {
        subtitle: TextSubtitle::Sidecar {
            path: ass.to_str().unwrap().to_owned(),
            charset: None,
        },
        layer: Some(TextLayer {
            width: 320,
            height: 240,
            frame_rate: "24".to_owned(),
        }),
    };

    let (stream, luma) = burned_into(&ffmpeg, "smpte2084", text, ReferenceWhite::Pq).await;

    assert_eq!(stream, "yuv420p10le,smpte2084");
    assert!(
        square(&luma).abs_diff(573) <= 10,
        "PQ's 203 nits is 573, not {}",
        square(&luma)
    );
}

/// An ASS subtitle on a 320×240 picture drawing a 16×16 white square at 100, 100 for 19 seconds.
const WHITE_SQUARE_ASS: &str = "[Script Info]\nScriptType: v4.00+\nPlayResX: 320\nPlayResY: 240\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, OutlineColour, BackColour, \
BorderStyle, Outline, Shadow, Alignment\nStyle: Default,Arial,20,&H00FFFFFF,&H00000000,\
&H00000000,1,0,0,7\n\n[Events]\nFormat: Layer, Start, End, Style, Text\nDialogue: 0,0:00:00.00,\
0:00:19.00,Default,{\\pos(100,100)\\p1}m 0 0 l 16 0 16 16 0 16{\\p0}\n";
