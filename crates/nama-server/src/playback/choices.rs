//! The steps for the User's audio and subtitle choices, and the Tracks their Preferences pick.

use nama_proto::nama::v1 as proto;
use proto::PlaybackReasonKind as Kind;
use proto::SubtitleFormat::{
    SUBTITLE_FORMAT_ASS as ASS, SUBTITLE_FORMAT_VOBSUB as VOBSUB, SUBTITLE_FORMAT_WEBVTT as WEBVTT,
};

use super::{
    AskedAudio, Candidates, Choice, DeviceDecoding, Facts, Method, Reason, Subtitle,
    codecs::{file_audio, is_audio, played_audio_place, undecodable},
    limits::picture,
    subtitles,
    track_preferences::{Description, Preferred, SubtitlePreference, same_language},
    tracks::{Probe, Track, TrackKind},
};
use crate::ffmpeg::{BurnIn, TextSubtitle};

/// The audio Track `asked` asks for, by its place in `probe`'s Tracks. One the Server chose by the
/// User's Audio Preference is then as if they chose it, so a plan that can't play it drops it
/// with the usual reason (ADR 0031).
pub(super) fn audio_asked(
    asked: AskedAudio,
    probe: Option<&Probe>,
    device: &DeviceDecoding,
) -> Option<usize> {
    match asked {
        AskedAudio::Chosen(place) => Some(place),
        AskedAudio::Preferred(preferred) => preferred_audio(preferred.as_ref()?, probe?, device),
    }
}

/// The subtitle Track the User's Subtitle Preference `preferred` asks to burn in and the one it
/// asks the Device to show, each by its place in the Version's Tracks, over the audio the plan for
/// `facts` plays: a Track it matches is shown by the Device if it can, since that is no input to
/// the plan (ADR 0033), and burned in if not.
pub(super) fn preferred_subtitles(
    facts: &Facts,
    preferred: &Preferred<SubtitlePreference>,
) -> (Option<usize>, Option<usize>) {
    let Some(probe) = &facts.probe else {
        return (None, None);
    };
    let audio = played_audio_place(facts).and_then(|place| probe.tracks.get(place));
    let preferred = over_audio(preferred, audio.and_then(|track| track.language.as_deref()));
    let place = preferred_subtitle(&preferred, probe, &facts.device);
    match place.and_then(|place| probe.tracks.get(place)) {
        Some(track) if shown_as(&facts.device, probe, track).is_some() => (None, place),
        _ => (place, None),
    }
}

/// What `preferred` wants over audio in `audio_language`: if it wants full subtitles only over
/// audio in another language and the audio is in theirs, that language's forced-only Track
/// (ADR 0035). Audio with no language, or a tag that isn't ISO 639, is in another.
fn over_audio(
    preferred: &Preferred<SubtitlePreference>,
    audio_language: Option<&str>,
) -> Preferred<SubtitlePreference> {
    match &preferred.preference {
        SubtitlePreference::Like(wanted)
            if preferred.foreign_audio_only
                && audio_language.is_some()
                && same_language(audio_language, wanted.language.as_deref()) =>
        {
            Preferred {
                preference: SubtitlePreference::Like(Description {
                    forced: true,
                    ..wanted.clone()
                }),
                ..preferred.clone()
            }
        }
        SubtitlePreference::Off | SubtitlePreference::Like(_) => preferred.clone(),
    }
}

/// The step for the audio Track the User chose: Direct Play only if it's the one browsers play
/// from the file, since only Safari switches audio inside a file.
pub(super) fn chosen_audio(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    if facts.dropped.contains(&Choice::Audio) {
        return vec![
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED,
                "The audio Track chosen can't be played here, so the file's own plays instead",
            )
            .deciding(),
        ];
    }
    match (&facts.probe, candidates) {
        (Some(probe), Some(candidates)) if candidates.audio != file_audio(probe) => vec![
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN,
                "The audio Track chosen isn't the one browsers play from its file, so it can't \
                 be Direct Played",
            )
            .ruling_out([Method::DirectPlay]),
        ],
        _ => vec![],
    }
}

/// The step for the subtitle Track the User chose to burn in, one the Device can't draw: only a
/// Transcode draws it into the picture.
pub(super) fn chosen_subtitle(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    if facts.dropped.contains(&Choice::Subtitle) {
        return vec![
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED,
                "The subtitles chosen can't be burned in here, so they're dropped",
            )
            .deciding(),
        ];
    }
    let transcode_burns_in =
        candidates.is_some_and(|candidates| candidates.transcode.video.burn_in.is_some());
    if let Some(track) = chosen_subtitle_track(facts).filter(|_| transcode_burns_in) {
        let name = subtitle_format_name(&track.codec);
        // It draws ASS only with fonts its file carries (`draws_with_styling`), and VobSub only
        // with the palette its Track carries (`subtitles::served_as`).
        let (kind, why) = if track.is_ass_subtitle() && lists(&facts.device, ASS) {
            (
                Kind::PLAYBACK_REASON_KIND_SUBTITLES_FILE_CARRIES_NO_FONTS,
                format!("Its file carries no fonts to draw its {name} subtitles with"),
            )
        } else if track.is_vobsub_subtitle() && lists(&facts.device, VOBSUB) {
            (
                Kind::PLAYBACK_REASON_KIND_SUBTITLES_TRACK_CARRIES_NO_PALETTE,
                format!("This Track carries no palette to draw its {name} subtitles with"),
            )
        } else {
            (
                Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN,
                format!("This Device can't draw {name} subtitles"),
            )
        };
        return vec![
            Reason::new(
                kind,
                format!(
                    "{why}, so the ones chosen are burned into its picture, which only a \
                     Transcode does"
                ),
            )
            .ruling_out([Method::DirectPlay, Method::Remux]),
        ];
    }
    vec![]
}

/// The subtitle Track the User chose, if the Version has it.
fn chosen_subtitle_track(facts: &Facts) -> Option<&Track> {
    facts.probe.as_ref()?.tracks.get(facts.subtitle?)
}

/// What a subtitle format whose codec ffprobe names `codec` is called, such as PGS.
fn subtitle_format_name(codec: &str) -> &str {
    match codec {
        "hdmv_pgs_subtitle" => "PGS",
        "dvd_subtitle" => "VobSub",
        "dvb_subtitle" => "DVB",
        "xsub" => "XSUB",
        "ass" => "ASS",
        "ssa" => "SSA",
        other => other,
    }
}

/// How a Transcode burns in the subtitle Track the User chose, unless the plan dropped it, if it's
/// one that [`burns_in`] on the Device. Any other choice, such as one it draws itself, or one made
/// before the Version changed, is the same as none.
pub(super) fn burn_in(facts: &Facts) -> Option<BurnIn> {
    let probe = facts.probe.as_ref()?;
    let place = facts
        .subtitle
        .filter(|_| !facts.dropped.contains(&Choice::Subtitle))?;
    let track = (probe.tracks.get(place)).filter(|track| burns_in(&facts.device, probe, track))?;
    let sidecar = track.sidecar().map(|sidecar| sidecar.path.clone());
    if matches!(track.kind, TrackKind::Subtitle { image: true, .. }) {
        // A sidecar holds just its one subtitle.
        let subtitle = if sidecar.is_some() {
            0
        } else {
            probe.ffmpeg_index(place)?
        };
        return Some(BurnIn::Image {
            subtitle,
            sidecar,
            picture: picture(facts),
        });
    }
    let subtitle = match sidecar {
        Some(sidecar) => TextSubtitle::Sidecar(sidecar),
        None => TextSubtitle::Stream {
            subtitle: probe.ffmpeg_index(place)?,
            track: place,
            fonts: probe.font_attachments.len(),
        },
    };
    Some(BurnIn::Text { subtitle })
}

/// The place in `probe`'s Tracks of the subtitle Track `preferred` starts playback with, `None` for off or
/// when none matches. A Track in its language and as forced matches; among those, the same SDH
/// wins, then the same title, then one the Device shows itself over one that needs Burn-in, then
/// the first. The User's own Preference skips the title: it was set without one, which doesn't
/// mean an untitled Track.
pub(super) fn preferred_subtitle(
    preferred: &Preferred<SubtitlePreference>,
    probe: &Probe,
    device: &DeviceDecoding,
) -> Option<usize> {
    let SubtitlePreference::Like(wanted) = &preferred.preference else {
        return None;
    };
    (probe.tracks.iter().enumerate())
        .filter(|(_, track)| shown_as(device, probe, track).is_some() || burns_in(device, probe, track))
        .filter(|(_, track)| {
            track.forced == wanted.forced
                && same_language(track.language.as_deref(), wanted.language.as_deref())
        })
        // `min_by_key` keeps the first of equals.
        .min_by_key(|(_, track)| {
            (
                track.sdh != wanted.sdh,
                !preferred.users_own && !same_title(track, wanted),
                shown_as(device, probe, track).is_none(),
            )
        })
        .map(|(place, _)| place)
}

/// The place in `probe`'s Tracks of the audio Track like `preferred` that playback starts with,
/// `None` when none matches. A Track in its language matches; among those, the same title wins,
/// since it tells a commentary from the main mix, then one `device` decodes without it being
/// converted, then the one the file plays by default, then the first. The User's own Preference
/// skips the title, as a subtitle's does.
fn preferred_audio(
    preferred: &Preferred<Description>,
    probe: &Probe,
    device: &DeviceDecoding,
) -> Option<usize> {
    let wanted = &preferred.preference;
    (probe.tracks.iter().enumerate())
        .filter(|(_, track)| {
            is_audio(track) && same_language(track.language.as_deref(), wanted.language.as_deref())
        })
        // `min_by_key` keeps the first of equals.
        .min_by_key(|(_, track)| {
            (
                !preferred.users_own && !same_title(track, wanted),
                !undecodable(track, device).is_empty(),
                !track.default,
            )
        })
        .map(|(place, _)| place)
}

/// Whether `track` has the title `wanted` has, in any case, or neither has one.
fn same_title(track: &Track, wanted: &Description) -> bool {
    let lowercase = |title: &Option<String>| title.as_deref().map(str::to_lowercase);
    lowercase(&track.title) == lowercase(&wanted.title)
}

/// Whether `track`, in the file `probe` read, is a subtitle `device` can't show as it should, so a
/// Transcode burns it in: an image one it doesn't draw, such as PGS, or an ASS or SSA one it
/// doesn't draw, whose styling its WebVTT drops.
pub(super) fn burns_in(device: &DeviceDecoding, probe: &Probe, track: &Track) -> bool {
    match track.kind {
        TrackKind::Subtitle { image, .. } => {
            let drawn = shown_as(device, probe, track).is_some_and(|format| format != WEBVTT);
            (image || track.is_ass_subtitle()) && !drawn
        }
        TrackKind::Video { .. } | TrackKind::Audio { .. } => false,
    }
}

/// The format `device` shows `track`, in the file `probe` read, in itself, if it does: ASS for
/// one it [`draws_with_styling`], WebVTT for a text one, which every Device shows, or else the
/// Track's own, if the Device draws it (ADR 0033).
fn shown_as(
    device: &DeviceDecoding,
    probe: &Probe,
    track: &Track,
) -> Option<proto::SubtitleFormat> {
    if draws_with_styling(device, probe, track) {
        return Some(ASS);
    }
    subtitles::served_as(track).filter(|&format| format == WEBVTT || lists(device, format))
}

/// Whether `device` draws `track`, in the file `probe` read, as ASS with its styling: only an ASS
/// or SSA one, on a Device that lists ASS, in a file that carries fonts, which a sidecar's never
/// does (ADR 0033), whatever its video's carries. A renderer on the Device sees only the fonts it's
/// handed and one fallback for Latin, Greek and Cyrillic, so a Track in Persian, Arabic, Chinese or
/// Japanese with no fonts of its own would draw as empty boxes, where its WebVTT shows in the
/// Device's own fonts.
fn draws_with_styling(device: &DeviceDecoding, probe: &Probe, track: &Track) -> bool {
    track.is_ass_subtitle()
        && lists(device, ASS)
        && track.sidecar().is_none()
        && !probe.font_attachments.is_empty()
}

/// Whether `device` says it draws `format` itself.
fn lists(device: &DeviceDecoding, format: proto::SubtitleFormat) -> bool {
    device.subtitle_formats.contains(&format)
}

/// The Version's subtitle Tracks that `device` shows itself.
pub(super) fn shown_subtitles(
    version: i64,
    probe: &Probe,
    device: &DeviceDecoding,
) -> Vec<Subtitle> {
    (probe.tracks.iter().enumerate())
        .filter_map(|(place, track)| {
            let format = shown_as(device, probe, track)?;
            let fonts = if format == ASS {
                probe.font_attachments.len()
            } else {
                0
            };
            Some(Subtitle {
                track: track.clone(),
                place,
                format,
                url: subtitles::url(version, place, format),
                font_urls: (0..fonts)
                    .map(|font| subtitles::font_url(version, font))
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "choices_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "choices/preferred_tests.rs"]
mod preferred_tests;
