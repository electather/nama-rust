use std::collections::BTreeSet;

use nama_proto::nama::v1::{
    AudioCodec, Container, PlaybackReasonKind as Kind, SubtitleFormat, VideoCodec,
};

use super::{
    AskedAudio, Candidates, DeviceDecoding, Facts, Fallback, Method, Network, Offers, PickedLimit,
    Plan, Playing, Reason, Subtitle, audio_asked, build,
    choices::preferred_subtitle,
    default_max_transcodes, narrowed, on_local_network, too_busy_to_play,
    track_preferences::{Description, Preferred, SubtitlePreference},
    tracks::{BaseLayer, Hdr, Probe, Track, TrackKind},
};
use crate::{
    ffmpeg::{
        Accel, CanDo, Hardware, Remux, RemuxAudio, ToneMapper, Transcode, VideoConversion,
        VideoLimit,
    },
    server_settings::{HardwareChoice, PickedGpu, PlaybackLimit},
};

/// How long the Version in [`facts`] is.
const LENGTH_MS: i64 = 7_200_000;

fn track(codec: &str, default: bool, kind: TrackKind) -> Track {
    Track {
        codec: codec.to_owned(),
        language: None,
        title: None,
        default,
        forced: false,
        sdh: false,
        kind,
    }
}

fn video(codec: &str) -> Track {
    hdr_video(codec, None)
}

fn hdr_video(codec: &str, hdr: Option<Hdr>) -> Track {
    let kind = TrackKind::Video {
        width: Some(1920),
        height: Some(1080),
        frame_rate: Some(24.0),
        bit_depth: Some(8),
        hdr,
    };
    track(codec, false, kind)
}

const fn dolby_vision_8(base_layer: Option<BaseLayer>) -> Hdr {
    Hdr::DolbyVision {
        profile: 8,
        base_layer,
    }
}

/// How the video among `tracks` is HDR.
fn video_hdr(tracks: &[Track]) -> Option<Hdr> {
    tracks.iter().find_map(|track| match track.kind {
        TrackKind::Video { hdr, .. } => hdr,
        TrackKind::Audio { .. } | TrackKind::Subtitle { .. } => None,
    })
}

fn audio(codec: &str, default: bool) -> Track {
    audio_with(codec, default, Some(2))
}

fn audio_with(codec: &str, default: bool, channels: Option<i64>) -> Track {
    track(codec, default, TrackKind::Audio { channels })
}

const MP4: &str = "mov,mp4,m4a,3gp,3g2,mj2";
const MKV: &str = "matroska,webm";

/// What Firefox on an SDR display says it decodes.
fn firefox() -> DeviceDecoding {
    DeviceDecoding {
        containers: vec![Container::CONTAINER_MP4, Container::CONTAINER_WEBM],
        video_codecs: vec![
            VideoCodec::VIDEO_CODEC_H264,
            VideoCodec::VIDEO_CODEC_VP8,
            VideoCodec::VIDEO_CODEC_VP9,
            VideoCodec::VIDEO_CODEC_AV1,
        ],
        audio_codecs: vec![
            AudioCodec::AUDIO_CODEC_AAC,
            AudioCodec::AUDIO_CODEC_MP3,
            AudioCodec::AUDIO_CODEC_OPUS,
            AudioCodec::AUDIO_CODEC_VORBIS,
            AudioCodec::AUDIO_CODEC_FLAC,
        ],
        hdr: false,
        hls: true,
        audio_channels: 2,
        subtitle_formats: vec![],
    }
}

fn facts(tracks: Option<Vec<Track>>) -> Facts {
    in_container(MP4, tracks)
}

fn in_container(container: &str, tracks: Option<Vec<Track>>) -> Facts {
    Facts {
        version: 7,
        probe: tracks.map(|tracks| Probe {
            container: container.to_owned(),
            duration_ms: Some(LENGTH_MS),
            bitrate: Some(5_000_000),
            tracks,
            font_attachments: vec![],
        }),
        device: firefox(),
        can_do: CanDo {
            runs: true,
            zscale: true,
            libx264: true,
            hardware: vec![],
            gpus: vec![],
        },
        failed: BTreeSet::new(),
        audio: None,
        transcodes_full: None,
        picked_limit: PickedLimit::Unpicked,
        network: Network::Local,
        subtitle: None,
        shown_subtitle: None,
        subtitle_preference: None,
        hardware_choice: HardwareChoice::Automatic,
        dropped: BTreeSet::new(),
    }
}

/// What `plan` Transcodes, failing the test if it doesn't Transcode.
fn transcoded(plan: &Plan) -> Transcode {
    match &plan.playing {
        Some(Playing::Transcode { transcode, .. }) => transcode.clone(),
        other => panic!("{other:?} isn't a Transcode"),
    }
}

/// What `plan` Transcodes in software once its hardware fails, failing the test if it
/// doesn't Transcode.
fn falls_back_to(plan: &Plan) -> Option<Fallback> {
    match &plan.playing {
        Some(Playing::Transcode { fallback, .. }) => fallback.clone(),
        other => panic!("{other:?} isn't a Transcode"),
    }
}

/// A Remux playing the audio Track `track`, converting it to AAC with `aac_channels` if given.
const fn remuxing(track: usize, aac_channels: Option<i64>) -> Playing {
    Playing::Remux {
        remux: Remux {
            audio: Some(RemuxAudio {
                track,
                aac_channels,
            }),
        },
        duration_ms: LENGTH_MS,
    }
}

fn subtitle() -> Track {
    Track {
        default: true,
        ..text_subtitle("subrip")
    }
}

fn text_subtitle(codec: &str) -> Track {
    track(
        codec,
        false,
        TrackKind::Subtitle {
            image: false,
            header: false,
        },
    )
}

/// A reason's kind and params, as a test expects them.
type Given<'a> = (Kind, Vec<(&'a str, &'a str)>);

/// The kind and params of each of `plan`'s reasons, in order.
fn given(plan: &Plan) -> Vec<Given<'_>> {
    plan.reasons.iter().map(kind_and_params).collect()
}

fn kind_and_params(reason: &Reason) -> Given<'_> {
    let params = (reason.params.iter())
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    (reason.kind, params)
}

/// The kind of each of `plan`'s reasons, in order.
fn kinds(plan: &Plan) -> Vec<Kind> {
    plan.reasons.iter().map(|reason| reason.kind).collect()
}

/// `plan`'s reason of `kind`, failing the test if it has none.
fn reason_of(plan: &Plan, kind: Kind) -> &Reason {
    (plan.reasons.iter())
        .find(|reason| reason.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} among {:?}", kinds(plan)))
}

/// Whether each of `plan`'s reasons decided it, in order.
fn decided(plan: &Plan) -> Vec<bool> {
    plan.reasons.iter().map(|reason| reason.decided).collect()
}

#[test]
fn an_h264_and_aac_mp4_direct_plays_with_no_reasons() {
    let plan = build(&mut facts(Some(vec![video("h264"), audio("aac", true)])));

    assert_eq!(
        plan,
        Plan {
            version: 7,
            playing: Some(Playing::DirectPlay),
            tracks: vec![video("h264"), audio("aac", true)],
            reasons: vec![],
            subtitles: vec![],
            limit: None,
            audio: Some(1),
            burn_in: None,
            starting_subtitle: None,
            offers: Offers {
                audio: vec![1],
                burn_in: vec![],
                limits: vec![PlaybackLimit::P720, PlaybackLimit::P480],
            },
        }
    );
}

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
fn a_version_whose_length_isnt_known_isnt_remuxed_or_transcoded_and_says_why() {
    let unknown_length = |container| {
        let mut facts = in_container(container, Some(vec![video("h264"), audio("aac", true)]));
        if let Some(probe) = &mut facts.probe {
            probe.duration_ms = None;
        }
        build(&mut facts)
    };

    let mkv = unknown_length(MKV);
    let mp4 = unknown_length(MP4);

    assert_eq!(mkv.method(), None);
    assert_eq!(
        kinds(&mkv),
        [
            Kind::PLAYBACK_REASON_KIND_LENGTH_UNKNOWN,
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
        ]
    );
    assert_eq!(decided(&mkv), [true, true], "nothing plays it");
    assert_eq!(mkv.offers.audio, [1]);
    assert_eq!(
        mp4.method(),
        Some(Method::DirectPlay),
        "Direct Play needs no length"
    );
}

#[test]
fn a_remux_and_a_transcode_carry_the_versions_length() {
    let remux = build(&mut in_container(
        MKV,
        Some(vec![video("h264"), audio("aac", true)]),
    ));
    let transcode = build(&mut facts(Some(vec![video("hevc"), audio("aac", true)])));

    assert!(
        matches!(
            remux.playing,
            Some(Playing::Remux {
                duration_ms: LENGTH_MS,
                ..
            })
        ),
        "{:?}",
        remux.playing
    );
    assert!(
        matches!(
            transcode.playing,
            Some(Playing::Transcode {
                duration_ms: LENGTH_MS,
                ..
            })
        ),
        "{:?}",
        transcode.playing
    );
}

#[test]
fn a_version_whose_tracks_arent_known_direct_plays_and_says_so_as_context() {
    let plan = build(&mut facts(None));

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert_eq!(plan.tracks, []);
    assert_eq!(
        kinds(&plan),
        [
            Kind::PLAYBACK_REASON_KIND_TRACKS_UNKNOWN,
            Kind::PLAYBACK_REASON_KIND_LENGTH_UNKNOWN,
        ]
    );
    assert_eq!(decided(&plan), [false, false]);
}

#[test]
fn a_version_whose_tracks_arent_known_cant_be_played_once_direct_play_failed() {
    let mut facts = Facts {
        failed: BTreeSet::from([Method::DirectPlay]),
        ..facts(None)
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), None);
}

#[test]
fn a_version_whose_tracks_arent_known_is_never_too_busy_to_play() {
    let facts = Facts {
        failed: BTreeSet::from([Method::DirectPlay, Method::Remux]),
        transcodes_full: Some(1),
        ..facts(None)
    };

    assert_eq!(too_busy_to_play(facts), None);
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
fn a_chosen_audio_track_other_than_the_files_own_is_remuxed_and_the_plan_says_so() {
    let (english, commentary) = (audio("aac", true), audio("aac", false));
    let mut facts = Facts {
        audio: Some(3),
        ..facts(Some(vec![
            video("h264"),
            english,
            subtitle(),
            commentary.clone(),
        ]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(1, None)), "counting only audio");
    assert_eq!(plan.tracks, [video("h264"), commentary]);
    assert_eq!(
        kinds(&plan),
        [Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN]
    );
    assert_eq!(decided(&plan), [true]);
}

#[test]
fn choosing_the_files_own_audio_track_or_no_audio_track_plays_the_files_own() {
    let (mp3, english) = (audio("mp3", false), audio("aac", true));
    let tracks = vec![video("h264"), mp3, english.clone(), subtitle()];
    for chosen in [Some(2), Some(0), Some(3), Some(9)] {
        let mut facts = Facts {
            audio: chosen,
            ..facts(Some(tracks.clone()))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{chosen:?}");
        assert_eq!(plan.tracks, [video("h264"), english.clone()]);
    }
}

#[test]
fn a_chosen_audio_track_the_device_cant_decode_is_converted_to_aac() {
    let mut facts = Facts {
        audio: Some(2),
        ..facts(Some(vec![
            video("h264"),
            audio("aac", true),
            audio("dts", false),
        ]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(1, Some(2))));
    assert_eq!(plan.tracks[1], audio("aac", false));
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
fn each_method_that_just_failed_on_the_device_is_ruled_out_and_the_plan_says_so() {
    use Method::{DirectPlay, Remux, Transcode};
    let failed_on_device = |method| {
        (
            Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
            vec![("method", method)],
        )
    };
    let cases = [
        (vec![], DirectPlay, vec![]),
        (
            vec![DirectPlay],
            Remux,
            vec![failed_on_device("PLAYBACK_METHOD_DIRECT_PLAY")],
        ),
        (
            vec![Remux, DirectPlay],
            Transcode,
            vec![
                failed_on_device("PLAYBACK_METHOD_DIRECT_PLAY"),
                failed_on_device("PLAYBACK_METHOD_REMUX"),
            ],
        ),
    ];
    for (failed, method, reasons) in cases {
        let mut facts = Facts {
            failed: BTreeSet::from_iter(failed),
            ..facts(Some(vec![video("h264"), audio("aac", true)]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(method));
        let failures: Vec<Given<'_>> = (given(&plan).into_iter())
            .filter(|(kind, _)| *kind == Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE)
            .collect();
        assert_eq!(failures, reasons, "{method:?}");
    }
}

#[test]
fn a_direct_play_or_remux_offers_the_text_subtitles_by_their_place_but_not_image_or_teletext() {
    let english = text_subtitle("subrip");
    let pgs = track(
        "hdmv_pgs_subtitle",
        false,
        TrackKind::Subtitle {
            image: true,
            header: false,
        },
    );
    let teletext = text_subtitle("dvb_teletext");
    let french = Track {
        language: Some("fre".to_owned()),
        ..english.clone()
    };
    let tracks = vec![
        video("h264"),
        english.clone(),
        audio("aac", true),
        pgs,
        teletext,
        french.clone(),
    ];

    let offered = [
        Subtitle {
            track: english,
            place: 1,
            format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
            url: "/media/versions/7/subtitles/1".to_owned(),
            font_urls: vec![],
        },
        Subtitle {
            track: french,
            place: 5,
            format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
            url: "/media/versions/7/subtitles/5".to_owned(),
            font_urls: vec![],
        },
    ];

    for (container, method) in [(MP4, Method::DirectPlay), (MKV, Method::Remux)] {
        let plan = build(&mut in_container(container, Some(tracks.clone())));

        assert_eq!(plan.method(), Some(method), "{container}");
        assert_eq!(plan.subtitles, offered, "{container}");
    }
}

#[test]
fn without_ffmpeg_on_the_server_or_hls_on_the_device_a_version_isnt_remuxed() {
    let tracks = Some(vec![video("h264"), audio("aac", true)]);
    let mut no_ffmpeg = in_container(MKV, tracks.clone());
    without_ffmpeg(&mut no_ffmpeg);
    let mut no_hls = in_container(MKV, tracks);
    no_hls.device.hls = false;

    let without_ffmpeg = build(&mut no_ffmpeg);
    let without_hls = build(&mut no_hls);

    assert_eq!(without_ffmpeg.method(), None);
    assert_eq!(
        kinds(&without_ffmpeg),
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
            Kind::PLAYBACK_REASON_KIND_SERVER_LACKS_FFMPEG,
        ]
    );
    assert_eq!(without_hls.method(), None);
    assert_eq!(
        kinds(&without_hls),
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_PLAY_HLS,
        ]
    );
}

#[test]
fn without_libx264_on_the_server_a_version_isnt_transcoded_but_still_remuxes_or_direct_plays() {
    let without_libx264 = |container, tracks| {
        let mut facts = in_container(container, Some(tracks));
        facts.can_do.libx264 = false;
        facts
    };
    let mut transcode = without_libx264(MP4, vec![video("hevc"), audio("aac", true)]);
    let mut remux = without_libx264(MKV, vec![video("h264"), audio("aac", true)]);
    let mut direct_play = without_libx264(MP4, vec![video("h264"), audio("aac", true)]);

    let transcode = build(&mut transcode);

    assert_eq!(transcode.method(), None);
    assert_eq!(
        kinds(&transcode),
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
            Kind::PLAYBACK_REASON_KIND_SERVER_CANT_ENCODE_VIDEO,
        ]
    );
    assert_eq!(build(&mut remux).method(), Some(Method::Remux));
    assert_eq!(build(&mut direct_play).method(), Some(Method::DirectPlay));
}

#[test]
fn a_plan_that_doesnt_transcode_doesnt_say_where_a_transcode_runs() {
    let tracks = || Some(vec![video("h264"), audio("aac", true)]);
    let mut no_hls = in_container(MKV, tracks());
    no_hls.device.hls = false;
    let mut no_ffmpeg = in_container(MKV, tracks());
    without_ffmpeg(&mut no_ffmpeg);
    let mut no_libx264 = in_container(MKV, tracks());
    no_libx264.can_do.libx264 = false;
    let cases = [
        ("no ffmpeg", no_ffmpeg),
        ("no libx264", no_libx264),
        ("unprobed Version", facts(None)),
        ("no HLS", no_hls),
        ("a Remux", in_container(MKV, tracks())),
    ];
    for (case, mut facts) in cases {
        let plan = build(&mut facts);

        assert!(
            !kinds(&plan).iter().any(|kind| {
                matches!(
                    kind,
                    Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE
                        | Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED
                )
            }),
            "{case}: {:?}",
            kinds(&plan)
        );
    }
}

fn rules_out_direct_play(_: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    vec![
        Reason::new(Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN, "")
            .ruling_out([Method::DirectPlay]),
    ]
}

fn rules_nothing_out(_: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    vec![Reason::new(
        Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT,
        "",
    )]
}

#[test]
fn a_later_step_cant_allow_what_an_earlier_step_ruled_out() {
    let mut reasons = vec![];

    let allowed = narrowed(
        &facts(None),
        None,
        &[rules_out_direct_play, rules_nothing_out],
        BTreeSet::from(Method::ALL),
        &mut reasons,
    );

    assert_eq!(allowed, BTreeSet::from([Method::Remux, Method::Transcode]));
    assert_eq!(reasons.len(), 2);
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
fn hdr_transcoded_for_a_device_that_cant_show_it_is_tone_mapped_and_the_plan_says_so() {
    let cases = [
        (Hdr::Hdr10, "HDR_FORMAT_HDR10"),
        (Hdr::Hlg, "HDR_FORMAT_HLG"),
        (
            dolby_vision_8(Some(BaseLayer::Hdr10)),
            "HDR_FORMAT_DOLBY_VISION",
        ),
        (
            dolby_vision_8(Some(BaseLayer::Hlg)),
            "HDR_FORMAT_DOLBY_VISION",
        ),
    ];
    for (hdr, name) in cases {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));

        let plan = build(&mut facts);

        assert_eq!(
            transcoded(&plan).video.tone_map,
            Some(ToneMapper::Software),
            "{name}"
        );
        assert_eq!(video_hdr(&plan.tracks), None, "{hdr:?} comes out SDR");
        let tone_mapped = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED);
        assert_eq!(
            kind_and_params(tone_mapped).1,
            [("hdrFormat", name)],
            "{name}"
        );
        assert!(tone_mapped.decided, "{name}");
    }
}

#[test]
fn without_zscale_hdr_isnt_tone_mapped_and_the_plan_says_its_colours_look_washed_out() {
    let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));
    facts.can_do.zscale = false;

    let plan = build(&mut facts);

    assert_eq!(transcoded(&plan).video.tone_map, None);
    let hdr10 = vec![("hdrFormat", "HDR_FORMAT_HDR10")];
    assert_eq!(
        given(&plan)[..3],
        [
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                vec![("codec", "hevc")]
            ),
            (
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
                hdr10.clone()
            ),
            (Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED, hdr10),
        ]
    );
}

#[test]
fn hdr_transcoded_for_a_device_that_shows_it_isnt_tone_mapped() {
    let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));
    facts.device.hdr = true;

    let plan = build(&mut facts);

    assert_eq!(transcoded(&plan).video.tone_map, None);
    assert_eq!(
        kinds(&plan),
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
            Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
        ]
    );
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
fn dolby_vision_over_sdr_isnt_tone_mapped_for_a_device_that_cant_show_hdr() {
    let hdr = dolby_vision_8(Some(BaseLayer::Sdr));
    let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));

    let plan = build(&mut facts);

    assert_eq!(transcoded(&plan).video.tone_map, None);
    assert!(
        !kinds(&plan).iter().any(|kind| matches!(
            kind,
            Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED
                | Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED
        )),
        "{:?}",
        kinds(&plan)
    );
    assert_eq!(video_hdr(&plan.tracks), None);
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
fn a_busy_server_remuxes_what_the_device_can_play_remuxed_and_says_so_as_context() {
    let mut facts = Facts {
        transcodes_full: Some(1),
        ..in_container(MKV, Some(vec![video("h264"), audio("aac", true)]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Remux));
    assert_eq!(
        given(&plan)[1],
        (
            Kind::PLAYBACK_REASON_KIND_SERVER_BUSY,
            vec![("maxTranscodes", "1")]
        )
    );
    assert_eq!(decided(&plan), [true, false], "a Remux would play anyway");
    assert_eq!(too_busy_to_play(facts), None);
}

#[test]
fn a_plan_only_a_transcode_would_play_is_too_busy_to_play_while_the_server_is_busy() {
    let hevc = || Some(vec![video("hevc"), audio("aac", true)]);
    let busy = |facts| Facts {
        transcodes_full: Some(2),
        ..facts
    };
    let mut without_hls = busy(facts(hevc()));
    without_hls.device.hls = false;

    assert_eq!(build(&mut busy(facts(hevc()))).method(), None);
    assert_eq!(too_busy_to_play(busy(facts(hevc()))), Some(2));
    assert_eq!(too_busy_to_play(facts(hevc())), None, "not busy");
    assert_eq!(too_busy_to_play(without_hls), None, "nothing plays it");
}

#[test]
fn audio_the_device_decodes_is_copied_into_a_remux() {
    let mut facts = in_container(MKV, Some(vec![video("h264"), audio("aac", true)]));

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(0, None)));
    assert_eq!(plan.tracks, [video("h264"), audio("aac", true)]);
}

#[test]
fn a_playback_limit_below_the_versions_picture_transcodes_it_down_to_fit_and_says_so() {
    let mut facts = Facts {
        picked_limit: PickedLimit::Picked(Some(PlaybackLimit::P720)),
        ..facts(Some(vec![video("h264"), audio("aac", true)]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    let scaled = TrackKind::Video {
        width: Some(1280),
        height: Some(720),
        frame_rate: Some(24.0),
        bit_depth: Some(8),
        hdr: None,
    };
    assert_eq!(
        plan.tracks,
        [track("h264", false, scaled), audio("aac", true)]
    );
    let scaled_down = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_PICTURE_SCALED_DOWN);
    assert_eq!(
        kind_and_params(scaled_down).1,
        [
            ("playbackLimit", "PLAYBACK_LIMIT_720P"),
            ("widthPixels", "1280"),
            ("heightPixels", "720"),
        ]
    );
    assert!(scaled_down.decided);
}

#[test]
fn outside_the_local_network_the_remote_default_limits_a_device_until_its_user_picks_one() {
    let remote = |picked_limit| Facts {
        picked_limit,
        network: Network::Remote(Some(PlaybackLimit::P720)),
        ..facts(Some(vec![video("h264"), audio("aac", true)]))
    };

    let defaulted = build(&mut remote(PickedLimit::Unpicked));
    let picked = build(&mut remote(PickedLimit::Picked(None)));

    assert_eq!(defaulted.method(), Some(Method::Transcode));
    assert_eq!(defaulted.limit, Some(PlaybackLimit::P720));
    let remote_default = reason_of(&defaulted, Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT);
    assert_eq!(
        kind_and_params(remote_default).1,
        [("playbackLimit", "PLAYBACK_LIMIT_720P")]
    );
    assert!(!remote_default.decided, "the limit's own reason decided it");
    assert_eq!(picked.method(), Some(Method::DirectPlay));
    assert_eq!(picked.limit, None);
    assert_eq!(picked.reasons, []);
}

#[test]
fn loopback_private_link_local_and_unique_local_addresses_are_on_the_local_network() {
    let cases = [
        ("127.0.0.1", true),
        ("10.1.2.3", true),
        ("172.16.0.9", true),
        ("192.168.1.5", true),
        ("169.254.10.20", true),
        ("::1", true),
        ("fe80::1", true),
        ("fd12:3456::1", true),
        ("203.0.113.7", false),
        ("100.64.0.1", false),
        ("2001:db8::1", false),
    ];
    for (address, local) in cases {
        assert_eq!(
            on_local_network(address.parse().unwrap()),
            local,
            "{address}"
        );
    }
}

#[test]
fn a_playback_limit_the_versions_picture_fits_or_original_leaves_it_direct_played() {
    let probed = Some(vec![video("h264"), audio("aac", true)]);
    let size_unknown = (
        Kind::PLAYBACK_REASON_KIND_PICTURE_SIZE_UNKNOWN,
        vec![("playbackLimit", "PLAYBACK_LIMIT_480P")],
    );
    let cases = [
        (Some(PlaybackLimit::P1080), probed.clone(), None),
        (None, probed, None),
        (Some(PlaybackLimit::P480), None, Some(size_unknown)),
    ];
    for (limit, tracks, last) in cases {
        let mut facts = Facts {
            picked_limit: PickedLimit::Picked(limit),
            ..facts(tracks)
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{limit:?}");
        assert_eq!(given(&plan).pop(), last, "{limit:?}");
    }
}

#[test]
fn a_transcode_scales_a_picture_down_to_a_playback_limit_keeping_its_shape_with_even_sides() {
    let cases = [
        ((1920, 800), PlaybackLimit::P720, (1280, 532)),
        ((3840, 2160), PlaybackLimit::P480, (852, 480)),
        ((1440, 1080), PlaybackLimit::P720, (960, 720)),
        ((3840, 2160), PlaybackLimit::P1080, (1920, 1080)),
        ((1080, 1920), PlaybackLimit::P720, (720, 1280)),
    ];
    for (picture, limit, scaled) in cases {
        let plan = build(&mut with_picture_limited_to(picture, limit));

        assert_eq!(
            transcoded(&plan).video.limit,
            Some(VideoLimit {
                size: Some(scaled),
                bits_per_second: limit.bits_per_second,
            }),
            "{picture:?} at {limit:?}"
        );
    }
}

#[test]
fn a_picture_within_a_playback_limit_on_its_short_side_is_direct_played() {
    let cases = [
        ((1920, 1080), PlaybackLimit::P1080),
        ((1280, 536), PlaybackLimit::P720),
        ((1080, 1920), PlaybackLimit::P1080),
    ];
    for (picture, limit) in cases {
        let plan = build(&mut with_picture_limited_to(picture, limit));

        assert_eq!(
            plan.playing,
            Some(Playing::DirectPlay),
            "{picture:?} at {limit:?}"
        );
    }
}

/// An H.264 Version with a picture `width` by `height`, whose User picked `limit`.
fn with_picture_limited_to((width, height): (i64, i64), limit: PlaybackLimit) -> Facts {
    let picture = TrackKind::Video {
        width: Some(width),
        height: Some(height),
        frame_rate: None,
        bit_depth: None,
        hdr: None,
    };
    Facts {
        picked_limit: PickedLimit::Picked(Some(limit)),
        ..facts(Some(vec![
            track("h264", false, picture),
            audio("aac", true),
        ]))
    }
}

fn pgs() -> Track {
    image_subtitle("hdmv_pgs_subtitle")
}

fn image_subtitle(codec: &str) -> Track {
    track(
        codec,
        false,
        TrackKind::Subtitle {
            image: true,
            header: false,
        },
    )
}

/// A Device that draws PGS subtitles itself.
fn drawing_pgs() -> DeviceDecoding {
    DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_PGS],
        ..firefox()
    }
}

#[test]
fn a_chosen_subtitle_the_device_cant_draw_is_burned_in_by_a_transcode_and_the_plan_names_it() {
    let cases = [
        (pgs(), firefox(), "PGS"),
        (vobsub(), drawing_pgs(), "VobSub"),
        (image_subtitle("dvb_subtitle"), drawing_pgs(), "DVB"),
        (image_subtitle("xsub"), drawing_pgs(), "XSUB"),
        (ass(), drawing_pgs(), "ASS"),
    ];
    for (chosen, device, name) in cases {
        let mut facts = Facts {
            subtitle: Some(2),
            device,
            ..facts(Some(vec![
                video("h264"),
                audio("aac", true),
                chosen.clone(),
            ]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode), "{}", chosen.codec);
        assert!(
            reason_of(&plan, Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN).decided,
            "{}",
            chosen.codec
        );
        assert_eq!(plan.burn_in, Some(2), "{name}");
        assert_eq!(plan.offers.burn_in, vec![2], "{name}");
    }
}

#[test]
fn a_device_that_draws_pgs_shows_each_pgs_track_itself_even_when_the_plan_transcodes() {
    for (video_codec, method) in [("h264", Method::DirectPlay), ("hevc", Method::Transcode)] {
        let tracks = vec![video(video_codec), audio("aac", true), pgs(), subtitle()];
        let mut facts = Facts {
            device: drawing_pgs(),
            ..facts(Some(tracks))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(method), "{video_codec}");
        assert_eq!(
            plan.subtitles,
            [
                Subtitle {
                    track: pgs(),
                    place: 2,
                    format: SubtitleFormat::SUBTITLE_FORMAT_PGS,
                    url: "/media/versions/7/subtitles/2".to_owned(),
                    font_urls: vec![],
                },
                Subtitle {
                    track: subtitle(),
                    place: 3,
                    format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
                    url: "/media/versions/7/subtitles/3".to_owned(),
                    font_urls: vec![],
                },
            ],
            "{video_codec}"
        );
        assert_eq!(plan.offers.burn_in, Vec::<usize>::new(), "{video_codec}");
    }
}

#[test]
fn a_pgs_track_asked_to_burn_in_on_a_device_that_draws_it_burns_nothing_in() {
    let mut facts = Facts {
        subtitle: Some(2),
        device: drawing_pgs(),
        ..facts(Some(vec![video("h264"), audio("aac", true), pgs()]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert!(
        (plan.reasons.iter())
            .all(|reason| reason.kind != Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN)
    );
    assert_eq!(plan.burn_in, None);
}

/// A VobSub Track whose file carries its header.
fn vobsub() -> Track {
    track(
        "dvd_subtitle",
        false,
        TrackKind::Subtitle {
            image: true,
            header: true,
        },
    )
}

/// A Device that draws VobSub subtitles itself.
fn drawing_vobsub() -> DeviceDecoding {
    DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_VOBSUB],
        ..firefox()
    }
}

#[test]
fn a_device_that_draws_vobsub_shows_each_vobsub_track_with_its_header_itself_even_when_transcoding()
{
    for (video_codec, method) in [("h264", Method::DirectPlay), ("hevc", Method::Transcode)] {
        let tracks = vec![video(video_codec), audio("aac", true), vobsub()];
        let mut facts = Facts {
            device: drawing_vobsub(),
            ..facts(Some(tracks))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(method), "{video_codec}");
        assert_eq!(
            plan.subtitles,
            [Subtitle {
                track: vobsub(),
                place: 2,
                format: SubtitleFormat::SUBTITLE_FORMAT_VOBSUB,
                url: "/media/versions/7/subtitles/2".to_owned(),
                font_urls: vec![],
            }],
            "{video_codec}"
        );
        assert_eq!(plan.offers.burn_in, Vec::<usize>::new(), "{video_codec}");
    }
}

#[test]
fn a_vobsub_track_without_its_header_is_burned_in_on_a_device_that_lists_vobsub() {
    let mut facts = Facts {
        subtitle: Some(2),
        device: drawing_vobsub(),
        ..facts(Some(vec![
            video("h264"),
            audio("aac", true),
            image_subtitle("dvd_subtitle"),
        ]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    let reason = reason_of(
        &plan,
        Kind::PLAYBACK_REASON_KIND_SUBTITLES_TRACK_CARRIES_NO_PALETTE,
    );
    assert!(reason.decided);
    assert_eq!(
        reason.text,
        "This Track carries no palette to draw its VobSub subtitles with, so the ones chosen \
         are burned into its picture, which only a Transcode does"
    );
    assert_eq!(plan.subtitles, []);
    assert_eq!(plan.burn_in, Some(2));
    assert_eq!(plan.offers.burn_in, vec![2]);
}

fn ass() -> Track {
    text_subtitle("ass")
}

/// A Device that draws ASS subtitles itself.
fn drawing_ass() -> DeviceDecoding {
    DeviceDecoding {
        subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_ASS],
        ..firefox()
    }
}

/// [`facts`] for an H.264 and AAC MP4 with an ASS Track third, whose file carries the fonts
/// `font_attachments`, played on `device` with the Track at `subtitle` asked to burn in.
fn with_ass(
    font_attachments: Vec<usize>,
    device: DeviceDecoding,
    subtitle: Option<usize>,
) -> Facts {
    let mut facts = Facts {
        device,
        subtitle,
        ..facts(Some(vec![video("h264"), audio("aac", true), ass()]))
    };
    if let Some(probe) = &mut facts.probe {
        probe.font_attachments = font_attachments;
    }
    facts
}

#[test]
fn an_ass_track_in_a_file_with_fonts_is_drawn_with_them_on_a_device_that_lists_ass() {
    let mut facts = with_ass(vec![1, 3], drawing_ass(), Some(2));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert_eq!(
        plan.subtitles,
        [Subtitle {
            track: ass(),
            place: 2,
            format: SubtitleFormat::SUBTITLE_FORMAT_ASS,
            url: "/media/versions/7/subtitles/2/ass".to_owned(),
            font_urls: vec![
                "/media/versions/7/fonts/0".to_owned(),
                "/media/versions/7/fonts/1".to_owned(),
            ],
        }]
    );
    assert_eq!(plan.burn_in, None);
    assert_eq!(plan.offers.burn_in, Vec::<usize>::new());
}

#[test]
fn an_ass_track_in_a_file_without_fonts_is_burned_in_on_a_device_that_lists_ass() {
    let mut facts = with_ass(vec![], drawing_ass(), Some(2));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert!(
        reason_of(
            &plan,
            Kind::PLAYBACK_REASON_KIND_SUBTITLES_FILE_CARRIES_NO_FONTS
        )
        .decided
    );
    assert_eq!(plan.burn_in, Some(2));
    assert_eq!(plan.offers.burn_in, vec![2]);
    assert_eq!(
        plan.subtitles[0].format,
        SubtitleFormat::SUBTITLE_FORMAT_WEBVTT
    );
    assert_eq!(plan.subtitles[0].url, "/media/versions/7/subtitles/2");
}

#[test]
fn an_ass_track_in_a_file_with_fonts_keeps_both_entries_on_a_device_that_doesnt_list_ass() {
    let plan = build(&mut with_ass(vec![0], drawing_pgs(), None));

    assert_eq!(
        plan.subtitles,
        [Subtitle {
            track: ass(),
            place: 2,
            format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
            url: "/media/versions/7/subtitles/2".to_owned(),
            font_urls: vec![],
        }]
    );
    assert_eq!(plan.offers.burn_in, vec![2]);
}

#[test]
fn automatic_transcodes_on_the_first_hardware_found_and_the_plan_says_so() {
    let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV, VAAPI], None);

    let plan = build(&mut facts);

    let on_hardware = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE);
    assert_eq!(
        kind_and_params(on_hardware).1,
        [("hardwareAcceleration", "HARDWARE_ACCELERATION_QSV")]
    );
    assert!(!on_hardware.decided);
    assert_eq!(
        transcoded(&plan).video,
        VideoConversion {
            hardware: Some(QSV),
            tone_map: None,
            limit: None,
            burn_in: None,
            in_band_headers: true,
        }
    );
}

#[test]
fn a_chosen_api_is_used_if_it_was_found_and_else_transcodes_run_in_software() {
    let cases = [
        (
            HardwareChoice::Api(Accel::Vaapi, None),
            vec![QSV, VAAPI],
            Some(Accel::Vaapi),
            (
                Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE,
                vec![("hardwareAcceleration", "HARDWARE_ACCELERATION_VAAPI")],
            ),
        ),
        (
            HardwareChoice::Api(Accel::Nvenc, None),
            vec![VAAPI],
            None,
            (
                Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_FAILED_TEST,
                vec![("hardwareAcceleration", "HARDWARE_ACCELERATION_NVENC")],
            ),
        ),
        (
            HardwareChoice::Off,
            vec![VAAPI],
            None,
            (Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_OFF, vec![]),
        ),
        (
            HardwareChoice::Automatic,
            vec![],
            None,
            (
                Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
                vec![],
            ),
        ),
    ];
    for (choice, found, on, reason) in cases {
        let label = format!("{choice:?}");
        let mut facts = hevc_on(choice, found, None);

        let plan = build(&mut facts);

        assert_eq!(given(&plan).last(), Some(&reason), "{label}");
        let accel = transcoded(&plan)
            .video
            .hardware
            .map(|hardware| hardware.accel);
        assert_eq!(accel, on, "{label}");
    }
}

/// A subtitle Track of `codec` in `language`, forced or not, SDH or not, and titled `title`.
fn subtitle_in(
    codec: &str,
    language: Option<&str>,
    (forced, sdh): (bool, bool),
    title: Option<&str>,
) -> Track {
    let image = codec == "hdmv_pgs_subtitle";
    Track {
        language: language.map(str::to_owned),
        title: title.map(str::to_owned),
        forced,
        sdh,
        ..track(
            codec,
            false,
            TrackKind::Subtitle {
                image,
                header: false,
            },
        )
    }
}

/// The place [`preferred_subtitle`] picks among `tracks`, in a file that carries no fonts.
fn preferred_among(
    preferred: &Preferred<SubtitlePreference>,
    tracks: &[Track],
    device: &DeviceDecoding,
) -> Option<usize> {
    let probe = facts(Some(tracks.to_vec())).probe.unwrap();
    preferred_subtitle(preferred, &probe, device)
}

/// A Show's or Movie's Subtitle Preference for a Track like `track`.
fn like(track: &Track) -> Preferred<SubtitlePreference> {
    Preferred {
        preference: SubtitlePreference::Like(Description::of(track)),
        users_own: false,
        foreign_audio_only: false,
    }
}

/// The User's own Subtitle Preference for a Track like `track`, which keeps no title.
fn users_own_like(track: &Track) -> Preferred<SubtitlePreference> {
    Preferred {
        preference: SubtitlePreference::Like(Description {
            title: None,
            ..Description::of(track)
        }),
        users_own: true,
        foreign_audio_only: false,
    }
}

const NOT_FORCED_OR_SDH: (bool, bool) = (false, false);

#[test]
fn a_preferred_subtitle_must_be_in_its_language_and_as_forced() {
    let device = firefox();
    let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
    let tracks = [
        video("h264"),
        subtitle_in("subrip", Some("eng"), (true, false), None),
        subtitle_in("subrip", Some("ger"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", Some("en"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", None, NOT_FORCED_OR_SDH, None),
    ];

    assert_eq!(preferred_among(&like(&english), &tracks, &device), Some(3));
    let unlabelled = subtitle_in("subrip", None, NOT_FORCED_OR_SDH, None);
    assert_eq!(
        preferred_among(&like(&unlabelled), &tracks, &device),
        Some(4)
    );
    let french = subtitle_in("subrip", Some("fre"), NOT_FORCED_OR_SDH, None);
    assert_eq!(preferred_among(&like(&french), &tracks, &device), None);
    let off = Preferred {
        preference: SubtitlePreference::Off,
        users_own: false,
        foreign_audio_only: false,
    };
    assert_eq!(preferred_among(&off, &tracks, &device), None);
}

#[test]
fn the_users_own_preference_ranks_sdh_then_one_the_device_shows_whatever_the_title() {
    let device = firefox();
    let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
    let english_sdh = subtitle_in("subrip", Some("eng"), (false, true), None);
    let tracks = [
        subtitle_in("subrip", Some("eng"), (false, true), None),
        subtitle_in("hdmv_pgs_subtitle", Some("eng"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("English")),
        subtitle_in("subrip", Some("eng"), (true, false), None),
    ];

    assert_eq!(
        preferred_among(&users_own_like(&english), &tracks, &device),
        Some(2)
    );
    assert_eq!(preferred_among(&like(&english), &tracks, &device), Some(1));
    assert_eq!(
        preferred_among(&users_own_like(&english_sdh), &tracks, &device),
        Some(0)
    );
    assert_eq!(
        preferred_among(&users_own_like(&english), &tracks[3..], &device),
        None,
        "a full Preference never takes a forced-only Track"
    );
}

#[test]
fn a_preferred_subtitle_ranks_sdh_then_title_then_one_the_device_shows_then_the_first() {
    let device = firefox();
    let wanted = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full"));
    let cases = [
        (
            "the same SDH beats the same title",
            vec![
                subtitle_in("subrip", Some("eng"), (false, true), Some("Full")),
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Signs")),
            ],
            Some(1),
        ),
        (
            "the same title, in any case, beats one the Device shows",
            vec![
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None),
                subtitle_in(
                    "hdmv_pgs_subtitle",
                    Some("eng"),
                    NOT_FORCED_OR_SDH,
                    Some("FULL"),
                ),
            ],
            Some(1),
        ),
        (
            "one the Device shows beats one that needs Burn-in",
            vec![
                subtitle_in(
                    "hdmv_pgs_subtitle",
                    Some("eng"),
                    NOT_FORCED_OR_SDH,
                    Some("Full"),
                ),
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
            ],
            Some(1),
        ),
        (
            "a tie goes to the first",
            vec![
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
                subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
            ],
            Some(0),
        ),
        (
            "one neither shown nor burned in never matches",
            vec![subtitle_in(
                "dvb_teletext",
                Some("eng"),
                NOT_FORCED_OR_SDH,
                Some("Full"),
            )],
            None,
        ),
    ];

    for (case, tracks, place) in cases {
        let preferred = preferred_among(&like(&wanted), &tracks, &device);
        assert_eq!(preferred, place, "{case}");
    }
}

#[test]
fn a_preferred_text_subtitle_is_shown_by_the_device_and_the_plan_starts_with_it() {
    let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
    let tracks = vec![video("h264"), audio("aac", true), english.clone()];
    let mut facts = Facts {
        subtitle_preference: Some(like(&english)),
        ..facts(Some(tracks))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::DirectPlay));
    assert_eq!((plan.burn_in, plan.starting_subtitle), (None, Some(2)));
}

#[test]
fn a_preferred_image_subtitle_is_burned_in_or_dropped_as_a_chosen_one_is() {
    let pgs = subtitle_in("hdmv_pgs_subtitle", Some("eng"), NOT_FORCED_OR_SDH, None);
    let tracks = vec![video("h264"), audio("aac", true), pgs.clone()];
    let preferred = || Facts {
        subtitle_preference: Some(like(&pgs)),
        ..facts(Some(tracks.clone()))
    };

    let burned_in = build(&mut preferred());
    let mut no_ffmpeg = preferred();
    without_ffmpeg(&mut no_ffmpeg);
    let dropped = build(&mut no_ffmpeg);

    assert_eq!(burned_in.method(), Some(Method::Transcode));
    assert_eq!(
        (burned_in.burn_in, burned_in.starting_subtitle),
        (Some(2), Some(2))
    );
    assert_eq!(dropped.method(), Some(Method::DirectPlay));
    assert_eq!((dropped.burn_in, dropped.starting_subtitle), (None, None));
    assert!(
        reason_of(
            &dropped,
            Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED
        )
        .decided
    );
}

const FORCED: (bool, bool) = (true, false);

/// Full and forced-only English subtitles, at places 3 and 4 after a video and two audio
/// Tracks.
fn english_subtitles() -> [Track; 2] {
    [
        subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None),
        subtitle_in("subrip", Some("eng"), FORCED, None),
    ]
}

/// The User's own Subtitle Preference for full English subtitles only over audio in another
/// language.
fn english_over_foreign_audio() -> Preferred<SubtitlePreference> {
    let [full, _] = english_subtitles();
    Preferred {
        foreign_audio_only: true,
        ..users_own_like(&full)
    }
}

/// The subtitle a plan starts with by `preferred`, of a Version with a video Track, then the
/// audio Tracks `audio`, then the subtitle Tracks `subtitles`.
fn starting_subtitle(
    preferred: Preferred<SubtitlePreference>,
    audio: &[Track],
    subtitles: &[Track],
) -> Option<usize> {
    let tracks = [&[video("h264")], audio, subtitles].concat();
    let mut facts = Facts {
        subtitle_preference: Some(preferred),
        ..facts(Some(tracks))
    };
    build(&mut facts).starting_subtitle
}

#[test]
fn subtitles_wanted_over_foreign_audio_start_full_over_japanese_or_untagged_audio() {
    let japanese = audio_in("aac", "jpn", None, true);
    let untagged = audio("aac", true);
    let undetermined = audio_in("aac", "und", None, true);
    let not_iso_639 = audio_in("aac", "en-US", None, true);
    let english = audio_in("aac", "eng", None, false);

    for (case, played) in [
        ("Japanese", japanese),
        ("untagged", untagged),
        ("`und`", undetermined),
        ("`en-US`", not_iso_639),
    ] {
        let audio = [played, english.clone()];
        assert_eq!(
            starting_subtitle(english_over_foreign_audio(), &audio, &english_subtitles()),
            Some(3),
            "{case}"
        );
    }
}

#[test]
fn subtitles_wanted_over_foreign_audio_start_forced_or_none_over_audio_in_their_language() {
    let audio = || {
        [
            audio_in("aac", "en", None, true),
            audio_in("aac", "jpn", None, false),
        ]
    };
    let [full, _] = english_subtitles();

    let forced = starting_subtitle(english_over_foreign_audio(), &audio(), &english_subtitles());
    let none = starting_subtitle(english_over_foreign_audio(), &audio(), &[full]);

    assert_eq!(forced, Some(4));
    assert_eq!(none, None);
}

#[test]
fn subtitles_wanted_over_foreign_audio_look_at_the_files_own_once_the_plan_drops_the_asked() {
    let tracks = [
        vec![
            video("h264"),
            audio_in("aac", "eng", None, true),
            audio_in("aac", "jpn", None, false),
        ],
        english_subtitles().to_vec(),
    ]
    .concat();
    let japanese_asked = || Facts {
        audio: Some(2),
        subtitle_preference: Some(english_over_foreign_audio()),
        ..facts(Some(tracks.clone()))
    };

    let remuxed = build(&mut japanese_asked());
    let mut no_ffmpeg = japanese_asked();
    without_ffmpeg(&mut no_ffmpeg);
    let dropped = build(&mut no_ffmpeg);

    assert_eq!(
        (remuxed.audio, remuxed.starting_subtitle),
        (Some(2), Some(3))
    );
    assert_eq!(
        (dropped.audio, dropped.starting_subtitle),
        (Some(1), Some(4))
    );
}

#[test]
fn a_shows_subtitle_preference_starts_its_subtitles_over_audio_in_their_language() {
    let [full, _] = english_subtitles();
    let audio = [
        audio_in("aac", "eng", None, true),
        audio_in("aac", "jpn", None, false),
    ];

    assert_eq!(
        starting_subtitle(like(&full), &audio, &english_subtitles()),
        Some(3)
    );
}

/// A stereo audio Track of `codec` in `language`, titled `title`, played by default or not.
fn audio_in(codec: &str, language: &str, title: Option<&str>, default: bool) -> Track {
    Track {
        language: Some(language.to_owned()),
        title: title.map(str::to_owned),
        ..audio(codec, default)
    }
}

/// The place [`audio_asked`] gives for the Audio Preference `preferred` among `tracks`.
fn preferred_audio_among(preferred: Preferred<Description>, tracks: &[Track]) -> Option<usize> {
    let facts = facts(Some(tracks.to_vec()));
    let asked = AskedAudio::Preferred(Some(preferred));
    audio_asked(asked, facts.probe.as_ref(), &facts.device)
}

/// A Show's or Movie's Audio Preference for a Track like `track`.
fn audio_like(track: &Track) -> Preferred<Description> {
    Preferred {
        preference: Description::of(track),
        users_own: false,
        foreign_audio_only: false,
    }
}

#[test]
fn a_preferred_audio_track_must_be_in_its_language_or_none_is_chosen() {
    let japanese = audio_in("aac", "jpn", None, false);
    let tracks = [
        video("h264"),
        audio_in("aac", "eng", None, true),
        audio_in("aac", "ja", None, false),
    ];

    assert_eq!(
        preferred_audio_among(audio_like(&japanese), &tracks),
        Some(2)
    );
    let french = audio_in("aac", "fre", None, false);
    assert_eq!(preferred_audio_among(audio_like(&french), &tracks), None);
    let none = facts(Some(tracks.to_vec()));
    let unset = audio_asked(
        AskedAudio::Preferred(None),
        none.probe.as_ref(),
        &none.device,
    );
    assert_eq!(unset, None);
}

#[test]
fn a_preferred_audio_track_ranks_title_then_one_the_device_decodes_then_the_default_then_the_first()
{
    let wanted = audio_in("aac", "eng", Some("Commentary"), false);
    let cases = [
        (
            "the same title, in any case, beats one the Device decodes",
            vec![
                audio_in("aac", "eng", None, true),
                audio_in("dts", "eng", Some("COMMENTARY"), false),
            ],
            Some(1),
        ),
        (
            "one the Device decodes beats the default",
            vec![
                audio_in("dts", "eng", None, true),
                audio_in("aac", "eng", None, false),
            ],
            Some(1),
        ),
        (
            "the default beats the first",
            vec![
                audio_in("aac", "eng", None, false),
                audio_in("aac", "eng", None, true),
            ],
            Some(1),
        ),
        (
            "a tie goes to the first",
            vec![
                audio_in("aac", "eng", None, false),
                audio_in("aac", "eng", None, false),
            ],
            Some(0),
        ),
    ];

    for (case, tracks, place) in cases {
        assert_eq!(
            preferred_audio_among(audio_like(&wanted), &tracks),
            place,
            "{case}"
        );
    }
}

#[test]
fn the_users_own_audio_preference_skips_the_title_rank() {
    let tracks = [
        audio_in("aac", "eng", Some("Commentary"), false),
        audio_in("dts", "eng", None, true),
        audio_in("aac", "eng", Some("Stereo"), true),
    ];
    let users_own = Preferred {
        preference: Description {
            language: Some("en".to_owned()),
            ..Description::default()
        },
        users_own: true,
        foreign_audio_only: false,
    };
    let untitled = audio_in("aac", "eng", None, false);

    assert_eq!(preferred_audio_among(users_own, &tracks), Some(2));
    assert_eq!(
        preferred_audio_among(audio_like(&untitled), &tracks),
        Some(1),
        "a Show's untitled Preference wants the untitled Track"
    );
}

#[test]
fn a_preferred_audio_track_is_remuxed_or_dropped_as_a_chosen_one_is() {
    let german = audio_in("aac", "ger", None, false);
    let tracks = vec![
        video("h264"),
        audio_in("aac", "eng", None, true),
        german.clone(),
    ];
    let preferred = || {
        let mut facts = facts(Some(tracks.clone()));
        let asked = AskedAudio::Preferred(Some(audio_like(&german)));
        facts.audio = audio_asked(asked, facts.probe.as_ref(), &facts.device);
        facts
    };

    let remuxed = build(&mut preferred());
    let mut no_ffmpeg = preferred();
    without_ffmpeg(&mut no_ffmpeg);
    let dropped = build(&mut no_ffmpeg);

    assert_eq!(remuxed.method(), Some(Method::Remux));
    assert_eq!(remuxed.audio, Some(2));
    assert_eq!(dropped.method(), Some(Method::DirectPlay));
    assert_eq!(dropped.audio, Some(1));
    assert!(reason_of(&dropped, Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED).decided);
}

#[test]
fn a_subtitle_choice_that_isnt_an_image_or_ass_subtitle_burns_nothing_in() {
    let teletext = text_subtitle("dvb_teletext");
    let tracks = vec![video("h264"), audio("aac", true), teletext, subtitle()];
    for chosen in [None, Some(1), Some(2), Some(3), Some(9)] {
        let mut facts = Facts {
            subtitle: chosen,
            ..facts(Some(tracks.clone()))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay), "{chosen:?}");
        assert_eq!(plan.reasons, [], "{chosen:?}");
    }
}

const VAAPI: Hardware = Hardware {
    accel: Accel::Vaapi,
    gpu: None,
    tone_maps: false,
};
const QSV: Hardware = Hardware {
    accel: Accel::Qsv,
    gpu: None,
    tone_maps: true,
};

/// VAAPI on a dedicated AMD Radeon RX 9070 XT, then on an integrated AMD GPU, as Automatic
/// orders them, with the integrated one on `renderD128`.
#[cfg(unix)]
fn vaapi_on_two_gpus(root: &std::path::Path) -> Vec<Hardware> {
    vaapi_on_two_gpus_at(root, ["renderD128", "renderD129"])
}

/// As [`vaapi_on_two_gpus`], with the integrated GPU and the dedicated one on `nodes`.
#[cfg(unix)]
fn vaapi_on_two_gpus_at(root: &std::path::Path, nodes: [&str; 2]) -> Vec<Hardware> {
    use crate::ffmpeg::{fake, find_gpus};

    let (sys, dri) = (root.join("sys"), root.join("dri"));
    let [integrated, dedicated] = nodes;
    fake::gpu(&sys, &dri, integrated, "0000:18:00.0", "0x1002");
    fake::pci_function(&sys, "0000:18:00.2", "0x1022");
    fake::gpu(&sys, &dri, dedicated, "0000:03:00.0", "0x1002");
    let mesa = "VAAPI driver: Mesa Gallium driver 26.2.6 for AMD Radeon RX 9070 XT (radeonsi)";
    find_gpus(&dri, &sys.join("class/drm"))
        .into_iter()
        .map(|gpu| Hardware {
            gpu: Some(if gpu.id() == "0000:03:00.0" {
                gpu.with_model(mesa)
            } else {
                gpu
            }),
            ..VAAPI
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn a_stored_api_runs_on_the_first_gpu_that_passed_with_it_and_the_plan_names_it() {
    let root = tempfile::tempdir().unwrap();
    let found = vaapi_on_two_gpus(root.path());
    for choice in [
        HardwareChoice::Automatic,
        HardwareChoice::Api(Accel::Vaapi, None),
    ] {
        let mut facts = hevc_on(choice.clone(), found.clone(), None);

        let plan = build(&mut facts);

        assert_eq!(
            reason_of(&plan, Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE).text,
            "It Transcodes on the AMD Radeon RX 9070 XT with VAAPI",
            "{choice:?}"
        );
        assert_eq!(transcoded(&plan).video.hardware.as_ref(), found.first());
    }
}

#[cfg(unix)]
#[test]
fn a_transcode_on_an_integrated_gpu_names_it_as_integrated() {
    let root = tempfile::tempdir().unwrap();
    let integrated = vaapi_on_two_gpus(root.path()).split_off(1);
    let mut facts = hevc_on(HardwareChoice::Automatic, integrated, None);

    let plan = build(&mut facts);

    assert_eq!(
        reason_of(&plan, Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE).text,
        "It Transcodes on the AMD integrated GPU with VAAPI"
    );
}

/// HEVC 4K that Firefox can't decode, on a Server whose ffmpeg found `found` and whose admin
/// chose `choice`.
fn hevc_on(choice: HardwareChoice, found: Vec<Hardware>, hdr: Option<Hdr>) -> Facts {
    let mut facts = Facts {
        hardware_choice: choice,
        ..in_container(MKV, Some(vec![hdr_video("hevc", hdr), audio("aac", true)]))
    };
    facts.can_do.gpus = found.iter().filter_map(|found| found.gpu.clone()).collect();
    facts.can_do.hardware = found;
    facts
}

/// The choice of VAAPI on the GPU at the PCI address `id`, last found as `name`.
fn vaapi_picked(id: &str, name: &str) -> HardwareChoice {
    let gpu = PickedGpu {
        id: id.to_owned(),
        name: name.to_owned(),
    };
    HardwareChoice::Api(Accel::Vaapi, Some(gpu))
}

#[cfg(unix)]
#[test]
fn a_picked_gpu_runs_every_transcode_and_is_found_by_its_address_whatever_its_render_node() {
    for nodes in [["renderD128", "renderD129"], ["renderD129", "renderD128"]] {
        let root = tempfile::tempdir().unwrap();
        let found = vaapi_on_two_gpus_at(root.path(), nodes);
        let picked = vaapi_picked("0000:18:00.0", "AMD integrated GPU");
        let mut facts = hevc_on(picked, found.clone(), None);

        let plan = build(&mut facts);

        assert_eq!(
            plan.reasons.last().unwrap().text,
            "It Transcodes on the AMD integrated GPU with VAAPI"
        );
        assert_eq!(transcoded(&plan).video.hardware.as_ref(), found.get(1));
    }
}

#[cfg(unix)]
#[test]
fn a_picked_gpu_missing_at_startup_leaves_transcodes_in_software_though_another_passed() {
    let root = tempfile::tempdir().unwrap();
    let found = vaapi_on_two_gpus(root.path());
    let mut facts = hevc_on(
        vaapi_picked("0000:0a:00.0", "integrated GPU (AMD Ryzen 7 7840U)"),
        found,
        None,
    );

    let plan = build(&mut facts);

    assert_eq!(
        plan.reasons.last().unwrap().text,
        "Integrated GPU (AMD Ryzen 7 7840U) wasn't found when this Server started, so it \
         Transcodes in software"
    );
    assert_eq!(transcoded(&plan).video.hardware, None);
}

#[cfg(unix)]
#[test]
fn a_picked_gpu_that_failed_its_test_leaves_transcodes_in_software_and_says_so() {
    let root = tempfile::tempdir().unwrap();
    let found = vaapi_on_two_gpus(root.path());
    let mut facts = hevc_on(
        vaapi_picked("0000:18:00.0", "AMD integrated GPU"),
        found,
        None,
    );
    facts.can_do.hardware.truncate(1);

    let plan = build(&mut facts);

    assert_eq!(
        plan.reasons.last().unwrap().text,
        "AMD integrated GPU didn't pass its VAAPI test when this Server started, so it \
         Transcodes in software"
    );
    assert_eq!(transcoded(&plan).video.hardware, None);
}

#[test]
fn hdr_is_tone_mapped_on_the_gpu_if_it_passed_that_test_and_else_in_software() {
    let cases = [
        (QSV, Some(ToneMapper::Hardware)),
        (VAAPI, Some(ToneMapper::Software)),
    ];
    for (hardware, tone_map) in cases {
        let mut facts = hevc_on(
            HardwareChoice::Automatic,
            vec![hardware.clone()],
            Some(Hdr::Hdr10),
        );

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, tone_map, "{hardware:?}");
        let tone_mapped = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED);
        assert_eq!(
            kind_and_params(tone_mapped).1,
            [("hdrFormat", "HDR_FORMAT_HDR10")]
        );
    }
}

#[test]
fn without_zscale_hdr_is_tone_mapped_by_hardware_that_can() {
    let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));
    facts.can_do.zscale = false;

    let plan = build(&mut facts);

    assert_eq!(transcoded(&plan).video.tone_map, Some(ToneMapper::Hardware));
}

#[test]
fn a_hardware_transcode_of_hdr_falls_back_to_tone_mapping_in_software_with_zscale() {
    let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));

    let plan = build(&mut facts);

    let on_hardware = transcoded(&plan);
    let fallback = falls_back_to(&plan).unwrap();
    assert_eq!(
        fallback.transcode,
        Transcode {
            video: VideoConversion {
                hardware: None,
                tone_map: Some(ToneMapper::Software),
                ..on_hardware.video
            },
            ..on_hardware
        }
    );
    let fell_back = (
        Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
        vec![("hardwareAcceleration", "HARDWARE_ACCELERATION_QSV")],
    );
    let mut expected = given(&plan);
    expected.retain(|(kind, _)| *kind != Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE);
    expected.push(fell_back);
    let reasons: Vec<Given<'_>> = fallback.reasons.iter().map(kind_and_params).collect();
    assert_eq!(reasons, expected);
}

#[test]
fn without_zscale_a_hardware_transcode_of_hdr_falls_back_without_tone_mapping_and_says_so() {
    let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));
    facts.can_do.zscale = false;

    let plan = build(&mut facts);

    let fallback = falls_back_to(&plan).unwrap();
    assert_eq!(fallback.transcode.video.hardware, None);
    assert_eq!(fallback.transcode.video.tone_map, None);
    let fallback_kinds: Vec<Kind> = fallback.reasons.iter().map(|reason| reason.kind).collect();
    assert_eq!(
        fallback_kinds,
        [
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
            Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
            Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED,
        ],
        "on hardware: {:?}",
        kinds(&plan)
    );
}

#[test]
fn a_software_transcode_has_no_fallback() {
    let mut facts = hevc_on(HardwareChoice::Off, vec![QSV], Some(Hdr::Hdr10));

    let plan = build(&mut facts);

    assert_eq!(transcoded(&plan).video.hardware, None);
    assert_eq!(falls_back_to(&plan), None);
}

#[test]
fn the_transcode_cap_is_unlimited_by_default_only_with_hardware_in_use() {
    assert_eq!(
        default_max_transcodes(&hevc_on(HardwareChoice::Automatic, vec![VAAPI], None)),
        u32::MAX
    );
    assert_eq!(
        default_max_transcodes(&hevc_on(HardwareChoice::Off, vec![VAAPI], None)),
        2
    );
    assert_eq!(
        default_max_transcodes(&hevc_on(HardwareChoice::Automatic, vec![], None)),
        2
    );
}

/// An ffprobe in `bin` that finds an H.264 video and an AAC audio Track in any file. It's a
/// shell script.
#[cfg(unix)]
fn h264_aac_ffprobe(bin: &std::path::Path) -> crate::ffmpeg::Ffmpeg {
    let script = r#"#!/bin/sh
echo '{"streams": [{"codec_name": "h264", "codec_type": "video"},
                   {"codec_name": "aac", "codec_type": "audio"}],
       "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2"}}'
"#;
    crate::ffmpeg::Ffmpeg {
        ffprobe: Ok(crate::ffmpeg::fake::script(bin, "ffprobe", script)),
        ..crate::ffmpeg::Ffmpeg::default()
    }
}

fn ripley_on_the_nostromo() -> crate::users::Caller {
    use crate::users::{Caller, Device, Permissions, User};

    Caller {
        user: User {
            id: 1,
            name: "Ripley".to_owned(),
            permissions: Permissions::NEW_USER,
            created_at: 0,
        },
        device: Device {
            id: 1,
            name: "Nostromo".to_owned(),
            created_at: 0,
            last_seen_at: 0,
            authenticated_at: 0,
        },
    }
}

// The fake ffprobe is a shell script. The Version is stored without a scan, so no background
// pass probes it.
#[cfg(unix)]
#[tokio::test]
async fn a_version_without_tracks_is_probed_before_its_playback_plan_is_built() {
    use std::sync::Arc;

    use crate::{
        db::Database,
        events::EventBus,
        ffmpeg::{Abilities, Ffmpeg},
        library::contents::{
            Contents,
            fixture::{MatchedMovie, tmdb_movie},
        },
        playback::{
            Asked, AskedAudio, AskedSubtitle, Planned, Probing, StartedPlan, plan,
            running::RunningPlans, subtitles,
        },
        plugins::ExtensionPoints,
        server_settings::{ServerUrl, Settings},
    };

    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let root = media.path().canonicalize().unwrap().join("library");
    std::fs::create_dir(&root).unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = tmdb_movie(348, "Alien");
    let version = MatchedMovie::new(&db, &root, alien).await.version;
    let probing = Probing::new(
        Arc::clone(&db),
        Arc::new(h264_aac_ffprobe(data.path())),
        subtitles::Cache::new(data.path()),
    );
    let settings = Settings::new(Arc::clone(&db), data.path().join("transcodes"));
    let ffmpeg = Arc::new(Ffmpeg::default());
    let running = RunningPlans::new(
        settings.playback(),
        Arc::clone(&ffmpeg),
        Contents::new(Arc::clone(&db)),
        probing.clone(),
        EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
    );

    let asked = Asked {
        version,
        decoding: DeviceDecoding::default(),
        failed: BTreeSet::new(),
        audio: AskedAudio::Preferred(None),
        subtitle: AskedSubtitle::None,
        limit: PickedLimit::Unpicked,
        client: std::net::Ipv4Addr::LOCALHOST.into(),
    };
    let planned = plan(
        &probing,
        &Abilities::new(ffmpeg),
        &running,
        &ripley_on_the_nostromo(),
        asked,
        settings.playback().now(),
    )
    .await
    .unwrap();

    let tracks = match planned {
        Some(Planned::Started(StartedPlan { plan, .. }) | Planned::Unplayable(plan)) => plan.tracks,
        None => vec![],
    };
    let codecs: Vec<&str> = tracks.iter().map(|track| track.codec.as_str()).collect();
    assert_eq!(codecs, ["h264", "aac"]);
}

/// A 1080p H.264 Version this Device Direct Plays, with a second audio Track and a PGS one.
fn direct_played() -> Facts {
    let pgs = track(
        "hdmv_pgs_subtitle",
        false,
        TrackKind::Subtitle {
            image: true,
            header: false,
        },
    );
    facts(Some(vec![
        video("h264"),
        audio("aac", true),
        audio("aac", false),
        pgs,
    ]))
}

fn without_ffmpeg(facts: &mut Facts) {
    facts.can_do.runs = false;
}

/// A Dolby Vision profile 5 picture, which this Device shows, in place of the video.
fn in_dolby_vision_5(facts: &mut Facts) {
    let profile_5 = Hdr::DolbyVision {
        profile: 5,
        base_layer: None,
    };
    if let Some(probe) = &mut facts.probe {
        probe.tracks[0] = hdr_video("h264", Some(profile_5));
    }
    facts.device.hdr = true;
}

fn after_a_failed_transcode(facts: &mut Facts) {
    facts.failed = BTreeSet::from([Method::Transcode]);
}

fn while_busy(facts: &mut Facts) {
    facts.transcodes_full = Some(2);
}

/// What a test changes in the facts it plans for.
type Change = fn(&mut Facts);

/// Each way a Transcode is ruled out, by a lasting fact or only for now, no ffmpeg first.
const NO_TRANSCODE: [(&str, Change); 4] = [
    ("no ffmpeg", without_ffmpeg),
    ("Dolby Vision profile 5", in_dolby_vision_5),
    ("a failed Transcode", after_a_failed_transcode),
    ("a busy Server", while_busy),
];

fn picking_720p(facts: &mut Facts) {
    facts.picked_limit = PickedLimit::Picked(Some(PlaybackLimit::P720));
}

fn remote_at_720p(facts: &mut Facts) {
    facts.network = Network::Remote(Some(PlaybackLimit::P720));
}

fn burning_in_pgs(facts: &mut Facts) {
    facts.subtitle = Some(3);
}

#[test]
fn a_choice_only_a_transcode_honours_is_dropped_when_it_cant_transcode_and_that_decided_it() {
    let limit_dropped = || {
        (
            Kind::PLAYBACK_REASON_KIND_PLAYBACK_LIMIT_DROPPED,
            vec![("playbackLimit", "PLAYBACK_LIMIT_720P")],
        )
    };
    let choices: [(&str, Change, Given<'_>); 3] = [
        ("720p", picking_720p, limit_dropped()),
        ("the remote default", remote_at_720p, limit_dropped()),
        (
            "PGS",
            burning_in_pgs,
            (Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED, vec![]),
        ),
    ];
    for (chosen, choose, dropped) in choices {
        for (ruled_out, rule_out) in NO_TRANSCODE {
            let mut facts = direct_played();
            choose(&mut facts);
            rule_out(&mut facts);

            let plan = build(&mut facts);

            let case = format!("{chosen} with {ruled_out}");
            assert_eq!(plan.method(), Some(Method::DirectPlay), "{case}");
            assert_eq!((plan.limit, plan.burn_in), (None, None), "{case}");
            assert_eq!(given(&plan).last(), Some(&dropped), "{case}");
            assert!(reason_of(&plan, dropped.0).decided, "{case}");
            assert_eq!(too_busy_to_play(facts), None, "{case}");
        }
    }
}

#[test]
fn the_remote_default_reason_says_the_device_asks_for_it_when_it_is_dropped() {
    let mut facts = direct_played();
    remote_at_720p(&mut facts);
    without_ffmpeg(&mut facts);

    let plan = build(&mut facts);

    assert!(kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_PLAYBACK_LIMIT_DROPPED));
    assert_eq!(
        reason_of(&plan, Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT).text,
        "This Device is outside the local network, so it asks for the default Playback \
         Limit for remote playback, 720p"
    );
}

#[test]
fn a_choice_only_a_transcode_honours_is_kept_while_it_can_transcode() {
    for choose in [picking_720p, remote_at_720p, burning_in_pgs] {
        let mut facts = direct_played();
        choose(&mut facts);

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert!(facts.dropped.is_empty());
    }
}

/// [`direct_played`] with its second audio Track `chosen`, and chosen.
fn choosing_audio(chosen: &Track) -> Facts {
    let mut facts = direct_played();
    if let Some(probe) = &mut facts.probe {
        probe.tracks[2] = chosen.clone();
    }
    facts.audio = Some(2);
    facts
}

fn after_a_failed_remux_and_transcode(facts: &mut Facts) {
    facts.failed = BTreeSet::from([Method::Remux, Method::Transcode]);
}

fn after_a_failed_remux_while_busy(facts: &mut Facts) {
    facts.failed = BTreeSet::from([Method::Remux]);
    while_busy(facts);
}

/// Commentary audio this Device decodes, and DTS, which it can't: DTS keeps it from Direct
/// Playing only while it's chosen.
fn commentaries() -> [Track; 2] {
    [audio("aac", false), audio("dts", false)]
}

#[test]
fn a_chosen_audio_track_is_dropped_when_neither_a_remux_nor_a_transcode_is_left() {
    let cases: [(&str, Change); 3] = [
        ("no ffmpeg", without_ffmpeg),
        (
            "a failed Remux and Transcode",
            after_a_failed_remux_and_transcode,
        ),
        (
            "a failed Remux on a busy Server",
            after_a_failed_remux_while_busy,
        ),
    ];
    for chosen in commentaries() {
        for (ruled_out, rule_out) in cases {
            let mut facts = choosing_audio(&chosen);
            rule_out(&mut facts);

            let plan = build(&mut facts);

            let case = format!("{} with {ruled_out}", chosen.codec);
            assert_eq!(plan.method(), Some(Method::DirectPlay), "{case}");
            assert_eq!(plan.audio, Some(1), "{case}");
            assert_eq!(
                kinds(&plan).last(),
                Some(&Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED),
                "{case}"
            );
        }
    }
}

#[test]
fn a_chosen_audio_track_is_remuxed_while_only_a_transcode_is_ruled_out() {
    for chosen in commentaries() {
        for (ruled_out, rule_out) in &NO_TRANSCODE[1..] {
            let mut facts = choosing_audio(&chosen);
            rule_out(&mut facts);

            let plan = build(&mut facts);

            let case = format!("{} with {ruled_out}", chosen.codec);
            assert_eq!(plan.method(), Some(Method::Remux), "{case}");
            assert_eq!(plan.audio, Some(2), "{case}");
        }
    }
}

fn without_hls(facts: &mut Facts) {
    facts.device.hls = false;
}

fn choosing_the_second_audio_track(facts: &mut Facts) {
    facts.audio = Some(2);
}

#[test]
fn a_device_without_hls_that_decodes_everything_direct_plays_or_nothing_whatever_is_chosen() {
    let choices: [(&str, Change); 2] = [
        ("the second audio Track", choosing_the_second_audio_track),
        ("720p", picking_720p),
    ];
    for (chosen, choose) in choices {
        let chosen_without_hls = || {
            let mut facts = direct_played();
            choose(&mut facts);
            without_hls(&mut facts);
            facts
        };
        let mut facts = chosen_without_hls();
        let mut after_a_failed_direct_play = chosen_without_hls();
        after_a_failed_direct_play.failed = BTreeSet::from([Method::DirectPlay]);

        let played = build(&mut facts);
        let unplayable = build(&mut after_a_failed_direct_play);

        assert_eq!(played.method(), Some(Method::DirectPlay), "{chosen}");
        assert_eq!(unplayable.method(), None, "{chosen}");
        for plan in [played, unplayable] {
            let no_hls = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_PLAY_HLS);
            assert_eq!(no_hls.decided, plan.method().is_none(), "{chosen}");
        }
    }
}

#[test]
fn a_plan_offers_the_choices_its_lasting_facts_allow_whatever_is_busy_or_failed() {
    let everything = || Offers {
        audio: vec![1, 2],
        burn_in: vec![3],
        limits: vec![PlaybackLimit::P720, PlaybackLimit::P480],
    };
    let cases: [(&str, Change, Offers); 5] = [
        (
            "no ffmpeg",
            without_ffmpeg,
            Offers {
                audio: vec![1],
                ..Offers::default()
            },
        ),
        (
            "no HLS",
            without_hls,
            Offers {
                audio: vec![1],
                ..Offers::default()
            },
        ),
        (
            "Dolby Vision profile 5",
            in_dolby_vision_5,
            Offers {
                audio: vec![1, 2],
                ..Offers::default()
            },
        ),
        ("a failed Transcode", after_a_failed_transcode, everything()),
        ("a busy Server", while_busy, everything()),
    ];
    for (ruled_out, rule_out, offers) in cases {
        let mut facts = direct_played();
        rule_out(&mut facts);

        assert_eq!(build(&mut facts).offers, offers, "{ruled_out}");
    }
}

#[test]
fn a_plan_offers_no_playback_limit_its_picture_already_fits() {
    let mut facts = facts(Some(vec![
        track(
            "h264",
            false,
            TrackKind::Video {
                width: Some(1280),
                height: Some(720),
                frame_rate: None,
                bit_depth: None,
                hdr: None,
            },
        ),
        audio("aac", true),
    ]));

    assert_eq!(build(&mut facts).offers.limits, [PlaybackLimit::P480]);
}
