use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use crate::playback::{
    Facts, Method, build,
    tests::{
        Given, MKV, MP4, audio, audio_with, decided, dolby_vision_8, facts, firefox, given,
        hdr_video, in_container, kind_and_params, kinds, remuxing, subtitle, transcoded, video,
        video_hdr,
    },
    tracks::{BaseLayer, Hdr, Track, TrackKind},
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
            .map(|(kind, channels)| ((kind, vec![("channels", channels)]), true))
            .collect();
        assert_eq!(channel_reasons, expected, "{case}");
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
                vec![("channels", "2")]
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
