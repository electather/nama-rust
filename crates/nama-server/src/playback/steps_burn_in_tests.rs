//! Burning a subtitle into a Transcode for a Device that shows HDR (ADR 0103).

use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use crate::{
    ffmpeg::{BurnIn, ReferenceWhite, TextLayer, ToneMapper},
    playback::{
        Facts, build,
        tests::{
            MP4, hdr_video, in_container, kind_and_params, kinds, reason_of, text_subtitle, track,
            transcoded,
        },
        tracks::{Hdr, TrackKind},
    },
};

/// An HDR Version of `hdr` played on a Device that shows HDR, with its second Track, a subtitle
/// of `codec` the Device can't draw, turned on.
fn burning_in(hdr: Hdr, codec: &str, image: bool) -> Facts {
    let subtitle = if image {
        let kind = TrackKind::Subtitle {
            image: true,
            header: false,
            sidecar: None,
        };
        track(codec, false, kind)
    } else {
        text_subtitle(codec)
    };
    let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr)), subtitle]));
    facts.device.hdr = true;
    facts.subtitle = Some(1);
    facts
}

#[test]
fn a_text_burn_in_ffmpeg_cant_draw_over_hdr_tone_maps_an_hdr_devices_picture_and_says_why() {
    let mut facts = burning_in(Hdr::Hdr10, "ass", false);

    let plan = build(&mut facts);

    let video = transcoded(&plan).video;
    assert_eq!(video.tone_map, Some(ToneMapper::Software));
    assert_eq!(
        video.subtitle_white, None,
        "SDR takes the subtitle as it is"
    );
    let reason = reason_of(
        &plan,
        Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED_FOR_BURN_IN,
    );
    assert_eq!(
        kind_and_params(reason).1,
        [("hdrFormat", "HDR_FORMAT_HDR10")]
    );
    assert!(reason.decided);
    assert!(!kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED));
}

#[test]
fn a_text_burn_in_ffmpeg_draws_on_its_own_layer_keeps_hdr_at_reference_white() {
    let mut facts = burning_in(Hdr::Hdr10, "ass", false);
    facts.can_do.subtitle_layer = true;

    let plan = build(&mut facts);

    let video = transcoded(&plan).video;
    assert_eq!(video.tone_map, None);
    assert_eq!(video.subtitle_white, Some(ReferenceWhite::Pq));
    let Some(BurnIn::Text { layer, .. }) = video.burn_in else {
        panic!("{:?} isn't a text Burn-in", video.burn_in);
    };
    let picture = TextLayer {
        width: 1920,
        height: 1080,
        frame_rate: "24".to_owned(),
    };
    assert_eq!(layer, Some(picture));
    assert!(!kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED_FOR_BURN_IN));
}

#[test]
fn with_nothing_to_tone_map_a_text_burn_in_is_drawn_onto_hdr_as_before_without_a_reason() {
    let mut facts = burning_in(Hdr::Hdr10, "ass", false);
    facts.can_do.zscale = false;

    let plan = build(&mut facts);

    let video = transcoded(&plan).video;
    assert_eq!(video.tone_map, None);
    assert!(
        matches!(video.burn_in, Some(BurnIn::Text { layer: None, .. })),
        "{:?}",
        video.burn_in
    );
    assert!(
        !kinds(&plan).iter().any(|kind| matches!(
            kind,
            Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED_FOR_BURN_IN
                | Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED
        )),
        "{:?}",
        kinds(&plan)
    );
}

#[test]
fn an_image_burn_in_keeps_hdr_at_its_transfers_reference_white_whatever_ffmpeg_lacks() {
    for (hdr, white) in [
        (Hdr::Hdr10, ReferenceWhite::Pq),
        (Hdr::Hlg, ReferenceWhite::Hlg),
    ] {
        let mut facts = burning_in(hdr, "hdmv_pgs_subtitle", true);

        let plan = build(&mut facts);

        let video = transcoded(&plan).video;
        assert_eq!(video.tone_map, None, "{hdr:?}");
        assert_eq!(video.subtitle_white, Some(white), "{hdr:?}");
        assert!(
            !kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED_FOR_BURN_IN),
            "{hdr:?}"
        );
    }
}

#[test]
fn a_burn_in_on_an_sdr_device_tone_maps_as_before_with_the_subtitle_as_it_is() {
    let mut facts = burning_in(Hdr::Hdr10, "ass", false);
    facts.device.hdr = false;
    facts.can_do.subtitle_layer = true;

    let plan = build(&mut facts);

    let video = transcoded(&plan).video;
    assert_eq!(video.tone_map, Some(ToneMapper::Software));
    assert_eq!(video.subtitle_white, None);
    assert!(
        matches!(video.burn_in, Some(BurnIn::Text { layer: None, .. })),
        "{:?}",
        video.burn_in
    );
    assert!(kinds(&plan).contains(&Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED));
}
