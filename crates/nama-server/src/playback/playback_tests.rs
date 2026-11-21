use std::{collections::BTreeSet, num::NonZeroU32};

use nama_proto::nama::v1::{AudioCodec, Container, PlaybackReasonKind as Kind, VideoCodec};

use super::{
    Candidates, DeviceDecoding, Facts, Fallback, Method, Network, Offers, PickedLimit, Plan,
    Playing, Reason, RunningTranscodes, SubtitleOffsets, build, narrowed, too_busy_to_play,
    tracks::{BaseLayer, Hdr, Probe, Track, TrackKind},
};
use crate::{
    ffmpeg::{AudioConversion, AudioEncoder, CanDo, Remux, RemuxAudio, Transcode},
    server_settings::{HardwareChoice, PlaybackLimit},
};

/// How long the Version in [`facts`] is.
const LENGTH_MS: i64 = 7_200_000;

pub(super) fn track(codec: &str, default: bool, kind: TrackKind) -> Track {
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

pub(super) fn video(codec: &str) -> Track {
    hdr_video(codec, None)
}

pub(super) fn hdr_video(codec: &str, hdr: Option<Hdr>) -> Track {
    let kind = TrackKind::Video {
        width: Some(1920),
        height: Some(1080),
        frame_rate: Some(24.0),
        bit_depth: Some(8),
        hdr,
    };
    track(codec, false, kind)
}

pub(super) const fn dolby_vision_8(base_layer: Option<BaseLayer>) -> Hdr {
    Hdr::DolbyVision {
        profile: 8,
        base_layer,
    }
}

/// How the video among `tracks` is HDR.
pub(super) fn video_hdr(tracks: &[Track]) -> Option<Hdr> {
    tracks.iter().find_map(|track| match track.kind {
        TrackKind::Video { hdr, .. } => hdr,
        TrackKind::Audio { .. } | TrackKind::Subtitle { .. } => None,
    })
}

pub(super) fn audio(codec: &str, default: bool) -> Track {
    audio_with(codec, default, Some(2))
}

pub(super) fn audio_with(codec: &str, default: bool, channels: Option<i64>) -> Track {
    track(
        codec,
        default,
        TrackKind::Audio {
            channels,
            atmos: false,
            audio_description: false,
        },
    )
}

pub(super) const MP4: &str = "mov,mp4,m4a,3gp,3g2,mj2";
pub(super) const MKV: &str = "matroska,webm";

/// What Firefox on an SDR display says it decodes.
pub(super) fn firefox() -> DeviceDecoding {
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
        h264_10bit: false,
        hls: true,
        audio_channels: 2,
        subtitle_formats: vec![],
    }
}

pub(super) fn facts(tracks: Option<Vec<Track>>) -> Facts {
    in_container(MP4, tracks)
}

pub(super) fn in_container(container: &str, tracks: Option<Vec<Track>>) -> Facts {
    Facts {
        version: 7,
        probe: tracks.map(|tracks| Probe {
            container: container.to_owned(),
            duration_ms: Some(LENGTH_MS),
            bitrate: Some(5_000_000),
            tracks,
            font_attachments: vec![],
            chapters: vec![],
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
        failed_on_server: BTreeSet::new(),
        failed_recipes: BTreeSet::new(),
        keyframes_unlisted: false,
        audio: None,
        running_transcodes: RunningTranscodes::default(),
        max_transcodes: None,
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
pub(super) fn transcoded(plan: &Plan) -> Transcode {
    match &plan.playing {
        Some(Playing::Transcode { transcode, .. }) => transcode.clone(),
        other => panic!("{other:?} isn't a Transcode"),
    }
}

/// What `plan` Transcodes in software once its hardware fails, failing the test if it
/// doesn't Transcode.
pub(super) fn falls_back_to(plan: &Plan) -> Option<Fallback> {
    match &plan.playing {
        Some(Playing::Transcode { fallback, .. }) => fallback.clone(),
        other => panic!("{other:?} isn't a Transcode"),
    }
}

/// A Remux playing the audio Track `track`, converting it with the encoder and channels
/// `converted` gives, if given.
pub(super) fn remuxing(track: usize, converted: Option<(AudioEncoder, i64)>) -> Playing {
    Playing::Remux {
        remux: Remux {
            audio: Some(RemuxAudio {
                track,
                conversion: converted
                    .map(|(encoder, channels)| AudioConversion { encoder, channels }),
            }),
            hevc: false,
        },
        duration_ms: LENGTH_MS,
    }
}

pub(super) fn subtitle() -> Track {
    Track {
        default: true,
        ..text_subtitle("subrip")
    }
}

pub(super) fn text_subtitle(codec: &str) -> Track {
    track(
        codec,
        false,
        TrackKind::Subtitle {
            image: false,
            header: false,
            sidecar: None,
        },
    )
}

/// A reason's kind and params, as a test expects them.
pub(super) type Given<'a> = (Kind, Vec<(&'a str, &'a str)>);

/// The kind and params of each of `plan`'s reasons, in order.
pub(super) fn given(plan: &Plan) -> Vec<Given<'_>> {
    plan.reasons.iter().map(kind_and_params).collect()
}

pub(super) fn kind_and_params(reason: &Reason) -> Given<'_> {
    let params = (reason.params.iter())
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    (reason.kind, params)
}

/// The kind of each of `plan`'s reasons, in order.
pub(super) fn kinds(plan: &Plan) -> Vec<Kind> {
    plan.reasons.iter().map(|reason| reason.kind).collect()
}

/// `plan`'s reason of `kind`, failing the test if it has none.
pub(super) fn reason_of(plan: &Plan, kind: Kind) -> &Reason {
    (plan.reasons.iter())
        .find(|reason| reason.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} among {:?}", kinds(plan)))
}

/// Whether each of `plan`'s reasons decided it, in order.
pub(super) fn decided(plan: &Plan) -> Vec<bool> {
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
            chapters: vec![],
            segments: vec![],
            preview_frames: None,
            subtitle_offsets: SubtitleOffsets::default(),
        }
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
fn a_remux_whose_keyframes_couldnt_be_listed_transcodes_and_says_why() {
    let mut facts = Facts {
        keyframes_unlisted: true,
        ..in_container(MKV, Some(vec![video("h264"), audio("aac", true)]))
    };

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert!(reason_of(&plan, Kind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED).decided);
}

/// An E-AC-3 Matroska Version, on Firefox that says it decodes E-AC-3, which a Remux copies, after
/// the Recipes `failed` failed on it.
fn eac3_remux_after(failed: &[&str]) -> Facts {
    let mut facts = in_container(MKV, Some(vec![video("h264"), audio("eac3", true)]));
    facts.device.audio_codecs.push(AudioCodec::AUDIO_CODEC_EAC3);
    facts.failed_recipes = failed.iter().map(|&recipe| recipe.to_owned()).collect();
    facts
}

#[test]
fn a_remux_names_its_container_and_the_audio_codec_it_sends_as_its_recipe() {
    let plan = build(&mut eac3_remux_after(&[]));

    assert_eq!(plan.playing, Some(remuxing(0, None)));
    assert_eq!(plan.recipe(), "remux/fmp4/eac3");
}

#[test]
fn a_failed_remux_recipe_remuxes_another_way_and_says_why() {
    let plan = build(&mut eac3_remux_after(&["remux/fmp4/eac3"]));

    assert_eq!(
        plan.playing,
        Some(remuxing(0, Some((AudioEncoder::Aac, 2))))
    );
    assert_eq!(plan.recipe(), "remux/fmp4/aac");
    let failed = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_RECIPE_FAILED_ON_DEVICE);
    assert_eq!(
        kind_and_params(failed).1,
        [("method", "PLAYBACK_METHOD_REMUX"), ("codec", "eac3")]
    );
    assert!(failed.decided);
}

#[test]
fn a_failed_recipe_the_plan_wouldnt_send_anyway_gives_no_reason() {
    let mut facts = in_container(MKV, Some(vec![video("h264"), audio("aac", true)]));
    facts.device.audio_codecs.push(AudioCodec::AUDIO_CODEC_EAC3);
    facts.failed_recipes =
        BTreeSet::from(["remux/fmp4/eac3".to_owned(), "remux/fmp4/xyz".to_owned()]);

    let plan = build(&mut facts);

    assert_eq!(plan.playing, Some(remuxing(0, None)));
    assert!(
        !kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_RECIPE_FAILED_ON_DEVICE),
        "{:?}",
        kinds(&plan)
    );
}

#[test]
fn a_method_whose_every_recipe_failed_is_ruled_out_as_failed_on_the_device() {
    let plan = build(&mut eac3_remux_after(&[
        "remux/fmp4/eac3",
        "remux/fmp4/aac",
    ]));

    assert_eq!(plan.method(), Some(Method::Transcode));
    let failed = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE);
    assert_eq!(
        kind_and_params(failed).1,
        [("method", "PLAYBACK_METHOD_REMUX")]
    );
    assert!(failed.decided);
}

#[test]
fn a_failed_direct_play_recipe_rules_out_direct_play() {
    let mut facts = facts(Some(vec![video("h264"), audio("aac", true)]));
    let recipe = build(&mut facts.clone()).recipe();
    facts.failed_recipes = BTreeSet::from([recipe]);

    let plan = build(&mut facts);

    assert_eq!(plan.method(), Some(Method::Remux));
    assert_eq!(
        given(&plan),
        [(
            Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
            vec![("method", "PLAYBACK_METHOD_DIRECT_PLAY")]
        )]
    );
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

/// A 1080p H.264 Version this Device Direct Plays, with a second audio Track and a PGS one.
fn direct_played() -> Facts {
    let pgs = track(
        "hdmv_pgs_subtitle",
        false,
        TrackKind::Subtitle {
            image: true,
            header: false,
            sidecar: None,
        },
    );
    facts(Some(vec![
        video("h264"),
        audio("aac", true),
        audio("aac", false),
        pgs,
    ]))
}

pub(super) fn without_ffmpeg(facts: &mut Facts) {
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

/// The admin let the Server run 2 Transcodes at once, and other Devices run 2.
pub(super) fn while_busy(facts: &mut Facts) {
    facts.max_transcodes = NonZeroU32::new(2);
    facts.running_transcodes = RunningTranscodes {
        all: 2,
        cpu_bound: 0,
    };
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
fn an_unplayable_plan_keeps_the_choices_asked_and_drops_none() {
    let p720 = Some(PlaybackLimit::P720);
    let choices: [(&str, Change, _); 3] = [
        (
            "the second audio Track",
            choosing_the_second_audio_track,
            (Some(2), None, None),
        ),
        ("720p", picking_720p, (Some(1), p720, None)),
        ("PGS", burning_in_pgs, (Some(1), None, Some(3))),
    ];
    for (chosen, choose, asked) in choices {
        let mut facts = direct_played();
        choose(&mut facts);
        without_ffmpeg(&mut facts);
        facts.failed = BTreeSet::from([Method::DirectPlay]);

        let plan = build(&mut facts);

        assert_eq!(plan.method(), None, "{chosen}");
        assert_eq!((plan.audio, plan.limit, plan.burn_in), asked, "{chosen}");
        let dropped = [
            Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED,
            Kind::PLAYBACK_REASON_KIND_PLAYBACK_LIMIT_DROPPED,
            Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED,
        ];
        assert!(
            !kinds(&plan).iter().any(|kind| dropped.contains(kind)),
            "{chosen}"
        );
    }
}

#[test]
fn a_busy_server_refuses_a_device_that_needs_a_transcode_whatever_is_chosen() {
    let choices: [(&str, Change); 3] = [
        ("the second audio Track", choosing_the_second_audio_track),
        ("720p", picking_720p),
        ("PGS", burning_in_pgs),
    ];
    for (chosen, choose) in choices {
        let mut facts = direct_played();
        if let Some(probe) = &mut facts.probe {
            probe.tracks[0] = video("hevc");
        }
        choose(&mut facts);
        while_busy(&mut facts);

        assert_eq!(build(&mut facts).method(), None, "{chosen}");
        assert_eq!(too_busy_to_play(facts), Some(2), "{chosen}");
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
