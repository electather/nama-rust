//! Playback Plans: how the Server plays one Version on one Device, and why (ADR 0027).
//!
//! [`plan`] gathers what a plan is based on, probing the Version first if its Tracks aren't
//! current, and runs [`STEPS`] over it in order. Each step says which Methods it allows and why,
//! and the plan keeps only what every step allows, so no step can widen what an earlier one ruled
//! out.
//!
//! Direct Play serves a Version's file as it is, over plain HTTP at `/media/versions/<id>` (ADR
//! 0007: media bytes are not RPC), to anyone with a session. A browser's `<video src>` presents
//! that session in its cookie (ADR 0014). Range requests let a `<video>` seek. Only Versions,
//! files a scan stored for a Library and Matched, are served, by id, so a client can never name a
//! path.

use std::collections::BTreeSet;

use axum::{
    Router,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::services::ServeFile;

use crate::{
    contents::Contents,
    ffmpeg::ProbeError,
    scanner::{ProbeNowError, Scanner},
    tracks::{Probe, Track, TrackKind},
};

/// How a plan plays a Version, the one the Server prefers first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Method {
    DirectPlay,
    Remux,
    Transcode,
}

impl Method {
    const ALL: [Self; 3] = [Self::DirectPlay, Self::Remux, Self::Transcode];
}

/// How the Server will play one Version on one Device, and why.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    /// `None` when it can't be played.
    pub(crate) method: Option<Method>,
    /// Where to play it from, on this Server. `None` when it can't be played.
    pub(crate) url: Option<String>,
    /// Its video, then its audio. Empty when its Tracks aren't known.
    pub(crate) tracks: Vec<Track>,
    /// Why, one for each step, in the order they ran.
    pub(crate) reasons: Vec<String>,
}

/// Plans playing the Version `version`, probing it first unless its Tracks are current. `None`
/// if there is no such Version.
pub(crate) async fn plan(scanner: &Scanner, version: i64) -> Result<Option<Plan>, sqlx::Error> {
    let probe = match scanner.probe_now(version).await {
        Ok(probe) => Some(probe),
        Err(ProbeNowError::Probe(err)) => {
            if !matches!(*err, ProbeError::Unavailable) {
                tracing::warn!(
                    version,
                    "could not probe a Version to plan playing it: {err}"
                );
            }
            None
        }
        Err(ProbeNowError::NoSuchVersion(_)) => return Ok(None),
        Err(ProbeNowError::Database(err)) => return Err(err),
    };
    Ok(Some(build(&Facts { version, probe }, &STEPS)))
}

/// What a plan is based on.
struct Facts {
    version: i64,
    /// What probing found in the Version, or `None` if it couldn't be probed.
    probe: Option<Probe>,
}

/// What one step lets a plan be, and why.
struct Narrowing {
    allows: BTreeSet<Method>,
    reason: String,
}

type Step = fn(&Facts) -> Narrowing;

/// In ADR 0027's order. A new input is a new step here.
const STEPS: [Step; 1] = [version_tracks];

/// The plan for `facts` that every one of `steps` allows.
fn build(facts: &Facts, steps: &[Step]) -> Plan {
    let mut allowed = BTreeSet::from(Method::ALL);
    let mut reasons = Vec::with_capacity(steps.len());
    for step in steps {
        let Narrowing { allows, reason } = step(facts);
        allowed.retain(|method| allows.contains(method));
        reasons.push(reason);
    }
    // shortcut: only Direct Play is served; take the first of `allowed` once #319 serves Remux.
    let method = allowed
        .contains(&Method::DirectPlay)
        .then_some(Method::DirectPlay);
    Plan {
        method,
        url: method.map(|_| direct_play_url(facts.version)),
        tracks: facts.probe.as_ref().map(played_tracks).unwrap_or_default(),
        reasons,
    }
}

/// The step for the Version's own Tracks. None needs changing to play yet, so it allows every
/// Method.
fn version_tracks(facts: &Facts) -> Narrowing {
    let reason = if facts.probe.is_some() {
        "Its Tracks need no change"
    } else {
        // ADR 0026: without ffprobe, a Server Direct Plays.
        "Its Tracks aren't known, so they're taken to need no change"
    };
    Narrowing {
        allows: BTreeSet::from(Method::ALL),
        reason: reason.to_owned(),
    }
}

/// The Tracks a browser plays from the file: its first video, and the audio the file marks to
/// play by default, or else its first.
fn played_tracks(probe: &Probe) -> Vec<Track> {
    let video = (probe.tracks.iter()).find(|track| matches!(track.kind, TrackKind::Video { .. }));
    let mut audio =
        (probe.tracks.iter()).filter(|track| matches!(track.kind, TrackKind::Audio { .. }));
    let audio = audio
        .clone()
        .find(|track| track.default)
        .or_else(|| audio.next());
    [video, audio].into_iter().flatten().cloned().collect()
}

fn direct_play_url(version: i64) -> String {
    format!("/media/versions/{version}")
}

/// Serves `GET /media/versions/<id>` with the file of the Version `id` in `contents`, and 404 if
/// there is none. Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(contents: Contents) -> Router {
    Router::new()
        .route("/media/versions/{id}", get(serve))
        .with_state(contents)
}

async fn serve(
    State(contents): State<Contents>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    let path = match contents.version_file(id).await {
        Ok(Some((path, _))) => path,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {id}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // `ServeFile` answers `Range` (206 and `Content-Range`), sets `Accept-Ranges`, `ETag` and
    // `Last-Modified`, takes the `Content-Type` from the extension, and is 404 for a file gone
    // since the last scan.
    match ServeFile::new(&path).try_call(request).await {
        Ok(response) => response.into_response(),
        Err(err) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Facts, Method, Narrowing, Plan, STEPS, build};
    use crate::tracks::{Probe, Track, TrackKind};

    fn track(codec: &str, default: bool, kind: TrackKind) -> Track {
        Track {
            codec: codec.to_owned(),
            language: None,
            title: None,
            default,
            forced: false,
            kind,
        }
    }

    fn video(codec: &str) -> Track {
        let kind = TrackKind::Video {
            width: Some(1920),
            height: Some(1080),
            frame_rate: Some(24.0),
            bit_depth: Some(8),
            hdr: None,
        };
        track(codec, false, kind)
    }

    fn audio(codec: &str, default: bool) -> Track {
        track(codec, default, TrackKind::Audio { channels: Some(2) })
    }

    fn facts(tracks: Option<Vec<Track>>) -> Facts {
        Facts {
            version: 7,
            probe: tracks.map(|tracks| Probe {
                container: "mov,mp4,m4a,3gp,3g2,mj2".to_owned(),
                duration_ms: Some(7_200_000),
                bitrate: Some(5_000_000),
                tracks,
            }),
        }
    }

    #[test]
    fn an_h264_and_aac_mp4_direct_plays_from_its_versions_url_because_its_tracks_need_no_change() {
        let plan = build(
            &facts(Some(vec![video("h264"), audio("aac", true)])),
            &STEPS,
        );

        assert_eq!(
            plan,
            Plan {
                method: Some(Method::DirectPlay),
                url: Some("/media/versions/7".to_owned()),
                tracks: vec![video("h264"), audio("aac", true)],
                reasons: vec!["Its Tracks need no change".to_owned()],
            }
        );
    }

    #[test]
    fn a_version_whose_tracks_arent_known_direct_plays_and_says_so() {
        let plan = build(&facts(None), &STEPS);

        assert_eq!(plan.method, Some(Method::DirectPlay));
        assert_eq!(plan.tracks, []);
        assert_eq!(
            plan.reasons,
            ["Its Tracks aren't known, so they're taken to need no change"]
        );
    }

    #[test]
    fn the_plan_plays_the_default_audio_track_and_else_the_first() {
        let subtitle = track("subrip", true, TrackKind::Subtitle { image: false });
        let (first, default) = (audio("ac3", false), audio("aac", true));
        let cases = [
            (vec![video("h264"), first.clone(), default.clone()], default),
            (
                vec![first.clone(), video("h264"), audio("aac", false)],
                first,
            ),
        ];
        for (tracks, played) in cases {
            let with_subtitle = [tracks.clone(), vec![subtitle.clone()]].concat();

            let plan = build(&facts(Some(with_subtitle)), &STEPS);

            assert_eq!(plan.tracks, [video("h264"), played], "{tracks:?}");
        }
    }

    fn rules_out_direct_play(_: &Facts) -> Narrowing {
        Narrowing {
            allows: BTreeSet::from([Method::Remux, Method::Transcode]),
            reason: "ruled out Direct Play".to_owned(),
        }
    }

    fn allows_every_method(_: &Facts) -> Narrowing {
        Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "allowed every Method".to_owned(),
        }
    }

    #[test]
    fn a_later_step_cant_allow_what_an_earlier_step_ruled_out() {
        let plan = build(&facts(None), &[rules_out_direct_play, allows_every_method]);

        assert_eq!(plan.method, None);
        assert_eq!(plan.url, None);
        assert_eq!(
            plan.reasons,
            ["ruled out Direct Play", "allowed every Method"]
        );
    }
}
