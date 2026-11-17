use std::path::Path;

use super::{
    Accel, BurnIn, Ffmpeg, FoundProgram, FromCut, Hardware, HlsRun, Keyframe, Remux, RemuxAudio,
    TextSubtitle, ToneMapper, Transcode, TranscodeAudio, VideoConversion, VideoLimit,
};

/// An ffmpeg at `version` that isn't on this machine, which building arguments never runs.
fn ffmpeg(version: &str) -> Ffmpeg {
    Ffmpeg {
        ffmpeg: Ok(FoundProgram {
            path: "ffmpeg".into(),
            version: version.to_owned(),
        }),
        ..Ffmpeg::default()
    }
}

fn args(run: &HlsRun) -> Vec<String> {
    (run.0.as_std().get_args())
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

/// A Transcode's video converted in software, with nothing else asked of it.
const SOFTWARE: VideoConversion = VideoConversion {
    hardware: None,
    tone_map: None,
    limit: None,
    burn_in: None,
    in_band_headers: false,
};

/// The filters that tone-map HDR to SDR in software, pinned apart from the `TONE_MAP` they
/// test.
const ZSCALE_TONE_MAP: &str = "setparams=color_primaries=bt2020:colorspace=bt2020nc,\
                               zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,\
                               tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,\
                               format=yuv420p";

/// The arguments for a run of `remux` of `alien.mkv` from `from`, with ffmpeg 7.1.1.
fn remux_args(remux: Remux, from: Keyframe) -> Vec<String> {
    let run = ffmpeg("7.1.1").remux("alien.mkv", &remux, Path::new("plan"), from);
    args(&run.unwrap())
}

/// The arguments for a run of a silent Transcode of `alien.mkv` whose video `video` converts,
/// from its first cut of three, with ffmpeg 7.1.1.
fn transcode_args(video: VideoConversion) -> Vec<String> {
    let transcode = Transcode { audio: None, video };
    let from = FromCut {
        n: 0,
        cuts: &[0, 6000, 12_000],
    };
    let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, Path::new("plan"), from);
    args(&run.unwrap())
}

/// What `args` gives the first `option`, if anything.
fn value_of<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == option)?;
    args.get(at + 1).map(String::as_str)
}

/// What `args` gives the input's `-ss`, if anything.
fn input_seek(args: &[String]) -> Option<&str> {
    let input = args.iter().position(|arg| arg == "-i").unwrap();
    value_of(&args[..input], "-ss")
}

const fn keyframe(n: usize, ms: i64, previous_ms: Option<i64>) -> Keyframe {
    Keyframe { n, ms, previous_ms }
}

/// `name` in the folder the tests write to, as ffmpeg is given it on this platform.
fn in_plan(name: &str) -> String {
    Path::new("plan").join(name).display().to_string()
}

#[test]
fn a_remux_copies_the_video_and_converts_the_audio_from_its_keyframe_on() {
    let remux = Remux {
        audio: Some(RemuxAudio {
            track: 1,
            aac_channels: Some(2),
        }),
        hevc: false,
    };

    let args = remux_args(remux, keyframe(3, 6000, Some(4000)));

    let expected = [
        "-v",
        "error",
        "-nostdin",
        "-ss",
        "5.917",
        "-itsoffset",
        "1.000",
        "-i",
        "alien.mkv",
        "-copyts",
        "-start_at_zero",
        "-ss",
        "6.500",
        "-output_ts_offset",
        "6.500",
        "-map",
        "0:V:0",
        "-map",
        "0:a:1",
        "-c",
        "copy",
        "-c:a",
        "aac",
        "-ac",
        "2",
        "-af",
        "atrim=start=6.941",
        "-f",
        "hls",
        "-hls_segment_type",
        "fmp4",
        "-hls_time",
        "0.001",
        "-hls_playlist_type",
        "vod",
        "-hls_flags",
        "temp_file",
        "-hls_fmp4_init_filename",
        "init-3.mp4",
        "-hls_segment_options",
        "movflags=+frag_discont",
        "-start_number",
        "3",
        "-hls_segment_filename",
        &in_plan("%d.m4s"),
        &in_plan("ffmpeg.m3u8"),
    ];
    assert_eq!(args, expected);
}

#[test]
fn a_transcode_converts_the_video_and_audio_from_its_cut_on() {
    let transcode = Transcode {
        audio: Some(TranscodeAudio {
            track: 0,
            aac_channels: 6,
        }),
        video: SOFTWARE,
    };
    let from = FromCut {
        n: 2,
        cuts: &[12_000, 18_000],
    };

    let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, Path::new("plan"), from);
    let args = args(&run.unwrap());

    let expected = [
        "-v",
        "error",
        "-nostdin",
        "-ss",
        "12.000",
        "-i",
        "alien.mkv",
        "-copyts",
        "-start_at_zero",
        "-map",
        "0:V:0",
        "-map",
        "0:a:0",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-pix_fmt",
        "yuv420p",
        "-x264-params",
        "keyint=infinite:scenecut=0",
        "-force_key_frames",
        "12.000,18.000",
        "-c:a",
        "aac",
        "-ac",
        "6",
        "-output_ts_offset",
        "1.000",
        "-f",
        "hls",
        "-hls_segment_type",
        "fmp4",
        "-hls_time",
        "0.001",
        "-hls_playlist_type",
        "vod",
        "-hls_flags",
        "temp_file",
        "-hls_fmp4_init_filename",
        "init-2.mp4",
        "-hls_segment_options",
        "movflags=+frag_discont",
        "-start_number",
        "2",
        "-hls_segment_filename",
        &in_plan("%d.m4s"),
        &in_plan("ffmpeg.m3u8"),
    ];
    assert_eq!(args, expected);
}

#[test]
fn an_hevc_remux_tags_its_copied_video_hvc1() {
    let hevc = Remux {
        audio: None,
        hevc: true,
    };

    let args = remux_args(hevc, keyframe(0, 0, None));

    let copy = args.windows(2).position(|pair| pair == ["-c", "copy"]);
    let after_copy = copy.map(|at| &args[at + 2..at + 4]);
    assert_eq!(after_copy, Some(&["-tag:v", "hvc1"].map(str::to_owned)[..]));
}

#[test]
fn a_remux_without_audio_seeks_halfway_back_to_the_keyframe_before_and_at_most_half_a_second() {
    let seek_from = |from| {
        input_seek(&remux_args(
            Remux {
                audio: None,
                hevc: false,
            },
            from,
        ))
        .map(str::to_owned)
    };

    assert_eq!(
        seek_from(keyframe(3, 6000, Some(5600))).as_deref(),
        Some("5.800")
    );
    assert_eq!(
        seek_from(keyframe(3, 6000, Some(2000))).as_deref(),
        Some("5.500")
    );
    assert_eq!(
        seek_from(keyframe(0, 400, None)),
        None,
        "from the file's start"
    );
}

#[test]
fn a_remux_past_the_first_keyframe_starts_its_audio_at_the_keyframes_decode_time() {
    let audio = |aac_channels| Remux {
        audio: Some(RemuxAudio {
            track: 0,
            aac_channels,
        }),
        hevc: false,
    };
    let third = keyframe(3, 6000, Some(4000));

    let copied = remux_args(audio(None), third);
    let converted = remux_args(audio(Some(2)), third);
    let silent = remux_args(
        Remux {
            audio: None,
            hevc: false,
        },
        third,
    );

    assert_eq!(input_seek(&copied), Some("5.917"));
    assert_eq!(value_of(&copied, "-copypriorss:a"), Some("0"));
    assert_eq!(value_of(&copied, "-af"), None);
    assert_eq!(input_seek(&converted), Some("5.917"));
    assert_eq!(value_of(&converted, "-af"), Some("atrim=start=6.941"));
    assert_eq!(value_of(&converted, "-copypriorss:a"), None);
    assert_eq!(input_seek(&silent), Some("5.500"));
    assert_eq!(value_of(&silent, "-copypriorss:a"), None);
    assert_eq!(value_of(&silent, "-af"), None);
}

#[test]
fn a_remux_from_the_first_keyframe_keeps_the_audio_from_the_files_start() {
    let remux = Remux {
        audio: Some(RemuxAudio {
            track: 0,
            aac_channels: None,
        }),
        hevc: false,
    };

    let args = remux_args(remux, keyframe(0, 0, None));

    assert_eq!(input_seek(&args), None);
    assert_eq!(value_of(&args, "-copypriorss:a"), None);
}

#[test]
fn hdr_is_tone_mapped_to_sdr_in_software_with_zscale() {
    let tone_mapped = VideoConversion {
        tone_map: Some(ToneMapper::Software),
        ..SOFTWARE
    };

    let args = transcode_args(tone_mapped);

    assert_eq!(value_of(&args, "-vf"), Some(ZSCALE_TONE_MAP));
}

/// A playback limit of 720p at 4 Mb/s.
const LIMIT_720P: Option<VideoLimit> = Some(VideoLimit {
    size: Some((1280, 720)),
    bits_per_second: 4_000_000,
});

#[test]
fn a_limited_tone_map_with_nothing_burned_in_scales_down_first() {
    let tone_mapped = VideoConversion {
        tone_map: Some(ToneMapper::Software),
        limit: LIMIT_720P,
        ..SOFTWARE
    };
    let on_vaapi = VideoConversion {
        hardware: Some(on_default_gpu(Accel::Vaapi)),
        ..tone_mapped.clone()
    };

    let args = transcode_args(tone_mapped);
    let vaapi_args = transcode_args(on_vaapi);

    let scaled_first = format!("scale=1280:720,{ZSCALE_TONE_MAP}");
    assert_eq!(value_of(&args, "-vf"), Some(scaled_first.as_str()));
    let uploaded_last = format!("{scaled_first},format=nv12,hwupload");
    assert_eq!(value_of(&vaapi_args, "-vf"), Some(uploaded_last.as_str()));
}

#[test]
fn a_limited_tone_map_with_a_burn_in_scales_down_after_the_subtitle() {
    let burned = |burn_in| VideoConversion {
        tone_map: Some(ToneMapper::Software),
        limit: LIMIT_720P,
        burn_in: Some(burn_in),
        ..SOFTWARE
    };

    let image = transcode_args(burned(BurnIn::Image {
        subtitle: 1,
        picture: None,
    }));
    let text = transcode_args(burned(BurnIn::Text {
        subtitle: TextSubtitle::Stream {
            subtitle: 2,
            track: 4,
            fonts: 1,
        },
    }));

    let graph = format!(
        "[0:V:0]{ZSCALE_TONE_MAP}[picture];[0:s:1]null[subtitle];\
         [picture][subtitle]overlay=(W-w)/2:(H-h)/2,scale=1280:720[video]"
    );
    assert_eq!(value_of(&image, "-filter_complex"), Some(graph.as_str()));
    let filters =
        format!(r"{ZSCALE_TONE_MAP},subtitles=filename=\'alien.mkv\':si=2,scale=1280:720");
    assert_eq!(value_of(&text, "-vf"), Some(filters.as_str()));
}

#[test]
fn a_playback_limit_scales_the_picture_and_caps_the_bitrate() {
    let limited = VideoConversion {
        limit: Some(VideoLimit {
            size: Some((1280, 720)),
            bits_per_second: 4_000_000,
        }),
        ..SOFTWARE
    };

    let args = transcode_args(limited);

    assert_eq!(value_of(&args, "-vf"), Some("scale=1280:720"));
    assert_eq!(value_of(&args, "-b:v"), Some("4000000"));
    assert_eq!(value_of(&args, "-maxrate"), Some("4000000"));
    assert_eq!(value_of(&args, "-bufsize"), Some("8000000"));
}

#[test]
fn an_image_subtitle_is_burned_in_fitted_and_centred_over_the_picture() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Image {
            subtitle: 1,
            picture: Some((1920, 1080)),
        }),
        ..SOFTWARE
    };

    let args = transcode_args(burned);

    let graph = "[0:V:0]null[picture];\
                 [0:s:1]scale=1920:1080:force_original_aspect_ratio=decrease[subtitle];\
                 [picture][subtitle]overlay=(W-w)/2:(H-h)/2[video]";
    assert_eq!(value_of(&args, "-filter_complex"), Some(graph));
    assert_eq!(value_of(&args, "-map"), Some("[video]"));
}

#[test]
fn a_run_that_seeks_reads_its_image_subtitle_from_an_input_that_doesnt() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Image {
            subtitle: 1,
            picture: None,
        }),
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
    let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, Path::new("plan"), from);
    let args = args(&run.unwrap());

    let graph = value_of(&args, "-filter_complex").unwrap();
    assert!(graph.contains("[1:s:1]"), "{graph}");
    assert_eq!(args.iter().filter(|arg| *arg == "-i").count(), 2);
    assert_eq!(input_seek(&args), Some("6.000"));
}

#[test]
fn a_text_subtitle_is_burned_in_from_the_file_by_libass() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Text {
            subtitle: TextSubtitle::Stream {
                subtitle: 2,
                track: 4,
                fonts: 1,
            },
        }),
        ..SOFTWARE
    };

    let args = transcode_args(burned);

    assert_eq!(
        value_of(&args, "-vf"),
        Some(r"subtitles=filename=\'alien.mkv\':si=2")
    );
}

#[test]
fn a_sidecar_text_subtitle_is_burned_in_from_its_own_file_by_libass() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Text {
            subtitle: TextSubtitle::Sidecar("Alien.en.ass".to_owned()),
        }),
        ..SOFTWARE
    };

    let args = transcode_args(burned);

    assert_eq!(
        value_of(&args, "-vf"),
        Some(r"subtitles=filename=\'Alien.en.ass\'")
    );
}

#[test]
fn a_cached_text_subtitle_is_burned_in_from_its_ass_with_its_fonts_by_libass() {
    let burned = VideoConversion {
        burn_in: Some(BurnIn::Text {
            subtitle: TextSubtitle::Cached {
                ass: "cache/4.ass".to_owned(),
                fonts: "cache/fonts".to_owned(),
            },
        }),
        ..SOFTWARE
    };

    let args = transcode_args(burned);

    assert_eq!(
        value_of(&args, "-vf"),
        Some(r"subtitles=filename=\'cache/4.ass\':fontsdir=\'cache/fonts\'")
    );
}

#[test]
fn audio_is_converted_to_aac_with_its_channels_or_else_copied() {
    let audio = |aac_channels| Remux {
        audio: Some(RemuxAudio {
            track: 0,
            aac_channels,
        }),
        hevc: false,
    };
    let first = keyframe(0, 0, None);

    let converted = remux_args(audio(Some(6)), first);
    let copied = remux_args(audio(None), first);

    assert_eq!(value_of(&converted, "-c:a"), Some("aac"));
    assert_eq!(value_of(&converted, "-ac"), Some("6"));
    assert_eq!(value_of(&copied, "-c:a"), None);
    assert_eq!(value_of(&copied, "-ac"), None);
}

#[test]
fn a_transcode_forces_a_keyframe_at_each_cut_from_its_first() {
    let args = transcode_args(SOFTWARE);

    assert_eq!(
        value_of(&args, "-force_key_frames"),
        Some("0.000,6.000,12.000")
    );
}

/// `accel` on the GPU ffmpeg picks.
const fn on_default_gpu(accel: Accel) -> Hardware {
    Hardware {
        accel,
        gpu: None,
        tone_maps: false,
    }
}

/// What a Transcode on `accel`, tone-mapping with `tone_map`, starts ffmpeg with up to the
/// keyframes it forces, which the software tests pin from there on, as words split at spaces.
/// `None` for tone-mapping on the GPU of an API with no filter for it, which the plan never
/// asks for: only an API with one can pass its tone-map test at startup.
fn pinned_hardware_args(accel: Accel, tone_map: Option<ToneMapper>) -> Option<String> {
    const IN_BAND_HEADERS: &str = "-bsf:v dump_extra=freq=keyframe -tag:v avc3";
    let args = match (accel, tone_map) {
        (Accel::Nvenc, None) => format!(
            "-v error -nostdin -init_hw_device cuda=nama -filter_hw_device nama \
             -hwaccel cuda -hwaccel_device nama -hwaccel_output_format cuda \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|cuda,hwupload,scale_cuda=format=nv12 \
             -c:v h264_nvenc -g 10000000 -forced-idr 1 -no-scenecut 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Nvenc, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -init_hw_device cuda=nama -filter_hw_device nama \
             -hwaccel cuda -hwaccel_device nama \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12 \
             -c:v h264_nvenc -g 10000000 -forced-idr 1 -no-scenecut 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Qsv, None) => format!(
            "-v error -nostdin -init_hw_device qsv=nama -filter_hw_device nama \
             -hwaccel qsv -hwaccel_device nama -hwaccel_output_format qsv \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|qsv,hwupload=extra_hw_frames=64,scale_qsv=format=nv12 \
             -c:v h264_qsv -g 10000000 -forced_idr 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Qsv, Some(ToneMapper::Hardware)) => format!(
            "-v error -nostdin -init_hw_device qsv=nama -filter_hw_device nama \
             -hwaccel qsv -hwaccel_device nama -hwaccel_output_format qsv \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|qsv,hwupload=extra_hw_frames=64,\
             vpp_qsv=tonemap=1:format=nv12,scale_qsv=format=nv12 \
             -c:v h264_qsv -g 10000000 -forced_idr 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Qsv, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -init_hw_device qsv=nama -filter_hw_device nama \
             -hwaccel qsv -hwaccel_device nama \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12,hwupload=extra_hw_frames=64 \
             -c:v h264_qsv -g 10000000 -forced_idr 1 {IN_BAND_HEADERS}"
        ),
        (Accel::Vaapi, None) => format!(
            "-v error -nostdin -init_hw_device vaapi=nama -filter_hw_device nama \
             -hwaccel vaapi -hwaccel_device nama -hwaccel_output_format vaapi \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|vaapi,hwupload,scale_vaapi=format=nv12 \
             -c:v h264_vaapi -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::Vaapi, Some(ToneMapper::Hardware)) => format!(
            "-v error -nostdin -init_hw_device vaapi=nama -filter_hw_device nama \
             -hwaccel vaapi -hwaccel_device nama -hwaccel_output_format vaapi \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf format=nv12|p010le|vaapi,hwupload,\
             tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709,scale_vaapi=format=nv12 \
             -c:v h264_vaapi -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::Vaapi, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -init_hw_device vaapi=nama -filter_hw_device nama \
             -hwaccel vaapi -hwaccel_device nama \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12,hwupload \
             -c:v h264_vaapi -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::VideoToolbox, None) => format!(
            "-v error -nostdin -hwaccel videotoolbox \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 -vf format=nv12 \
             -c:v h264_videotoolbox -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::VideoToolbox, Some(ToneMapper::Software)) => format!(
            "-v error -nostdin -hwaccel videotoolbox \
             -i alien.mkv -copyts -start_at_zero -map 0:V:0 \
             -vf {ZSCALE_TONE_MAP},format=nv12 \
             -c:v h264_videotoolbox -g 10000000 {IN_BAND_HEADERS}"
        ),
        (Accel::Nvenc | Accel::VideoToolbox, Some(ToneMapper::Hardware)) => return None,
    };
    Some(args)
}

#[test]
fn a_hardware_transcode_runs_with_its_apis_arguments_for_each_tone_mapper() {
    let tone_mappers = [None, Some(ToneMapper::Software), Some(ToneMapper::Hardware)];
    let cases =
        (Accel::ALL.into_iter()).flat_map(|accel| tone_mappers.map(|tone_map| (accel, tone_map)));
    for (accel, tone_map) in cases {
        let Some(expected) = pinned_hardware_args(accel, tone_map) else {
            assert!(
                !accel.has_tone_map_filter(),
                "{accel:?} can tone-map on its GPU"
            );
            continue;
        };
        let video = VideoConversion {
            hardware: Some(on_default_gpu(accel)),
            tone_map,
            // As the plan sets it on hardware, so software can carry on if the hardware fails.
            in_band_headers: true,
            ..SOFTWARE
        };

        let args = transcode_args(video);

        let forced = (args.iter())
            .position(|arg| arg == "-force_key_frames")
            .expect("a Transcode forces keyframes");
        assert_eq!(
            args[..forced],
            expected.split(' ').collect::<Vec<_>>(),
            "{accel:?} tone-mapped by {tone_map:?}"
        );
    }
}

#[test]
fn a_playback_limit_on_hardware_scales_the_picture_on_its_gpu_if_it_can() {
    for accel in Accel::ALL {
        let limited = VideoConversion {
            hardware: Some(on_default_gpu(accel)),
            limit: Some(VideoLimit {
                size: Some((1280, 720)),
                bits_per_second: 4_000_000,
            }),
            in_band_headers: true,
            ..SOFTWARE
        };

        let args = transcode_args(limited);

        let filters = match accel {
            Accel::Nvenc => "format=nv12|p010le|cuda,hwupload,scale_cuda=w=1280:h=720:format=nv12",
            Accel::Qsv => {
                "format=nv12|p010le|qsv,hwupload=extra_hw_frames=64,\
                 scale_qsv=w=1280:h=720:format=nv12"
            }
            Accel::Vaapi => {
                "format=nv12|p010le|vaapi,hwupload,scale_vaapi=w=1280:h=720:format=nv12"
            }
            Accel::VideoToolbox => "scale=1280:720,format=nv12",
        };
        assert_eq!(value_of(&args, "-vf"), Some(filters), "{accel:?}");
    }
}

#[test]
fn ffmpeg_before_5_0_is_given_segment_options_by_their_old_name() {
    let segment_options = |version| {
        let silent = Remux {
            audio: None,
            hevc: false,
        };
        let run = ffmpeg(version).remux(
            "alien.mkv",
            &silent,
            Path::new("plan"),
            keyframe(0, 0, None),
        );
        let args = args(&run.unwrap());
        ["-hls_ts_options", "-hls_segment_options"]
            .into_iter()
            .filter(|option| value_of(&args, option) == Some("movflags=+frag_discont"))
            .collect::<Vec<_>>()
    };

    assert_eq!(segment_options("4.4.2"), ["-hls_ts_options"]);
    assert_eq!(segment_options("n5.0"), ["-hls_segment_options"]);
    assert_eq!(
        segment_options("N-113021-g5b3fc5d"),
        ["-hls_segment_options"]
    );
}

#[test]
fn a_path_in_a_filter_option_survives_both_of_ffmpegs_unescapings() {
    let path = r"/films/it's: a [test], x; y\z.mkv";

    assert_eq!(
        super::filter_value(path),
        r"\'/films/it\'\\\'\'s: a \[test\]\, x\; y\\z.mkv\'"
    );
}
