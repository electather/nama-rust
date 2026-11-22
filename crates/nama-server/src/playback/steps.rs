//! The lasting and now steps of a plan, and what they work out (ADR 0027).

use std::collections::BTreeSet;

use buffa::Enumeration;
use nama_proto::nama::v1::PlaybackReasonKind as Kind;

use super::{
    Candidates, Facts, Fallback, Method, Playing, Reason, RunningTranscodes, accel_to_proto, build,
    choices::burn_in,
    codecs::{
        audio_codec_name, audio_conversion, channels_of, hdr_name, played_hdr, played_tracks,
        played_video, remux_audio_conversion, transcoded_hdr, undecodable, undecodable_container,
        untried_by, wants_hevc,
    },
    limits::video_limit,
    method_to_proto, sent_by, sent_recipe,
    tracks::{Hdr, Track, hdr_to_proto},
};
use crate::{
    ffmpeg::{
        Accel, AudioConversion, Gpu, Hardware, Remux, RemuxAudio, ToneMapper, Transcode,
        TranscodeAudio, VideoConversion, VideoTag,
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
/// Transcode; and without `tonemapx`, no Transcode of a Dolby Vision picture with no Base Layer
/// (ADR 0102).
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
    } else if !facts.can_do.converts_dolby_vision() && needs_reshaping(facts) {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_DOLBY_VISION_NEEDS_BASE_LAYER,
            "This Server's ffmpeg lacks jellyfin-ffmpeg's tonemapx, so it can't Transcode a \
             Dolby Vision picture with no Base Layer, as in profile 5",
        )
        .ruling_out([Method::Transcode])
    } else {
        return vec![];
    };
    vec![reason]
}

/// Whether the played picture is Dolby Vision with no Base Layer, in Dolby Vision's own colours,
/// which only its RPU reshapes into colours a player without Dolby Vision shows.
fn needs_reshaping(facts: &Facts) -> bool {
    matches!(
        played_hdr(facts),
        Some(Hdr::DolbyVision {
            base_layer: None,
            ..
        })
    )
}

/// What `transcode` goes on with in software if it runs on hardware making H.264: the same
/// conversion, tone-mapping in software if it did on the GPU and ffmpeg can, with the plan's
/// `reasons` but those that no longer hold, and why it fell back (ADR 0034). HEVC has none, as its
/// playlist can't change codec partway: failing ends the plan, and its Recipe with it.
pub(super) fn fallback(
    facts: &Facts,
    transcode: &Transcode,
    reasons: &[Reason],
) -> Option<Fallback> {
    let video = &transcode.video;
    let accel = video.hardware.as_ref().filter(|_| !video.hevc)?.accel;
    let tone_map = match video.tone_map {
        Some(ToneMapper::Hardware) => facts.can_do.tone_mapper(None),
        in_software => in_software,
    };
    let stops_tone_mapping = tone_map.is_none() && video.tone_map.is_some();
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
                ..video.clone()
            },
            ..*transcode
        },
        reasons,
    })
}

/// What converts the played video's HDR in a Transcode. HDR the Device can't show is tone-mapped
/// to SDR by the GPU, if a Transcode runs on one that passed its tone-map test and burns nothing
/// in, which takes the frames off it, or else by software, if ffmpeg can. Dolby Vision with no
/// Base Layer is always reshaped by `tonemapx`: into HDR10 if the Transcode makes `hevc`, which it
/// does only for a Device showing HDR, and else tone-mapped to SDR.
fn tone_mapper(facts: &Facts, hevc: bool) -> Option<ToneMapper> {
    if needs_reshaping(facts) {
        let reshaped = if hevc {
            ToneMapper::DolbyVisionToHdr10
        } else {
            ToneMapper::Tonemapx { dolby_vision: true }
        };
        return facts.can_do.converts_dolby_vision().then_some(reshaped);
    }
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
    let (method, conversion) = match playing {
        Some(Playing::Remux { remux, .. }) => (
            Method::Remux,
            remux.audio.and_then(|audio| audio.conversion),
        ),
        Some(Playing::Transcode { transcode, .. }) => (
            Method::Transcode,
            transcode.audio.map(|audio| audio.conversion),
        ),
        None | Some(Playing::DirectPlay) => return vec![],
    };
    let mut reasons: Vec<Reason> = (passed_over_recipes(candidates, method).into_iter())
        .chain(
            conversion.and_then(|conversion| fewer_channels(facts, candidates, conversion, method)),
        )
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

/// Why `method` converts the played audio as `conversion` says, if to fewer channels than it has.
fn fewer_channels(
    facts: &Facts,
    candidates: Option<&Candidates>,
    AudioConversion { encoder, channels }: AudioConversion,
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
    let codec = encoder.ffmpeg_name();
    let text = format!(
        "Its audio is converted to {} {name}, {why}",
        audio_codec_name(codec)
    );
    Some(
        Reason::new(kind, text)
            .with("channels", &channels)
            .with("codec", &codec)
            .deciding(),
    )
}

/// Whether a Transcode tone-maps the played picture to SDR with `tone_map`, which only
/// [`tone_mapper`] sets, or not when it's HDR the Device can't show and the Transcode keeps. A
/// Dolby Vision picture reshaped to HDR10 is neither.
fn tone_mapping(facts: &Facts, tone_map: Option<ToneMapper>) -> Option<Reason> {
    let hdr = played_hdr(facts)?;
    if tone_map.is_some_and(|tone_map| !tone_map.makes_sdr()) {
        return None;
    }
    if tone_map.is_none() {
        return (!facts.device.hdr && transcoded_hdr(hdr).is_some()).then(|| not_tone_mapped(hdr));
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

/// The most Transcodes that work the CPU hard the Server runs at once when the admin hasn't set a
/// cap, as many as a modest Server's CPU keeps up with. Those fully on a GPU have no limit.
const DEFAULT_MAX_CPU_BOUND: u32 = 2;

/// The most Transcodes the Server may run at once, if the ones running leave no room for one
/// converting `video`: an admin's cap counts every Transcode, the default only those on the CPU.
fn transcodes_full(facts: &Facts, video: Option<&VideoConversion>) -> Option<u32> {
    let running = facts.running_transcodes;
    match facts.max_transcodes {
        Some(max) => (running.all >= max.get()).then_some(max.get()),
        None => (video.is_some_and(VideoConversion::cpu_bound)
            && running.cpu_bound >= DEFAULT_MAX_CPU_BOUND)
            .then_some(DEFAULT_MAX_CPU_BOUND),
    }
}

/// How a Transcode for `facts` converts the played video.
fn video_conversion(facts: &Facts) -> VideoConversion {
    let hardware = transcodes_on(facts).ok();
    let hevc = hardware.as_ref().is_some_and(|hardware| hardware.hevc) && wants_hevc(facts);
    VideoConversion {
        // Hardware making H.264 may fail, and software carry on after its init segment.
        in_band_headers: hardware.is_some() && !hevc,
        hevc,
        hardware,
        tone_map: tone_mapper(facts, hevc),
        limit: video_limit(facts),
        burn_in: burn_in(facts),
    }
}

/// What a Remux for `facts` converts, playing `audio`: the index ffmpeg knows an audio Track by,
/// and that Track; and the audio codecs it passes over, as [`untried_by`] says.
pub(super) fn remux(facts: &Facts, audio: Option<(usize, &Track)>) -> (Remux, Vec<String>) {
    let mut passed_over = vec![];
    let audio = audio.map(|(track, played)| {
        let (device, passed) = untried_by(Method::Remux, facts, None, played);
        passed_over = passed;
        RemuxAudio {
            track,
            conversion: remux_audio_conversion(&device, played),
        }
    });
    let video = facts.probe.as_ref().and_then(played_video);
    let video_tag = match (video.map(|video| video.codec.as_str()), played_hdr(facts)) {
        (Some("hevc"), Some(Hdr::DolbyVision { profile: 5, .. })) => Some(VideoTag::Dvh1),
        (Some("hevc"), _) => Some(VideoTag::Hvc1),
        _ => None,
    };
    let remux = Remux { audio, video_tag };
    (remux, passed_over)
}

/// What a Transcode for `facts` converts, playing `audio`, and the codecs it passes over, as
/// [`remux`] does.
pub(super) fn transcode(facts: &Facts, audio: Option<(usize, &Track)>) -> (Transcode, Vec<String>) {
    let mut passed_over = vec![];
    let video = video_conversion(facts);
    let audio = audio.map(|(track, played)| {
        let (device, passed) = untried_by(Method::Transcode, facts, Some(video.codec()), played);
        passed_over = passed;
        TranscodeAudio {
            track,
            conversion: audio_conversion(&device, played),
        }
    });
    let transcode = Transcode { audio, video };
    (transcode, passed_over)
}

/// The step for how busy the Server is: no Transcode while as many run as it may.
pub(super) fn server_busy(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    let video = candidates.map(|candidates| &candidates.transcode.video);
    let busy = transcodes_full(facts, video).map(|max| {
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
/// what the Device needs counts: `build` keeps a choice only when nothing plays without it either,
/// so a Transcode the choice needs is one the Device needs too.
pub(super) fn too_busy_to_play(facts: Facts) -> Option<u32> {
    let running_transcodes = facts.running_transcodes;
    if running_transcodes == RunningTranscodes::default() {
        return None;
    }
    let mut not_busy = Facts {
        running_transcodes: RunningTranscodes::default(),
        ..facts
    };
    let plan = build(&mut not_busy);
    let Some(Playing::Transcode { transcode, .. }) = &plan.playing else {
        return None;
    };
    not_busy.running_transcodes = running_transcodes;
    transcodes_full(&not_busy, Some(&transcode.video))
}

/// The step for what just failed to play on the Device: it rules out each method that did, or
/// whose every Recipe did, so the player can ask again without looping.
pub(super) fn failed_on_device(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    // A candidate sends another codec than one whose Recipe failed if it can, so its own Recipe
    // failed only once every one it could send has.
    let every_recipe_failed = sending_recipes(&facts.failed_recipes, facts, candidates);
    failures(
        &facts
            .failed
            .iter()
            .copied()
            .chain(every_recipe_failed)
            .collect(),
        Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
        "on this Device",
    )
}

/// Why the Remux or Transcode `method` sends audio in another codec than one of its Recipes that
/// just failed on the Device: one reason for each codec it passed over.
fn passed_over_recipes(candidates: Option<&Candidates>, method: Method) -> Vec<Reason> {
    let passed_over = match (candidates, method) {
        (Some(candidates), Method::Remux) => candidates.remux_passed_over.as_slice(),
        (Some(candidates), Method::Transcode) => candidates.transcode_passed_over.as_slice(),
        (None, _) | (_, Method::DirectPlay) => &[],
    };
    (passed_over.iter())
        .map(|codec| {
            let text = format!(
                "A {} sending {} audio just failed on this Device, so it sends another",
                method.name(),
                audio_codec_name(codec)
            );
            Reason::new(Kind::PLAYBACK_REASON_KIND_RECIPE_FAILED_ON_DEVICE, text)
                .with("method", &method_to_proto(Some(method)).proto_name())
                .with("codec", codec)
                .deciding()
        })
        .collect()
}

/// The step for plans whose ffmpeg stopped before writing anything: it rules out each method whose
/// plan would have the same Recipe, so planning goes on to the next, or to another way of playing
/// it, such as H.264 once HEVC failed.
pub(super) fn failed_on_server(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    failures(
        &sending_recipes(&facts.failed_on_server, facts, candidates).collect(),
        Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_SERVER,
        "on the Server, as its ffmpeg stopped before writing anything",
    )
}

/// The methods whose plan for `facts` would have one of `recipes`.
fn sending_recipes(
    recipes: &BTreeSet<String>,
    facts: &Facts,
    candidates: Option<&Candidates>,
) -> impl Iterator<Item = Method> {
    Method::ALL.into_iter().filter(move |&method| {
        sent_by(method, facts, candidates)
            .is_some_and(|tracks| recipes.contains(&sent_recipe(method, &tracks)))
    })
}

/// A reason of `kind` for each of `methods`, ruling it out, saying it failed `where_`.
fn failures(methods: &BTreeSet<Method>, kind: Kind, where_: &str) -> Vec<Reason> {
    (methods.iter())
        .map(|&method| {
            Reason::new(kind, format!("{} failed {where_}", method.name()))
                .with("method", &method_to_proto(Some(method)).proto_name())
                .ruling_out([method])
        })
        .collect()
}

/// The step for keyframes the Server couldn't list: it rules out a Remux, which cuts its
/// segments at them. A failure kept for the file rules it out until the file changes (#850), and
/// any other only for this plan.
pub(super) fn keyframes_unlisted(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let unlisted = facts.keyframes_unlisted.then(|| {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED,
            "The Version's keyframes couldn't be listed, so it can't be Remuxed",
        )
        .ruling_out([Method::Remux])
    });
    unlisted.into_iter().collect()
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
