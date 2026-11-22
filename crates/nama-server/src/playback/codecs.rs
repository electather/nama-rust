//! What a Device decodes, and the Tracks a plan plays once converted.

use buffa::Enumeration;
use nama_proto::nama::v1 as proto;
use proto::PlaybackReasonKind as Kind;

use super::{
    Choice, DeviceDecoding, Facts, Method, Playing, Reason, makes_hevc, recipe,
    steps::hdr_reason,
    tracks::{Hdr, Probe, Track, TrackKind},
};
use crate::ffmpeg::{AudioConversion, AudioEncoder, Remux, RemuxAudio, Transcode, VideoConversion};

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
            let hdr = hdr.and_then(|hdr| unshown(hdr, device));
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

/// Why `device` can't show a picture that's `hdr`, if it can't: it shows no HDR, or the picture
/// is Dolby Vision with no Base Layer in a profile it doesn't list.
fn unshown(hdr: Hdr, device: &DeviceDecoding) -> Option<Reason> {
    if !device.hdr {
        let text = format!(
            "This Device can't show its {} picture, so it can't be Direct Played or Remuxed",
            hdr_name(hdr)
        );
        let reason = hdr_reason(Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR, hdr, text);
        return Some(reason.ruling_out([Method::DirectPlay, Method::Remux]));
    }
    let Hdr::DolbyVision {
        profile,
        base_layer: None,
    } = hdr
    else {
        return None;
    };
    let shown = (device.dolby_vision_profiles.iter()).any(|&shown| i64::from(shown) == profile);
    (!shown).then(|| {
        let text = format!(
            "This Device can't show Dolby Vision profile {profile}, which has no Base Layer, so \
             it can't be Direct Played or Remuxed"
        );
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_DOLBY_VISION,
            text,
        )
        .with("profile", &profile)
        .ruling_out([Method::DirectPlay, Method::Remux])
    })
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
                            conversion: Some(conversion),
                            ..
                        }),
                    ..
                },
            ..
        }) => converted_audio(tracks, *conversion),
        Some(Playing::Transcode {
            transcode: Transcode { audio, video },
            ..
        }) => {
            let tracks = match audio {
                Some(audio) => converted_audio(tracks, audio.conversion),
                None => tracks,
            };
            converted_video(tracks, video)
        }
        None | Some(Playing::DirectPlay | Playing::Remux { .. }) => tracks,
    }
}

/// `facts.device` without each audio codec `method` would send `audio` as, beside `video` for a
/// Transcode, whose Recipe by it just failed on the Device, so it sends another if it can; and
/// those codecs, as ffprobe names them.
pub(super) fn untried_by(
    method: Method,
    facts: &Facts,
    video: Option<&str>,
    audio: &Track,
) -> (DeviceDecoding, Vec<String>) {
    let mut device = facts.device.clone();
    let mut passed_over = vec![];
    loop {
        let sent = match method {
            Method::Remux => remux_audio_conversion(&device, audio),
            Method::DirectPlay | Method::Transcode => Some(audio_conversion(&device, audio)),
        }
        .map_or(audio.codec.as_str(), |conversion| {
            conversion.encoder.ffmpeg_name()
        });
        // Stereo is sent as AAC whatever the Device decodes, so passing over a codec it no longer
        // lists would never end.
        let failed = (AUDIO_CODECS.iter())
            .find(|(name, ..)| *name == sent)
            .map(|&(_, codec, _)| codec)
            .filter(|codec| device.audio_codecs.contains(codec))
            .filter(|_| (facts.failed_recipes).contains(&recipe(method, video, Some(sent))));
        let Some(failed) = failed else {
            passed_over.retain(|codec| codec != sent);
            return (device, passed_over);
        };
        device.audio_codecs.retain(|&codec| codec != failed);
        passed_over.push(sent.to_owned());
    }
}

/// What a Remux converts the audio Track `audio` to, if `device` can't decode it, as
/// [`audio_conversion`] says.
pub(super) fn remux_audio_conversion(
    device: &DeviceDecoding,
    audio: &Track,
) -> Option<AudioConversion> {
    if undecodable(audio, device).is_empty() {
        return None;
    }
    Some(audio_conversion(device, audio))
}

/// What the audio Track `audio` is converted to for `device`: as many channels as it has, up to
/// 5.1 if the Device plays 5.1 and stereo otherwise, as E-AC-3, or else AC-3, if that's more than
/// stereo and the Device decodes it, or else as AAC.
pub(super) fn audio_conversion(device: &DeviceDecoding, audio: &Track) -> AudioConversion {
    let most = if device.audio_channels >= 6 { 6 } else { 2 };
    // Stereo when the file doesn't say, which every Device plays.
    let channels = channels_of(audio).unwrap_or(2).min(most);
    let decodes = |codec| device.audio_codecs.contains(&codec);
    // Many TVs and soundbars downmix surround AAC to stereo, but pass Dolby's codecs on as
    // surround.
    let encoder = if channels <= 2 {
        AudioEncoder::Aac
    } else if decodes(proto::AudioCodec::AUDIO_CODEC_EAC3) {
        AudioEncoder::Eac3
    } else if decodes(proto::AudioCodec::AUDIO_CODEC_AC3) {
        AudioEncoder::Ac3
    } else {
        AudioEncoder::Aac
    };
    AudioConversion { encoder, channels }
}

/// The audio codec ffprobe names `ffprobe_name`, in plain words if it's one a Device can decode.
pub(super) fn audio_codec_name(ffprobe_name: &str) -> &str {
    (AUDIO_CODECS.iter())
        .find(|(name, ..)| *name == ffprobe_name)
        .map_or(ffprobe_name, |(.., plain)| plain)
}

/// How many channels the audio Track `audio` has, if the file says.
pub(super) const fn channels_of(audio: &Track) -> Option<i64> {
    match audio.kind {
        TrackKind::Audio { channels, .. } => channels,
        TrackKind::Video { .. } | TrackKind::Subtitle { .. } => None,
    }
}

/// `tracks` with their audio converted as `conversion` says.
fn converted_audio(mut tracks: Vec<Track>, conversion: AudioConversion) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Audio {
            channels, atmos, ..
        } = &mut track.kind
        {
            conversion
                .encoder
                .ffmpeg_name()
                .clone_into(&mut track.codec);
            *channels = Some(conversion.channels);
            // ffmpeg's encoders write no Atmos objects.
            *atmos = false;
        }
    }
    tracks
}

/// `tracks` with their video converted to 8-bit H.264 or 10-bit HEVC as `video` says, keeping the
/// HDR it decodes unless it tone-maps it.
// ffmpeg tags what it encodes with the colours it decoded, so an HDR10 or HLG picture stays one,
// even at 8 bits.
// ponytail: checked only for libx264, and for HEVC on a GPU only by an ignored test run by hand;
// a GPU encoder that drops those tags sends an untagged picture the Track calls HDR.
fn converted_video(mut tracks: Vec<Track>, video: &VideoConversion) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Video {
            bit_depth,
            width,
            height,
            hdr,
            ..
        } = &mut track.kind
        {
            video.codec().clone_into(&mut track.codec);
            *bit_depth = Some(if video.hevc { 10 } else { 8 });
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

/// Whether a Transcode for `facts` would rather make 10-bit HEVC than H.264, on hardware that can:
/// the Device decodes it, the picture is 4K, which H.264 sends larger, or HDR it shows, which
/// H.264 bands, and no plan making HEVC just failed (#1415). An HDR picture the Device can't show
/// is tone-mapped to H.264 instead.
pub(super) fn wants_hevc(facts: &Facts) -> bool {
    let device = &facts.device;
    let decodes =
        device.hevc_10bit && (device.video_codecs).contains(&proto::VideoCodec::VIDEO_CODEC_HEVC);
    let Some(TrackKind::Video {
        width, height, hdr, ..
    }) = facts
        .probe
        .as_ref()
        .and_then(played_video)
        .map(|video| &video.kind)
    else {
        return false;
    };
    // The longer side, so a portrait 4K clip counts too.
    let uhd = width.max(height).is_some_and(|side| side >= 3840);
    let hdr = hdr.and_then(transcoded_hdr).is_some();
    let failed = (facts.failed_recipes.iter())
        .chain(&facts.failed_on_server)
        .any(|recipe| makes_hevc(recipe));
    decodes && (uhd || hdr) && (!hdr || device.hdr) && !failed
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
