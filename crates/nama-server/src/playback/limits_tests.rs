use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use super::on_local_network;
use crate::{
    ffmpeg::VideoLimit,
    playback::{
        Facts, Method, Network, PickedLimit, Playing, build,
        tests::{audio, facts, given, kind_and_params, reason_of, track, transcoded, video},
        tracks::TrackKind,
    },
    server_settings::PlaybackLimit,
};

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
