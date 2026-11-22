//! Converting Dolby Vision with no Base Layer, and tone-mapping with `tonemapx` (ADR 0102).

use std::{path::Path, time::Duration};

use super::{
    Accel, FromCut, Keyframe, Priority, Program, Remux, ToneMapper, Transcode, VideoConversion,
    VideoTag,
    hls_run_tests::{LIMIT_720P, SOFTWARE, on_default_gpu, transcode_args, value_of},
    hls_tests::{remux_with, system_ffmpeg},
};

/// A second of Dolby Vision profile 5, made as `tests/fixtures/README.md` says.
const CLIP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/dolby-vision-5.mp4"
);

const RESHAPED: Option<ToneMapper> = Some(ToneMapper::Tonemapx { dolby_vision: true });

/// What ffmpeg and ffprobe may take on the clip.
const TIMEOUT: Duration = Duration::from_secs(60);

#[test]
fn dolby_vision_5_is_tagged_as_pq_scaled_then_reshaped_and_tone_mapped_by_tonemapx() {
    let video = VideoConversion {
        tone_map: RESHAPED,
        limit: LIMIT_720P,
        ..SOFTWARE
    };

    let args = transcode_args(video);

    assert_eq!(
        value_of(&args, "-vf"),
        Some(
            "setparams=range=tv:color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc,\
             scale=1280:720,tonemapx=tonemap=bt2390:peak=100:apply_dovi=1,\
             zscale=p=bt709:t=bt709:m=bt709:r=tv,format=yuv420p"
        )
    );
}

#[test]
fn hdr_is_tone_mapped_by_tonemapx_without_applying_dolby_vision() {
    let video = VideoConversion {
        tone_map: Some(ToneMapper::Tonemapx {
            dolby_vision: false,
        }),
        ..SOFTWARE
    };

    let args = transcode_args(video);

    assert_eq!(
        value_of(&args, "-vf"),
        Some(
            "setparams=color_primaries=bt2020:colorspace=bt2020nc,\
             tonemapx=tonemap=bt2390:peak=100:apply_dovi=0,\
             zscale=p=bt709:t=bt709:m=bt709:r=tv,format=yuv420p"
        )
    );
}

#[test]
fn a_dolby_vision_5_transcode_on_hardware_decodes_in_software_and_encodes_on_the_gpu() {
    for accel in Accel::ALL {
        let video = VideoConversion {
            hardware: Some(on_default_gpu(accel)),
            tone_map: RESHAPED,
            ..SOFTWARE
        };

        let args = transcode_args(video);

        assert!(
            !args.iter().any(|arg| arg.starts_with("-hwaccel")),
            "{args:?}"
        );
        assert_eq!(value_of(&args, "-c:v"), Some(accel.encoder()));
        let filters = value_of(&args, "-vf").unwrap();
        assert!(filters.contains("apply_dovi=1"), "{filters}");
        if filters.contains("hwupload") {
            assert_eq!(value_of(&args, "-filter_hw_device"), Some("nama"));
        }
    }
}

#[tokio::test]
async fn a_dolby_vision_5_remux_is_tagged_dvh1_and_keeps_its_dolby_vision_record() {
    let ffmpeg = system_ffmpeg();
    let dir = tempfile::tempdir().unwrap();
    let remux = Remux {
        audio: None,
        video_tag: Some(VideoTag::Dvh1),
    };
    let first = Keyframe {
        n: 0,
        ms: 0,
        previous_ms: None,
    };

    remux_with(&ffmpeg, Path::new(CLIP), &remux, dir.path(), first, 0).await;

    let init = dir.path().join("init-0.mp4");
    let probed = probe(&ffmpeg, &init, &["-show_streams"]).await;
    assert!(probed.contains("codec_tag_string=dvh1"), "{probed}");
    assert!(probed.contains("dv_profile=5"), "{probed}");
    assert!(
        probed.contains("dv_bl_signal_compatibility_id=0"),
        "{probed}"
    );
}

#[tokio::test]
async fn a_dolby_vision_5_transcode_is_bt709_sdr_in_the_colours_its_rpu_reshapes_it_to() {
    let ffmpeg = system_ffmpeg();
    // CI's jellyfin-ffmpeg has it, and its job fails without it.
    if !ffmpeg.has_tonemapx().await.unwrap() {
        eprintln!("skipped: the ffmpeg on PATH lacks jellyfin-ffmpeg's tonemapx");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let transcode = Transcode {
        audio: None,
        video: VideoConversion {
            tone_map: RESHAPED,
            ..SOFTWARE
        },
    };
    let from = FromCut { n: 0, cuts: &[0] };

    let run = ffmpeg
        .transcode(CLIP, &transcode, dir.path(), from)
        .unwrap();
    let output = run.spawn().unwrap().wait_with_output().await.unwrap();

    assert!(output.status.success(), "{output:?}");
    // The init segment and the one file a cut at 0 alone makes, as a player joins them.
    let whole = dir.path().join("whole.mp4");
    let init = std::fs::read(dir.path().join("init-0.mp4")).unwrap();
    let file = std::fs::read(dir.path().join("ffmpeg-0-0.m4s")).unwrap();
    std::fs::write(&whole, [init, file].concat()).unwrap();
    let stream = probe(&ffmpeg, &whole, &["-show_streams"]).await;
    for tag in [
        "color_range=tv",
        "color_space=bt709",
        "color_transfer=bt709",
        "color_primaries=bt709",
    ] {
        assert!(stream.contains(tag), "{tag}: {stream}");
    }
    let frame = probe(
        &ffmpeg,
        &whole,
        &["-show_frames", "-read_intervals", "%+#1"],
    )
    .await;
    for side_data in ["Dolby Vision", "DOVI", "Mastering display", "Content light"] {
        let probed = format!("{stream}{frame}");
        assert!(!probed.contains(side_data), "{side_data}: {probed}");
    }
    // Read as plain PQ, without its RPU, its colours come out purple and green.
    let unreshaped = "setparams=range=tv:color_primaries=bt2020:color_trc=smpte2084:\
                      colorspace=bt2020nc,tonemapx=tonemap=bt2390:peak=100:apply_dovi=0,\
                      zscale=p=bt709:t=bt709:m=bt709:r=tv,format=yuv420p";
    let (reshaped, unreshaped) = (
        first_frame_rgb(&ffmpeg, &whole, None).await,
        first_frame_rgb(&ffmpeg, Path::new(CLIP), Some(unreshaped)).await,
    );
    assert_eq!(reshaped.len(), unreshaped.len());
    let apart: u64 = (reshaped.iter().zip(&unreshaped))
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    let mean_apart = apart / u64::try_from(reshaped.len()).unwrap();
    assert!(
        mean_apart >= 8,
        "the RPU moved each colour by {mean_apart} on average"
    );
}

/// What ffprobe prints of the file at `path` given `args`, in its default format.
async fn probe(ffmpeg: &super::Ffmpeg, path: &Path, args: &[&str]) -> String {
    let mut args: Vec<&str> = args.to_vec();
    args.extend(["-select_streams", "v", path.to_str().unwrap()]);
    let printed = ffmpeg.run(Program::Ffprobe, &args, TIMEOUT, Priority::Normal);
    String::from_utf8(printed.await.unwrap()).unwrap()
}

/// The first picture of the file at `path`, through `filters` if given, as 8-bit RGB.
async fn first_frame_rgb(ffmpeg: &super::Ffmpeg, path: &Path, filters: Option<&str>) -> Vec<u8> {
    let mut args = vec!["-i", path.to_str().unwrap(), "-frames:v", "1"];
    args.extend(
        filters
            .map(|filters| ["-vf", filters])
            .into_iter()
            .flatten(),
    );
    args.extend(["-pix_fmt", "rgb24", "-f", "rawvideo", "-"]);
    let printed = ffmpeg.run(Program::Ffmpeg, &args, TIMEOUT, Priority::Normal);
    printed.await.unwrap()
}
