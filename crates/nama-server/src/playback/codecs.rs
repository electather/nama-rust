//! What a Device decodes, and the Tracks a plan plays once converted.

use buffa::Enumeration;
use nama_proto::nama::v1 as proto;
use proto::PlaybackReasonKind as Kind;

use super::{
    Choice, DeviceDecoding, Facts, Method, Playing, Reason,
    steps::hdr_reason,
    tracks::{Hdr, Probe, Track, TrackKind},
};
use crate::ffmpeg::{Remux, RemuxAudio, Transcode, VideoConversion};

/// Each video codec a Device can say it decodes, as ffprobe names it, with its name in plain words.
const VIDEO_CODECS: [(&str, proto::VideoCodec, &str); 5] = [
    ("h264", proto::VideoCodec::VIDEO_CODEC_H264, "H.264"),
    ("hevc", proto::VideoCodec::VIDEO_CODEC_HEVC, "HEVC"),
    ("vp8", proto::VideoCodec::VIDEO_CODEC_VP8, "VP8"),
    ("vp9", proto::VideoCodec::VIDEO_CODEC_VP9, "VP9"),
    ("av1", proto::VideoCodec::VIDEO_CODEC_AV1, "AV1"),
];

/// Each audio codec a Device can say it decodes, as ffprobe names it, with its name in plain words.
const AUDIO_CODECS: [(&str, proto::AudioCodec, &str); 9] = [
    ("aac", proto::AudioCodec::AUDIO_CODEC_AAC, "AAC"),
    ("mp3", proto::AudioCodec::AUDIO_CODEC_MP3, "MP3"),
    ("ac3", proto::AudioCodec::AUDIO_CODEC_AC3, "AC-3"),
    ("eac3", proto::AudioCodec::AUDIO_CODEC_EAC3, "E-AC-3"),
    ("opus", proto::AudioCodec::AUDIO_CODEC_OPUS, "Opus"),
    ("vorbis", proto::AudioCodec::AUDIO_CODEC_VORBIS, "Vorbis"),
    ("flac", proto::AudioCodec::AUDIO_CODEC_FLAC, "FLAC"),
    ("dts", proto::AudioCodec::AUDIO_CODEC_DTS, "DTS"),
    ("truehd", proto::AudioCodec::AUDIO_CODEC_TRUEHD, "TrueHD"),
];

/// Why `device` can't play `probe`'s container, if it can't.
pub(super) fn undecodable_container(probe: &Probe, device: &DeviceDecoding) -> Option<Reason> {
    let (param, name) = match container(probe) {
        Some((container, _)) if device.containers.contains(&container) => return None,
        Some((container, name)) => (container.proto_name(), name),
        None => (probe.container.as_str(), probe.container.as_str()),
    };
    let reason = Reason::new(
        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
        format!("This Device can't decode its {name} container, so it can't be Direct Played"),
    );
    Some(
        reason
            .with("container", &param)
            .ruling_out([Method::DirectPlay]),
    )
}

/// What of `track` `device` can't decode or show, one reason for each.
pub(super) fn undecodable(track: &Track, device: &DeviceDecoding) -> Vec<Reason> {
    let codec = &track.codec;
    match track.kind {
        TrackKind::Video { hdr, bit_depth, .. } => {
            let video = undecodable_codec(codec, &VIDEO_CODECS, &device.video_codecs)
                .map(|name| cant_decode_video(name, codec))
                .or_else(|| undecodable_h264_depth(codec, bit_depth, device));
            let hdr = hdr.filter(|_| !device.hdr).map(|hdr| {
                let text = format!(
                    "This Device can't show its {} picture, so it can't be Direct Played or \
                     Remuxed",
                    hdr_name(hdr)
                );
                hdr_reason(Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR, hdr, text)
                    .ruling_out([Method::DirectPlay, Method::Remux])
            });
            video.into_iter().chain(hdr).collect()
        }
        TrackKind::Audio { .. } => {
            let audio = undecodable_codec(codec, &AUDIO_CODECS, &device.audio_codecs).map(|name| {
                Reason::new(
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_AUDIO,
                    format!(
                        "This Device can't decode its {name} audio, so it can't be Direct Played"
                    ),
                )
                .with("codec", &codec)
                .ruling_out([Method::DirectPlay])
            });
            audio.into_iter().collect()
        }
        // A plan plays only a video and an audio Track as Tracks of their own.
        TrackKind::Subtitle { .. } => vec![],
    }
}

/// Why `device` can't decode the H.264 video Track `codec` names at `bit_depth`, if it can't.
fn undecodable_h264_depth(
    codec: &str,
    bit_depth: Option<i64>,
    device: &DeviceDecoding,
) -> Option<Reason> {
    let depth = bit_depth.filter(|&depth| codec == "h264" && depth > 8 && !device.h264_10bit)?;
    Some(cant_decode_video(&format!("{depth}-bit H.264"), codec).with("bitDepth", &depth))
}

/// That the Device can't decode the video Track `codec` names, `name` in plain words.
fn cant_decode_video(name: &str, codec: &str) -> Reason {
    Reason::new(
        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
        format!(
            "This Device can't decode its {name} video, so it can't be Direct Played or Remuxed"
        ),
    )
    .with("codec", &codec)
    .ruling_out([Method::DirectPlay, Method::Remux])
}

/// The codec ffprobe names `ffprobe_name`, in plain words from `codecs` if it has it, unless
/// `decoded` lists it.
fn undecodable_codec<'a, C: PartialEq>(
    ffprobe_name: &'a str,
    codecs: &[(&str, C, &'static str)],
    decoded: &[C],
) -> Option<&'a str> {
    match codecs.iter().find(|(name, ..)| *name == ffprobe_name) {
        Some((_, codec, _)) if decoded.contains(codec) => None,
        Some((_, _, name)) => Some(name),
        None => Some(ffprobe_name),
    }
}

/// `tracks`, the ones a plan plays as they are in the file, as `playing` plays them once
/// converted, or as they are when it can't be played.
pub(super) fn played_by(playing: Option<&Playing>, tracks: Vec<Track>) -> Vec<Track> {
    match playing {
        Some(Playing::Remux {
            remux:
                Remux {
                    audio:
                        Some(RemuxAudio {
                            aac_channels: Some(channels),
                            ..
                        }),
                },
            ..
        }) => converted_to_aac(tracks, *channels),
        Some(Playing::Transcode {
            transcode: Transcode { audio, video },
            ..
        }) => {
            let tracks = match audio {
                Some(audio) => converted_to_aac(tracks, audio.aac_channels),
                None => tracks,
            };
            converted_to_h264(tracks, video)
        }
        None | Some(Playing::DirectPlay | Playing::Remux { .. }) => tracks,
    }
}

/// How many channels a Remux converts the audio Track `audio` to, as AAC, if `device` can't
/// decode it, as [`converted_channels`] says.
pub(super) fn aac_channels(device: &DeviceDecoding, audio: &Track) -> Option<i64> {
    if undecodable(audio, device).is_empty() {
        return None;
    }
    Some(converted_channels(device, audio))
}

/// How many channels the audio Track `audio` has once converted to AAC for `device`: as many as
/// it has, up to 5.1 if the Device plays 5.1 and stereo otherwise.
pub(super) fn converted_channels(device: &DeviceDecoding, audio: &Track) -> i64 {
    let most = if device.audio_channels >= 6 { 6 } else { 2 };
    // Stereo when the file doesn't say, which every Device plays.
    channels_of(audio).unwrap_or(2).min(most)
}

/// How many channels the audio Track `audio` has, if the file says.
pub(super) const fn channels_of(audio: &Track) -> Option<i64> {
    match audio.kind {
        TrackKind::Audio { channels } => channels,
        TrackKind::Video { .. } | TrackKind::Subtitle { .. } => None,
    }
}

/// `tracks` with their audio converted to AAC with `channels` channels.
fn converted_to_aac(mut tracks: Vec<Track>, channels: i64) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Audio { channels: played } = &mut track.kind {
            "aac".clone_into(&mut track.codec);
            *played = Some(channels);
        }
    }
    tracks
}

/// `tracks` with their video converted to 8-bit H.264 as `video` says, keeping the HDR it decodes
/// unless it tone-maps it.
// ffmpeg tags the H.264 with the colours it decoded, so an HDR10 or HLG picture stays one at 8
// bits.
// ponytail: checked only for libx264; a GPU encoder that drops those tags sends an untagged
// picture the Track calls HDR.
fn converted_to_h264(mut tracks: Vec<Track>, video: &VideoConversion) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Video {
            bit_depth,
            width,
            height,
            hdr,
            ..
        } = &mut track.kind
        {
            "h264".clone_into(&mut track.codec);
            *bit_depth = Some(8);
            *hdr = hdr
                .and_then(transcoded_hdr)
                .filter(|_| video.tone_map.is_none());
            if let Some((scaled_width, scaled_height)) = video.limit.and_then(|limit| limit.size) {
                (*width, *height) = (Some(scaled_width), Some(scaled_height));
            }
        }
    }
    tracks
}

/// `probe`'s container and its name in plain words, if a Device can play it at all.
fn container(probe: &Probe) -> Option<(proto::Container, &'static str)> {
    // ffprobe names WebM and Matroska alike; WebM is Matroska holding only these codecs.
    const WEBM: [&str; 6] = ["vp8", "vp9", "av1", "opus", "vorbis", "webvtt"];
    let webm = (probe.tracks.iter()).all(|track| WEBM.contains(&track.codec.as_str()));
    match probe.container.as_str() {
        "mov,mp4,m4a,3gp,3g2,mj2" => Some((proto::Container::CONTAINER_MP4, "MP4")),
        "matroska,webm" if webm => Some((proto::Container::CONTAINER_WEBM, "WebM")),
        "matroska,webm" => Some((proto::Container::CONTAINER_MATROSKA, "Matroska")),
        _ => None,
    }
}

pub(super) const fn hdr_name(hdr: Hdr) -> &'static str {
    match hdr {
        Hdr::Hdr10 => "HDR10",
        Hdr::Hlg => "HLG",
        Hdr::DolbyVision { .. } => "Dolby Vision",
    }
}

/// The Tracks of `probe` a plan plays as they are in the file: its first video, and the audio
/// Track at `audio`.
pub(super) fn played_tracks(probe: &Probe, audio: Option<usize>) -> Vec<Track> {
    let audio = audio.and_then(|place| probe.tracks.get(place));
    [played_video(probe), audio]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

/// The first video Track, which a browser plays.
pub(super) fn played_video(probe: &Probe) -> Option<&Track> {
    (probe.tracks.iter()).find(|track| matches!(track.kind, TrackKind::Video { .. }))
}

/// How the played video's picture is HDR, or `None` for SDR or no video.
pub(super) fn played_hdr(facts: &Facts) -> Option<Hdr> {
    let TrackKind::Video { hdr, .. } = facts.probe.as_ref().and_then(played_video)?.kind else {
        return None;
    };
    hdr
}

/// How the picture a Transcode decodes from video that's `hdr` is HDR, or `None` for SDR: a Dolby
/// Vision Track's Base Layer, since re-encoding drops Dolby Vision, or else its own.
pub(super) fn transcoded_hdr(hdr: Hdr) -> Option<Hdr> {
    match hdr {
        Hdr::DolbyVision { base_layer, .. } => base_layer?.hdr(),
        hdr @ (Hdr::Hdr10 | Hdr::Hlg) => Some(hdr),
    }
}

/// The place in the Version's Tracks of the audio Track the plan plays: the one chosen, unless the
/// plan dropped it, or else [`file_audio`]. A choice that isn't an audio Track, as when the
/// Version changed since the User saw it, is the same as none.
pub(super) fn played_audio_place(facts: &Facts) -> Option<usize> {
    let probe = facts.probe.as_ref()?;
    let chosen = (facts.audio)
        .filter(|_| !facts.dropped.contains(&Choice::Audio))
        .filter(|&place| probe.tracks.get(place).is_some_and(is_audio));
    chosen.or_else(|| file_audio(probe))
}

/// The place in `probe`'s Tracks of the audio Track a browser plays from the file: the one the
/// file marks to play by default, or else its first.
pub(super) fn file_audio(probe: &Probe) -> Option<usize> {
    let audio = || (probe.tracks.iter().enumerate()).filter(|(_, track)| is_audio(track));
    (audio().find(|(_, track)| track.default))
        .or_else(|| audio().next())
        .map(|(place, _)| place)
}

pub(super) const fn is_audio(track: &Track) -> bool {
    matches!(track.kind, TrackKind::Audio { .. })
}

#[cfg(test)]
#[path = "codecs_tests.rs"]
mod tests;
