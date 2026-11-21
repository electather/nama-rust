//! Tracks as the API shows them.

use nama_proto::nama::v1 as proto;

use super::{BaseLayer, Hdr, Track, TrackKind};

/// `track` as the API shows it.
pub(crate) fn track_to_proto(track: &Track) -> proto::Track {
    // 0 is what the proto says for a fact the file doesn't give. Stored ones are never negative.
    let known = |n: Option<i64>| n.and_then(|n| u32::try_from(n).ok()).unwrap_or_default();
    let kind = match track.kind {
        TrackKind::Video {
            width,
            height,
            frame_rate,
            bit_depth,
            hdr,
        } => {
            let (hdr, dolby_vision) = hdr_to_proto(hdr);
            proto::VideoTrack {
                width: known(width),
                height: known(height),
                frame_rate: frame_rate.unwrap_or_default(),
                bit_depth: known(bit_depth),
                hdr: hdr.into(),
                dolby_vision: dolby_vision.into(),
                ..Default::default()
            }
            .into()
        }
        TrackKind::Audio { channels } => proto::AudioTrack {
            channels: known(channels),
            ..Default::default()
        }
        .into(),
        TrackKind::Subtitle { image, .. } => proto::SubtitleTrack {
            image,
            ..Default::default()
        }
        .into(),
    };
    proto::Track {
        codec: track.codec.clone(),
        language: track.language.clone(),
        title: track.title.clone(),
        default: track.default,
        forced: track.forced,
        sdh: track.sdh,
        kind,
        ..Default::default()
    }
}

/// `hdr` as a video Track's `hdr` and `dolby_vision`.
pub(in crate::playback) fn hdr_to_proto(
    hdr: Option<Hdr>,
) -> (proto::HdrFormat, Option<proto::DolbyVision>) {
    match hdr {
        None => (proto::HdrFormat::HDR_FORMAT_UNSPECIFIED, None),
        Some(Hdr::Hdr10) => (proto::HdrFormat::HDR_FORMAT_HDR10, None),
        Some(Hdr::Hlg) => (proto::HdrFormat::HDR_FORMAT_HLG, None),
        Some(Hdr::DolbyVision {
            profile,
            base_layer,
        }) => {
            let base_layer = match base_layer {
                None => proto::BaseLayer::BASE_LAYER_UNSPECIFIED,
                Some(BaseLayer::Hdr10) => proto::BaseLayer::BASE_LAYER_HDR10,
                Some(BaseLayer::Hlg) => proto::BaseLayer::BASE_LAYER_HLG,
                Some(BaseLayer::Sdr) => proto::BaseLayer::BASE_LAYER_SDR,
            };
            let dolby_vision = proto::DolbyVision {
                // ffprobe reads it from a byte, so it's never negative.
                profile: u32::try_from(profile).unwrap_or_default(),
                base_layer: base_layer.into(),
                ..Default::default()
            };
            (
                proto::HdrFormat::HDR_FORMAT_DOLBY_VISION,
                Some(dolby_vision),
            )
        }
    }
}
