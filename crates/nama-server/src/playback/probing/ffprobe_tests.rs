use super::{Starts, parse_probe};
use crate::playback::tracks::{BaseLayer, Hdr, Probe, Track, TrackKind};

/// Cut down from ffprobe 7.0's output for a Blu-ray remux with cover art.
const BLU_RAY_REMUX: &str = r#"{
        "streams": [
            {"codec_name": "hevc", "codec_type": "video", "width": 3840, "height": 1600,
             "avg_frame_rate": "24000/1001", "disposition": {"default": 1, "forced": 0}},
            {"codec_name": "truehd", "codec_type": "audio", "channels": 8,
             "disposition": {"default": 1, "forced": 0},
             "tags": {"language": "eng", "title": "TrueHD Atmos 7.1"}},
            {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle",
             "disposition": {"default": 0, "forced": 1}, "tags": {"language": "fre"}},
            {"codec_name": "subrip", "codec_type": "subtitle",
             "disposition": {"default": 0, "forced": 0, "hearing_impaired": 1},
             "tags": {"language": "eng"}},
            {"codec_name": "mjpeg", "codec_type": "video", "width": 600, "height": 900,
             "avg_frame_rate": "0/0", "disposition": {"attached_pic": 1}},
            {"codec_type": "attachment", "tags": {"filename": "font.ttf"}}
        ],
        "format": {"format_name": "matroska,webm", "duration": "7299.4565", "bit_rate": "61734512"}
    }"#;

fn track(codec: &str, language: Option<&str>, default: bool, kind: TrackKind) -> Track {
    Track {
        codec: codec.to_owned(),
        language: language.map(str::to_owned),
        title: None,
        default,
        forced: false,
        sdh: false,
        kind,
    }
}

#[test]
fn ffprobe_output_reads_as_tracks_without_cover_art_and_its_attachment_as_a_font() {
    let (probe, _) = parse_probe(BLU_RAY_REMUX.as_bytes()).unwrap();

    let Some(TrackKind::Video { frame_rate, .. }) = probe.tracks.first().map(|t| &t.kind) else {
        panic!("no video Track first: {probe:?}");
    };
    assert!(frame_rate.is_some_and(|fps| (fps - 23.976).abs() < 0.001));
    let ntsc = frame_rate.unwrap();
    assert_eq!(
        probe,
        Probe {
            container: "matroska,webm".to_owned(),
            duration_ms: Some(7_299_456),
            bitrate: Some(61_734_512),
            tracks: vec![
                track(
                    "hevc",
                    None,
                    true,
                    TrackKind::Video {
                        width: Some(3840),
                        height: Some(1600),
                        frame_rate: Some(ntsc),
                        bit_depth: None,
                        hdr: None,
                    },
                ),
                Track {
                    title: Some("TrueHD Atmos 7.1".to_owned()),
                    ..track(
                        "truehd",
                        Some("eng"),
                        true,
                        TrackKind::Audio {
                            channels: Some(8),
                            atmos: false,
                            audio_description: false
                        },
                    )
                },
                Track {
                    forced: true,
                    ..track(
                        "hdmv_pgs_subtitle",
                        Some("fre"),
                        false,
                        TrackKind::Subtitle {
                            image: true,
                            header: false,
                            sidecar: None,
                        },
                    )
                },
                Track {
                    sdh: true,
                    ..track(
                        "subrip",
                        Some("eng"),
                        false,
                        TrackKind::Subtitle {
                            image: false,
                            header: false,
                            sidecar: None,
                        },
                    )
                },
            ],
            font_attachments: vec![0],
            chapters: vec![],
        }
    );
}

#[test]
fn a_subtitle_carries_its_header_only_when_ffprobe_counts_bytes_of_one() {
    let json = r#"{
            "streams": [
                {"codec_name": "dvd_subtitle", "codec_type": "subtitle", "extradata_size": 391},
                {"codec_name": "dvd_subtitle", "codec_type": "subtitle", "extradata_size": 0},
                {"codec_name": "dvd_subtitle", "codec_type": "subtitle"}
            ],
            "format": {"format_name": "mpeg"}
        }"#;

    let (probe, _) = parse_probe(json.as_bytes()).unwrap();

    let headers: Vec<_> = (probe.tracks.iter())
        .map(|track| matches!(track.kind, TrackKind::Subtitle { header: true, .. }))
        .collect();
    assert_eq!(headers, [true, false, false]);
}

#[test]
fn a_font_is_an_attachment_whose_mime_type_or_else_file_name_says_so() {
    let json = r#"{
            "streams": [
                {"codec_name": "ass", "codec_type": "subtitle"},
                {"codec_type": "attachment",
                 "tags": {"filename": "cover.jpg", "mimetype": "image/jpeg"}},
                {"codec_type": "attachment",
                 "tags": {"filename": "Gandhi Sans", "mimetype": "application/x-truetype-font"}},
                {"codec_type": "attachment", "tags": {"filename": "vazir.otf"}},
                {"codec_type": "attachment",
                 "tags": {"filename": "NotoSansCJK.TTC", "mimetype": "application/octet-stream"}},
                {"codec_type": "attachment", "tags": {"filename": "chapters.txt"}},
                {"codec_type": "attachment",
                 "tags": {"filename": "a", "mimetype": "application/vnd.ms-opentype"}},
                {"codec_type": "attachment", "tags": {"filename": "b", "mimetype": "font/sfnt"}}
            ],
            "format": {"format_name": "matroska,webm"}
        }"#;

    let (probe, _) = parse_probe(json.as_bytes()).unwrap();

    assert_eq!(probe.font_attachments, [1, 2, 3, 5, 6]);
}

/// ffprobe 7.0's output for a Dolby Vision file of `profile`, whose base layer's
/// `dv_bl_signal_compatibility_id` is `compatibility` and transfer is `transfer`, cut down to
/// its video stream.
fn dolby_vision(profile: u8, compatibility: u8, transfer: &str) -> String {
    format!(
        r#"{{
            "streams": [
                {{"codec_name": "hevc", "codec_type": "video", "width": 3840, "height": 2160,
                 "pix_fmt": "yuv420p10le", "color_transfer": "{transfer}",
                 "side_data_list": [
                    {{"side_data_type": "DOVI configuration record", "dv_version_major": 1,
                     "dv_version_minor": 0, "dv_profile": {profile}, "dv_level": 6,
                     "rpu_present_flag": 1, "el_present_flag": 0, "bl_present_flag": 1,
                     "dv_bl_signal_compatibility_id": {compatibility}}}
                 ]}}
            ],
            "format": {{"format_name": "matroska,webm"}}
        }}"#
    )
}

fn video_of(json: &str) -> TrackKind {
    let (probe, _) = parse_probe(json.as_bytes()).unwrap();
    probe.tracks.into_iter().next().unwrap().kind
}

fn hdr_of(json: &str) -> Option<Hdr> {
    let TrackKind::Video { hdr, .. } = video_of(json) else {
        panic!("not a video Track");
    };
    hdr
}

#[test]
fn dolby_vision_is_read_with_its_profile_and_base_layer() {
    let dolby_vision_with = |profile, base_layer| {
        Some(Hdr::DolbyVision {
            profile,
            base_layer,
        })
    };
    assert_eq!(
        hdr_of(&dolby_vision(8, 1, "smpte2084")),
        dolby_vision_with(8, Some(BaseLayer::Hdr10))
    );
    // Ultra HD Blu-ray's, with an enhancement layer over HDR10.
    assert_eq!(
        hdr_of(&dolby_vision(7, 6, "smpte2084")),
        dolby_vision_with(7, Some(BaseLayer::Hdr10))
    );
    // Streaming's, which only a Dolby Vision player shows right.
    assert_eq!(
        hdr_of(&dolby_vision(5, 0, "unknown")),
        dolby_vision_with(5, None)
    );
    // Broadcast's, over HLG.
    assert_eq!(
        hdr_of(&dolby_vision(8, 4, "arib-std-b67")),
        dolby_vision_with(8, Some(BaseLayer::Hlg))
    );
    // Over SDR.
    assert_eq!(
        hdr_of(&dolby_vision(8, 2, "bt709")),
        dolby_vision_with(8, Some(BaseLayer::Sdr))
    );
    // A reserved id says nothing a player without Dolby Vision could show.
    assert_eq!(
        hdr_of(&dolby_vision(8, 3, "unknown")),
        dolby_vision_with(8, None)
    );
}

#[test]
fn bit_depth_is_read_from_the_pixel_format() {
    let depth_of = |pix_fmt: &str| {
        let json = format!(
            r#"{{"streams": [{{"codec_name": "hevc", "codec_type": "video",
                    "pix_fmt": "{pix_fmt}"}}], "format": {{"format_name": "matroska,webm"}}}}"#
        );
        let TrackKind::Video { bit_depth, .. } = video_of(&json) else {
            panic!("not a video Track");
        };
        bit_depth
    };

    assert_eq!(depth_of("yuv420p"), Some(8));
    assert_eq!(depth_of("yuvj420p"), Some(8));
    assert_eq!(depth_of("yuv420p10le"), Some(10));
    assert_eq!(depth_of("yuv422p12be"), Some(12));
    assert_eq!(depth_of("p010le"), Some(10));
    assert_eq!(depth_of("p210le"), Some(10));
    assert_eq!(depth_of("p416le"), Some(16));
    // These name bits per pixel, not per sample.
    assert_eq!(depth_of("rgb24"), None);
    assert_eq!(depth_of("nv12"), None);
}

#[test]
fn keyframes_are_the_packets_flagged_k_in_time_order() {
    let csv = "0.000000,K__\n0.083000,___\n2.002000,K_D\n1.001000,K__\nN/A,K__\n";

    assert_eq!(super::parse_keyframes(csv).keyframes_ms, [0, 1001, 2002]);
}

#[test]
fn a_missing_or_zero_format_duration_reads_as_unknown() {
    let with_format = |format: &str| {
        let json = format!(r#"{{"format": {{"format_name": "matroska,webm"{format}}}}}"#);
        parse_probe(json.as_bytes()).unwrap().0.duration_ms
    };

    assert_eq!(with_format(""), None);
    assert_eq!(with_format(r#", "duration": "0.000000""#), None);
    assert_eq!(with_format(r#", "duration": "2.000000""#), Some(2000));
}

#[test]
fn an_avi_with_the_placeholder_frame_count_has_no_known_duration() {
    let avi_with = |nb_frames: &str| {
        let json = format!(
            r#"{{"format": {{"format_name": "avi", "duration": "2866.375000"}},
                "streams": [{{"codec_type": "video", "nb_frames": "{nb_frames}"}}]}}"#
        );
        parse_probe(json.as_bytes()).unwrap().0.duration_ms
    };

    assert_eq!(avi_with("1073741824"), None);
    assert_eq!(avi_with("68793"), Some(2_866_375));
}

#[test]
fn only_an_avi_reads_the_placeholder_frame_count_as_unfilled() {
    let json = r#"{"format": {"format_name": "matroska,webm", "duration": "2.000000"},
            "streams": [{"codec_type": "video", "nb_frames": "1073741824"}]}"#;

    assert_eq!(
        parse_probe(json.as_bytes()).unwrap().0.duration_ms,
        Some(2000)
    );
}

#[test]
fn video_ends_where_its_latest_packet_does_from_the_files_start() {
    // In decoding order, with a packet whose duration isn't known.
    let csv = "1.523000,1.440000,0.041000\n3.523000,3.440000,0.041000\n\
               3.481000,3.481000,N/A\nN/A,N/A,0.041000\n1.500000\n";

    assert_eq!(super::parse_video_end(csv), Some(2064));
    assert_eq!(super::parse_video_end("0.000000\n"), None);
}

#[test]
fn a_packet_without_a_presentation_time_ends_by_its_decoding_time() {
    // As an AVI's packets have.
    let csv = "N/A,0.000000,0.040000\nN/A,19.960000,0.040000\n0.000000\n";

    assert_eq!(super::parse_video_end(csv), Some(20_000));
}

/// ffmpeg shifts a file that starts late to start at 0, and cuts its segments from there.
#[test]
fn keyframes_are_timed_from_the_files_start_which_is_kept_beside_them() {
    let csv = "1.523000,K__\n4.023000,K__\n1.500000\n";

    let listed = super::parse_keyframes(csv);

    assert_eq!(listed.keyframes_ms, [23, 2523]);
    assert_eq!(listed.file_start_ms, 1500);
}

#[test]
fn a_subtitle_is_sdh_when_flagged_or_titled_so_but_audio_titled_so_is_not() {
    let json = r#"{
            "streams": [
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "English (SDH)"}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "SDH"}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "English CC"}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "Hearing Impaired"}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "hearing-impaired"}},
                {"codec_name": "subrip", "codec_type": "subtitle",
                 "disposition": {"hearing_impaired": 1}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "Accent"}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "Access"}},
                {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"title": "English"}},
                {"codec_name": "aac", "codec_type": "audio", "tags": {"title": "SDH"}},
                {"codec_name": "aac", "codec_type": "audio", "tags": {"title": "CC"}}
            ],
            "format": {"format_name": "matroska,webm"}
        }"#;

    let (probe, _) = parse_probe(json.as_bytes()).unwrap();

    let sdh: Vec<_> = (probe.tracks.iter())
        .map(|track| (track.title.as_deref(), track.sdh))
        .collect();
    assert_eq!(
        sdh,
        [
            (Some("English (SDH)"), true),
            (Some("SDH"), true),
            (Some("English CC"), true),
            (Some("Hearing Impaired"), true),
            (Some("hearing-impaired"), true),
            (None, true),
            (Some("Accent"), false),
            (Some("Access"), false),
            (Some("English"), false),
            (Some("SDH"), false),
            (Some("CC"), false),
        ]
    );
}

#[test]
fn the_video_starts_where_its_first_stream_other_than_cover_art_does() {
    let json = r#"{
            "streams": [
                {"codec_type": "video", "start_time": "0.000000",
                 "disposition": {"attached_pic": 1}},
                {"codec_type": "audio", "start_time": "0.000000"},
                {"codec_type": "video", "start_time": "0.023000"}
            ],
            "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2", "start_time": "0.000000"}
        }"#;

    let (_, starts) = parse_probe(json.as_bytes()).unwrap();

    assert_eq!(
        starts,
        Starts {
            file_ms: 0,
            video_ms: Some(23),
        }
    );
}

#[test]
fn chapters_count_from_the_files_start_and_an_untitled_one_reads_titled_empty() {
    let json = r#"{
            "format": {"format_name": "matroska,webm", "start_time": "1.500000"},
            "chapters": [
                {"start_time": "1.500000", "tags": {"title": "Opening"}},
                {"start_time": "601.750000"},
                {"start_time": "3001.500000", "tags": {"title": "Credits"}}
            ]
        }"#;

    let (probe, _) = parse_probe(json.as_bytes()).unwrap();

    let chapters: Vec<_> = (probe.chapters.into_iter())
        .map(|chapter| (chapter.start_ms, chapter.title))
        .collect();
    assert_eq!(
        chapters,
        [
            (0, "Opening".to_owned()),
            (600_250, String::new()),
            (3_000_000, "Credits".to_owned()),
        ]
    );
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_whose_packets_cant_be_read_keeps_its_tracks_without_a_length() {
    use crate::ffmpeg::{Ffmpeg, fake};

    let bin = tempfile::tempdir().unwrap();
    let script = r#"#!/bin/sh
case "$*" in *packet=*) echo 'Invalid data found when processing input' >&2; exit 1 ;; esac
echo '{"format": {"format_name": "matroska,webm"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'
"#;
    let ffmpeg = Ffmpeg {
        ffprobe: Ok(fake::script(bin.path(), "ffprobe", script)),
        ..Ffmpeg::default()
    };

    let (probe, _) = super::probe(&ffmpeg, "Alien (1979).mkv").await.unwrap();

    assert_eq!((probe.tracks.len(), probe.duration_ms), (1, None));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn a_whole_file_listing_past_its_timeout_times_out() {
    use super::{ProbeError, WHOLE_FILE_TIMEOUT, keyframes};
    use crate::ffmpeg::{Ffmpeg, Program, RunError, fake};

    let bin = tempfile::tempdir().unwrap();
    // `exec`, so the timeout's kill stops the sleep rather than leaving it orphaned.
    let script = "#!/bin/sh\nexec sleep 3600\n";
    let ffmpeg = Ffmpeg {
        ffprobe: Ok(fake::script(bin.path(), "ffprobe", script)),
        ..Ffmpeg::default()
    };
    let started = tokio::time::Instant::now();

    let listed = keyframes(&ffmpeg, "Alien (1979).mkv").await;

    assert!(
        matches!(
            listed,
            Err(ProbeError::Run(RunError::TimedOut(Program::Ffprobe)))
        ),
        "{listed:?}"
    );
    assert!(started.elapsed() >= WHOLE_FILE_TIMEOUT);
}

#[test]
fn audio_is_atmos_when_its_profile_names_it_and_audio_description_when_flagged_so() {
    // ffprobe 7.0's profile names, from libavcodec's `profiles.c`.
    let json = r#"{
            "streams": [
                {"codec_name": "truehd", "codec_type": "audio", "channels": 8,
                 "profile": "Dolby TrueHD + Dolby Atmos"},
                {"codec_name": "eac3", "codec_type": "audio", "channels": 6,
                 "profile": "Dolby Digital Plus + Dolby Atmos"},
                {"codec_name": "eac3", "codec_type": "audio", "channels": 6},
                {"codec_name": "eac3", "codec_type": "audio", "channels": 2,
                 "disposition": {"visual_impaired": 1}}
            ],
            "format": {"format_name": "matroska,webm"}
        }"#;

    let (probe, _) = parse_probe(json.as_bytes()).unwrap();

    let audio: Vec<_> = (probe.tracks.iter())
        .map(|track| match track.kind {
            TrackKind::Audio {
                atmos,
                audio_description,
                ..
            } => (track.codec.as_str(), atmos, audio_description),
            TrackKind::Video { .. } | TrackKind::Subtitle { .. } => panic!("not audio: {track:?}"),
        })
        .collect();
    assert_eq!(
        audio,
        [
            ("truehd", true, false),
            ("eac3", true, false),
            ("eac3", false, false),
            ("eac3", false, true),
        ]
    );
}
