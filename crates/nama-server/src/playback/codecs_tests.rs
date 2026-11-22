use std::collections::BTreeSet;

use nama_proto::nama::v1::{AudioCodec, PlaybackReasonKind as Kind, VideoCodec};

use crate::{
    ffmpeg::{Accel, Hardware, ToneMapper, VideoTag},
    playback::{
        DeviceDecoding, Facts, Method, Plan, Playing, build,
        tests::{
            DOLBY_VISION_5, Given, MKV, MP4, audio, audio_with, decided, dolby_vision_8, facts,
            falls_back_to, firefox, given, hdr_video, in_container, kind_and_params, kinds,
            reason_of, remuxing, subtitle, track, transcoded, video, video_hdr,
        },
        tracks::{BaseLayer, Hdr, Track, TrackKind},
    },
};

#[test]
fn a_version_the_device_cant_decode_cant_be_direct_played_and_each_thing_decided_it() {
    let hdr10 = Some(Hdr::Hdr10);
    let cases: [(&str, Vec<Track>, Vec<Given<'_>>); 3] = [
        (
            MKV,
            vec![video("hevc"), audio("aac", true)],
            vec![
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                    vec![("container", "CONTAINER_MATROSKA")],
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                    vec![("codec", "hevc")],
                ),
            ],
        ),
        (
            MP4,
            vec![hdr_video("av1", hdr10), audio("aac", true)],
            vec![(
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
                vec![("hdrFormat", "HDR_FORMAT_HDR10")],
            )],
        ),
        (
            "avi",
            vec![video("mpeg4"), audio("pcm_s16le", true)],
            vec![
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                    vec![("container", "avi")],
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                    vec![("codec", "mpeg4")],
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_AUDIO,
                    vec![("codec", "pcm_s16le")],
                ),
            ],
        ),
    ];
    for (container, tracks, cant) in cases {
        let plan = build(&mut in_container(container, Some(tracks)));

        assert_eq!(plan.method(), Some(Method::Transcode), "{cant:?}");
        assert_eq!(given(&plan)[..cant.len()], cant);
        assert!(
            plan.reasons[..cant.len()]
                .iter()
                .all(|reason| reason.decided),
            "{cant:?}"
        );
    }
}

#[test]
fn a_device_decodes_what_its_codecs_and_display_say() {
    let mut hdr_display = firefox();
    hdr_display.hdr = true;
    let cases = [
        (MKV, vec![video("vp9"), audio("opus", true)], firefox()),
        (MKV, vec![video("vp8"), audio("vorbis", true)], firefox()),
        (MP4, vec![video("h264"), audio("mp3", true)], firefox()),
        (
            MP4,
            vec![hdr_video("av1", Some(Hdr::Hlg)), audio("flac", true)],
            hdr_display,
        ),
        // Only the Tracks the plan plays need decoding.
        (
            MP4,
            vec![video("h264"), audio("dts", false), audio("aac", true)],
            firefox(),
        ),
    ];
    for (container, tracks, device) in cases {
        let mut facts = Facts {
            device,
            ..in_container(container, Some(tracks.clone()))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{tracks:?}");
    }
}

#[test]
fn only_a_transcode_helps_with_video_the_device_cant_decode() {
    let cases = [
        (MKV, vec![video("h264"), audio("aac", true)], Method::Remux),
        (
            MKV,
            vec![video("hevc"), audio("aac", true)],
            Method::Transcode,
        ),
        (MP4, vec![video("h264"), audio("dts", true)], Method::Remux),
        (
            MP4,
            vec![video("hevc"), audio("dts", true)],
            Method::Transcode,
        ),
        (
            MP4,
            vec![hdr_video("av1", Some(Hdr::Hdr10)), audio("aac", true)],
            Method::Transcode,
        ),
    ];
    for (container, tracks, method) in cases {
        let plan = build(&mut in_container(container, Some(tracks.clone())));

        assert_eq!(plan.method(), Some(method), "{tracks:?}");
        let rules_out_a_remux = kinds(&plan).iter().any(|kind| {
            matches!(
                kind,
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO
                    | Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR
            )
        });
        assert_eq!(rules_out_a_remux, method == Method::Transcode, "{tracks:?}");
    }
}

#[test]
fn the_plan_plays_the_default_audio_track_and_else_the_first() {
    let (first, default) = (audio("mp3", false), audio("aac", true));
    let cases = [
        (vec![video("h264"), first.clone(), default.clone()], default),
        (
            vec![first.clone(), video("h264"), audio("aac", false)],
            first,
        ),
    ];
    for (tracks, played) in cases {
        let with_subtitle = [tracks.clone(), vec![subtitle()]].concat();

        let plan = build(&mut facts(Some(with_subtitle)));

        assert_eq!(plan.tracks, [video("h264"), played], "{tracks:?}");
    }
}

#[test]
fn a_version_only_its_container_keeps_from_direct_playing_is_remuxed() {
    let plan = build(&mut in_container(
        MKV,
        Some(vec![video("h264"), audio("aac", true)]),
    ));

    assert_eq!(plan.method(), Some(Method::Remux));
    assert_eq!(
        given(&plan),
        [(
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
            vec![("container", "CONTAINER_MATROSKA")]
        )]
    );
    assert_eq!(decided(&plan), [true]);
}

#[test]
fn audio_the_device_cant_decode_is_remuxed_to_aac_and_the_plan_says_so() {
    let dts = audio_with("dts", true, Some(6));
    let mut facts = in_container(MKV, Some(vec![video("h264"), dts.clone()]));
    facts.device.audio_channels = 6;

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Remux));
    let aac = Track {
        codec: "aac".to_owned(),
        ..dts
    };
    assert_eq!(plan.tracks, [video("h264"), aac]);
    assert_eq!(
        given(&plan),
        [
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                vec![("container", "CONTAINER_MATROSKA")]
            ),
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_AUDIO,
                vec![("codec", "dts")]
            ),
        ],
        "no reason says what a Remux makes"
    );
}

#[test]
fn converted_audio_has_as_many_channels_as_the_device_plays_up_to_5_1_and_says_why_fewer() {
    let fewer = Kind::PLAYBACK_REASON_KIND_DEVICE_PLAYS_FEWER_CHANNELS;
    let unknown = Kind::PLAYBACK_REASON_KIND_DEVICE_CHANNELS_UNKNOWN;
    let capped = Kind::PLAYBACK_REASON_KIND_CONVERSION_CHANNELS_CAPPED;
    let cases = [
        (Some(6), 6, Some(6), None),
        (Some(8), 8, Some(6), Some((capped, "6"))),
        (Some(6), 2, Some(2), Some((fewer, "2"))),
        (Some(6), 4, Some(2), Some((fewer, "2"))),
        (Some(6), 0, Some(2), Some((unknown, "2"))),
        (Some(2), 6, Some(2), None),
        (Some(1), 6, Some(1), None),
        (None, 6, Some(2), None),
    ];
    for (source, plays, converted, why) in cases {
        let mut facts = in_container(
            MP4,
            Some(vec![video("h264"), audio_with("dts", true, source)]),
        );
        facts.device.audio_channels = plays;

        let plan = build(&mut facts);

        let case = format!("{source:?} on {plays}");
        assert_eq!(plan.tracks[1], audio_with("aac", true, converted), "{case}");
        let channel_reasons: Vec<(Given<'_>, bool)> = (plan.reasons.iter())
            .filter(|reason| [fewer, unknown, capped].contains(&reason.kind))
            .map(|reason| (kind_and_params(reason), reason.decided))
            .collect();
        let expected: Vec<_> = (why.into_iter())
            .map(|(kind, channels)| {
                let params = vec![("channels", channels), ("codec", "aac")];
                ((kind, params), true)
            })
            .collect();
        assert_eq!(channel_reasons, expected, "{case}");
    }
}

#[test]
fn surround_audio_is_converted_to_e_ac_3_or_else_ac_3_where_the_device_decodes_it_else_to_aac() {
    let (eac3, ac3) = (AudioCodec::AUDIO_CODEC_EAC3, AudioCodec::AUDIO_CODEC_AC3);
    let cases = [
        (vec![ac3, eac3], "eac3", "E-AC-3"),
        (vec![ac3], "ac3", "AC-3"),
        (vec![], "aac", "AAC"),
    ];
    for (decodes, codec, name) in cases {
        for (codec_of_video, method) in [("h264", Method::Remux), ("hevc", Method::Transcode)] {
            let truehd = audio_with("truehd", true, Some(8));
            let mut facts = in_container(MKV, Some(vec![video(codec_of_video), truehd]));
            facts.device.audio_channels = 6;
            facts.device.audio_codecs.extend(&decodes);

            let plan = build(&mut facts);

            let case = format!("{method:?} on a Device decoding {decodes:?}");
            assert_eq!(plan.method(), Some(method), "{case}");
            assert_eq!(plan.tracks[1], audio_with(codec, true, Some(6)), "{case}");
            let capped = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_CONVERSION_CHANNELS_CAPPED);
            let params = vec![("channels", "6"), ("codec", codec)];
            assert_eq!(kind_and_params(capped).1, params, "{case}");
            let text = format!("Its audio is converted to {name} 5.1, ");
            assert!(capped.text.starts_with(&text), "{case}: {}", capped.text);
        }
    }
}

#[test]
fn stereo_audio_or_audio_for_a_device_that_plays_only_stereo_is_converted_to_aac() {
    for (source, plays) in [(2, 6), (8, 2)] {
        let mut facts = in_container(
            MKV,
            Some(vec![
                video("h264"),
                audio_with("truehd", true, Some(source)),
            ]),
        );
        facts.device.audio_channels = plays;
        facts
            .device
            .audio_codecs
            .extend([AudioCodec::AUDIO_CODEC_AC3, AudioCodec::AUDIO_CODEC_EAC3]);

        let plan = build(&mut facts);

        assert_eq!(
            plan.tracks[1],
            audio_with("aac", true, Some(2)),
            "{source} on {plays}"
        );
    }
}

#[test]
fn video_the_device_cant_decode_is_transcoded_to_h264_and_aac_and_the_plan_says_why() {
    let hevc = Track {
        kind: TrackKind::Video {
            width: Some(1920),
            height: Some(1080),
            frame_rate: Some(24.0),
            bit_depth: Some(10),
            hdr: None,
        },
        ..video("hevc")
    };
    let mut facts = in_container(MKV, Some(vec![hevc, audio_with("aac", true, Some(6))]));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert_eq!(
        plan.tracks,
        [video("h264"), audio_with("aac", true, Some(2))]
    );
    assert_eq!(
        given(&plan),
        [
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                vec![("container", "CONTAINER_MATROSKA")]
            ),
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                vec![("codec", "hevc")]
            ),
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_PLAYS_FEWER_CHANNELS,
                vec![("channels", "2"), ("codec", "aac")]
            ),
            (
                Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
                vec![]
            ),
        ]
    );
    assert_eq!(decided(&plan), [true, true, true, false]);
}

#[test]
fn a_transcode_that_doesnt_tone_map_lists_the_hdr_of_what_it_decodes() {
    let cases = [
        (Hdr::Hdr10, Some(Hdr::Hdr10)),
        (Hdr::Hlg, Some(Hdr::Hlg)),
        (dolby_vision_8(Some(BaseLayer::Hdr10)), Some(Hdr::Hdr10)),
        (dolby_vision_8(Some(BaseLayer::Hlg)), Some(Hdr::Hlg)),
        (dolby_vision_8(Some(BaseLayer::Sdr)), None),
    ];
    for (hdr, played) in cases {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));
        facts.device.hdr = true;

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, None, "{hdr:?}");
        assert_eq!(video_hdr(&plan.tracks), played, "{hdr:?}");
    }
}

#[test]
fn dolby_vision_without_a_base_layer_isnt_transcoded_so_a_device_that_cant_decode_it_cant_play_it()
{
    let profile_5 = Hdr::DolbyVision {
        profile: 5,
        base_layer: None,
    };
    let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(profile_5))]));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), None);
    assert_eq!(
        kinds(&plan),
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
            Kind::PLAYBACK_REASON_KIND_DOLBY_VISION_NEEDS_BASE_LAYER,
        ]
    );
    assert_eq!(decided(&plan), [true, true, true]);
}

/// Dolby Vision profile 5 in HEVC in `container`, for an HDR Device that decodes HEVC and shows
/// the profiles `shown`.
fn dolby_vision_5_for(container: &str, shown: Vec<i32>) -> Facts {
    let mut facts = in_container(
        container,
        Some(vec![hdr_video("hevc", Some(DOLBY_VISION_5))]),
    );
    facts.device.video_codecs.push(VideoCodec::VIDEO_CODEC_HEVC);
    facts.device.hdr = true;
    facts.device.dolby_vision_profiles = shown;
    facts
}

#[test]
fn an_hdr_device_not_showing_dolby_vision_5_isnt_sent_it_by_direct_play_or_remux() {
    let mut facts = dolby_vision_5_for(MP4, vec![8]);

    let plan = build(&mut facts);

    assert_ne!(plan.method(), Some(Method::DirectPlay));
    assert_ne!(plan.method(), Some(Method::Remux));
    let unshown = reason_of(
        &plan,
        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_DOLBY_VISION,
    );
    assert_eq!(kind_and_params(unshown).1, [("profile", "5")]);
    assert!(unshown.decided);
}

#[test]
fn a_device_showing_dolby_vision_5_direct_plays_it() {
    let mut facts = dolby_vision_5_for(MP4, vec![5]);

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
}

#[test]
fn a_device_showing_dolby_vision_5_is_sent_it_remuxed_from_mkv_as_dvh1() {
    let mut facts = dolby_vision_5_for(MKV, vec![5]);

    let plan = build(&mut facts);

    let Some(Playing::Remux { remux, .. }) = plan.playing else {
        panic!("{:?} isn't a Remux", plan.playing);
    };
    assert_eq!(remux.video_tag, Some(VideoTag::Dvh1));
}

/// Dolby Vision profile 5 for Firefox, which neither decodes HEVC nor shows HDR, on a Server
/// whose ffmpeg has `tonemapx`.
fn dolby_vision_5_with_tonemapx() -> Facts {
    let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(DOLBY_VISION_5))]));
    facts.can_do.tonemapx = true;
    facts
}

const RESHAPED: Option<ToneMapper> = Some(ToneMapper::Tonemapx { dolby_vision: true });

#[test]
fn with_tonemapx_dolby_vision_5_transcodes_to_sdr_reshaped_by_its_rpu_and_says_so() {
    let mut facts = dolby_vision_5_with_tonemapx();

    let plan = build(&mut facts);

    assert_eq!(transcoded(&plan).video.tone_map, RESHAPED);
    assert_eq!(video_hdr(&plan.tracks), None);
    let tone_mapped = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED);
    assert_eq!(
        kind_and_params(tone_mapped).1,
        [("hdrFormat", "HDR_FORMAT_DOLBY_VISION")]
    );
    assert!(!kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_DOLBY_VISION_NEEDS_BASE_LAYER));
}

#[test]
fn with_tonemapx_a_dolby_vision_5_transcode_on_a_gpu_that_tone_maps_reshapes_in_software() {
    let mut facts = dolby_vision_5_with_tonemapx();
    let qsv = Hardware {
        accel: Accel::Qsv,
        gpu: None,
        tone_maps: true,
        hevc: false,
    };
    facts.can_do.hardware = vec![qsv.clone()];

    let plan = build(&mut facts);

    let video = transcoded(&plan).video;
    assert_eq!((video.hardware, video.tone_map), (Some(qsv), RESHAPED));
    let fallback = falls_back_to(&plan).unwrap().transcode.video;
    assert_eq!((fallback.hardware, fallback.tone_map), (None, RESHAPED));
}

#[test]
fn with_tonemapx_hdr10_and_hlg_tone_map_through_it_without_reshaping() {
    for hdr in [Hdr::Hdr10, Hdr::Hlg] {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));
        facts.can_do.tonemapx = true;

        let plan = build(&mut facts);

        let tone_map = transcoded(&plan).video.tone_map;
        assert_eq!(
            tone_map,
            Some(ToneMapper::Tonemapx {
                dolby_vision: false
            }),
            "{hdr:?}"
        );
    }
}

#[test]
fn a_transcode_of_a_version_without_audio_says_nothing_of_audio() {
    let mut facts = in_container(MP4, Some(vec![video("hevc")]));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert_eq!(
        kinds(&plan),
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
            Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
        ]
    );
}

#[test]
fn audio_the_device_decodes_is_copied_into_a_remux() {
    let mut facts = in_container(MKV, Some(vec![video("h264"), audio("aac", true)]));

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(0, None)));
    assert_eq!(plan.tracks, [video("h264"), audio("aac", true)]);
}

/// An H.264 video Track `bit_depth` bits deep.
fn h264_at(bit_depth: Option<i64>) -> Track {
    let mut track = video("h264");
    if let TrackKind::Video {
        bit_depth: depth, ..
    } = &mut track.kind
    {
        *depth = bit_depth;
    }
    track
}

#[test]
fn ten_bit_h264_a_device_cant_decode_is_transcoded_to_8_bit_and_the_plan_says_why() {
    let tracks = vec![h264_at(Some(10)), audio("aac", true)];

    let plan = build(&mut in_container(MKV, Some(tracks)));

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert_eq!(plan.tracks, [video("h264"), audio("aac", true)]);
    let video_reason = (
        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
        vec![("codec", "h264"), ("bitDepth", "10")],
    );
    let given = given(&plan);
    let place = given.iter().position(|reason| *reason == video_reason);
    assert!(
        place.is_some_and(|place| plan.reasons[place].decided),
        "{given:?}"
    );
}

#[test]
fn ten_bit_h264_a_device_decodes_is_remuxed_or_direct_played() {
    for (container, method) in [(MKV, Method::Remux), (MP4, Method::DirectPlay)] {
        let mut facts = in_container(container, Some(vec![h264_at(Some(10)), audio("aac", true)]));
        facts.device.h264_10bit = true;

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(method), "{container}");
        assert!(
            !kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO),
            "{container}"
        );
    }
}

#[test]
fn h264_of_unknown_or_8_bit_depth_isnt_ruled_out_for_its_depth() {
    for bit_depth in [None, Some(8)] {
        let plan = build(&mut facts(Some(vec![
            h264_at(bit_depth),
            audio("aac", true),
        ])));

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{bit_depth:?}");
    }
}

/// What Safari on an HDR display says it decodes: H.264 and HEVC, both up to 10 bits.
fn safari_on_hdr() -> DeviceDecoding {
    DeviceDecoding {
        video_codecs: vec![VideoCodec::VIDEO_CODEC_H264, VideoCodec::VIDEO_CODEC_HEVC],
        hdr: true,
        h264_10bit: true,
        hevc_10bit: true,
        ..firefox()
    }
}

/// AV1 `width` wide and HDR as `hdr` says, which [`safari_on_hdr`] decodes only Transcoded, on a
/// Server whose VAAPI passed its HEVC test.
fn av1_on_hevc_hardware(width: i64, hdr: Option<Hdr>) -> Facts {
    let kind = TrackKind::Video {
        width: Some(width),
        height: Some(width * 9 / 16),
        frame_rate: Some(24.0),
        bit_depth: Some(10),
        hdr,
    };
    let mut facts = in_container(
        MKV,
        Some(vec![track("av1", false, kind), audio("aac", true)]),
    );
    facts.device = safari_on_hdr();
    facts.can_do.hardware = vec![Hardware {
        accel: Accel::Vaapi,
        gpu: None,
        tone_maps: true,
        hevc: true,
    }];
    facts
}

/// The codec, bit depth and HDR of the video `plan` sends, and its Recipe.
fn video_made(plan: &Plan) -> (&str, Option<i64>, Option<Hdr>, String) {
    let (codec, bit_depth, hdr) = (plan.tracks.iter())
        .find_map(|track| match track.kind {
            TrackKind::Video { bit_depth, hdr, .. } => Some((track.codec.as_str(), bit_depth, hdr)),
            TrackKind::Audio { .. } | TrackKind::Subtitle { .. } => None,
        })
        .unwrap();
    (codec, bit_depth, hdr, plan.recipe())
}

#[test]
fn a_4k_or_hdr_transcode_makes_10_bit_hevc_on_hardware_keeping_its_hdr_with_no_fallback() {
    let dolby_vision_7 = Hdr::DolbyVision {
        profile: 7,
        base_layer: Some(BaseLayer::Hdr10),
    };
    let cases = [
        (3840, None, None),
        (1920, Some(Hdr::Hdr10), Some(Hdr::Hdr10)),
        (1920, Some(Hdr::Hlg), Some(Hdr::Hlg)),
        (1920, Some(dolby_vision_7), Some(Hdr::Hdr10)),
        (
            1920,
            Some(dolby_vision_8(Some(BaseLayer::Hlg))),
            Some(Hdr::Hlg),
        ),
    ];
    for (width, hdr, base_layer) in cases {
        let plan = build(&mut av1_on_hevc_hardware(width, hdr));

        let hevc = "transcode/fmp4/hevc/aac".to_owned();
        assert_eq!(
            video_made(&plan),
            ("hevc", Some(10), base_layer, hevc),
            "{width} {hdr:?}"
        );
        let video = transcoded(&plan).video;
        assert!(video.hevc && !video.in_band_headers && video.tone_map.is_none());
        assert_eq!(falls_back_to(&plan), None);
    }
}

#[test]
fn without_hevc_on_the_device_or_server_or_a_4k_or_hdr_picture_a_transcode_makes_h264() {
    let sdr_1080p = av1_on_hevc_hardware(1920, None);
    let mut no_hevc_10bit = av1_on_hevc_hardware(3840, Some(Hdr::Hdr10));
    no_hevc_10bit.device.hevc_10bit = false;
    let mut no_hevc = av1_on_hevc_hardware(3840, None);
    no_hevc.device.video_codecs = vec![VideoCodec::VIDEO_CODEC_H264];
    let mut no_hevc_hardware = av1_on_hevc_hardware(3840, None);
    no_hevc_hardware.can_do.hardware[0].hevc = false;

    for mut facts in [sdr_1080p, no_hevc_10bit, no_hevc, no_hevc_hardware] {
        let plan = build(&mut facts);

        let (codec, bit_depth, _, recipe) = video_made(&plan);
        assert_eq!((codec, bit_depth), ("h264", Some(8)), "{:?}", facts.device);
        assert_eq!(recipe, "transcode/fmp4/h264/aac");
        assert!(falls_back_to(&plan).is_some());
    }
}

#[test]
fn hdr_the_device_cant_show_is_tone_mapped_to_h264_even_in_4k() {
    let mut facts = av1_on_hevc_hardware(3840, Some(Hdr::Hdr10));
    facts.device.hdr = false;

    let plan = build(&mut facts);

    assert_eq!(
        video_made(&plan),
        ("h264", Some(8), None, "transcode/fmp4/h264/aac".to_owned())
    );
}

#[test]
fn an_hevc_transcode_that_failed_on_the_device_or_server_transcodes_to_h264_instead() {
    let failed = BTreeSet::from(["transcode/fmp4/hevc/aac".to_owned()]);
    let mut on_device = av1_on_hevc_hardware(3840, None);
    on_device.failed_recipes = failed.clone();
    let mut on_server = av1_on_hevc_hardware(3840, None);
    on_server.failed_on_server = failed;

    for mut facts in [on_device, on_server] {
        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert_eq!(plan.recipe(), "transcode/fmp4/h264/aac");
        let failures = [
            Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
            Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_SERVER,
        ];
        assert!(!kinds(&plan).iter().any(|kind| failures.contains(kind)));
    }
}

#[test]
fn an_h264_transcode_that_failed_on_the_server_is_ruled_out_and_the_plan_says_so() {
    let mut facts = av1_on_hevc_hardware(1920, None);
    facts.failed_on_server = BTreeSet::from(["transcode/fmp4/h264/aac".to_owned()]);

    let plan = build(&mut facts);

    assert_eq!(plan.method(), None);
    let failed = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_SERVER);
    assert_eq!(
        kind_and_params(failed).1,
        [("method", "PLAYBACK_METHOD_TRANSCODE")]
    );
}
