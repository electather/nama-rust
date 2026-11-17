use super::parse_probe;
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
    let probe = parse_probe(BLU_RAY_REMUX.as_bytes()).unwrap();

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
                        TrackKind::Audio { channels: Some(8) },
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

    let probe = parse_probe(json.as_bytes()).unwrap();

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

    let probe = parse_probe(json.as_bytes()).unwrap();

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
    let probe = parse_probe(json.as_bytes()).unwrap();
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

    assert_eq!(super::parse_keyframes(csv), [0, 1001, 2002]);
}

#[test]
fn a_missing_or_zero_format_duration_reads_as_unknown() {
    let with_format = |format: &str| {
        let json = format!(r#"{{"format": {{"format_name": "matroska,webm"{format}}}}}"#);
        parse_probe(json.as_bytes()).unwrap().duration_ms
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
        parse_probe(json.as_bytes()).unwrap().duration_ms
    };

    assert_eq!(avi_with("1073741824"), None);
    assert_eq!(avi_with("68793"), Some(2_866_375));
}

#[test]
fn only_an_avi_reads_the_placeholder_frame_count_as_unfilled() {
    let json = r#"{"format": {"format_name": "matroska,webm", "duration": "2.000000"},
            "streams": [{"codec_type": "video", "nb_frames": "1073741824"}]}"#;

    assert_eq!(
        parse_probe(json.as_bytes()).unwrap().duration_ms,
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
fn keyframes_are_timed_from_the_files_start() {
    let csv = "1.523000,K__\n4.023000,K__\n1.500000\n";

    assert_eq!(super::parse_keyframes(csv), [23, 2523]);
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

    let probe = parse_probe(json.as_bytes()).unwrap();

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
