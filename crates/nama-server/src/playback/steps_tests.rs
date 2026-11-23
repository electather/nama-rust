use std::{collections::BTreeSet, num::NonZeroU32};

use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use super::{keyframes_unlisted, remux, too_busy_to_play};
#[cfg(unix)]
use crate::ffmpeg::fake::{vaapi_on_two_gpus, vaapi_on_two_gpus_at};
use crate::{
    ffmpeg::{Accel, Hardware, ToneMapper, Transcode, VideoConversion, VideoTag},
    playback::{
        Facts, Method, Plan, RunningTranscodes, build,
        tests::{
            DOLBY_VISION_5, Given, MKV, MP4, audio, decided, dolby_vision_8, facts, falls_back_to,
            given, hdr_video, in_container, kind_and_params, kinds, reason_of, text_subtitle,
            transcoded, video, video_hdr, while_busy, without_ffmpeg,
        },
        tracks::{BaseLayer, Hdr},
    },
    server_settings::{HardwareChoice, PickedGpu},
};

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
    let mut facts = Facts {
        failed: BTreeSet::from([Method::DirectPlay, Method::Remux]),
        ..facts(None)
    };
    while_busy(&mut facts);

    assert_eq!(too_busy_to_play(facts), None);
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
fn keyframes_that_couldnt_be_listed_rule_out_only_a_remux() {
    let tracks = Some(vec![video("h264"), audio("aac", true)]);
    let unlisted = Facts {
        keyframes_unlisted: true,
        ..facts(tracks.clone())
    };

    let given = keyframes_unlisted(&unlisted, None);

    assert_eq!(
        (given.iter())
            .map(|reason| (reason.kind, &reason.rules_out))
            .collect::<Vec<_>>(),
        [(
            Kind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED,
            &BTreeSet::from([Method::Remux])
        )]
    );
    assert_eq!(keyframes_unlisted(&facts(tracks), None), []);
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
fn a_busy_server_remuxes_what_the_device_can_play_remuxed_and_says_so_as_context() {
    let mut facts = in_container(MKV, Some(vec![video("h264"), audio("aac", true)]));
    while_busy(&mut facts);

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Remux));
    assert_eq!(
        given(&plan)[1],
        (
            Kind::PLAYBACK_REASON_KIND_SERVER_BUSY,
            vec![("maxTranscodes", "2")]
        )
    );
    assert_eq!(decided(&plan), [true, false], "a Remux would play anyway");
    assert_eq!(too_busy_to_play(facts), None);
}

#[test]
fn a_remux_tags_hevc_hvc1_and_dolby_vision_5_dvh1_but_not_h264() {
    let remux_of = |track| remux(&in_container(MKV, Some(vec![track])), None).0;

    let dolby_vision_5 = hdr_video("hevc", Some(DOLBY_VISION_5));
    assert_eq!(remux_of(dolby_vision_5).video_tag, Some(VideoTag::Dvh1));
    assert_eq!(remux_of(video("hevc")).video_tag, Some(VideoTag::Hvc1));
    assert_eq!(remux_of(video("h264")).video_tag, None);
}

#[test]
fn a_plan_only_a_transcode_would_play_is_too_busy_to_play_while_the_server_is_busy() {
    let hevc = || Some(vec![video("hevc"), audio("aac", true)]);
    let busy = |mut facts| {
        while_busy(&mut facts);
        facts
    };
    let mut without_hls = busy(facts(hevc()));
    without_hls.device.hls = false;

    assert_eq!(build(&mut busy(facts(hevc()))).method(), None);
    assert_eq!(too_busy_to_play(busy(facts(hevc()))), Some(2));
    assert_eq!(too_busy_to_play(facts(hevc())), None, "not busy");
    assert_eq!(too_busy_to_play(without_hls), None, "nothing plays it");
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
            subtitle_white: None,
            in_band_headers: true,
            hevc: false,
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

const VAAPI: Hardware = Hardware {
    accel: Accel::Vaapi,
    gpu: None,
    tone_maps: false,
    hevc: false,
};
const QSV: Hardware = Hardware {
    accel: Accel::Qsv,
    gpu: None,
    tone_maps: true,
    hevc: false,
};

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
fn a_hardware_transcode_falls_back_to_software_with_every_reason_but_running_on_hardware() {
    let cases = [(None, None), (Some(Hdr::Hdr10), Some(ToneMapper::Software))];
    for (hdr, tone_map) in cases {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], hdr);

        let plan = build(&mut facts);

        let on_hardware = transcoded(&plan);
        let fallback = falls_back_to(&plan).unwrap();
        assert_eq!(
            fallback.transcode,
            Transcode {
                video: VideoConversion {
                    hardware: None,
                    tone_map,
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
        assert_eq!(reasons, expected, "{hdr:?}");
    }
}

#[test]
fn without_zscale_hardware_hdr_falls_back_without_tone_mapping_or_its_reasons_and_says_so() {
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
            Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED,
            Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
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

/// `facts` while other Devices run `all` Transcodes, `cpu_bound` of them on the CPU.
fn while_running(all: u32, cpu_bound: u32, facts: Facts) -> Facts {
    Facts {
        running_transcodes: RunningTranscodes { all, cpu_bound },
        ..facts
    }
}

/// HDR10 that Firefox can't show, on a Server whose ffmpeg found only `hardware`.
fn hdr_on(hardware: Hardware) -> Facts {
    hevc_on(HardwareChoice::Automatic, vec![hardware], Some(Hdr::Hdr10))
}

/// Fails unless the default cap's running Transcodes on the CPU keep `plan` from playing.
fn assert_busy_by_default(plan: &Plan) {
    let busy = (
        Kind::PLAYBACK_REASON_KIND_SERVER_BUSY,
        vec![("maxTranscodes", "2")],
    );
    assert_eq!(plan.method(), None);
    assert!(given(plan).contains(&busy), "{:?}", given(plan));
}

#[test]
fn by_default_an_hdr_transcode_tone_mapped_on_the_cpu_waits_for_two_cpu_bound_ones() {
    let mut facts = while_running(2, 2, hdr_on(VAAPI));

    let plan = build(&mut facts);

    assert_busy_by_default(&plan);
    assert_eq!(too_busy_to_play(facts), Some(2));
}

#[test]
fn by_default_an_hdr_transcode_tone_mapped_on_the_gpu_is_never_busy() {
    let mut facts = while_running(5, 5, hdr_on(QSV));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
}

#[test]
fn by_default_an_sdr_transcode_on_hardware_is_never_busy() {
    let mut facts = while_running(9, 2, hevc_on(HardwareChoice::Automatic, vec![VAAPI], None));

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
}

#[test]
fn by_default_an_hdr_transcode_burning_in_a_subtitle_is_cpu_bound_so_a_busy_server_drops_it() {
    let burning_in = |running| {
        let mut facts = while_running(running, running, hdr_on(QSV));
        if let Some(probe) = &mut facts.probe {
            probe.tracks.push(text_subtitle("ass"));
        }
        facts.subtitle = Some(2);
        build(&mut facts)
    };

    let idle = burning_in(0);
    let busy = burning_in(2);

    let burned_in = transcoded(&idle).video;
    assert!(burned_in.burn_in.is_some() && burned_in.cpu_bound());
    assert!(
        kinds(&busy).contains(&Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED),
        "{:?}",
        kinds(&busy)
    );
    assert_eq!(
        transcoded(&busy).video.burn_in,
        None,
        "with the subtitle dropped it tone-maps on the GPU"
    );
}

#[test]
fn by_default_a_third_software_transcode_is_refused() {
    let mut facts = while_running(2, 2, hevc_on(HardwareChoice::Off, vec![VAAPI], None));

    let plan = build(&mut facts);

    assert_busy_by_default(&plan);
}

#[test]
fn a_set_transcode_cap_counts_every_running_transcode() {
    let capped_at = |max, all| Facts {
        max_transcodes: NonZeroU32::new(max),
        ..while_running(all, 0, hdr_on(QSV))
    };

    assert_eq!(too_busy_to_play(capped_at(3, 3)), Some(3));
    assert_eq!(too_busy_to_play(capped_at(3, 2)), None);
    assert_eq!(too_busy_to_play(capped_at(u32::MAX, 3)), None);
}
