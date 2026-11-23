//! Where a Device plays from, and the Playback Limit a Transcode keeps the picture within.

use std::net::IpAddr;

use buffa::Enumeration;
use nama_proto::nama::v1::{self as proto, PlaybackReasonKind as Kind};

use super::{
    Candidates, Choice, Facts, Method, Network, PickedLimit, Reason, codecs::played_video,
    tracks::TrackKind,
};
use crate::{ffmpeg::VideoLimit, server_settings::PlaybackLimit};

/// Each Playback Limit below Original, by the API's name for it.
pub(crate) const NAMED_LIMITS: [(PlaybackLimit, proto::PlaybackLimit); 3] = [
    (
        PlaybackLimit::P1080,
        proto::PlaybackLimit::PLAYBACK_LIMIT_1080P,
    ),
    (
        PlaybackLimit::P720,
        proto::PlaybackLimit::PLAYBACK_LIMIT_720P,
    ),
    (
        PlaybackLimit::P480,
        proto::PlaybackLimit::PLAYBACK_LIMIT_480P,
    ),
];

/// `limit` as the API names it, `None` being Original.
pub(crate) fn limit_to_proto(limit: Option<PlaybackLimit>) -> proto::PlaybackLimit {
    (NAMED_LIMITS.into_iter())
        .find(|(ours, _)| Some(*ours) == limit)
        .map_or(
            proto::PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL,
            |(_, theirs)| theirs,
        )
}

/// The step for where the Device is: it rules nothing out, and says when the User's cap for
/// remote playback, or else the default Playback Limit for remote playback, chose a limit the next
/// step scales the picture down to or drops.
pub(super) fn network(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let scales = |limit| {
        picture(facts)
            .and_then(|picture| fitted(picture, limit))
            .is_some()
    };
    let Network::Remote { default, cap } = facts.network else {
        return vec![];
    };
    let wanted = match facts.picked_limit {
        PickedLimit::Picked(limit) => limit,
        PickedLimit::Unpicked => default,
    };
    if let Some(cap) = cap
        && lower(wanted, Some(cap)) != wanted
        && scales(cap)
    {
        let text = format!(
            "This Device is outside the local network, where its User's cap for remote playback is {}",
            cap.name
        );
        return vec![limit_reason(
            Kind::PLAYBACK_REASON_KIND_REMOTE_CAP,
            cap,
            text,
        )];
    }
    match lower(default, cap) {
        Some(limit) if facts.picked_limit == PickedLimit::Unpicked && scales(limit) => {
            vec![limit_reason(
                Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT,
                limit,
                format!(
                    "This Device is outside the local network, so it asks for the default \
                     Playback Limit for remote playback, {}",
                    limit.name
                ),
            )]
        }
        _ => vec![],
    }
}

/// The User's cap for remote playback, when the Device is outside the local network.
pub(super) const fn remote_cap_of(facts: &Facts) -> Option<PlaybackLimit> {
    match facts.network {
        Network::Remote { cap, .. } => cap,
        Network::Local => None,
    }
}

/// The lower of two Playback Limits, `None` being Original.
fn lower(one: Option<PlaybackLimit>, other: Option<PlaybackLimit>) -> Option<PlaybackLimit> {
    one.into_iter()
        .chain(other)
        .min_by_key(|limit| limit.height)
}

/// A reason of `kind` about the Playback Limit `limit`.
fn limit_reason(kind: Kind, limit: PlaybackLimit, text: String) -> Reason {
    Reason::new(kind, text).with("playbackLimit", &limit_to_proto(Some(limit)).proto_name())
}

/// Whether `address` is on the local network: loopback, private (RFC 1918), link-local, or IPv6
/// unique-local.
pub(super) fn on_local_network(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unicast_link_local() || v6.is_unique_local(),
    }
}

/// The Playback Limit in effect: [`limit_asked`], unless the plan dropped it. `None` is Original.
pub(super) fn limit_in_effect(facts: &Facts) -> Option<PlaybackLimit> {
    limit_asked(facts).filter(|_| !facts.dropped.contains(&Choice::Limit))
}

/// The Playback Limit the User picked, or else, outside the local network, the remote default,
/// either kept to their cap there. `None` is Original.
fn limit_asked(facts: &Facts) -> Option<PlaybackLimit> {
    match (facts.picked_limit, facts.network) {
        (PickedLimit::Picked(limit), Network::Local) => limit,
        (PickedLimit::Unpicked, Network::Local) => None,
        (PickedLimit::Picked(limit), Network::Remote { cap, .. })
        | (
            PickedLimit::Unpicked,
            Network::Remote {
                default: limit,
                cap,
            },
        ) => lower(limit, cap),
    }
}

/// The step for the Playback Limit in effect: only a Transcode, which scales the picture down to
/// fit it, when the Version's picture is larger.
pub(super) fn playback_limit(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    if let Some(limit) = limit_asked(facts)
        && facts.dropped.contains(&Choice::Limit)
    {
        let text = format!(
            "The {} Playback Limit can't be kept to here, so it's dropped for Original",
            limit.name
        );
        return vec![
            limit_reason(
                Kind::PLAYBACK_REASON_KIND_PLAYBACK_LIMIT_DROPPED,
                limit,
                text,
            )
            .deciding(),
        ];
    }
    let Some(limit) = limit_in_effect(facts) else {
        return vec![];
    };
    let scaled = candidates.and_then(|candidates| candidates.transcode.video.limit?.size);
    if let Some((width, height)) = scaled {
        let text = format!(
            "Its picture is larger than the {} Playback Limit, so a Transcode scales it down to \
             {width}×{height}",
            limit.name
        );
        return vec![
            limit_reason(Kind::PLAYBACK_REASON_KIND_PICTURE_SCALED_DOWN, limit, text)
                .with("widthPixels", &width)
                .with("heightPixels", &height)
                .ruling_out([Method::DirectPlay, Method::Remux]),
        ];
    }
    if picture(facts).is_some() {
        return vec![];
    }
    let text = format!(
        "Its picture size isn't known, so it's taken to fit the {} Playback Limit",
        limit.name
    );
    vec![limit_reason(
        Kind::PLAYBACK_REASON_KIND_PICTURE_SIZE_UNKNOWN,
        limit,
        text,
    )]
}

/// How a Transcode keeps the played video within the Playback Limit in effect, if there's one.
pub(super) fn video_limit(facts: &Facts) -> Option<VideoLimit> {
    let limit = limit_in_effect(facts)?;
    Some(VideoLimit {
        size: picture(facts).and_then(|picture| fitted(picture, limit)),
        bits_per_second: limit.bits_per_second,
    })
}

/// The width and height of the played video, if the file says.
pub(super) fn picture(facts: &Facts) -> Option<(i64, i64)> {
    match played_video(facts.probe.as_ref()?)?.kind {
        TrackKind::Video {
            width: Some(width),
            height: Some(height),
            ..
        } => Some((width, height)),
        TrackKind::Video { .. } | TrackKind::Audio { .. } | TrackKind::Subtitle { .. } => None,
    }
}

/// A picture `width` by `height`, scaled down to fit `limit` with its shape kept, if it's larger.
pub(super) fn fitted((width, height): (i64, i64), limit: PlaybackLimit) -> Option<(i64, i64)> {
    // A limit names the short side, so a portrait 1080×1920 picture is already 1080p.
    let (limit_width, limit_height) = if height > width {
        (limit.height, limit.width)
    } else {
        (limit.width, limit.height)
    };
    if width <= limit_width && height <= limit_height {
        return None;
    }
    let rounded = |numerator: i64, denominator: i64| (numerator + denominator / 2) / denominator;
    let (width, height) = if width * limit_height > height * limit_width {
        (limit_width, rounded(height * limit_width, width))
    } else {
        (rounded(width * limit_height, height), limit_height)
    };
    // H.264 in 4:2:0 needs even sides.
    Some((width / 2 * 2, height / 2 * 2))
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
