//! The lasting and now steps of a plan, and what they work out (ADR 0027).

use buffa::Enumeration;
use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use super::{
    Candidates, Facts, Fallback, Method, Playing, Reason, accel_to_proto, build,
    choices::burn_in,
    codecs::{
        aac_channels, channels_of, converted_channels, hdr_name, played_hdr, played_tracks,
        transcoded_hdr, undecodable, undecodable_container,
    },
    limits::video_limit,
    method_to_proto,
    tracks::{Hdr, Track, hdr_to_proto},
};
use crate::{
    ffmpeg::{
        Accel, Gpu, Hardware, Remux, RemuxAudio, ToneMapper, Transcode, TranscodeAudio,
        VideoConversion,
    },
    server_settings::HardwareChoice,
};

/// The step for the Version's own Tracks: a Remux or a Transcode needs them.
pub(super) fn version_tracks(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    if facts.probe.is_some() {
        return vec![];
    }
    vec![
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_TRACKS_UNKNOWN,
            "Its Tracks aren't known, so it can only be Direct Played",
        )
        .ruling_out([Method::Remux, Method::Transcode]),
    ]
}

/// The step for the Version's length: the HLS playlist of a Remux or a Transcode lists how long
/// each segment is.
pub(super) fn version_length(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    if facts
        .probe
        .as_ref()
        .is_some_and(|probe| probe.duration_ms.is_some())
    {
        return vec![];
    }
    vec![
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_LENGTH_UNKNOWN,
            "Its length isn't known, which a Remux's or a Transcode's playlist needs, so it can \
             only be Direct Played",
        )
        .ruling_out([Method::Remux, Method::Transcode]),
    ]
}

/// The step for what the Device decodes: Direct Play only if it decodes the Version's container
/// and the Tracks the plan plays, and its display shows their HDR, a Remux only if it decodes the
/// video, since a Remux converts only the audio, and neither a Remux nor a Transcode unless it
/// plays HLS.
pub(super) fn device_decoding(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    // `version_tracks` already rules out all but Direct Play, which is then taken to decode them.
    let (Some(probe), Some(candidates)) = (&facts.probe, candidates) else {
        return vec![];
    };
    let mut reasons: Vec<Reason> = undecodable_container(probe, &facts.device)
        .into_iter()
        .chain(
            (played_tracks(probe, candidates.audio).iter())
                .flat_map(|track| undecodable(track, &facts.device)),
        )
        .collect();
    if !facts.device.hls {
        reasons.push(
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_PLAY_HLS,
                "This Device can't play HLS, which a Remux and a Transcode need",
            )
            .ruling_out([Method::Remux, Method::Transcode]),
        );
    }
    reasons
}

/// The step for the Server's ffmpeg: without it, only Direct Play (ADR 0026); without libx264, no
/// Transcode; and no Transcode of a Dolby Vision picture with no Base Layer.
pub(super) fn server_ffmpeg(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let reason = if !facts.can_do.runs {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_SERVER_LACKS_FFMPEG,
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play",
        )
        .ruling_out([Method::Remux, Method::Transcode])
    } else if !facts.can_do.transcodes() {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_SERVER_CANT_ENCODE_VIDEO,
            "This Server's ffmpeg lacks libx264, so it can't Transcode",
        )
        .ruling_out([Method::Transcode])
    } else if let Some(Hdr::DolbyVision {
        base_layer: None, ..
    }) = played_hdr(facts)
    {
        // Its picture is in Dolby Vision's own colours, which only a Dolby Vision player knows.
        // ponytail: Dolby Vision profile 5 stays out of reach: none of the hardware APIs' ffmpeg
        // filters reshape its colours, which only libplacebo does.
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_DOLBY_VISION_NEEDS_BASE_LAYER,
            "This Server can't Transcode a Dolby Vision picture with no Base Layer, as in \
             profile 5",
        )
        .ruling_out([Method::Transcode])
    } else {
        return vec![];
    };
    vec![reason]
}

/// What `transcode` goes on with in software if it runs on hardware: the same conversion,
/// tone-mapping only if ffmpeg has zscale, with the plan's `reasons` but those that no longer
/// hold, and why it fell back (ADR 0034).
pub(super) fn fallback(
    facts: &Facts,
    transcode: &Transcode,
    reasons: &[Reason],
) -> Option<Fallback> {
    let accel = transcode.video.hardware.as_ref()?.accel;
    let tone_map = (transcode.video.tone_map).and_then(|_| facts.can_do.tone_mapper(None));
    let stops_tone_mapping = tone_map.is_none() && transcode.video.tone_map.is_some();
    let still_holds = |reason: &&Reason| {
        reason.kind != Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE
            && (reason.kind != Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED || !stops_tone_mapping)
    };
    let failed = accel_reason(
        Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
        accel,
        format!(
            "{} failed partway, so the rest of it Transcodes in software",
            accel.name()
        ),
    );
    let reasons = (reasons.iter().filter(still_holds).cloned())
        .chain([failed])
        .chain(
            played_hdr(facts)
                .filter(|_| stops_tone_mapping)
                .map(not_tone_mapped),
        )
        .collect();
    Some(Fallback {
        transcode: Transcode {
            video: VideoConversion {
                hardware: None,
                tone_map,
                ..transcode.video
            },
            ..*transcode
        },
        reasons,
    })
}

/// What tone-maps the played video's HDR to SDR in a Transcode, when the Device can't show it: the
/// GPU, if a Transcode runs on one that passed its tone-map test and burns nothing in, which
/// takes the frames off it, or else zscale, if ffmpeg has it.
fn tone_mapper(facts: &Facts) -> Option<ToneMapper> {
    if facts.device.hdr || played_hdr(facts).and_then(transcoded_hdr).is_none() {
        return None;
    }
    let gpu = transcodes_on(facts)
        .ok()
        .filter(|_| burn_in(facts).is_none());
    facts.can_do.tone_mapper(gpu.as_ref())
}

/// The step for what the method `playing` changes, which runs once the plan has one, as it rules
/// nothing out: the audio channels it drops, and for a Transcode, how it does with HDR the Device
/// can't show and what it runs on.
pub(super) fn converting(
    facts: &Facts,
    candidates: Option<&Candidates>,
    playing: Option<&Playing>,
) -> Vec<Reason> {
    let (method, channels) = match playing {
        Some(Playing::Remux { remux, .. }) => (
            Method::Remux,
            remux.audio.and_then(|audio| audio.aac_channels),
        ),
        Some(Playing::Transcode { transcode, .. }) => (
            Method::Transcode,
            transcode.audio.map(|audio| audio.aac_channels),
        ),
        None | Some(Playing::DirectPlay) => return vec![],
    };
    let mut reasons: Vec<Reason> = channels
        .and_then(|channels| fewer_channels(facts, candidates, channels, method))
        .into_iter()
        .collect();
    if let Some(Playing::Transcode { transcode, .. }) = playing {
        reasons.extend(tone_mapping(facts, transcode.video.tone_map));
        reasons.extend(match &transcode.video.hardware {
            Some(hardware) => {
                let gpu = (hardware.gpu.as_ref()).map_or_else(|| "GPU".to_owned(), Gpu::name);
                Some(accel_reason(
                    Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE,
                    hardware.accel,
                    format!("It Transcodes on the {gpu} with {}", hardware.accel.name()),
                ))
            }
            None => transcodes_on(facts).err(),
        });
    }
    reasons
}

/// Why `method` converts the played audio to `channels` channels, if that's fewer than it has.
fn fewer_channels(
    facts: &Facts,
    candidates: Option<&Candidates>,
    channels: i64,
    method: Method,
) -> Option<Reason> {
    let played = (candidates?.audio)
        .and_then(|place| facts.probe.as_ref()?.tracks.get(place))
        .and_then(channels_of)?;
    if channels >= played {
        return None;
    }
    let (kind, why) = match facts.device.audio_channels {
        6.. => (
            Kind::PLAYBACK_REASON_KIND_CONVERSION_CHANNELS_CAPPED,
            format!("the most a {} makes", method.name()),
        ),
        0 => (
            Kind::PLAYBACK_REASON_KIND_DEVICE_CHANNELS_UNKNOWN,
            "as this Device doesn't say it plays more".to_owned(),
        ),
        _ => (
            Kind::PLAYBACK_REASON_KIND_DEVICE_PLAYS_FEWER_CHANNELS,
            "as this Device doesn't play 5.1".to_owned(),
        ),
    };
    let name = match channels {
        1 => "mono".to_owned(),
        2 => "stereo".to_owned(),
        6 => "5.1".to_owned(),
        n => format!("{n}-channel"),
    };
    let text = format!("Its audio is converted to {name}, {why}");
    Some(
        Reason::new(kind, text)
            .with("channels", &channels)
            .deciding(),
    )
}

/// Whether a Transcode tone-maps the played picture to SDR with `tone_map`, when it's HDR the
/// Device can't show and the Transcode keeps.
fn tone_mapping(facts: &Facts, tone_map: Option<ToneMapper>) -> Option<Reason> {
    let hdr =
        played_hdr(facts).filter(|&hdr| !facts.device.hdr && transcoded_hdr(hdr).is_some())?;
    if tone_map.is_none() {
        return Some(not_tone_mapped(hdr));
    }
    let text = format!("Its {} picture is tone-mapped to SDR", hdr_name(hdr));
    Some(hdr_reason(Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED, hdr, text).deciding())
}

fn not_tone_mapped(hdr: Hdr) -> Reason {
    let text = format!(
        "Nothing on this Server tone-maps its {} picture to SDR, so its colours look washed out",
        hdr_name(hdr)
    );
    hdr_reason(Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED, hdr, text).deciding()
}

/// A reason of `kind` about the hardware acceleration API `accel`.
fn accel_reason(kind: Kind, accel: Accel, text: String) -> Reason {
    Reason::new(kind, text).with("hardwareAcceleration", &accel_to_proto(accel).proto_name())
}

/// A reason of `kind` about a picture that's `hdr`.
pub(super) fn hdr_reason(kind: Kind, hdr: Hdr, text: String) -> Reason {
    Reason::new(kind, text).with("hdrFormat", &hdr_to_proto(Some(hdr)).0.proto_name())
}

/// The hardware a Transcode runs on, as the admin chose, or why it runs in software.
fn transcodes_on(facts: &Facts) -> Result<Hardware, Reason> {
    let can_do = &facts.can_do;
    let failed = |accel, text: String| {
        accel_reason(
            Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_FAILED_TEST,
            accel,
            text,
        )
    };
    match &facts.hardware_choice {
        HardwareChoice::Off => Err(Reason::new(
            Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_OFF,
            "Hardware acceleration is off, so it Transcodes in software",
        )),
        HardwareChoice::Automatic => can_do.automatic().cloned().ok_or_else(|| {
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
                "No hardware acceleration passed its test when this Server started, so it \
                 Transcodes in software",
            )
        }),
        // Stored before GPUs could be picked, or where none was found, it means the API on
        // whichever GPU Automatic prefers.
        HardwareChoice::Api(accel, None) => (can_do.hardware.iter())
            .find(|found| found.accel == *accel)
            .cloned()
            .ok_or_else(|| {
                failed(
                    *accel,
                    format!(
                        "{} didn't pass its test when this Server started, so it Transcodes in \
                         software",
                        accel.name()
                    ),
                )
            }),
        // Software, not Automatic's pick: an Admin may have picked this GPU to keep another free.
        HardwareChoice::Api(accel, Some(picked)) => (can_do.hardware.iter())
            .find(|found| found.is_pair(*accel, &picked.id))
            .cloned()
            .ok_or_else(|| {
                // The name leads the sentence, and an integrated GPU's starts lowercase.
                let named = capitalised(&picked.name);
                let text = if can_do.gpus.iter().any(|gpu| gpu.id() == picked.id) {
                    format!(
                        "{named} didn't pass its {} test when this Server started, so it \
                         Transcodes in software",
                        accel.name()
                    )
                } else {
                    format!(
                        "{named} wasn't found when this Server started, so it Transcodes in \
                         software"
                    )
                };
                failed(*accel, text)
            }),
    }
}

/// `text` with its first letter upper case.
fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The most Transcodes the Server runs at once when the admin hasn't set it: no limit while they
/// run on hardware, which a modest Server has room for, and 2 in software.
pub(super) fn default_max_transcodes(facts: &Facts) -> u32 {
    /// As many as a modest Server's CPU keeps up with.
    const IN_SOFTWARE: u32 = 2;
    if transcodes_on(facts).is_ok() {
        u32::MAX
    } else {
        IN_SOFTWARE
    }
}

/// How a Transcode for `facts` converts the played video.
fn video_conversion(facts: &Facts) -> VideoConversion {
    let hardware = transcodes_on(facts).ok();
    VideoConversion {
        // Hardware may fail, and software carry on after its init segment.
        in_band_headers: hardware.is_some(),
        hardware,
        tone_map: tone_mapper(facts),
        limit: video_limit(facts),
        burn_in: burn_in(facts),
    }
}

/// What a Remux for `facts` converts, playing `audio`: the index ffmpeg knows an audio Track by,
/// and that Track.
pub(super) fn remux(facts: &Facts, audio: Option<(usize, &Track)>) -> Remux {
    let audio = audio.map(|(track, played)| RemuxAudio {
        track,
        aac_channels: aac_channels(&facts.device, played),
    });
    Remux { audio }
}

/// What a Transcode for `facts` converts, playing `audio` as [`remux`] does.
pub(super) fn transcode(facts: &Facts, audio: Option<(usize, &Track)>) -> Transcode {
    let audio = audio.map(|(track, played)| TranscodeAudio {
        track,
        aac_channels: converted_channels(&facts.device, played),
    });
    Transcode {
        audio,
        video: video_conversion(facts),
    }
}

/// The step for how busy the Server is: no Transcode while other Devices run as many as it may.
pub(super) fn server_busy(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let busy = facts.transcodes_full.map(|max| {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_SERVER_BUSY,
            format!(
                "This Server is busy, running as many Transcodes as it may at once ({max}), so \
                 it can't Transcode it"
            ),
        )
        .with("maxTranscodes", &max)
        .ruling_out([Method::Transcode])
    });
    busy.into_iter().collect()
}

/// The most Transcodes the Server may run at once, if running that many is all that keeps the
/// plan for `facts` from playing: a Transcode would play it otherwise. A plan nothing else plays
/// is refused as busy rather than shown as unplayable, since trying again later may play it. Only
/// what the Device needs counts: a choice that needs a Transcode is dropped first.
pub(super) fn too_busy_to_play(facts: Facts) -> Option<u32> {
    let max = facts.transcodes_full?;
    let mut not_busy = Facts {
        transcodes_full: None,
        ..facts
    };
    (build(&mut not_busy).method() == Some(Method::Transcode)).then_some(max)
}

/// The step for what just failed to play on the Device: it rules out each method that did, so
/// the player can ask again without looping.
pub(super) fn failed_on_device(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    (facts.failed.iter())
        .map(|&method| {
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
                format!("{} failed on this Device", method.name()),
            )
            .with("method", &method_to_proto(Some(method)).proto_name())
            .ruling_out([method])
        })
        .collect()
}
